use algorithm::buf::{BinaryMut, Bt, BtMut};
use log::warn;
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    time,
};

mod state;
pub use state::TcpState;

use crate::{NetConn, NetReceiver};

use super::{
    decode_message, encode_message, online_count::OnlineCount, stream::MaybeAcceptStream,
    CloseCode, MaybeTlsStream, NetError, Settings, TcpAcceptServer, WrapListener,
};

use super::{handler::Handler, message::Message, NetResult, NetSender};

enum Tcp {
    Stream(MaybeTlsStream),
    Listener(WrapListener),
    AcceptServer(TcpAcceptServer),
    Unconnect(Vec<SocketAddr>),
    Uninit,
}

impl Tcp {
    pub async fn try_accept(&mut self) -> NetResult<()> {
        match self {
            Tcp::AcceptServer(accept_server) => {
                let (stream, _addr) = accept_server.accept().await?;
                *self = Tcp::Stream(MaybeTlsStream::from(stream));
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

pub struct TcpConn {
    tcp: Tcp,
    settings: Settings,
    id: usize,
    state: TcpState,
    addr: Option<SocketAddr>,
    read: BinaryMut,
    write: BinaryMut,
    count: OnlineCount,
}

enum TcpReceiver {
    Accept(TcpConn),
    Read(Message),
    Next,
}

unsafe impl Sync for TcpConn {}
unsafe impl Send for TcpConn {}

impl Default for TcpConn {
    fn default() -> Self {
        Self {
            tcp: Tcp::Uninit,
            addr: None,
            id: 0,
            state: TcpState::Open,
            settings: Settings::default(),
            read: BinaryMut::new(),
            write: BinaryMut::new(),
            count: OnlineCount::default(),
        }
    }
}

impl TcpConn {
    pub fn new(addrs: Vec<SocketAddr>) -> TcpConn {
        TcpConn {
            tcp: Tcp::Unconnect(addrs),
            ..Default::default()
        }
    }

    pub async fn bind<A: ToSocketAddrs>(addr: A, settings: Settings) -> NetResult<TcpConn> {
        let listener = TcpListener::bind(addr).await?;
        let wrap = WrapListener::new(listener, &settings.tls).await?;
        Ok(TcpConn {
            tcp: Tcp::Listener(wrap),
            count: OnlineCount::new(),
            ..Default::default()
        })
    }

    pub async fn connect<A: ToSocketAddrs>(addr: A) -> NetResult<TcpConn> {
        let stream = TcpStream::connect(addr).await?;
        Ok(TcpConn {
            tcp: Tcp::Stream(MaybeTlsStream::from(stream)),
            ..Default::default()
        })
    }

    pub async fn connect_tls<A: ToSocketAddrs>(addr: A, domain: String) -> NetResult<TcpConn> {
        let stream = TcpStream::connect(addr).await?;
        let stream = MaybeTlsStream::connect_tls(stream, domain).await?;
        Ok(TcpConn {
            tcp: Tcp::Stream(stream),
            ..Default::default()
        })
    }

    pub async fn connect_with_timeout<A: ToSocketAddrs>(
        addr: A,
        timeout: Duration,
    ) -> NetResult<TcpConn> {
        match tokio::time::timeout(timeout, Self::connect(addr)).await {
            Ok(v) => Ok(v?),
            Err(_) => Err(NetError::Timeout),
        }
    }

    pub async fn connect_tls_with_timeout<A: ToSocketAddrs>(
        addr: A,
        domain: String,
        timeout: Duration,
    ) -> NetResult<TcpConn> {
        match tokio::time::timeout(timeout, Self::connect_tls(addr, domain)).await {
            Ok(v) => Ok(v?),
            Err(_) => Err(NetError::Timeout),
        }
    }

    async fn process(&mut self) -> NetResult<TcpReceiver> {
        match &mut self.tcp {
            Tcp::Listener(listener) => {
                let (stream, addr, id, accepter) = listener.accept().await?;
                let now = self.count.now();
                if now >= self.settings.max_connections {
                    warn!(
                        "当前连接数:{now}, 超出最大连接数: {}, 故关闭连接",
                        self.settings.max_connections
                    );
                    return Ok(TcpReceiver::Next);
                }
                Ok(TcpReceiver::Accept(TcpConn {
                    tcp: Tcp::AcceptServer(TcpAcceptServer::new(
                        MaybeAcceptStream::new(stream, accepter),
                        addr,
                    )),
                    addr: Some(addr),
                    id,
                    count: self.count.add(),
                    ..Default::default()
                }))
            }
            Tcp::Stream(stream) => {
                match &self.state {
                    TcpState::Open => {
                        let (mut reader, mut writer) = split(stream);
                        loop {
                            if let Some(v) = decode_message(&mut self.read, &self.settings)? {
                                return Ok(TcpReceiver::Read(v));
                            }

                            if self.read.len() >= self.settings.in_buffer_max {
                                return Err(NetError::OverInbufferSize);
                            }
                            let mut buf = ReadBuf::uninit(self.read.chunk_mut());
                            tokio::select! {
                                val = reader.read_buf(&mut buf) => {
                                    val?;
                                    let s = buf.filled().len();
                                    unsafe {
                                        self.read.advance_mut(s);
                                    }
                                    if s == 0 {
                                        return Ok(TcpReceiver::Read(Message::Shutdown));
                                    }
                                }
                                // 一旦有写数据，则尝试写入数据，写入成功后扣除相应的数据
                                r = writer.write(self.write.chunk()), if self.write.has_remaining() => {
                                    let n = r?;
                                    self.write.advance(n);
                                    if !self.write.has_remaining() {
                                        self.write.clear();
                                    }
                                }
                                _ = tokio::time::sleep(Duration::from_millis(self.settings.read_timeout as u64)) => {
                                    return Err(NetError::ReadTimeout.into());
                                }
                            }
                        }
                    }
                    TcpState::Closing((_, _)) => {
                        loop {
                            tokio::select! {
                                // 一旦有写数据，则尝试写入数据，写入成功后扣除相应的数据
                                r = stream.write(self.write.chunk()), if self.write.has_remaining() => {
                                    let n = r?;
                                    self.write.advance(n);
                                    if !self.write.has_remaining() {
                                        self.write.clear();
                                        self.state = TcpState::Closed;
                                        return Ok(TcpReceiver::Read(Message::Shutdown));
                                    }
                                }
                                _ = time::sleep(Duration::from_millis(self.settings.closing_time as u64)) => {
                                    self.state = TcpState::Closed;
                                    return Ok(TcpReceiver::Read(Message::Shutdown));
                                }
                            }
                        }
                    }
                    TcpState::Closed => return Ok(TcpReceiver::Read(Message::Shutdown)),
                }
            }
            Tcp::Unconnect(addr) => {
                let stream = Self::connect_with_timeout(
                    &addr[..],
                    Duration::from_millis(self.settings.connect_timeout as u64),
                )
                .await?;
                self.tcp = stream.tcp;
                return Ok(TcpReceiver::Next);
            }
            _ => {
                let pend = std::future::pending();
                let () = pend.await;
                unreachable!()
            }
        }
    }

    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.addr
    }

    pub(crate) fn close(&mut self, code: CloseCode, reason: String) -> NetResult<()> {
        encode_message(&mut self.write, Message::Close(code, reason.clone()))?;
        self.state = TcpState::Closing((code, reason));
        Ok(())
    }

    pub(crate) async fn inner_run_with_handler<H>(
        &mut self,
        mut handler: H,
        mut receiver: NetReceiver,
    ) -> NetResult<()>
    where
        H: Handler + 'static + Sync + Send,
    {
        self.tcp.try_accept().await?;
        handler.on_open().await?;
        loop {
            tokio::select! {
                ret = self.process() => {
                    let r = ret?;
                    match r {
                        TcpReceiver::Accept(tcp) => {
                            handler.on_accept(NetConn::from((tcp, self.settings.clone()))).await?
                        },
                        TcpReceiver::Read(msg) => {
                            match msg {
                                Message::Text(_) | Message::Binary(_) => handler.on_message(msg).await?,
                                Message::Close(code, reason) => {
                                    handler.on_close(code, reason).await;
                                    return Ok(())
                                }
                                Message::Ping(data) => {
                                    let ret = handler.on_ping(data).await?;
                                    encode_message(&mut self.write, Message::Pong(ret))?;
                                },
                                Message::Pong(data) => handler.on_pong(data).await?,
                                Message::Shutdown => return Ok(()),
                            }
                        },
                        TcpReceiver::Next => continue,
                    }
                }
                c = receiver.recv(), if self.write.len() < self.settings.out_buffer_max => {
                    let c = unwrap_or!(c, return Ok(()));
                    match c.msg {
                        Message::Close(code, reason) => {
                            self.close(code, reason)?;
                            continue;
                        },
                        Message::Shutdown => {
                            self.close(CloseCode::Away, "Shutdown".to_string())?;
                            continue;
                        },
                        _ => {}
                    }
                    encode_message(&mut self.write, c.msg)?;
                }
            };
        }
    }

    pub(crate) async fn inner_run_handler<F, H>(&mut self, factory: F) -> NetResult<()>
    where
        F: FnOnce(NetSender) -> H + Send + 'static,
        H: Handler + 'static + Sync + Send,
    {
        let (sender, receiver) = NetSender::new(self.settings.queue_size, self.id);
        let _avoid = sender.clone();
        let handler = factory(sender);
        self.inner_run_with_handler(handler, receiver).await
    }

    pub fn get_settings(&mut self) -> &mut Settings {
        &mut self.settings
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings
    }

    pub fn get_connection_id(&self) -> usize {
        self.id
    }
}
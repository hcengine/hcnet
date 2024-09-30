use algorithm::buf::{BinaryMut, Bt, BtMut};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt, ReadBuf},
    net::{lookup_host, ToSocketAddrs},
    time,
};
use tokio_kcp::{KcpConfig, KcpListener, KcpStream};
mod listener;
mod state;
use listener::WrapKcpListener;
pub use state::KcpState;

use crate::{NetConn, NetReceiver};

use super::{decode_message, encode_message, CloseCode, NetError, Settings};

use super::{handler::Handler, message::Message, NetResult, NetSender};

enum Kcp {
    Stream(KcpStream),
    Listener(WrapKcpListener),
    Unconnect(Vec<SocketAddr>),
    Uninit,
}

pub struct KcpConn {
    kcp: Kcp,
    settings: Settings,
    id: usize,
    state: KcpState,
    addr: Option<SocketAddr>,
    read: BinaryMut,
    write: BinaryMut,
}

enum TcpReceiver {
    Accept(KcpConn),
    Read(Message),
    Pending,
}

unsafe impl Sync for KcpConn {}
unsafe impl Send for KcpConn {}

impl Default for KcpConn {
    fn default() -> Self {
        Self {
            kcp: Kcp::Uninit,
            addr: None,
            id: 0,
            state: KcpState::Open,
            settings: Settings::default(),
            read: BinaryMut::new(),
            write: BinaryMut::new(),
        }
    }
}

impl KcpConn {
    pub fn new(addrs: Vec<SocketAddr>) -> KcpConn {
        KcpConn {
            kcp: Kcp::Unconnect(addrs),
            ..Default::default()
        }
    }

    pub async fn bind_with_listener(listener: KcpListener, _settings: Settings) -> NetResult<KcpConn> {
        Ok(KcpConn {
            kcp: Kcp::Listener(WrapKcpListener::new(listener)),
            ..Default::default()
        })
    }

    pub async fn bind<A: ToSocketAddrs>(addr: A, _settings: Settings) -> NetResult<KcpConn> {
        let config = KcpConfig::default();
        let listener = KcpListener::bind(config, addr).await?;
        Self::bind_with_listener(listener, _settings).await
    }

    pub async fn connect<A: ToSocketAddrs>(addr: A) -> NetResult<KcpConn> {
        Self::connect_with_settings(addr, Settings::default()).await
    }

    pub async fn connect_with_stream(stream: KcpStream) -> NetResult<KcpConn> {
        Ok(KcpConn {
            kcp: Kcp::Stream(stream),
            ..Default::default()
        })
    }

    pub async fn connect_with_settings<A: ToSocketAddrs>(addr: A, settings: Settings) -> NetResult<KcpConn> {
        let mut config = KcpConfig::default();
        config.session_expire = Duration::from_millis(settings.read_timeout as u64);
        let addrs = lookup_host(addr)
            .await?
            .into_iter()
            .collect::<Vec<SocketAddr>>();
        
        match tokio::time::timeout(Duration::from_millis(settings.connect_timeout as u64), KcpStream::connect(&config, addrs[0])).await {
            Ok(v) => {
                let stream = v?;
                Ok(KcpConn {
                    kcp: Kcp::Stream(stream),
                    settings,
                    ..Default::default()
                })
            }
            Err(_) => Err(NetError::Timeout),
        }
    }

    pub async fn connect_with_timeout<A: ToSocketAddrs>(
        addr: A,
        timeout: Duration,
    ) -> NetResult<KcpConn> {
        let mut settings = Settings::default();
        settings.connect_timeout = timeout.as_millis() as usize;
        Self::connect_with_settings(addr, settings).await
    }

    async fn process(&mut self) -> NetResult<TcpReceiver> {
        match &mut self.kcp {
            Kcp::Listener(listener) => {
                let (stream, addr, id) = listener.accept().await?;
                Ok(TcpReceiver::Accept(KcpConn {
                    kcp: Kcp::Stream(stream),
                    addr: Some(addr),
                    id,
                    ..Default::default()
                }))
            }
            Kcp::Stream(stream) => {
                match &self.state {
                    KcpState::Open => {
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
                    KcpState::Closing((_, _)) => {
                        loop {
                            tokio::select! {
                                // 一旦有写数据，则尝试写入数据，写入成功后扣除相应的数据
                                r = stream.write(self.write.chunk()), if self.write.has_remaining() => {
                                    let n = r?;
                                    self.write.advance(n);
                                    if !self.write.has_remaining() {
                                        self.write.clear();
                                        self.state = KcpState::Closed;
                                        return Ok(TcpReceiver::Read(Message::Shutdown));
                                    }
                                }
                                _ = time::sleep(Duration::from_millis(self.settings.closing_time as u64)) => {
                                    self.state = KcpState::Closed;
                                    return Ok(TcpReceiver::Read(Message::Shutdown));
                                }
                            }
                        }
                    }
                    KcpState::Closed => return Ok(TcpReceiver::Read(Message::Shutdown)),
                }
            }
            Kcp::Unconnect(addr) => {
                let stream = Self::connect_with_timeout(
                    &addr[..],
                    Duration::from_millis(self.settings.connect_timeout as u64),
                )
                .await?;
                self.kcp = stream.kcp;
                return Ok(TcpReceiver::Pending);
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
        encode_message(&mut self.write, Message::Close(code, reason.clone()), self.settings.is_raw)?;
        self.state = KcpState::Closing((code, reason));
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
                                    encode_message(&mut self.write, Message::Pong(ret), self.settings.is_raw)?;
                                },
                                Message::Pong(data) => handler.on_pong(data).await?,
                                Message::Shutdown => return Ok(()),
                            }
                        },
                        TcpReceiver::Pending => continue,
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
                    encode_message(&mut self.write, c.msg, self.settings.is_raw)?;
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

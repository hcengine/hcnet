use std::net::SocketAddr;

use log::warn;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use webparse::{Request, Response, Url};

mod client;
mod error;
mod handshake;
mod server;
mod state;

pub use client::WsClient;
pub use error::WsError;
pub use handshake::WsHandshake;
pub use server::WsServer;
pub use state::WsState;

use crate::{NetConn, NetReceiver};

use super::{
    online_count::OnlineCount, stream::MaybeAcceptStream, CloseCode, Handler, Message, NetError,
    NetResult, NetSender, Settings, TcpAcceptServer, WrapListener,
};

pub(crate) enum WsMsgReceiver {
    Accept(WsConn),
    Req(Request<Vec<u8>>),
    Res(Response<Vec<u8>>),
    Msg(Message),
    Next,
}

/// websocket的相关枚举状态
pub(crate) enum Ws {
    Client(WsClient),
    Server(WsServer),
    AcceptServer(TcpAcceptServer),
    Listener(WrapListener),
    Uninit,
}

impl Ws {
    pub async fn try_accept(&mut self) -> NetResult<()> {
        match self {
            Ws::AcceptServer(ws_accept_server) => {
                let (stream, addr) = ws_accept_server.accept().await?;
                *self = Ws::Server(WsServer::new(stream, addr));
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// websocket的中控端
pub struct WsConn {
    ws: Ws,
    id: usize,
    settings: Settings,
    count: OnlineCount,
}

impl Default for WsConn {
    fn default() -> Self {
        Self {
            ws: Ws::Uninit,
            id: 0,
            settings: Default::default(),
            count: Default::default(),
        }
    }
}

impl WsConn {
    pub async fn new(listener: TcpListener, settings: Settings) -> NetResult<WsConn> {
        let wrap = WrapListener::new(listener, settings.domain.clone(), &settings.tls).await?;
        Ok(WsConn {
            ws: Ws::Listener(wrap),
            settings,
            count: OnlineCount::new(),
            ..Default::default()
        })
    }

    pub async fn bind<A: ToSocketAddrs>(addr: A, settings: Settings) -> NetResult<WsConn> {
        let listener = TcpListener::bind(addr).await?;
        Self::new(listener, settings).await
    }

    pub async fn connect<U>(u: U) -> NetResult<WsConn>
    where
        Url: TryFrom<U>,
        <Url as TryFrom<U>>::Error: Into<NetError>,
    {
        Self::connect_with_settings(u, Settings::default()).await
    }

    pub async fn connect_with_settings<U>(u: U, settings: Settings) -> NetResult<WsConn>
    where
        Url: TryFrom<U>,
        <Url as TryFrom<U>>::Error: Into<NetError>,
    {
        let url = Url::try_from(u).map_err(|e| e.into())?;
        let client = WsClient::connect(url).await?;
        Ok(WsConn {
            ws: Ws::Client(client),
            settings,
            ..Default::default()
        })
    }

    pub async fn connect_with_stream<U>(stream: TcpStream, u: U, settings: Settings) -> NetResult<WsConn>
    where
        Url: TryFrom<U>,
        <Url as TryFrom<U>>::Error: Into<NetError>,
    {
        let url = Url::try_from(u).map_err(|e| e.into())?;
        let client = WsClient::new(stream, url).await?;
        Ok(WsConn {
            ws: Ws::Client(client),
            settings,
            ..Default::default()
        })
    }

    pub fn remote_addr(&self) -> Option<SocketAddr> {
        match &self.ws {
            Ws::Client(ws_client) => ws_client.remote_addr(),
            Ws::Server(ws_server) => ws_server.remote_addr(),
            _ => None,
        }
    }

    async fn process(&mut self) -> NetResult<WsMsgReceiver> {
        match &mut self.ws {
            Ws::Listener(listener) => {
                let (stream, addr, id, accepter) = listener.accept().await?;
                let now = self.count.now();
                if now >= self.settings.max_connections {
                    warn!(
                        "当前连接数:{now}, 超出最大连接数: {}, 故关闭连接",
                        self.settings.max_connections
                    );
                    return Ok(WsMsgReceiver::Next);
                }
                Ok(WsMsgReceiver::Accept(WsConn {
                    ws: Ws::AcceptServer(TcpAcceptServer::new(
                        MaybeAcceptStream::new(stream, accepter),
                        addr,
                    )),
                    id,
                    count: self.count.add(),
                    ..Default::default()
                }))
            }
            Ws::Client(client) => client.process(&self.settings).await,
            Ws::Server(server) => server.process(&self.settings).await,
            _ => {
                let pend = std::future::pending();
                let () = pend.await;
                unreachable!()
            }
        }
    }

    fn close(&mut self, code: CloseCode, reason: String) -> NetResult<()> {
        match &mut self.ws {
            Ws::Listener(_) => {
                self.ws = Ws::Uninit;
            }
            Ws::Client(client) => {
                client.close(code, reason)?;
            }
            Ws::Server(server) => {
                server.close(code, reason)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn send_message(&mut self, msg: Message) -> NetResult<()> {
        match &mut self.ws {
            Ws::Client(ws_client) => ws_client.send_message(msg)?,
            Ws::Server(ws_server) => ws_server.send_message(msg)?,
            _ => {}
        }
        Ok(())
    }

    fn is_outbuffer_full(&self) -> bool {
        match &self.ws {
            Ws::Client(ws_client) => ws_client.is_outbuffer_full(&self.settings),
            Ws::Server(ws_server) => ws_server.is_outbuffer_full(&self.settings),
            _ => true,
        }
    }

    pub(crate) async fn inner_run_with_handler<H>(
        &mut self,
        mut handler: H,
        mut receiver: NetReceiver,
    ) -> NetResult<()>
    where
        H: Handler + 'static + Sync + Send,
    {
        self.ws.try_accept().await?;
        let mut call_ready = false;
        loop {
            if !call_ready && self.is_ready() {
                handler.on_open().await?;
                call_ready = true;
            }
            tokio::select! {
                ret = self.process() => {
                    let r = ret?;
                    match r {
                        WsMsgReceiver::Accept(ws) => {
                            handler.on_accept(NetConn::from((ws, self.settings.clone()))).await?
                        },
                        WsMsgReceiver::Req(request) => {
                            let res = handler.on_request(request).await?;
                            let is_right = res.status() == 101;
                            match &mut self.ws {
                                Ws::Server(ws_server) => {
                                    ws_server.handler_response(res)?;
                                },
                                _ => unreachable!(),
                            }
                            if !is_right {
                                self.close(CloseCode::Normal, "handshake failed!".to_string())?;
                            }
                        },
                        WsMsgReceiver::Res(response) => {
                            if response.status() != 101 {
                                self.close(CloseCode::Normal, "handshake failed!".to_string())?;
                            } else {
                                match &mut self.ws {
                                    Ws::Client(ws_client) => {
                                        ws_client.handler_response(response)?;
                                    },
                                    _ => todo!(),
                                }
                            }
                        },
                        WsMsgReceiver::Msg(msg) => {
                            match msg {
                                Message::Text(_) | Message::Binary(_) => handler.on_message(msg).await?,
                                Message::Close(code, reason) => {
                                    handler.on_close(code.into(), reason).await;
                                    return Ok(())
                                }
                                Message::Ping(data) => {
                                    let ret = handler.on_ping(data).await?;
                                    self.send_message(Message::Pong(ret))?;
                                },
                                Message::Pong(data) => handler.on_pong(data).await?,
                                Message::Shutdown => return Ok(()),
                            }
                        },
                        WsMsgReceiver::Next => continue,
                    }
                }
                c = receiver.recv(), if !self.is_outbuffer_full() => {
                    let c = unwrap_or!(c, return Ok(()));
                    match c.msg {
                        Message::Close(code, reason) => {
                            self.close(code.into(), reason)?;
                            continue;
                        }
                        Message::Shutdown => {
                            self.close(CloseCode::Normal, "Shutdown".to_string())?;
                            continue;
                        }
                        _ => {}
                    }
                    self.send_message(c.msg)?;
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
        self.ws.try_accept().await?;
        self.inner_run_with_handler(handler, receiver).await
    }

    pub fn is_ready(&self) -> bool {
        match &self.ws {
            Ws::Client(ws_client) => ws_client.is_ready(),
            Ws::Server(ws_server) => ws_server.is_ready(),
            _ => false,
        }
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

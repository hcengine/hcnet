use std::net::SocketAddr;
use std::time::Duration;

use crate::{CloseCode, NetReceiver};

use super::handler::Handler;
use super::kcp::KcpConn;
use super::tcp::TcpConn;
use super::ws::WsConn;
use super::{NetError, NetResult, NetSender, Settings};
use tokio::net::{lookup_host, TcpListener, TcpStream, ToSocketAddrs};
use tokio::task::JoinHandle;
use tokio_kcp::{KcpListener, KcpStream};
use webparse::Url;

#[derive(Debug)]
pub enum NetType {
    Tcp,
    Quic,
    Websocket,
    UnixSocket,
}

/// 对外的网络对象, 仅提供相同的api来进行操作支持
pub enum NetConn {
    /// tcp的封装
    Tcp(TcpConn),
    /// websocket的封装
    Ws(WsConn),
    /// kcp的封装
    Kcp(KcpConn),
}

impl NetConn {
    pub async fn new<A: ToSocketAddrs>(t: NetType, addr: A) -> NetResult<NetConn> {
        let addrs = lookup_host(addr)
            .await?
            .into_iter()
            .collect::<Vec<SocketAddr>>();
        match t {
            NetType::Tcp => Ok(NetConn::Tcp(TcpConn::new(addrs))),
            _ => Err(NetError::OnlyTcp),
        }
    }

    pub async fn ws_bind_with_listener(
        listener: TcpListener,
        settings: Settings,
    ) -> NetResult<NetConn> {
        Ok(NetConn::Ws(WsConn::new(listener, settings).await?))
    }

    pub async fn ws_bind<A: ToSocketAddrs>(addr: A, settings: Settings) -> NetResult<NetConn> {
        Ok(NetConn::Ws(WsConn::bind(addr, settings).await?))
    }

    pub async fn ws_connect_with_stream<U>(
        stream: TcpStream,
        u: U,
        settings: Settings,
    ) -> NetResult<NetConn>
    where
        Url: TryFrom<U>,
        <Url as TryFrom<U>>::Error: Into<NetError>,
    {
        Ok(NetConn::Ws(
            WsConn::connect_with_stream(stream, u, settings).await?,
        ))
    }

    pub async fn ws_connect<U>(u: U) -> NetResult<NetConn>
    where
        Url: TryFrom<U>,
        <Url as TryFrom<U>>::Error: Into<NetError>,
    {
        Self::ws_connect_with_settings(u, Settings::default()).await
    }

    pub async fn ws_connect_with_settings<U>(u: U, settings: Settings) -> NetResult<NetConn>
    where
        Url: TryFrom<U>,
        <Url as TryFrom<U>>::Error: Into<NetError>,
    {
        Ok(NetConn::Ws(
            WsConn::connect_with_settings(u, settings).await?,
        ))
    }

    pub async fn tcp_bind_with_listener(
        listener: TcpListener,
        settings: Settings,
    ) -> NetResult<NetConn> {
        Ok(NetConn::Tcp(
            TcpConn::bind_with_listener(listener, settings).await?,
        ))
    }

    pub async fn tcp_bind<A: ToSocketAddrs>(addr: A, settings: Settings) -> NetResult<NetConn> {
        Ok(NetConn::Tcp(TcpConn::bind(addr, settings).await?))
    }

    pub async fn tcp_connect<A: ToSocketAddrs>(addr: A) -> NetResult<NetConn> {
        Ok(NetConn::Tcp(TcpConn::connect(addr).await?))
    }

    pub async fn tcp_connect_with_stream(stream: TcpStream) -> NetResult<NetConn> {
        Ok(NetConn::Tcp(
            TcpConn::connect_with_stream(stream, Settings::default()).await?,
        ))
    }

    pub async fn tcp_connect_with_settings<A: ToSocketAddrs>(
        addr: A,
        settings: Settings,
    ) -> NetResult<NetConn> {
        Ok(NetConn::Tcp(
            TcpConn::connect_with_settings(addr, settings).await?,
        ))
    }

    pub async fn tcp_connect_with_timeout<A: ToSocketAddrs>(
        addr: A,
        timeout: Duration,
    ) -> NetResult<NetConn> {
        Ok(NetConn::Tcp(
            TcpConn::connect_with_timeout(addr, timeout).await?,
        ))
    }

    pub async fn kcp_bind_with_listener(
        listener: KcpListener,
        settings: Settings,
    ) -> NetResult<NetConn> {
        Ok(NetConn::Kcp(
            KcpConn::bind_with_listener(listener, settings).await?,
        ))
    }

    pub async fn kcp_bind<A: ToSocketAddrs>(addr: A, settings: Settings) -> NetResult<NetConn> {
        Ok(NetConn::Kcp(KcpConn::bind(addr, settings).await?))
    }

    pub async fn kcp_connect<A: ToSocketAddrs>(addr: A) -> NetResult<NetConn> {
        Self::kcp_connect_with_settings(addr, Settings::default()).await
    }

    pub async fn kcp_connect_with_stream(stream: KcpStream) -> NetResult<NetConn> {
        Ok(NetConn::Kcp(KcpConn::connect_with_stream(stream).await?))
    }

    pub async fn kcp_connect_with_settings<A: ToSocketAddrs>(
        addr: A,
        settings: Settings,
    ) -> NetResult<NetConn> {
        Ok(NetConn::Kcp(
            KcpConn::connect_with_settings(addr, settings).await?,
        ))
    }

    pub async fn kcp_connect_with_timeout<A: ToSocketAddrs>(
        addr: A,
        timeout: Duration,
    ) -> NetResult<NetConn> {
        Ok(NetConn::Kcp(
            KcpConn::connect_with_timeout(addr, timeout).await?,
        ))
    }

    pub fn set_settings(&mut self, settings: Settings) {
        match self {
            NetConn::Tcp(tcp) => tcp.set_settings(settings),
            NetConn::Ws(ws) => ws.set_settings(settings),
            NetConn::Kcp(kcp) => kcp.set_settings(settings),
        }
    }

    pub fn get_settings(&mut self) -> &mut Settings {
        match self {
            NetConn::Tcp(tcp) => tcp.get_settings(),
            NetConn::Ws(ws) => ws.get_settings(),
            NetConn::Kcp(kcp) => kcp.get_settings(),
        }
    }

    async fn inner_run_handler<F, H>(&mut self, factory: F) -> NetResult<()>
    where
        F: FnOnce(NetSender) -> H + Send + 'static,
        H: Handler + 'static + Sync + Send,
    {
        match self {
            NetConn::Tcp(tcp) => tcp.inner_run_handler(factory).await?,
            NetConn::Ws(ws) => ws.inner_run_handler(factory).await?,
            NetConn::Kcp(kcp) => kcp.inner_run_handler(factory).await?,
        }
        Ok(())
    }

    async fn inner_run_with_handler<H>(
        &mut self,
        handler: &mut H,
        receiver: NetReceiver,
    ) -> NetResult<()>
    where
        H: Handler + 'static + Sync + Send,
    {
        match self {
            NetConn::Tcp(tcp) => tcp.inner_run_with_handler(handler, receiver).await?,
            NetConn::Ws(ws) => ws.inner_run_with_handler(handler, receiver).await?,
            NetConn::Kcp(kcp) => kcp.inner_run_with_handler(handler, receiver).await?,
        }
        Ok(())
    }

    pub async fn run_handler<F, H>(mut self, factory: F) -> NetResult<JoinHandle<()>>
    where
        F: FnOnce(NetSender) -> H + Send + 'static,
        H: Handler + 'static + Sync + Send,
    {
        let handler = tokio::spawn(async move {
            if let Err(e) = self.inner_run_handler(factory).await {
                println!("occur error = {e:?}");
            }
        });
        Ok(handler)
    }

    pub async fn run_with_handler<H>(
        mut self,
        handler: H,
        receiver: NetReceiver,
    ) -> NetResult<JoinHandle<()>>
    where
        H: Handler + 'static + Sync + Send,
    {
        let handler = tokio::spawn(async move {
            let mut handler = handler;
            if let Err(e) = self.inner_run_with_handler(&mut handler, receiver).await {
                handler
                    .on_close(CloseCode::Error, "NetError".to_string())
                    .await;
                println!("occur error = {e:?}");
            }
        });
        Ok(handler)
    }

    pub fn remote_addr(&self) -> Option<SocketAddr> {
        match self {
            NetConn::Tcp(tcp) => tcp.remote_addr(),
            NetConn::Ws(ws) => ws.remote_addr(),
            NetConn::Kcp(kcp) => kcp.remote_addr(),
        }
    }

    pub fn get_connection_id(&self) -> u64 {
        match self {
            NetConn::Tcp(tcp) => tcp.get_connection_id(),
            NetConn::Ws(ws) => ws.get_connection_id(),
            NetConn::Kcp(kcp) => kcp.get_connection_id(),
        }
    }
}

impl From<TcpConn> for NetConn {
    fn from(value: TcpConn) -> Self {
        NetConn::Tcp(value)
    }
}

impl From<(TcpConn, Settings)> for NetConn {
    fn from(value: (TcpConn, Settings)) -> Self {
        let mut conn = NetConn::Tcp(value.0);
        conn.set_settings(value.1);
        conn
    }
}

impl From<WsConn> for NetConn {
    fn from(value: WsConn) -> Self {
        NetConn::Ws(value)
    }
}

impl From<(WsConn, Settings)> for NetConn {
    fn from(value: (WsConn, Settings)) -> Self {
        let mut conn = NetConn::Ws(value.0);
        conn.set_settings(value.1);
        conn
    }
}

impl From<KcpConn> for NetConn {
    fn from(value: KcpConn) -> Self {
        NetConn::Kcp(value)
    }
}

impl From<(KcpConn, Settings)> for NetConn {
    fn from(value: (KcpConn, Settings)) -> Self {
        let mut conn = NetConn::Kcp(value.0);
        conn.set_settings(value.1);
        conn
    }
}

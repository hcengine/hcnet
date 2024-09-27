use tokio::net::ToSocketAddrs;
use webparse::Url;

use crate::{NetConn, NetError, NetResult, Settings, TlsSettings};

pub struct Builder {
    settings: Settings,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            settings: Settings::default(),
        }
    }

    pub fn max_connections(mut self, max_connections: usize) -> Self {
        self.settings.max_connections = max_connections;
        self
    }

    pub fn queue_size(mut self, queue_size: usize) -> Self {
        self.settings.queue_size = queue_size;
        self
    }

    pub fn in_buffer_max(mut self, in_buffer_max: usize) -> Self {
        self.settings.in_buffer_max = in_buffer_max;
        self
    }

    pub fn out_buffer_max(mut self, out_buffer_max: usize) -> Self {
        self.settings.out_buffer_max = out_buffer_max;
        self
    }

    pub fn onemsg_max_size(mut self, onemsg_max_size: usize) -> Self {
        self.settings.onemsg_max_size = onemsg_max_size;
        self
    }

    pub fn closing_time(mut self, closing_time: usize) -> Self {
        self.settings.closing_time = closing_time;
        self
    }

    pub fn connect_timeout(mut self, connect_timeout: usize) -> Self {
        self.settings.connect_timeout = connect_timeout;
        self
    }

    pub fn shake_timeout(mut self, shake_timeout: usize) -> Self {
        self.settings.shake_timeout = shake_timeout;
        self
    }

    pub fn read_timeout(mut self, read_timeout: usize) -> Self {
        self.settings.read_timeout = read_timeout;
        self
    }

    pub fn domain(mut self, domain: String) -> Self {
        self.settings.domain = Some(domain);
        self
    }

    pub fn tls(mut self, cert: String, key: String) -> Self {
        self.settings.tls = Some(TlsSettings { cert, key });
        self
    }

    pub fn settings(self) -> Settings {
        self.settings
    }

    pub async fn tcp_connect<A: ToSocketAddrs>(self, addr: A) -> NetResult<NetConn> {
        NetConn::tcp_connect_with_settings(addr, self.settings).await
    }

    pub async fn tcp_bind<A: ToSocketAddrs>(self, addr: A) -> NetResult<NetConn> {
        NetConn::tcp_bind(addr, self.settings).await
    }

    pub async fn kcp_connect<A: ToSocketAddrs>(self, addr: A) -> NetResult<NetConn> {
        NetConn::kcp_connect_with_settings(addr, self.settings).await
    }

    pub async fn kcp_bind<A: ToSocketAddrs>(self, addr: A) -> NetResult<NetConn> {
        NetConn::kcp_bind(addr, self.settings).await
    }

    pub async fn ws_connect<U>(self, u: U) -> NetResult<NetConn>
    where
        Url: TryFrom<U>,
        <Url as TryFrom<U>>::Error: Into<NetError>,
    {
        NetConn::ws_connect_with_settings(u, self.settings).await
    }
    
    pub async fn ws_bind<A: ToSocketAddrs>(self, addr: A) -> NetResult<NetConn> {
        NetConn::ws_bind(addr, self.settings).await
    }
}

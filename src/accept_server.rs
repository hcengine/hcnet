use std::net::SocketAddr;

use crate::{
    NetError, NetResult,
    {stream::MaybeAcceptStream, MaybeTlsStream},
};

pub struct TcpAcceptServer {
    stream: Option<MaybeAcceptStream>,
    pub addr: SocketAddr,
}

impl TcpAcceptServer {
    pub fn new(stream: MaybeAcceptStream, addr: SocketAddr) -> TcpAcceptServer {
        TcpAcceptServer {
            stream: Some(stream),
            addr,
        }
    }

    pub(crate) async fn accept(&mut self) -> NetResult<(MaybeTlsStream, SocketAddr)> {
        if self.stream.is_none() {
            return Err(NetError::Extension("can't accept twice"));
        } else {
            let stream = self.stream.take().unwrap();
            let stream = stream.accept().await?;
            Ok((stream, self.addr))
        }
    }
}

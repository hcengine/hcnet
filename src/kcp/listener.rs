use std::net::SocketAddr;


use tokio_kcp::{KcpListener, KcpStream};

use super::NetResult;

pub struct WrapKcpListener {
    pub listener: KcpListener,
    pub next_connection_id: usize,
}

impl WrapKcpListener {
    pub fn new(listener: KcpListener) -> Self {
        Self {
            listener,
            next_connection_id: 0,
        }
    }

    pub async fn accept(&mut self) -> NetResult<(KcpStream, SocketAddr, usize)> {
        let (stream, addr) = self.listener.accept().await?;
        self.next_connection_id = self.next_connection_id.wrapping_add(1);
        Ok((stream, addr, self.next_connection_id))
    }
}

use std::net::SocketAddr;


use tokio_kcp::{KcpListener, KcpStream};

use super::NetResult;

pub struct WrapKcpListener {
    pub listener: KcpListener,
    pub server_id: u64,
    pub next_connection_id: u32,
}

impl WrapKcpListener {
    pub fn new(server_id: u64, listener: KcpListener) -> Self {
        Self {
            listener,
            server_id: server_id << 32,
            next_connection_id: 0,
        }
    }

    pub async fn accept(&mut self) -> NetResult<(KcpStream, SocketAddr, u64)> {
        let (stream, addr) = self.listener.accept().await?;
        self.next_connection_id = self.next_connection_id.wrapping_add(1);
        Ok((stream, addr, self.server_id + self.next_connection_id as u64))
    }
}

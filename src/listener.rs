use std::{io, net::SocketAddr, sync::Arc};

use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{rustls, TlsAcceptor};

use crate::Settings;

use super::{helper::Helper, NetResult};

pub struct WrapListener {
    pub listener: TcpListener,
    pub server_id: u64,
    pub next_connection_id: u32,

    pub domain: Option<String>,
    pub accepter: Option<Arc<TlsAcceptor>>,
}

impl WrapListener {
    pub async fn new(listener: TcpListener, server_id: u64, domain: Option<String>, settings: &Settings) -> NetResult<Self> {
        if settings.cert.is_some() && settings.key.is_some() {
            let t = settings.cert.as_ref().unwrap();
            let one_cert = Helper::load_certs(&t)?;
            let one_key = Helper::load_keys(&settings.key.clone().unwrap_or(String::new()))?;
            let config = rustls::ServerConfig::builder();
            let mut config = config
                .with_no_client_auth()
                .with_single_cert(one_cert, one_key)
                .map_err(|e| {
                    log::warn!("添加证书时失败:{:?}", e);
                    io::Error::new(io::ErrorKind::Other, "key error")
                })?;
            config.alpn_protocols.push("http/1.1".as_bytes().to_vec());
            let accepter = TlsAcceptor::from(Arc::new(config));
            Ok(Self {
                listener,
                server_id: server_id<<32,
                next_connection_id: 0,
                domain: domain,
                accepter: Some(Arc::new(accepter)),
            })
        } else {
            Ok(Self {
                listener,
                server_id: server_id<<32,
                next_connection_id: 0,
                domain: None,
                accepter: None,
            })
        }
    }

    pub async fn accept(
        &mut self,
    ) -> NetResult<(TcpStream, SocketAddr, u64, Option<Arc<TlsAcceptor>>)> {
        let (stream, addr) = self.listener.accept().await?;
        self.next_connection_id = self.next_connection_id.wrapping_add(1);
        Ok((stream, addr, self.server_id + self.next_connection_id as u64, self.accepter.clone()))
    }
}

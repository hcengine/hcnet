use std::{io, net::SocketAddr, sync::Arc};

use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{rustls, TlsAcceptor};

use super::{helper::Helper, settings::TlsSettings, NetResult};

pub struct WrapListener {
    pub listener: TcpListener,
    pub next_connection_id: usize,

    pub domain: Option<String>,
    pub accepter: Option<Arc<TlsAcceptor>>,
}

impl WrapListener {
    pub async fn new(listener: TcpListener, domain: Option<String>, tls: &Option<TlsSettings>) -> NetResult<Self> {
        if let Some(t) = tls {
            let one_cert = Helper::load_certs(&t.cert)?;
            let one_key = Helper::load_keys(&t.key)?;
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
                next_connection_id: 0,
                domain: domain,
                accepter: Some(Arc::new(accepter)),
            })
        } else {
            Ok(Self {
                listener,
                next_connection_id: 0,
                domain: None,
                accepter: None,
            })
        }
    }

    pub async fn accept(
        &mut self,
    ) -> NetResult<(TcpStream, SocketAddr, usize, Option<Arc<TlsAcceptor>>)> {
        let (stream, addr) = self.listener.accept().await?;
        self.next_connection_id = self.next_connection_id.wrapping_add(1);
        Ok((stream, addr, self.next_connection_id, self.accepter.clone()))
    }
}

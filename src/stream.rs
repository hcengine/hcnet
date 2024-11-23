use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};
use tokio_rustls::{
    client::TlsStream,
    rustls::{self, ClientConfig, RootCertStore},
    server::TlsStream as TlsServer,
    TlsAcceptor, TlsConnector,
};

use super::NetResult;

/// 当前可能是tcp也可能是tcps的连接
pub enum MaybeTlsStream {
    /// 普通的socket
    Stream(TcpStream),
    /// tls客户端
    TlsStream(TlsStream<TcpStream>),
    /// tls服务端
    TlsServer(TlsServer<TcpStream>),
}

impl From<TcpStream> for MaybeTlsStream {
    fn from(inner: TcpStream) -> Self {
        MaybeTlsStream::Stream(inner)
    }
}

impl From<TlsStream<TcpStream>> for MaybeTlsStream {
    fn from(inner: TlsStream<TcpStream>) -> Self {
        MaybeTlsStream::TlsStream(inner)
    }
}

impl From<TlsServer<TcpStream>> for MaybeTlsStream {
    fn from(inner: TlsServer<TcpStream>) -> Self {
        MaybeTlsStream::TlsServer(inner)
    }
}

impl MaybeTlsStream {
    pub async fn connect_tls(stream: TcpStream, domain: String) -> NetResult<MaybeTlsStream> {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        // config.alpn_protocols = self.inner.get_alpn_protocol();
        let tls_client = Arc::new(config);
        let connector = TlsConnector::from(tls_client);

        // 这里的域名只为认证设置
        let domain = rustls::pki_types::ServerName::try_from(domain)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

        let outbound = connector.connect(domain, stream).await?;
        Ok(MaybeTlsStream::from(outbound))
    }

    pub async fn accept_tls(accept: &TlsAcceptor, stream: TcpStream) -> NetResult<MaybeTlsStream> {
        let stream = accept.accept(stream).await?;
        Ok(MaybeTlsStream::from(stream))
    }
}

impl AsyncRead for MaybeTlsStream {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self) {
            MaybeTlsStream::Stream(s) => Pin::new(s).poll_read(cx, buf),
            MaybeTlsStream::TlsStream(s) => Pin::new(s).poll_read(cx, buf),
            MaybeTlsStream::TlsServer(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for MaybeTlsStream {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match Pin::get_mut(self) {
            MaybeTlsStream::Stream(s) => Pin::new(s).poll_write(cx, buf),
            MaybeTlsStream::TlsStream(s) => Pin::new(s).poll_write(cx, buf),
            MaybeTlsStream::TlsServer(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self) {
            MaybeTlsStream::Stream(s) => Pin::new(s).poll_flush(cx),
            MaybeTlsStream::TlsStream(s) => Pin::new(s).poll_flush(cx),
            MaybeTlsStream::TlsServer(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self) {
            MaybeTlsStream::Stream(s) => Pin::new(s).poll_shutdown(cx),
            MaybeTlsStream::TlsStream(s) => Pin::new(s).poll_shutdown(cx),
            MaybeTlsStream::TlsServer(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

pub struct MaybeAcceptStream {
    stream: TcpStream,
    accepter: Option<Arc<TlsAcceptor>>,
}

impl MaybeAcceptStream {
    pub fn new(stream: TcpStream, accepter: Option<Arc<TlsAcceptor>>) -> Self {
        MaybeAcceptStream { stream, accepter }
    }

    pub async fn accept(self) -> NetResult<MaybeTlsStream> {
        if let Some(accepter) = self.accepter {
            let stream = accepter.accept(self.stream).await?;
            Ok(MaybeTlsStream::from(stream))
        } else {
            Ok(MaybeTlsStream::from(self.stream))
        }
    }
}

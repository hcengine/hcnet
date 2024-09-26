use std::{
    fs::File,
    io::{self, BufReader, Read},
};

use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

pub struct Helper;

impl Helper {
    pub fn load_certs(path: &String) -> io::Result<Vec<CertificateDer<'static>>> {
        match File::open(&path) {
            Ok(mut file) => {
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                let mut reader = BufReader::new(content.as_bytes());
                let certs = rustls_pemfile::certs(&mut reader);
                Ok(certs.into_iter().collect::<Result<Vec<_>, _>>()?)
            }
            Err(e) => {
                log::warn!("加载公钥{}出错，错误内容:{:?}", path, e);
                return Err(e);
            }
        }
    }

    pub fn load_keys(path: &String) -> io::Result<PrivateKeyDer<'static>> {
        match File::open(&path) {
            Ok(mut file) => {
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                {
                    let mut reader = BufReader::new(content.as_bytes());
                    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
                        .collect::<Result<Vec<_>, _>>()?;
                    if keys.len() == 1 {
                        return Ok(PrivateKeyDer::from(keys.remove(0)));
                    }
                }
                {
                    let mut reader = BufReader::new(content.as_bytes());
                    let mut keys =
                        rustls_pemfile::rsa_private_keys(&mut reader)
                            .collect::<Result<Vec<_>, _>>()?;
                    if keys.len() == 1 {
                        return Ok(PrivateKeyDer::from(keys.remove(0)));
                    }
                }
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("No pkcs8 or rsa private key found"),
                ));
            }
            Err(e) => {
                log::warn!("加载私钥{}出错，错误内容:{:?}", path, e);
                return Err(e);
            }
        }
    }
}

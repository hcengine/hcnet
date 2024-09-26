// Copyright 2022 - 2024 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2024/01/04 11:12:31

use std::net::SocketAddr;

use base64::{engine::general_purpose::STANDARD, Engine};
use sha1::{Digest, Sha1};
use tokio::sync::mpsc::Sender;
use webparse::{ws::OwnedMessage, Request, Response};

use crate::{NetResult};

use super::WsError;

static MAGIC_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub struct WsHandshake {
    pub sender: Sender<OwnedMessage>,
    /// The HTTP request sent to begin the handshake.
    pub request: Option<Request<Vec<u8>>>,
    /// The HTTP response from the server confirming the handshake.
    pub response: Response<Vec<u8>>,
    /// The socket address of the other endpoint. This address may
    /// be an intermediary such as a proxy server.
    pub peer_addr: Option<SocketAddr>,
    /// The socket address of this endpoint.
    pub local_addr: Option<SocketAddr>,
}

impl WsHandshake {
    pub fn new(
        sender: Sender<OwnedMessage>,
        request: Option<Request<Vec<u8>>>,
        response: Response<Vec<u8>>,
        peer_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            sender,
            request,
            response,
            peer_addr,
            local_addr: None,
        }
    }

    pub fn build_accept(key: &str) -> NetResult<String> {
        match STANDARD.decode(key) {
            Ok(vec) => {
                if vec.len() != 16 {
                    return Err(WsError::ProtocolError("Sec-WebSocket-Key must be 16 bytes").into());
                }
                let mut array = [0u8; 16];
                array[..16].clone_from_slice(&vec[..16]);

                let mut concat_key = String::with_capacity(array.len() + 36);
                concat_key.push_str(&key[..]);
                concat_key.push_str(MAGIC_GUID);
                let hash = Sha1::digest(concat_key.as_bytes());
                let key: [u8; 20] = hash.into();
                Ok(STANDARD.encode(key))
            }
            Err(_) => Err(WsError::ProtocolError("Invalid Sec-WebSocket-Accept").into()),
        }
    }

    pub fn build_request(req: &Request<Vec<u8>>) -> NetResult<Response<Vec<u8>>> {
        let key = req.headers().get_str_value(&"Sec-WebSocket-Key");
        let protocol = req.headers().get_str_value(&"Sec-WebSocket-Protocol");
        let version = req.headers().get_str_value(&"Sec-WebSocket-Version");
        if key.is_none() || version.as_ref().map(|s| &**s) != Some("13") {
            return Ok(Response::builder()
                .status(400)
                .body("invalid websocket version")
                .unwrap()
                .into_type());
        }
        let (key, protocol) = (key.unwrap(), protocol.unwrap_or("chat".to_string()));
        let accept = Self::build_accept(&key)?;
        let protocols: Vec<&str> = protocol
            .split(|c| c == ',' || c == ' ')
            .filter(|s| !s.is_empty())
            .collect();
        return Ok(Response::builder()
            .status(101)
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Accept", accept)
            .header("Sec-WebSocket-Protocol", protocols[0].to_string())
            .body(vec![])
            .unwrap());
    }
}

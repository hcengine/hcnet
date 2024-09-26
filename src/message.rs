use std::fmt;

use webparse::ws::{CloseData, OwnedMessage};
use OpCode::*;

use super::protocol::CloseCode;

pub enum OpCode {
    Text = 1,
    Binary = 2,
    Close = 8,
    Ping = 9,
    Pong = 10,
    Bad,
}

impl OpCode {
    /// Test whether the opcode indicates a control frame.
    pub fn is_control(&self) -> bool {
        match *self {
            Text | Binary => false,
            _ => true,
        }
    }
}

impl fmt::Display for OpCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Text => write!(f, "TEXT"),
            Binary => write!(f, "BINARY"),
            Close => write!(f, "CLOSE"),
            Ping => write!(f, "PING"),
            Pong => write!(f, "PONG"),
            Bad => write!(f, "BAD"),
        }
    }
}

impl Into<u8> for OpCode {
    fn into(self) -> u8 {
        match self {
            Text => 1,
            Binary => 2,
            Close => 8,
            Ping => 9,
            Pong => 10,
            Bad => {
                debug_assert!(
                    false,
                    "Attempted to convert invalid opcode to u8. This is a bug."
                );
                8 // if this somehow happens, a close frame will help us tear down quickly
            }
        }
    }
}

impl From<u8> for OpCode {
    fn from(byte: u8) -> OpCode {
        match byte {
            1 => Text,
            2 => Binary,
            8 => Close,
            9 => Ping,
            10 => Pong,
            _ => Bad,
        }
    }
}

#[derive(Debug)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
    Close(CloseCode, String),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Shutdown,
}

impl Message {}

impl From<OwnedMessage> for Message {
    fn from(value: OwnedMessage) -> Self {
        match value {
            OwnedMessage::Text(t) => Message::Text(t),
            OwnedMessage::Binary(vec) => Message::Binary(vec),
            OwnedMessage::Close(close_data) => {
                if let Some(c) = close_data {
                    Message::Close(c.status_code.into(), c.reason)
                } else {
                    Message::Close(CloseCode::Normal, String::new())
                }
            }
            OwnedMessage::Ping(vec) => Message::Ping(vec),
            OwnedMessage::Pong(vec) => Message::Pong(vec),
        }
    }
}

impl Into<OwnedMessage> for Message {
    fn into(self) -> OwnedMessage {
        match self {
            Message::Text(t) => OwnedMessage::Text(t),
            Message::Binary(vec) => OwnedMessage::Binary(vec),
            Message::Close(code, reason) => OwnedMessage::Close(Some(CloseData::new(code, reason))),
            Message::Ping(vec) => OwnedMessage::Ping(vec),
            Message::Pong(vec) => OwnedMessage::Pong(vec),
            _ => OwnedMessage::Close(None),
        }
    }
}
#[macro_use]
mod macros;

mod accept_server;
mod builder;
mod conn;
mod decode;
mod encode;
mod error;
mod handler;
mod helper;
mod id_center;
mod kcp;
mod listener;
mod message;
mod online_count;
mod protocol;
mod sender;
mod settings;
mod stream;
mod tcp;
mod ws;

use algorithm::buf::{Bt, BtMut};
pub use builder::Builder;
pub use conn::{NetConn, NetType};
pub use decode::*;
pub use encode::*;
pub use error::NetError;
pub use handler::Handler;
pub use listener::WrapListener;
pub use message::{Message, OpCode};
pub use protocol::CloseCode;
pub use sender::{NetReceiver, NetSender};
pub use settings::Settings;
pub use tcp::TcpConn;

pub use stream::MaybeTlsStream;

pub use accept_server::TcpAcceptServer;

pub type NetResult<T> = std::result::Result<T, NetError>;

#[inline(always)]
pub fn read_u24<T: Bt>(buf: &mut T) -> u32 {
    if buf.remaining() < 3 {
        return 0;
    }
    (buf.get_u8() as u32) << 16 | (buf.get_u8() as u32) << 8 | buf.get_u8() as u32
}

#[inline(always)]
pub fn encode_u24<B: Bt + BtMut>(buf: &mut B, val: u32) -> usize {
    buf.put_u8((val >> 16) as u8);
    buf.put_u8((val >> 8) as u8);
    buf.put_u8((val >> 0) as u8);
    3
}

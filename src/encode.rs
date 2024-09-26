use algorithm::buf::{BinaryMut, BtMut};

use super::{encode_u24, Message, NetResult, OpCode};


pub fn encode_message(data: &mut BinaryMut, msg: Message) -> NetResult<()> {
    match msg {
        Message::Text(text) => {
            let bytes = text.into_bytes();
            encode_u24(data, (bytes.len() + 4) as u32);
            data.put_u8(OpCode::Text.into());
            data.put_slice(&bytes);
        },
        Message::Binary(bytes) => {
            encode_u24(data, (bytes.len() + 4) as u32);
            data.put_u8(OpCode::Binary.into());
            data.put_slice(&bytes);
        },
        Message::Close(code, reason) => {
            let bytes = reason.into_bytes();
            encode_u24(data, (bytes.len() + 5) as u32);
            data.put_u8(OpCode::Close.into());
            data.put_u16(code.into());
            data.put_slice(&bytes);
        },
        Message::Ping(bytes) => {
            encode_u24(data, (bytes.len() + 4) as u32);
            data.put_u8(OpCode::Ping.into());
            data.put_slice(&bytes);
        },
        Message::Pong(bytes) => {
            encode_u24(data, (bytes.len() + 4) as u32);
            data.put_u8(OpCode::Pong.into());
            data.put_slice(&bytes);
        },
        Message::Shutdown => {
            // encode_u24(data, 4);
            // data.put_u8(OpCode::Shutdown.into());
        },
    }

    Ok(())
}
use algorithm::buf::{BinaryMut, Bt};

use super::{read_u24, Message, NetError, NetResult, OpCode, Settings};

pub fn decode_message(data: &mut BinaryMut, settings: &Settings) -> NetResult<Option<Message>> {
    if settings.is_raw {
        if data.len() == 0 {
            return Ok(None)
        }
        let val = data.chunk().to_vec();
        data.advance_all();
        return Ok(Some(Message::Binary(val)))
    }
    
    if data.len() < 4 {
        return Ok(None);
    }
    data.mark();
    let length = read_u24(data) as usize;
    if length < 4 {
        return Err(NetError::TooShort);
    }
    if length > settings.onemsg_max_size {
        return Err(NetError::OverMsgSize);
    }
    if data.len() + 3 < length {
        data.rewind_mark();
        return Ok(None);
    }
    let op = OpCode::from(data.get_u8());
    let mut val = vec![0; length - 4];
    data.copy_to_slice(&mut val);
    match op {
        OpCode::Text => {
            if let Ok(v) = String::from_utf8(val) {
                return Ok(Some(Message::Text(v)));
            } else {
                return Err(NetError::BadText);
            }
        },
        OpCode::Binary => return Ok(Some(Message::Binary(val))),
        OpCode::Close => {
            if val.len() < 2 {
                return Err(NetError::TooShort);
            }
            let code = (val[0] as u16).wrapping_shl(8) + val[1] as u16;
            if let Ok(v) = String::from_utf8(val[2..].to_vec()) {
                return Ok(Some(Message::Close(code.into(), v)));
            } else {
                return Err(NetError::BadText);
            }
        },
        OpCode::Ping => return Ok(Some(Message::Ping(val))),
        OpCode::Pong => return Ok(Some(Message::Pong(val))),

        OpCode::Bad | OpCode::Shutdown => return Err(NetError::BadCode),
    }
}
use std::io;

use webparse::WebError;
use kcp::Error as KcpError;

use super::{sender::Command, ws::WsError};

#[derive(Debug)]
pub enum NetError {
    /// 长度太小
    TooShort,
    /// 长度太小
    TooShortLength,
    /// 当前只允许Tcp连接
    OnlyTcp,
    /// 错误的CODE
    BadCode,
    /// 错误的文本
    BadText,
    /// 超时
    Timeout,
    /// 读数据超时
    ReadTimeout,
    /// 超过信息大小
    OverMsgSize,
    /// 读数据超出大小
    OverInbufferSize,
    /// 写数据超出大小
    OverOutbufferSize,

    /// SendClosed
    SendClosed(Command),
    /// SendFull
    SendFull(Command),
    /// 其它类型错误
    Extension(&'static str),
    /// websocket相关错误
    Ws(WsError),
    /// io错误
    Io(io::Error),
    /// webparse
    Web(WebError),
    /// kcp错误 
    Kcp(KcpError)
}

impl From<io::Error> for NetError {
    fn from(value: io::Error) -> Self {
        NetError::Io(value)
    }
}

impl From<WebError> for NetError {
    fn from(value: WebError) -> Self {
        NetError::Web(value)
    }
}

impl From<WsError> for NetError {
    fn from(value: WsError) -> Self {
        NetError::Ws(value)
    }
}

impl From<KcpError> for NetError {
    fn from(value: KcpError) -> Self {
        NetError::Kcp(value)
    }
}

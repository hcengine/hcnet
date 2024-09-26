
#[derive(Debug)]
pub enum WsError {
    UnknowHost,
    BadStatus,
    FailStatus(u16),
    ProtocolError(&'static str),
}
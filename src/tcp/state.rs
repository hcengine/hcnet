use crate::CloseCode;

pub enum TcpState {
    Open,
    Closing((CloseCode, String)),
    Closed,
}

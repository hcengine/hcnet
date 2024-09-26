use crate::CloseCode;

pub enum KcpState {
    Open,
    Closing((CloseCode, String)),
    Closed,
}

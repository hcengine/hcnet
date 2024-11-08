use std::usize;

use tokio::sync::mpsc::{channel, error::TrySendError};

use super::{CloseCode, Message, NetError, NetResult};

#[derive(Debug)]
pub struct Command {
    pub msg: Message,
}

pub type NetReceiver = tokio::sync::mpsc::Receiver<Command>;

#[derive(Clone)]
pub struct NetSender {
    channel: tokio::sync::mpsc::Sender<Command>,
    id: usize,
}

// unsafe impl Sync for NetSender {}
// unsafe impl Send for NetSender {}

impl NetSender {
    pub fn new(mut capacity: usize, id: usize) -> (NetSender, NetReceiver) {
        capacity = capacity.min(usize::MAX >> 3);
        let (channel, rv) = channel(capacity);
        (NetSender { channel, id }, rv)
    }

    pub fn send_message(&mut self, msg: Message) -> NetResult<()> {
        match self.channel.try_send(Command { msg }) {
            Ok(_) => return Ok(()),
            Err(TrySendError::Full(msg)) => return Err(NetError::SendFull(msg)),
            Err(TrySendError::Closed(msg)) => return Err(NetError::SendClosed(msg)),
        };
    }

    pub fn get_connection_id(&self) -> usize {
        self.id
    }

    pub fn close_with_reason(&mut self, code: CloseCode, reason: String) -> NetResult<()> {
        self.send_message(Message::Close(code, reason))?;
        Ok(())
    }

    pub async fn closed(&self) {
        self.channel.closed().await
    }

    pub fn is_closed(&self) -> bool {
        self.channel.is_closed()
    }
}

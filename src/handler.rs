use crate::ws::WsHandshake;

use super::{CloseCode, Message, NetConn, NetResult};
use async_trait::async_trait;
use log::trace;
use webparse::{Request, Response};

#[async_trait]
pub trait Handler {
    async fn on_accept(&mut self, conn: NetConn) -> NetResult<()> {
        let _conn = conn;
        unreachable!("Listener must impl accept")
    }

    async fn on_ready(&mut self) -> NetResult<()> {
        println!("ws on_handle");
        Ok(())
    }

    async fn on_open(&mut self) -> NetResult<()> {
        trace!("ws on_open");
        Ok(())
    }

    async fn on_close(&mut self, code: CloseCode, reason: String) {
        trace!(
            "on_close code = {}, reason = {reason}",
            Into::<u16>::into(code)
        );
    }

    async fn on_ping(&mut self, data: Vec<u8>) -> NetResult<Vec<u8>> {
        trace!("on_ping");
        Ok(data)
    }

    async fn on_pong(&mut self, data: Vec<u8>) -> NetResult<()> {
        let _data = data;
        trace!("on_pong");
        Ok(())
    }

    async fn on_message(&mut self, msg: Message) -> NetResult<()> {
        let _ = msg;
        Ok(())
    }

    async fn on_request(&mut self, req: Request<Vec<u8>>) -> NetResult<Response<Vec<u8>>> {
        WsHandshake::build_request(&req)
    }

    async fn on_response(&mut self, res: Request<Vec<u8>>) -> NetResult<()> {
        let _ = res;
        Ok(())
    }
}

use crate::ws::WsHandshake;

use super::{CloseCode, Message, NetConn, NetResult};
use async_trait::async_trait;
use log::trace;
use webparse::{Request, Response};

/// 连接处理回调函数
#[async_trait]
pub trait Handler {
    /// 此接口只有在服务端接受服务时进行触发
    /// 接收新的端口连接, 如果处理将在此触发
    async fn on_accept(&mut self, conn: NetConn) -> NetResult<()> {
        let _conn = conn;
        unreachable!("Listener must impl accept")
    }

    /// 此接口在可以发送消息时触发
    /// 例如websocket将在握手成功后触发该函数
    async fn on_open(&mut self) -> NetResult<()> {
        trace!("ws on_open");
        Ok(())
    }

    /// 此接口在远程服务端被关闭时进行触发
    async fn on_close(&mut self, code: CloseCode, reason: String) {
        trace!(
            "on_close code = {}, reason = {reason}",
            Into::<u16>::into(code)
        );
    }

    /// ping消息收到, 将会自动返回pong消息
    async fn on_ping(&mut self, data: Vec<u8>) -> NetResult<Vec<u8>> {
        trace!("on_ping");
        Ok(data)
    }

    /// pong消息
    async fn on_pong(&mut self, data: Vec<u8>) -> NetResult<()> {
        let _data = data;
        trace!("on_pong");
        Ok(())
    }

    /// message信息收到
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

    async fn on_logic(&mut self) -> NetResult<()> {
        let future = std::future::pending();
        let () = future.await;
        unreachable!()
    }
}

use std::usize;

use async_trait::async_trait;
use hcnet::{CloseCode, Handler, Message, NetConn, NetResult, NetSender};
use tokio::io::{AsyncBufReadExt, BufReader};

struct ClientHandler {
    sender: NetSender,
}

#[async_trait]
impl Handler for ClientHandler {
    async fn on_open(&mut self) -> NetResult<()> {
        println!("client on_handle");
        Ok(())
    }

    async fn on_message(&mut self, msg: Message) -> NetResult<()> {
        let _ = self.sender;
        println!("client read !!!!!!!!! receiver msg = {:?}", msg);
        Ok(())
    }

    /// 此接口在远程服务端被关闭时进行触发
    async fn on_close(&mut self, code: CloseCode, reason: String) {
        println!(
            "on_close code = {}, reason = {reason}",
            Into::<u16>::into(code)
        );
    }

    
    /// ping消息收到, 将会自动返回pong消息
    async fn on_ping(&mut self, data: Vec<u8>) -> NetResult<Option<Vec<u8>>> {
        println!("on_ping {:?}", data);
        Ok(Some(data))
    }

    /// pong消息
    async fn on_pong(&mut self, data: Vec<u8>) -> NetResult<()> {
        println!("on_pong = {:?}", data);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // let x = AtomicU64::new(0);
    // println!("val = {}", x.fetch_add(1, std::sync::atomic::Ordering::Relaxed));

    let args = std::env::args().collect::<Vec<String>>();
    let mut t = "tcp".to_string();
    if args.len() > 1 {
        t = args[1].clone();
    }
    let (mut sender, receiver) = NetSender::new(usize::MAX, 1);
    let conn = match &*t {
        "ws" => NetConn::ws_connect("ws://127.0.0.1:2003")
            .await
            .unwrap(),
        "wss" => NetConn::ws_connect("wss://test.wmproxy.net:2003")
            .await
            .unwrap(),
        "kcp" => NetConn::kcp_connect("127.0.0.1:2003").await.unwrap(),
        _ => NetConn::tcp_connect("127.0.0.1:2003").await.unwrap(),
    };
    let _ = conn
        .run_with_handler(
            ClientHandler {
                sender: sender.clone(),
            },
            receiver,
        )
        .await
        .unwrap();

    let mut stdin = BufReader::new(tokio::io::stdin());
    let mut content = String::new();
    loop {
        tokio::select! {
            _ = stdin.read_line(&mut content) => {
                content = content.trim().to_string();
                let v = content.split_whitespace().map(|v| v.to_string()).collect::<Vec<String>>();
                if v.len() == 0 {
                    continue;
                }
                match &*v[0] {
                    "exit" => {
                        sender
                            .close_with_reason(hcnet::CloseCode::Away, "exit".to_string())
                            .expect("ok");
                        break;
                    }
                    "ping" => {
                        let data = if v.len() > 1 {
                            v[1].as_bytes().to_vec()
                        } else {
                            "empty".as_bytes().to_vec()
                        };
                        sender
                            .send_message(Message::Ping(data))
                            .expect("ok");
                    }
                    "pong" => {
                        let data = if v.len() > 1 {
                            v[1].as_bytes().to_vec()
                        } else {
                            "empty".as_bytes().to_vec()
                        };
                        sender
                            .send_message(Message::Pong(data))
                            .expect("ok");
                    }
                    "bin" => {
                        let data = if v.len() > 1 {
                            v[1].as_bytes().to_vec()
                        } else {
                            "empty".as_bytes().to_vec()
                        };
                        sender
                            .send_message(Message::Binary(data))
                            .expect("ok");
                    }
                    _ => {

                        sender
                        .send_message(Message::Text(content.clone()))
                        .expect("ok");
                    }
                }
                content.clear();
            }
            _ = sender.closed() => {
                println!("closed!!!!!!!");
                break;
            }
        }
    }
}

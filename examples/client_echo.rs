use std::usize;

use async_trait::async_trait;
use hcnet::{Handler, Message, NetConn, NetResult, NetSender};
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
        "ws" => NetConn::ws_connect("ws://test.wmproxy.net:2003")
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
                if content == "exit" {
                    sender
                        .close_with_reason(hcnet::CloseCode::Away, "exit".to_string())
                        .expect("ok");
                    break;
                }
                sender
                    .send_message(Message::Text(content.clone()))
                    .expect("ok");
                content.clear();
            }
            _ = sender.closed() => {
                println!("closed!!!!!!!");
                break;
            }
        }
    }
}

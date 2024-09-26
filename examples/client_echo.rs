use async_trait::async_trait;
use hcnet::{Handler, Message, NetConn, NetResult, NetSender};
use tokio::io::{AsyncBufReadExt, BufReader};

struct ServerHandler;

struct ClientHandler {
    sender: NetSender,
}

#[async_trait]
impl Handler for ClientHandler {
    async fn on_message(&mut self, msg: Message) -> NetResult<()> {
        let _ = self.sender;
        println!("client read !!!!!!!!! receiver msg = {:?}", msg);
        Ok(())
    }
}

#[async_trait]
impl Handler for ServerHandler {
    async fn on_open(&mut self) -> NetResult<()> {
        println!("server on_handle");
        Ok(())
    }

    async fn on_accept(&mut self, conn: NetConn) -> NetResult<()> {
        println!("on accept remote = {:?}", conn.remote_addr());
        let _ = conn.run_handler(|sender| ClientHandler { sender }).await;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = std::env::args().collect::<Vec<String>>();
    let mut t = "tcp".to_string();
    if args.len() > 1 {
        t = args[1].clone();
    }
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
    let (mut sender, receiver) = NetSender::new(10, 1);
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

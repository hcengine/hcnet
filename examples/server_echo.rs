use async_trait::async_trait;
use hcnet::{Handler, Message, NetConn, NetResult, NetSender, Settings, TlsSettings};

struct ServerHandler;

struct ClientHandler {
    sender: NetSender,
}

#[async_trait]
impl Handler for ClientHandler {
    async fn on_message(&mut self, msg: Message) -> NetResult<()> {
        println!("server read !!!!!!!!! receiver msg = {:?}", msg);
        match msg {
            Message::Text(_) => self.sender.send_message(msg)?,
            Message::Binary(_) => self.sender.send_message(msg)?,
            _ => {}
        }
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
        "ws" => NetConn::ws_bind("0.0.0.0:2003", Settings::default())
            .await
            .unwrap(),
        "wss" => {
            let mut settings = Settings::default();
            settings.tls = Some(TlsSettings {
                domain: Some("test.wmproxy.net".to_string()),
                cert: "key/test.wmproxy.net.pem".to_string(),
                key: "key/test.wmproxy.net.key".to_string(),
            });

            NetConn::ws_bind("0.0.0.0:2003", settings).await.unwrap()
        }
        _ => NetConn::tcp_bind("0.0.0.0:2003", Settings::default())
            .await
            .unwrap(),
    };

    let h = conn.run_handler(|_| ServerHandler).await.unwrap();
    let _ = tokio::join!(h);
}

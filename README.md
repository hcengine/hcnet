
# hcnet

虚拟化服务端及客户端的底层, 可以轻松的切换tcp/tcps/ws/wss/udp等协议, 以方便在任意情况下切换协议.

## 回调处理

统一用回调进行处理函数
```rust

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
}


```

## 服务端监听

#### tcp监听
```rust
NetConn::tcp_bind("0.0.0.0:2003", Settings::default()).await
```

#### ws监听
```rust
NetConn::ws_bind("0.0.0.0:2003", Settings::default()).await
```

#### wss监听
监听时将证书信息配置即可轻松的支持wss协议
```rust
let mut settings = Settings::default();
settings.tls = Some(TlsSettings {
    domain: Some("example.com".to_string()),
    cert: "key/example.com.pem".to_string(),
    key: "key/example.com.key".to_string(),
});
NetConn::ws_bind("0.0.0.0:2003", Settings::default()).await
```

#### kcp(udp)监听
```rust
NetConn::kcp_bind("0.0.0.0:2003").await
```

#### 启动监听
```rust
let h = conn.run_handler(|_| ServerHandler).await.unwrap();
let _ = tokio::join!(h);
```
具体示例可参考[server_echo](./examples/server_echo.rs)

## 客户端连接

#### tcp连接
```rust
NetConn::tcp_connect("127.0.0.1:2003").await
```

#### ws连接
```rust
NetConn::ws_connect("ws://example.com:2003").await
```

#### wss连接
```rust
NetConn::ws_connect("wss://example.com:2003").await
```

#### kcp(udp)连接
```rust
NetConn::kcp_connect("wss://example.com:2003").await
```

#### 启动监听
```rust
let (mut sender, receiver) = NetSender::new(10, 1);
let _ = conn.run_with_handler(
        ClientHandler {
            sender: sender.clone(),
        },
        receiver,
    ).await;
```
或者
```rust
let _ = conn.run_handler(|sender| ClientHandler { sender }).await;
```

具体示例可参考[client_echo](./examples/client_echo.rs)


## 启动demo
先启动服务端
```bash
cargo run --example server_echo tcp
```
再启动客户端
```bash
cargo run --example client_echo tcp
```

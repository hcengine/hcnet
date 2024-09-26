use std::time::Duration;

use crate::{
    Message, NetError, NetResult, {CloseCode, MaybeTlsStream, Settings},
};
use algorithm::buf::{BinaryMut, Bt, BtMut};
use base64::{prelude::BASE64_STANDARD, Engine};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt, ReadBuf},
    net::TcpStream,
    time::{self, Instant},
};
use webparse::{
    ws::{DataFrame, DataFrameable, OwnedMessage},
    HttpError, Request, Response, Url, WebError,
};

use super::{WsError, WsMsgReceiver, WsState};

pub struct WsClient {
    stream: MaybeTlsStream,
    url: Url,
    state: WsState,

    read: BinaryMut,
    write: BinaryMut,
}

impl WsClient {
    pub async fn connect(url: Url) -> NetResult<WsClient> {
        let domain = unwrap_or!(url.domain.clone(), return Err(WsError::UnknowHost.into()));
        let port = unwrap_or!(url.port, return Err(WsError::UnknowHost.into()));
        match url.scheme {
            webparse::Scheme::Ws => {
                let stream = TcpStream::connect(format!("{domain}:{port}")).await?;
                Ok(WsClient {
                    stream: MaybeTlsStream::from(stream),
                    url,
                    state: WsState::Wait,
                    read: BinaryMut::new(),
                    write: BinaryMut::new(),
                })
            }
            webparse::Scheme::Wss => {
                let stream = TcpStream::connect(format!("{domain}:{port}")).await?;
                let stream = MaybeTlsStream::connect_tls(stream, domain).await?;
                Ok(WsClient {
                    stream,
                    url,
                    state: WsState::Wait,
                    read: BinaryMut::new(),
                    write: BinaryMut::new(),
                })
            }
            _ => return Err(WsError::ProtocolError("dismatch scheme only support ws, wss").into()),
        }
    }

    pub(crate) fn send_message(&mut self, msg: Message) -> NetResult<()> {
        let msg: OwnedMessage = msg.into();
        msg.write_to(&mut self.write, Some(rand::random()))?;
        Ok(())
    }

    pub(crate) fn close(&mut self, code: CloseCode, reason: String) -> NetResult<()> {
        self.send_message(Message::Close(code.into(), reason.clone()))?;
        self.state = WsState::Closing((code, reason));
        Ok(())
    }

    pub(crate) fn handler_response(&mut self, _res: Response<Vec<u8>>) -> NetResult<()> {
        match &self.state {
            WsState::WaitRet => {
                self.state = WsState::Open;
            }
            _ => return Err(WsError::BadStatus.into()),
        }
        Ok(())
    }

    async fn process_io(&mut self, only_write: bool, settings: &Settings) -> NetResult<bool> {
        if self.is_inbuffer_full(settings) {
            return Err(crate::NetError::OverInbufferSize);
        }
        let (mut reader, mut writer) = split(&mut self.stream);
        loop {
            let mut buf = ReadBuf::uninit(self.read.chunk_mut());
            tokio::select! {
                val = reader.read_buf(&mut buf), if !only_write => {
                    val?;
                    let s = buf.filled().len();
                    unsafe {
                        self.read.advance_mut(s);
                    }
                    return Ok(s == 0);
                }
                // 一旦有写数据，则尝试写入数据，写入成功后扣除相应的数据
                r = writer.write(self.write.chunk()), if self.write.has_remaining() => {

                    let n = r?;
                    self.write.advance(n);
                    if !self.write.has_remaining() {
                        self.write.clear();
                    }
                }
            }

            if only_write && self.write.is_empty() {
                return Ok(true);
            }
        }
    }

    pub(crate) async fn process(&mut self, settings: &Settings) -> NetResult<WsMsgReceiver> {
        let mut vec = vec![];
        loop {
            match &self.state {
                WsState::Wait => {
                    let mut req = Request::builder()
                        .method("GET")
                        .url(self.url.clone())
                        .body(vec![])
                        .unwrap();
                    let header = req.headers_mut();
                    header.insert("Connection", "Upgrade");
                    header.insert("Upgrade", "websocket");
                    let key: [u8; 16] = rand::random();
                    header.insert("Sec-WebSocket-Key", BASE64_STANDARD.encode(&key));
                    header.insert("Sec-WebSocket-Version", "13");
                    header.insert("Sec-WebSocket-Protocol", "chat, superchat");
                    let data = req.http1_data()?;
                    println!("all request = {:?}", String::from_utf8_lossy(&data));
                    self.write.put(&data[..]);

                    self.state = WsState::WaitRet;
                }
                WsState::WaitRet => {
                    let util_time =
                        Instant::now() + Duration::from_millis(settings.shake_timeout as u64);

                    let mut response = Response::new(vec![]);
                    loop {
                        tokio::select! {
                            is_end = self.process_io(false, settings) => {
                                if is_end? {
                                    return Ok(WsMsgReceiver::Msg(Message::Shutdown));
                                }
                            }
                            _ = time::sleep_until(util_time) => {
                                return Err(NetError::Timeout.into());
                            }
                        }
                        self.read.mark();
                        println!("all ret = {:?}", String::from_utf8_lossy(self.read.chunk()));
                        let s = match response.parse_buffer(&mut self.read.chunk()) {
                            Ok(s) => s,
                            Err(WebError::Http(HttpError::Partial)) => {
                                self.read.rewind_mark();
                                continue;
                            }
                            Err(e) => {
                                return Err(e.into());
                            }
                        };

                        if !response.is_partial() {
                            self.read.advance(s);
                            if response.status() != 101 {
                                println!("body = {:?}", String::from_utf8_lossy(response.body()));
                                return Err(WsError::FailStatus(response.status().as_u16()).into());
                            }
                            return Ok(WsMsgReceiver::Res(response));
                        }
                    }
                }
                WsState::Open => {
                    loop {
                        self.read.mark();
                        let frame = match DataFrame::read_dataframe_with_limit(
                            &mut self.read,
                            false,
                            100000,
                        ) {
                            Ok(frame) => frame,
                            Err(WebError::Io(e))
                                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                            {
                                self.read.rewind_mark();
                                break;
                            }
                            Err(e) => return Err(e.into()),
                        };
                        let is_finish = frame.is_last();
                        vec.push(frame);
                        if is_finish {
                            let msg = Message::from(OwnedMessage::from_dataframes(vec)?);
                            return Ok(WsMsgReceiver::Msg(msg));
                        }
                    }
                    let is_end = self.process_io(false, settings).await?;
                    if is_end {
                        return Ok(WsMsgReceiver::Msg(Message::Shutdown));
                    }
                }
                WsState::Closing(_) => {
                    tokio::select! {
                        _ = self.process_io(true, settings) => {
                            self.closing_to_closed();
                        }
                        _ = time::sleep(Duration::from_millis(settings.closing_time as u64)) => {
                            self.closing_to_closed();
                        }
                    }
                }
                WsState::Closed(_) => return Ok(WsMsgReceiver::Msg(Message::Shutdown)),
            }
        }
    }

    fn closing_to_closed(&mut self) {
        match &self.state {
            WsState::Closing(v) => self.state = WsState::Closed(v.clone()),
            _ => unreachable!(),
        }
    }

    pub fn is_ready(&self) -> bool {
        match &self.state {
            WsState::Open => true,
            _ => false,
        }
    }

    pub(crate) fn is_inbuffer_full(&self, settings: &Settings) -> bool {
        self.read.len() >= settings.in_buffer_max
    }

    pub(crate) fn is_outbuffer_full(&self, settings: &Settings) -> bool {
        self.write.len() >= settings.out_buffer_max
    }
}

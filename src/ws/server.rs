use std::{net::SocketAddr, time::Duration};

use crate::{
    Message, NetError, NetResult,
    {protocol::CloseCode, MaybeTlsStream, Settings},
};
use algorithm::buf::{BinaryMut, Bt, BtMut};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt, ReadBuf},
    time::{self, Instant},
};
use webparse::{
    ws::{DataFrame, DataFrameable, OwnedMessage},
    HttpError, Request, Response, Serialize, WebError,
};

use super::{WsError, WsMsgReceiver, WsState};

/// websocket的服务端
pub struct WsServer {
    /// 当前可能是tcp也可能是tcps的连接
    stream: MaybeTlsStream,
    /// 远程连接的socket地址
    addr: SocketAddr,
    /// ws的连接状态
    state: WsState,
    /// 读缓存
    read: BinaryMut,
    /// 写缓存
    write: BinaryMut,
}

impl WsServer {
    pub fn new(stream: MaybeTlsStream, addr: SocketAddr) -> WsServer {
        WsServer {
            stream,
            addr,
            state: WsState::Wait,
            read: BinaryMut::new(),
            write: BinaryMut::new(),
        }
    }

    /// 构建返回结果, 如果非101的状态码后续将直接关闭状态
    pub(crate) fn handler_response(&mut self, mut res: Response<Vec<u8>>) -> NetResult<()> {
        match &self.state {
            WsState::WaitRet => {
                res.serialize(&mut self.write)?;
                self.state = WsState::Open;
            }
            _ => return Err(WsError::BadStatus.into()),
        }
        Ok(())
    }

    pub(crate) fn send_message(&mut self, msg: Message) -> NetResult<()> {
        let msg: OwnedMessage = msg.into();
        msg.write_to(&mut self.write, None)?;
        Ok(())
    }

    pub(crate) fn close(&mut self, code: CloseCode, reason: String) -> NetResult<()> {
        match self.state {
            WsState::Open => self.send_message(Message::Close(code.into(), reason.clone()))?,
            WsState::Closing(_) => return Ok(()),
            WsState::Closed(_) => return Ok(()),
            _ => {}
        }
        self.state = WsState::Closing((code, reason));
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
                // 读取数据, 在非仅写入的情况下
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
                _ = tokio::time::sleep(Duration::from_millis(settings.read_timeout as u64)) => {
                    return Err(NetError::ReadTimeout.into());
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
                    let util_time =
                        Instant::now() + Duration::from_millis(settings.shake_timeout as u64);
                    let mut request = Request::new();
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
                        let s = match request.parse_buffer(&mut self.read.chunk()) {
                            Ok(s) => s,
                            Err(WebError::Http(HttpError::Partial)) => {
                                self.read.rewind_mark();
                                continue;
                            }
                            Err(e) => {
                                return Err(e.into());
                            }
                        };
                        if !request.is_partial() {
                            self.read.advance(s);
                            self.state = WsState::WaitRet;
                            return Ok(WsMsgReceiver::Req(request.into(vec![]).0));
                        }
                    }
                }
                WsState::WaitRet => {
                    self.state = WsState::Open;
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

    pub fn remote_addr(&self) -> Option<SocketAddr> {
        Some(self.addr)
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

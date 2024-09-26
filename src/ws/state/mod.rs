use crate::CloseCode;

#[derive(Debug)]
pub enum WsState {
    /// 等待握手
    Wait,
    /// 等待握手返回, 客户端使用
    WaitRet,
    /// 建立链接
    Open,
    /// 状态正在关闭中
    Closing((CloseCode, String)),
    /// 已关闭
    Closed((CloseCode, String)),
}

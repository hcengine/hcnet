#[derive(Debug, Clone)]
pub struct Settings {
    /// 最大监听连接数
    /// 默认值: 1024
    pub max_connections: usize,
    /// 默认队列大小
    /// 默认值: 10
    pub queue_size: usize,
    /// 读数据的最大容量
    /// 默认值: 1024 * 1024 * 100 = 10M
    pub in_buffer_max: usize,
    /// 写数据的最大容量
    /// 默认值: 1024 * 1024 * 100 = 10M
    pub out_buffer_max: usize,
    /// 单信息最大的数量
    /// 默认值: 65535
    pub onemsg_max_size: usize,
    /// 最关闭状态下留给写入的最长时间, 单位毫秒
    /// 默认值: 1000ms
    pub closing_time: usize,
    /// 连接的最大时长
    /// 默认值: 30000ms
    pub connect_timeout: usize,
    /// 握手的最大时长
    /// 默认值: 30000ms
    pub shake_timeout: usize,

    /// 跟Tls相关内容
    pub tls: Option<TlsSettings>,
}

#[derive(Debug, Clone)]
pub struct TlsSettings {
    /// 证书所用域名
    pub domain: Option<String>,
    /// 证书的公钥文件
    pub cert: String,
    /// 证书的私钥文件
    pub key: String,

}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_connections: 1024,
            queue_size: 10,
            in_buffer_max: 10485760,
            out_buffer_max: 10485760,
            onemsg_max_size: 65535,
            closing_time: 1000,
            connect_timeout: 30000,
            shake_timeout: 30000,

            tls: None,
        }
    }
}


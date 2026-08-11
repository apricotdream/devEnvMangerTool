//! 统一错误类型（对应 Python 里 `ValueError` / `FileNotFoundError` / `RuntimeError` 的语义）。

use std::path::PathBuf;

/// 核心库错误。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// 用户输入/业务校验失败（对应 Python `ValueError`）
    #[error("{0}")]
    Validation(String),

    /// 命令未找到（对应 Python `FileNotFoundError`）
    #[error("{0}")]
    CommandNotFound(String),

    /// 运行时失败（对应 Python `RuntimeError`，保留 stderr 输出）
    #[error("{0}")]
    Runtime(String),

    /// 路径不存在
    #[error("路径不存在：{0}")]
    PathNotFound(PathBuf),

    /// 文件读写失败
    #[error("文件操作失败：{context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    /// JSON 解析失败
    #[error("JSON 解析失败：{0}")]
    Json(#[from] serde_json::Error),

    /// 子进程执行超时（对应 Python `subprocess.TimeoutExpired`）
    #[error("子进程执行超时")]
    Timeout,
}

impl Error {
    /// 便捷构造业务校验错误
    pub fn validation(message: impl Into<String>) -> Self {
        Error::Validation(message.into())
    }

    /// 便捷构造命令未找到错误
    pub fn command_not_found(message: impl Into<String>) -> Self {
        Error::CommandNotFound(message.into())
    }

    /// 便捷构造运行时错误
    pub fn runtime(message: impl Into<String>) -> Self {
        Error::Runtime(message.into())
    }
}

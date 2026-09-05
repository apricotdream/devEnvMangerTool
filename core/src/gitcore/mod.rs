//! git-env-core：git-env-gui 核心业务逻辑。
//!
//! 从 Python 版 `src/service.py` + `src/platform_utils.py` 移植而来，零 GUI 依赖，
//! 同时被 `cli`（命令行）与 `app`（Tauri GUI）复用。
//!
//! 模块映射：
//! - [`account`]    —— Python `GitAccount` 数据类
//! - [`ssh_config`] —— SSH config 解析/写入（含 `git_meta:` 注释块，逐字节兼容）
//! - [`service`]    —— Python `GitAccountService` 核心业务
//! - [`platform`]   —— Python `platform_utils.py`（命令解析、目录回退、打开文件管理器）
//! - [`stats`]      —— `git_account_stats.json` 读写
//! - [`error`]      —— 统一错误类型

pub mod account;
pub mod error;
pub mod platform;
pub mod service;
pub mod ssh_config;
pub mod stats;

/// 元数据注释前缀，与 Python `service.META_PREFIX` 保持一致。
pub const META_PREFIX: &str = "git_meta:";

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_smoke() {
        assert_eq!(2 + 2, 4);
    }
}

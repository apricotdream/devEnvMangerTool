//! 账号数据结构（对应 Python `service.GitAccount`）。
//!
//! `start` / `end` / `comment_start` 表示该账号在 SSH config 文件中的行区间，
//! 用于增删改时精确替换文本块。

use std::path::PathBuf;

/// 单个 Git 账号（对应 SSH config 中的一个 `Host` 块）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Host 别名，如 `github-work`
    pub alias: String,
    /// 主机地址（HostName）
    pub host_name: String,
    /// SSH 端口
    pub port: String,
    /// SSH 用户
    pub ssh_user: String,
    /// 私钥路径
    pub key_path: Option<PathBuf>,
    /// Git 用户名
    pub git_name: String,
    /// Git 邮箱
    pub git_email: String,
    /// 该 Host 块在 config 中的起始行号
    pub start: i64,
    /// 该 Host 块结束后的行号
    pub end: i64,
    /// 前置注释块起始行号
    pub comment_start: i64,
}

impl Account {
    /// 公钥路径 = 私钥路径 + `.pub`
    pub fn public_key_path(&self) -> Option<PathBuf> {
        self.key_path.as_ref().map(|p| {
            let mut os_str = p.as_os_str().to_owned();
            os_str.push(".pub");
            PathBuf::from(os_str)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_key_path_appends_suffix() {
        let account = Account {
            alias: "github".into(),
            host_name: "github.com".into(),
            port: "22".into(),
            ssh_user: "git".into(),
            key_path: Some(PathBuf::from("/home/u/.ssh/id_rsa_github")),
            git_name: "n".into(),
            git_email: "e@x.com".into(),
            start: 0,
            end: 0,
            comment_start: 0,
        };
        assert_eq!(
            account.public_key_path(),
            Some(PathBuf::from("/home/u/.ssh/id_rsa_github.pub"))
        );
    }
}

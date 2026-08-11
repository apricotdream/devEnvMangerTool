//! SSH config 解析/写入（含 `git_meta:` 注释块）。
//!
//! 目标：与 Python 版 `service.py` 的解析与写出结果**逐字节兼容**，
//! 由 `tests/compat.rs` 用 `tests/fixtures/` 下的金丝雀样例验证。
//!
//! 移植自 `service.py` 的私有方法：
//! - `_parse_accounts_from_config` → [`parse_accounts`]
//! - `_parse_block_fields`          → [`parse_block_fields`]
//! - `_parse_name_email`            → [`parse_name_email`]
//! - `_find_comment_start`          → [`find_comment_start`]
//! - `_render_account_block`        → [`render_account_block`]
//! - `_write_lines`（规范化）       → [`write_lines`]
//! - `_meta_escape`                 → [`meta_escape`]

use std::path::PathBuf;

use crate::gitcore::account::Account;
use crate::gitcore::META_PREFIX;

/// 把 config 文本拆成行（与 Python `read_text().splitlines()` 行为对齐：
/// 兼容 `\n` 与 `\r\n`）。
pub fn read_lines(text: &str) -> Vec<String> {
    text.lines().map(|line| line.to_string()).collect()
}

/// 解析 config 中的所有账号块，按 `alias` 小写排序。
///
/// 对应 Python `GitAccountService._parse_accounts_from_config`。
pub fn parse_accounts(lines: &[String]) -> Vec<Account> {
    let mut accounts: Vec<Account> = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let stripped = lines[index].trim();
        if !stripped.to_ascii_lowercase().starts_with("host ") {
            index += 1;
            continue;
        }

        let alias = stripped
            .splitn(2, ' ')
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or("")
            .to_string();

        let start = index as i64;
        index += 1;
        while index < lines.len() {
            let stripped = lines[index].trim();
            if stripped.is_empty() || stripped.to_ascii_lowercase().starts_with("host ") {
                break;
            }
            index += 1;
        }
        let end = index as i64;

        let comment_start = find_comment_start(lines, start as usize) as i64;
        let comment_lines: Vec<String> = lines[comment_start as usize..start as usize]
            .iter()
            .filter(|line| line.trim_start().starts_with('#'))
            .cloned()
            .collect();

        let fields = parse_block_fields(&lines[start as usize..end as usize]);
        let (git_name, git_email) = parse_name_email(&comment_lines);
        let key_path = fields
            .iter()
            .find(|(key, _)| key == "identityfile")
            .map(|(_, value)| PathBuf::from(value));
        let host_name = fields
            .iter()
            .find(|(key, _)| key == "hostname")
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| alias.clone());

        accounts.push(Account {
            alias,
            host_name,
            port: fields
                .iter()
                .find(|(key, _)| key == "port")
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| "22".to_string()),
            ssh_user: fields
                .iter()
                .find(|(key, _)| key == "user")
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| "git".to_string()),
            key_path,
            git_name,
            git_email,
            start,
            end,
            comment_start,
        });
    }

    accounts.sort_by(|a, b| a.alias.to_lowercase().cmp(&b.alias.to_lowercase()));
    accounts
}

/// 解析一个 Host 块内（去掉首行 `Host ...`）的 key/value 字段。
///
/// 对应 Python `GitAccountService._parse_block_fields`。
/// 规则：每行按第一个空白切分；只有 key 没有 value 的行跳过；
/// value 去除两端空白和 `"`/`'`，key 转为小写。
pub fn parse_block_fields(block_lines: &[String]) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    for line in block_lines.iter().skip(1) {
        let trimmed = line.trim();
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or("");
        let value = parts.next().map(|v| v.trim()).unwrap_or("");
        if key.is_empty() || value.is_empty() {
            continue;
        }
        let value = value.trim_matches(|c| c == '"' || c == '\'');
        fields.push((key.to_ascii_lowercase(), value.to_string()));
    }
    fields
}

/// 从注释行中提取 Git 用户名与邮箱。
///
/// 对应 Python `GitAccountService._parse_name_email`。
/// 优先解析 `git_meta: name=...;email=...` 元数据；
/// 没有则回退到 `# Git account: Name <email>` 正则匹配。
pub fn parse_name_email(comment_lines: &[String]) -> (String, String) {
    for line in comment_lines {
        let stripped = line.trim();
        if !stripped.contains(META_PREFIX) {
            continue;
        }
        let meta = stripped
            .split_once(META_PREFIX)
            .map(|(_, rest)| rest)
            .unwrap_or("");
        let mut name = String::new();
        let mut email = String::new();
        for part in meta.split(';') {
            if let Some((key, value)) = part.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if key == "name" {
                    name = value.to_string();
                } else if key == "email" {
                    email = value.to_string();
                }
            }
        }
        return (name, email);
    }

    for line in comment_lines {
        if let Some((name, email)) = parse_name_email_from_comment(line) {
            return (name, email);
        }
    }

    (String::new(), String::new())
}

/// 正则回退：`#\s*(?:Git\s*(?:account|账号)\s*:\s*)?(.*?)\s*<([^<>]+@[^<>]+)>`（忽略大小写）。
fn parse_name_email_from_comment(line: &str) -> Option<(String, String)> {
    let lt = line.find('<')?;
    let gt = line[lt..].find('>')? + lt;
    let inner = &line[lt + 1..gt];
    if !inner.contains('@') {
        return None;
    }
    let email = inner.trim().to_string();

    let before = line[..lt].trim();
    let before = before.trim_start_matches('#').trim();
    let before = strip_prefix_ignore_case(before, "Git account:");
    let before = strip_prefix_ignore_case(before, "Git 账号:");
    let name = before.trim().to_string();
    Some((name, email))
}

/// 忽略大小写地去掉前缀 `prefix`，不匹配则原样返回。
fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> &'a str {
    let lower = s.to_ascii_lowercase();
    let prefix_lower = prefix.to_ascii_lowercase();
    if lower.starts_with(&prefix_lower) {
        &s[prefix.len()..]
    } else {
        s
    }
}

/// 向前找到该 Host 块所属注释块的起始行（不吞并前一个块后的空行分隔符）。
///
/// 对应 Python `GitAccountService._find_comment_start`（已修复：只在连续注释行上回退）。
pub fn find_comment_start(lines: &[String], host_start: usize) -> usize {
    let mut index = host_start;
    while index > 0 {
        let previous = lines[index - 1].trim();
        if previous.starts_with('#') {
            index -= 1;
        } else {
            break;
        }
    }
    index
}

/// 渲染一个账号的完整 config 文本块（注释 + Host 块）。
///
/// 对应 Python `GitAccountService._render_account_block`。
/// `updated_at` 由调用方注入（便于测试做确定性断言）。
pub fn render_account_block(account: &Account, updated_at: &str) -> Vec<String> {
    let meta_name = meta_escape(&account.git_name);
    let meta_email = meta_escape(&account.git_email);
    let key_path = account
        .key_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();

    vec![
        format!("# {}", "=".repeat(56)),
        format!("# Git account: {} <{}>", account.git_name, account.git_email),
        format!("# host: {} (port: {})", account.host_name, account.port),
        format!("# SSH user: {}", account.ssh_user),
        format!("# IdentityFile: {}", key_path),
        format!(
            "# {} name={};email={};updated_at={}",
            META_PREFIX, meta_name, meta_email, updated_at
        ),
        format!("# {}", "=".repeat(56)),
        format!("Host {}", account.alias),
        format!("    HostName {}", account.host_name),
        format!("    Port {}", account.port),
        format!("    User {}", account.ssh_user),
        format!("    IdentityFile {}", key_path),
        "    IdentitiesOnly yes".to_string(),
        "    PreferredAuthentications publickey".to_string(),
    ]
}

/// 转义元数据字段中的 `;` 与换行。
///
/// 对应 Python `GitAccountService._meta_escape`。
pub fn meta_escape(value: &str) -> String {
    value.replace(';', ",").replace('\n', " ").trim().to_string()
}

/// 把行列表写为规范化文本（对应 Python `_write_lines`）。
///
/// 规则：`\n` 连接；3 个以上连续换行折叠为 2 个；去掉首尾换行；非空末尾补一个换行。
pub fn write_lines(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    let text = text.trim_matches('\n');
    if text.is_empty() {
        String::new()
    } else {
        format!("{text}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_parses_to_empty() {
        let lines = read_lines("");
        assert!(parse_accounts(&lines).is_empty());
    }

    #[test]
    fn meta_escape_replaces_semicolon_and_newline() {
        assert_eq!(meta_escape("a;b"), "a,b");
        assert_eq!(meta_escape("line1\nline2"), "line1 line2");
        assert_eq!(meta_escape("  padded  "), "padded");
    }

    #[test]
    fn write_lines_normalizes_newlines() {
        assert_eq!(write_lines(&["a".into()]), "a\n");
        assert_eq!(write_lines(&["a".into(), "".into(), "".into(), "b".into()]), "a\n\nb\n");
        assert_eq!(write_lines(&[]), "");
        assert_eq!(write_lines(&["".into()]), "");
    }

    #[test]
    fn find_comment_start_stops_at_blank_separator() {
        let lines = vec![
            "# top comment".to_string(),
            "".to_string(),
            "Host github".to_string(),
            "    HostName github.com".to_string(),
        ];
        // 空行是块分隔符，不属于注释块
        assert_eq!(find_comment_start(&lines, 2), 2);
        assert_eq!(find_comment_start(&lines, 3), 3);
    }

    #[test]
    fn find_comment_start_absorbs_contiguous_comments() {
        let lines = vec![
            "# a".to_string(),
            "# b".to_string(),
            "Host github".to_string(),
        ];
        assert_eq!(find_comment_start(&lines, 2), 0);
    }

    #[test]
    fn find_comment_start_stops_at_real_content() {
        let lines = vec![
            "Host other".to_string(),
            "".to_string(),
            "Host github".to_string(),
        ];
        assert_eq!(find_comment_start(&lines, 2), 2);
    }

    #[test]
    fn parse_block_fields_skips_key_only_lines() {
        let block = vec![
            "Host github".to_string(),
            "    HostName github.com".to_string(),
            "    IdentitiesOnly".to_string(),
            "    Port \"2222\"".to_string(),
        ];
        let fields = parse_block_fields(&block);
        assert_eq!(
            fields,
            vec![
                ("hostname".to_string(), "github.com".to_string()),
                ("port".to_string(), "2222".to_string()),
            ]
        );
    }

    #[test]
    fn parse_name_email_prefers_meta() {
        let comments = vec![
            "# Git account: Old <old@example.com>".to_string(),
            format!("# {} name=New;email=new@example.com", META_PREFIX),
        ];
        let (name, email) = parse_name_email(&comments);
        assert_eq!(name, "New");
        assert_eq!(email, "new@example.com");
    }

    #[test]
    fn parse_name_email_falls_back_to_comment() {
        let comments = vec!["# Git account: Old Dev <old.dev@example.com>".to_string()];
        let (name, email) = parse_name_email(&comments);
        assert_eq!(name, "Old Dev");
        assert_eq!(email, "old.dev@example.com");
    }

    #[test]
    fn render_block_matches_python_layout() {
        let account = Account {
            alias: "github-work".into(),
            host_name: "github.com".into(),
            port: "22".into(),
            ssh_user: "git".into(),
            key_path: Some(PathBuf::from("__FIXTURE_SSH_DIR__\\id_rsa_github-work")),
            git_name: "Jane Doe".into(),
            git_email: "jane.doe@example.com".into(),
            start: 0,
            end: 0,
            comment_start: 0,
        };
        let rendered = render_account_block(&account, "2026-08-11 15:55:19");
        let expected = vec![
            "# ========================================================",
            "# Git account: Jane Doe <jane.doe@example.com>",
            "# host: github.com (port: 22)",
            "# SSH user: git",
            "# IdentityFile: __FIXTURE_SSH_DIR__\\id_rsa_github-work",
            "# git_meta: name=Jane Doe;email=jane.doe@example.com;updated_at=2026-08-11 15:55:19",
            "# ========================================================",
            "Host github-work",
            "    HostName github.com",
            "    Port 22",
            "    User git",
            "    IdentityFile __FIXTURE_SSH_DIR__\\id_rsa_github-work",
            "    IdentitiesOnly yes",
            "    PreferredAuthentications publickey",
        ];
        assert_eq!(rendered, expected);
    }
}

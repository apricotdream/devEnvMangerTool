//! `git_account_stats.json` 读写（对应 Python `service.GitAccountService` 中的 stats 部分）。
//!
//! 格式与 Python 完全一致：`{"accounts": {alias: {...}}}`，用 `IndexMap` 保持插入顺序，
//! 与 Python dict 的写入顺序一致。

use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::gitcore::error::Error;

/// 单个账号的连接/同步统计（字段与 Python `_empty_account_stats` 一一对应）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountStats {
    #[serde(default)]
    pub last_sync_at: String,
    #[serde(default)]
    pub last_sync_repo: String,
    #[serde(default)]
    pub last_connection_ok: Option<bool>,
    #[serde(default)]
    pub last_connection_at: String,
    #[serde(default)]
    pub last_connection_message: String,
}

/// stats 文件顶层结构。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatsData {
    #[serde(default)]
    pub accounts: IndexMap<String, AccountStats>,
}

impl StatsData {
    /// 从文件读取；文件不存在或损坏时返回空数据（与 Python `_load_stats` 行为一致）。
    pub fn load(path: &Path) -> Result<StatsData, Error> {
        if !path.exists() {
            return Ok(StatsData::default());
        }
        let text = std::fs::read_to_string(path).map_err(|e| Error::Io {
            context: format!("读取 stats 失败：{}", path.display()),
            source: e,
        })?;
        let data = serde_json::from_str(&text).unwrap_or_else(|_| StatsData::default());
        Ok(data)
    }

    /// 写入文件（与 Python `json.dumps(data, ensure_ascii=False, indent=2) + "\n"` 对齐）。
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let text = serde_json::to_string_pretty(self).map_err(Error::Json)?;
        let text = format!("{text}\n");
        std::fs::write(path, text).map_err(|e| Error::Io {
            context: format!("写入 stats 失败：{}", path.display()),
            source: e,
        })
    }

    /// 取若干账号的统计快照；未记录的账号返回空统计（对应 Python `get_stats_snapshot`）。
    pub fn snapshot(&self, aliases: &[String]) -> HashMap<String, AccountStats> {
        let mut out = HashMap::with_capacity(aliases.len());
        for alias in aliases {
            let entry = self
                .accounts
                .get(alias)
                .cloned()
                .unwrap_or_default();
            out.insert(alias.clone(), entry);
        }
        out
    }

    /// 确保某个账号存在统计条目（对应 Python `_ensure_stats_entry`）。
    pub fn entry(&mut self, alias: &str) -> &mut AccountStats {
        self.accounts
            .entry(alias.to_string())
            .or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stats_roundtrip() {
        let data = StatsData::default();
        let json = serde_json::to_string(&data).unwrap();
        assert_eq!(json, r#"{"accounts":{}}"#);
    }

    #[test]
    fn snapshot_returns_empty_for_missing() {
        let data = StatsData::default();
        let snap = data.snapshot(&["gh".to_string()]);
        assert_eq!(snap["gh"], AccountStats::default());
    }

    #[test]
    fn account_stats_default_fields_match_python() {
        let s = AccountStats::default();
        assert_eq!(s.last_sync_at, "");
        assert_eq!(s.last_sync_repo, "");
        assert_eq!(s.last_connection_ok, None);
        assert_eq!(s.last_connection_at, "");
        assert_eq!(s.last_connection_message, "");
    }
}

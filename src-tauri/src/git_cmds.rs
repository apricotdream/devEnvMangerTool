//! Git 账号管理命令层（来自 gitAccessTool）。
//!
//! 命令统一加 `git_` 前缀，避免与 npm 账号命令（list_accounts/delete_account 等）冲突。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::gitcore::account::Account;
use crate::gitcore::error::Error;
use crate::gitcore::platform;
use crate::gitcore::service::Service;
use crate::gitcore::stats::AccountStats;
use serde::{Deserialize, Serialize};
use tauri::State;

/// 可共享的 Git 服务状态。
#[derive(Clone)]
pub struct ServiceState(pub Arc<Mutex<Service>>);

// ── DTO ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInput {
    pub alias: String,
    pub host_name: String,
    pub port: String,
    pub ssh_user: String,
    pub key_path: Option<String>,
    pub git_name: String,
    pub git_email: String,
}

impl AccountInput {
    fn into_account(self, key_path: Option<PathBuf>) -> Account {
        Account {
            alias: self.alias,
            host_name: self.host_name,
            port: self.port,
            ssh_user: self.ssh_user,
            key_path,
            git_name: self.git_name,
            git_email: self.git_email,
            start: 0,
            end: 0,
            comment_start: 0,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub alias: String,
    pub host_name: String,
    pub port: String,
    pub ssh_user: String,
    pub key_path: Option<String>,
    pub git_name: String,
    pub git_email: String,
}

impl From<&Account> for AccountDto {
    fn from(a: &Account) -> Self {
        AccountDto {
            alias: a.alias.clone(),
            host_name: a.host_name.clone(),
            port: a.port.clone(),
            ssh_user: a.ssh_user.clone(),
            key_path: a.key_path.as_ref().map(|p| p.display().to_string()),
            git_name: a.git_name.clone(),
            git_email: a.git_email.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsDto {
    pub last_sync_at: String,
    pub last_sync_repo: String,
    pub last_connection_ok: Option<bool>,
    pub last_connection_at: String,
    pub last_connection_message: String,
}

impl From<&AccountStats> for StatsDto {
    fn from(s: &AccountStats) -> Self {
        StatsDto {
            last_sync_at: s.last_sync_at.clone(),
            last_sync_repo: s.last_sync_repo.clone(),
            last_connection_ok: s.last_connection_ok,
            last_connection_at: s.last_connection_at.clone(),
            last_connection_message: s.last_connection_message.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDto {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResultDto {
    pub success: bool,
    pub output: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResultDto {
    pub alias: String,
    pub output: String,
}

// ── 辅助 ────────────────────────────────────────────────────────

async fn run_blocking<T, F>(state: &ServiceState, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut Service) -> Result<T, Error> + Send + 'static,
{
    let arc = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut svc = arc.lock().unwrap();
        f(&mut svc).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── 命令 ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn git_list_accounts(state: State<'_, ServiceState>) -> Result<Vec<AccountDto>, String> {
    run_blocking(&state, |svc| {
        svc.list_accounts().map(|v| v.iter().map(AccountDto::from).collect())
    })
    .await
}

#[tauri::command]
pub async fn git_get_identity(
    state: State<'_, ServiceState>,
    local: bool,
    cwd: Option<String>,
) -> Result<IdentityDto, String> {
    run_blocking(&state, move |svc| {
        let cwd = cwd.as_deref().map(Path::new);
        let (name, email) = svc.get_current_identity(local, cwd)?;
        Ok(IdentityDto { name, email })
    })
    .await
}

#[tauri::command]
pub async fn git_get_stats(
    state: State<'_, ServiceState>,
    aliases: Vec<String>,
) -> Result<std::collections::HashMap<String, StatsDto>, String> {
    run_blocking(&state, move |svc| {
        let snapshot = svc.get_stats_snapshot(&aliases)?;
        Ok(snapshot
            .iter()
            .map(|(k, v)| (k.clone(), StatsDto::from(v)))
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn git_add_account(
    state: State<'_, ServiceState>,
    account: AccountInput,
    generate_key: bool,
) -> Result<AccountDto, String> {
    run_blocking(&state, move |svc| {
        let key_path = account
            .key_path
            .as_deref()
            .map(PathBuf::from)
            .or_else(|| Some(svc.default_key_path(&account.alias)));
        let acc = account.into_account(key_path);
        svc.add_account(&acc, generate_key).map(|a| AccountDto::from(&a))
    })
    .await
}

#[tauri::command]
pub async fn git_update_account(
    state: State<'_, ServiceState>,
    source_alias: String,
    account: AccountInput,
    generate_key: bool,
) -> Result<AccountDto, String> {
    run_blocking(&state, move |svc| {
        let key_path = account
            .key_path
            .as_deref()
            .map(PathBuf::from)
            .or_else(|| Some(svc.default_key_path(&account.alias)));
        let acc = account.into_account(key_path);
        svc.update_account(&source_alias, &acc, generate_key)
            .map(|a| AccountDto::from(&a))
    })
    .await
}

#[tauri::command]
pub async fn git_delete_account(
    state: State<'_, ServiceState>,
    alias: String,
    delete_keys: bool,
) -> Result<(), String> {
    run_blocking(&state, move |svc| svc.delete_account(&alias, delete_keys)).await
}

#[tauri::command]
pub async fn git_test_connection(
    state: State<'_, ServiceState>,
    alias: String,
) -> Result<TestResultDto, String> {
    run_blocking(&state, move |svc| {
        let (success, output) = svc.test_connection(&alias)?;
        Ok(TestResultDto { success, output })
    })
    .await
}

#[tauri::command]
pub async fn git_switch_identity(
    state: State<'_, ServiceState>,
    alias: String,
    local: bool,
    cwd: Option<String>,
) -> Result<AccountDto, String> {
    run_blocking(&state, move |svc| {
        let cwd = cwd.as_deref().map(Path::new);
        svc.switch_identity(&alias, local, cwd)
            .map(|a| AccountDto::from(&a))
    })
    .await
}

#[tauri::command]
pub async fn git_sync_repo(
    state: State<'_, ServiceState>,
    repo: String,
) -> Result<SyncResultDto, String> {
    run_blocking(&state, move |svc| {
        let (account, output) = svc.sync_repo(Path::new(&repo))?;
        Ok(SyncResultDto {
            alias: account.alias,
            output,
        })
    })
    .await
}

#[tauri::command]
pub async fn git_is_git_repo(state: State<'_, ServiceState>, repo: String) -> Result<bool, String> {
    run_blocking(&state, move |svc| svc.is_git_repo(Path::new(&repo))).await
}

#[tauri::command]
pub async fn git_generate_key(
    state: State<'_, ServiceState>,
    alias: String,
    email: String,
    key_path: Option<String>,
) -> Result<String, String> {
    run_blocking(&state, move |svc| {
        let path = match key_path.as_deref() {
            Some(p) => PathBuf::from(p),
            None => svc.default_key_path(&alias),
        };
        svc.generate_key_pair(Some(&path), &email)
            .map(|p| p.display().to_string())
    })
    .await
}

#[tauri::command]
pub async fn git_regenerate_key(
    state: State<'_, ServiceState>,
    alias: String,
    email: String,
    key_path: Option<String>,
) -> Result<String, String> {
    run_blocking(&state, move |svc| {
        let path = match key_path.as_deref() {
            Some(p) => PathBuf::from(p),
            None => svc.default_key_path(&alias),
        };
        svc.regenerate_key_pair(Some(&path), &email)
            .map(|p| p.display().to_string())
    })
    .await
}

#[tauri::command]
pub async fn git_read_public_key(
    state: State<'_, ServiceState>,
    key_path: Option<String>,
) -> Result<String, String> {
    run_blocking(&state, move |svc| {
        Ok(svc.read_public_key(key_path.as_deref().map(Path::new)))
    })
    .await
}

#[tauri::command]
pub async fn git_get_key_url(state: State<'_, ServiceState>, alias: String) -> Result<Option<String>, String> {
    run_blocking(&state, move |svc| svc.get_ssh_key_management_url(&alias)).await
}

#[tauri::command]
pub async fn git_default_key_path(state: State<'_, ServiceState>, alias: String) -> Result<String, String> {
    run_blocking(&state, move |svc| {
        Ok(svc.default_key_path(&alias).display().to_string())
    })
    .await
}

#[tauri::command]
pub async fn git_home_dir() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .display()
        .to_string()
}

#[tauri::command]
pub async fn git_ssh_config_path(state: State<'_, ServiceState>) -> Result<String, String> {
    run_blocking(&state, |svc| Ok(svc.ssh_config_path().display().to_string())).await
}

#[tauri::command]
pub async fn git_ssh_dir(state: State<'_, ServiceState>) -> Result<String, String> {
    run_blocking(&state, |svc| Ok(svc.ssh_dir().display().to_string())).await
}

#[tauri::command]
pub async fn git_open_in_manager(path: String, select: bool) -> Result<(), String> {
    platform::open_in_file_manager(Path::new(&path), select).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_open_url(url: String) -> Result<(), String> {
    platform::open_url(&url).map_err(|e| e.to_string())
}

/// 检测 git / ssh 是否可用（跨平台，用 PATH 查找可执行文件）。
#[tauri::command]
pub async fn git_check_env() -> serde_json::Value {
    let env = |k: &str| std::env::var(k).ok();
    let git = platform::which(&env, "git").is_some();
    let ssh = platform::which(&env, "ssh").is_some();
    serde_json::json!({ "git": git, "ssh": ssh })
}

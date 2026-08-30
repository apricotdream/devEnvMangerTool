//! 核心业务服务（对应 Python `service.GitAccountService`）。
//!
//! 通过 subprocess 调用 `git` / `ssh` / `ssh-keygen`，覆盖：
//! 账号增删改查、SSH 连接测试（含 known_hosts 指纹冲突重试）、
//! 身份切换、仓库同步、公钥读写、错误诊断。

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;

use crate::gitcore::account::Account;
use crate::gitcore::error::Error;
use crate::gitcore::platform;
use crate::gitcore::ssh_config;
use crate::gitcore::stats::{AccountStats, StatsData};

/// 命令执行结果（对应 Python `subprocess.CompletedProcess`）。
struct CommandOutput {
    returncode: i32,
    stdout: String,
    stderr: String,
}

/// 当前时间字符串，格式 `YYYY-MM-DD HH:MM:SS`（对应 Python `datetime.now().strftime`）。
fn now_text() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 读取真实环境的访问器。
fn env_getter(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Git 账号服务（对应 Python `GitAccountService`）。
pub struct Service {
    ssh_dir: PathBuf,
    ssh_config_path: PathBuf,
    known_hosts_path: PathBuf,
    stats_path: PathBuf,
    command_cache: HashMap<String, PathBuf>,
    accounts_cache: Option<Vec<Account>>,
    stats_cache: Option<StatsData>,
}

impl Service {
    /// 创建服务。`ssh_config_path` 为 `None` 时自动解析 SSH 目录。
    pub fn new(ssh_config_path: Option<&Path>) -> Result<Service, Error> {
        let (ssh_dir, ssh_config_path) = match ssh_config_path {
            Some(path) => {
                let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
                (dir, path.to_path_buf())
            }
            None => {
                let ssh_dir = Self::resolve_ssh_dir()?;
                (ssh_dir.clone(), ssh_dir.join("config"))
            }
        };
        let known_hosts_path = ssh_dir.join("known_hosts");
        let stats_path = ssh_dir.join("git_account_stats.json");

        let mut svc = Service {
            ssh_dir,
            ssh_config_path,
            known_hosts_path,
            stats_path,
            command_cache: HashMap::new(),
            accounts_cache: None,
            stats_cache: None,
        };
        svc.ensure_ssh_environment()?;
        Ok(svc)
    }

    /// 解析 SSH 目录（对应 Python `_resolve_ssh_dir`）。
    fn resolve_ssh_dir() -> Result<PathBuf, Error> {
        let home = dirs::home_dir().unwrap_or_default();
        let preferred = home.join(".ssh");
        if Self::is_writable_directory(&preferred) {
            return Ok(preferred);
        }

        let temp_dir = std::env::temp_dir();
        let fallback =
            platform::fallback_ssh_dir(platform::current_platform_name(), &env_getter, &home, &temp_dir);
        if Self::is_writable_directory(&fallback) {
            return Ok(fallback);
        }

        let temp_fallback = temp_dir.join("git-env-gui").join("ssh");
        mkdir_dir(&temp_fallback)?;
        Ok(temp_fallback)
    }

    /// 目录是否可写（对应 Python `_is_writable_directory`）。
    fn is_writable_directory(path: &Path) -> bool {
        let _ = mkdir_dir(path);
        let probe = path.join(".git_env_gui_probe");
        let ok = fs::write(&probe, "").is_ok();
        let _ = fs::remove_file(&probe);
        ok
    }

    /// SSH 目录路径。
    pub fn ssh_dir(&self) -> &Path {
        &self.ssh_dir
    }

    /// SSH config 文件路径。
    pub fn ssh_config_path(&self) -> &Path {
        &self.ssh_config_path
    }

    /// 确保 SSH 环境就绪（对应 Python `ensure_ssh_environment`）。
    pub fn ensure_ssh_environment(&mut self) -> Result<(), Error> {
        mkdir_dir(&self.ssh_dir)?;
        if !self.ssh_config_path.exists() {
            fs::write(&self.ssh_config_path, "").map_err(|e| Error::Io {
                context: format!("创建 config 失败：{}", self.ssh_config_path.display()),
                source: e,
            })?;
        }
        try_chmod(&self.ssh_dir, 0o700);
        try_chmod(&self.ssh_config_path, 0o600);
        Ok(())
    }

    /// 读取 config 行（对应 Python `_read_lines`）。
    fn read_lines(&mut self) -> Result<Vec<String>, Error> {
        self.ensure_ssh_environment()?;
        let text = fs::read_to_string(&self.ssh_config_path).map_err(|e| Error::Io {
            context: format!("读取 config 失败：{}", self.ssh_config_path.display()),
            source: e,
        })?;
        Ok(ssh_config::read_lines(&text))
    }

    /// 写回 config 并规范化（对应 Python `_write_lines`）。
    fn write_lines(&mut self, lines: &[String]) -> Result<(), Error> {
        let text = ssh_config::write_lines(lines);
        fs::write(&self.ssh_config_path, text).map_err(|e| Error::Io {
            context: format!("写入 config 失败：{}", self.ssh_config_path.display()),
            source: e,
        })?;
        try_chmod(&self.ssh_config_path, 0o600);
        self.accounts_cache = None;
        Ok(())
    }

    /// 列出所有账号（按 alias 小写排序，带缓存）。
    pub fn list_accounts(&mut self) -> Result<Vec<Account>, Error> {
        if let Some(cache) = &self.accounts_cache {
            return Ok(cache.clone());
        }
        let lines = self.read_lines()?;
        let accounts = ssh_config::parse_accounts(&lines);
        self.accounts_cache = Some(accounts.clone());
        Ok(accounts)
    }

    /// 按别名获取账号。
    pub fn get_account(&mut self, alias: &str) -> Result<Account, Error> {
        for account in self.list_accounts()? {
            if account.alias == alias {
                return Ok(account);
            }
        }
        Err(Error::validation(format!("未找到 Host 别名：{alias}")))
    }

    /// 当前 Git 身份（对应 Python `get_current_identity`）。
    pub fn get_current_identity(
        &mut self,
        local: bool,
        cwd: Option<&Path>,
    ) -> Result<(Option<String>, Option<String>), Error> {
        let scope = if local { "--local" } else { "--global" };
        let name = self.git_config_get(scope, "user.name", cwd)?;
        let email = self.git_config_get(scope, "user.email", cwd)?;
        Ok((name, email))
    }

    /// 新增账号。
    pub fn add_account(&mut self, account: &Account, generate_key: bool) -> Result<Account, Error> {
        let normalized = self.normalize_account(account)?;
        if self
            .list_accounts()?
            .iter()
            .any(|item| item.alias == normalized.alias)
        {
            return Err(Error::validation(format!(
                "Host 别名已存在：{}",
                normalized.alias
            )));
        }

        if generate_key {
            self.generate_key_pair(normalized.key_path.as_deref(), &normalized.git_email)?;
        }
        self.ensure_key_exists(normalized.key_path.as_deref())?;

        let mut lines = self.read_lines()?;
        if let Some(last) = lines.last() {
            if !last.trim().is_empty() {
                lines.push(String::new());
            }
        }
        let block = ssh_config::render_account_block(&normalized, &now_text());
        lines.extend(block);
        self.write_lines(&lines)?;
        self.get_account(&normalized.alias)
    }

    /// 更新账号。
    pub fn update_account(
        &mut self,
        source_alias: &str,
        account: &Account,
        generate_key: bool,
    ) -> Result<Account, Error> {
        let accounts = self.list_accounts()?;
        let source = accounts
            .iter()
            .find(|item| item.alias == source_alias)
            .ok_or_else(|| Error::validation(format!("未找到 Host 别名：{source_alias}")))?
            .clone();

        let normalized = self.normalize_account(account)?;
        if normalized.alias != source_alias
            && accounts.iter().any(|item| item.alias == normalized.alias)
        {
            return Err(Error::validation(format!(
                "Host 别名已存在：{}",
                normalized.alias
            )));
        }

        if generate_key {
            self.generate_key_pair(normalized.key_path.as_deref(), &normalized.git_email)?;
        }
        self.ensure_key_exists(normalized.key_path.as_deref())?;

        let lines = self.read_lines()?;
        let replacement = ssh_config::render_account_block(&normalized, &now_text());
        let mut new_lines = lines[..source.comment_start as usize].to_vec();
        new_lines.extend(replacement);
        new_lines.extend_from_slice(&lines[source.end as usize..]);
        self.write_lines(&new_lines)?;

        if normalized.alias != source_alias {
            self.move_stats_key(source_alias, &normalized.alias)?;
        }
        self.get_account(&normalized.alias)
    }

    /// 删除账号。
    pub fn delete_account(&mut self, alias: &str, delete_keys: bool) -> Result<(), Error> {
        let account = self.get_account(alias)?;
        let lines = self.read_lines()?;
        let mut new_lines = lines[..account.comment_start as usize].to_vec();
        new_lines.extend_from_slice(&lines[account.end as usize..]);
        self.write_lines(&new_lines)?;

        if delete_keys {
            if let Some(key_path) = &account.key_path {
                self.delete_key_files(key_path)?;
            }
        }
        self.remove_stats_key(alias)
    }

    /// 切换 Git 身份（全局或本地仓库）。
    pub fn switch_identity(
        &mut self,
        alias: &str,
        local: bool,
        cwd: Option<&Path>,
    ) -> Result<Account, Error> {
        let account = self.get_account(alias)?;
        if account.git_name.is_empty() || account.git_email.is_empty() {
            return Err(Error::validation(
                "当前账号缺少 Git 用户名或邮箱，无法切换身份",
            ));
        }

        let scope = if local { "--local" } else { "--global" };
        let target_cwd = if local { cwd } else { None };
        self.git_config_set(scope, "user.name", &account.git_name, target_cwd)?;
        self.git_config_set(scope, "user.email", &account.git_email, target_cwd)?;
        Ok(account)
    }

    /// 测试 SSH 连接（对应 Python `test_connection`）。
    pub fn test_connection(&mut self, alias: &str) -> Result<(bool, String), Error> {
        let account = self.get_account(alias)?;
        let target = format!("{}@{}", account.ssh_user, account.alias);
        let ssh_cmd = vec![
            "ssh".to_string(),
            "-F".to_string(),
            self.ssh_config_path.display().to_string(),
            "-T".to_string(),
            "-p".to_string(),
            account.port.clone(),
            "-o".to_string(),
            "BatchMode=yes".to_string(),
            "-o".to_string(),
            "ConnectTimeout=15".to_string(),
            "-o".to_string(),
            "StrictHostKeyChecking=accept-new".to_string(),
            target,
        ];

        let result = match self.run_command_captured(&ssh_cmd, None, Some(Duration::from_secs(20))) {
            Ok(output) => output,
            Err(Error::Timeout) => {
                let message = format!(
                    "SSH 连接超时。\n目标别名：{}\n主机地址：{}\n端口：{}\n\n建议检查：\n1. 当前网络是否可达目标主机。\n2. 目标 SSH 端口是否正确。\n3. 服务器是否允许当前来源 IP 访问。\n\n原始信息：timeout",
                    account.alias, account.host_name, account.port
                );
                self.record_connection_result(alias, false, &message)?;
                return Ok((false, message));
            }
            Err(Error::CommandNotFound(_)) => {
                let message = "未找到 `ssh` 命令，请先安装或启用 OpenSSH 客户端。";
                self.record_connection_result(alias, false, message)?;
                return Ok((false, message.to_string()));
            }
            Err(e) => return Err(e),
        };

        let output = join_output(&result);
        let lowered = output.to_lowercase();
        if lowered.contains("host key verification failed") {
            let cleared = self.clear_known_host_entries(alias)?;
            let retry = self
                .run_command_captured(&ssh_cmd, None, Some(Duration::from_secs(20)))?;
            let retry_output = join_output(&retry);
            let retry_success = retry.returncode == 0 || has_success_marker(&retry_output);
            if retry_success {
                let mut prefix = format!(
                    "检测到旧的主机指纹记录，已自动清理当前账号对应的 known_hosts 项并重新连接成功。{}",
                    if cleared.is_empty() {
                        String::new()
                    } else {
                        format!("\n已清理记录：{}", cleared.join(", "))
                    }
                );
                let message = build_ssh_success_message(&account, &retry_output, Some(&mut prefix));
                self.record_connection_result(alias, true, &message)?;
                return Ok((true, message));
            }
            return self.finish_test(alias, &account, &retry, retry_output);
        }

        self.finish_test(alias, &account, &result, output)
    }

    /// SSH 测试的收尾：判定成功/失败并记录（对应 Python 的 success 判定）。
    fn finish_test(
        &mut self,
        alias: &str,
        account: &Account,
        result: &CommandOutput,
        output: String,
    ) -> Result<(bool, String), Error> {
        let success = result.returncode == 0 || has_success_marker(&output);
        if success {
            let message = build_ssh_success_message(account, &output, None);
            self.record_connection_result(alias, true, &message)?;
            Ok((true, message))
        } else {
            let message = build_ssh_failure_message(account, &output);
            self.record_connection_result(alias, false, &message)?;
            Ok((false, message))
        }
    }

    /// 同步仓库（对应 Python `sync_repo`）。
    pub fn sync_repo(&mut self, repo_dir: &Path) -> Result<(Account, String), Error> {
        if !self.is_git_repo(repo_dir)? {
            return Err(Error::validation(format!(
                "当前目录不是 Git 仓库：{}",
                repo_dir.display()
            )));
        }

        let remote_url = self.get_repo_remote_url(repo_dir)?;
        let account = self.resolve_account_for_remote_url(&remote_url)?;

        let result = self.run_command_captured(&["git".to_string(), "pull".to_string(), "--ff-only".to_string()], Some(repo_dir), None)?;
        let output = join_output(&result);
        if result.returncode != 0 {
            let message = if output.is_empty() {
                "仓库更新失败".to_string()
            } else {
                output
            };
            return Err(Error::runtime(message));
        }

        self.record_sync_success(&account.alias, repo_dir)?;
        let message = format!(
            "仓库更新成功。\n账号别名：{}\n仓库目录：{}\n远程地址：{}\n\n原始输出：\n{}",
            account.alias,
            repo_dir.display(),
            remote_url,
            if output.is_empty() { "无额外输出".to_string() } else { output }
        );
        Ok((account, message))
    }

    /// 判断路径是否为 Git 仓库（对应 Python `is_git_repo`）。
    pub fn is_git_repo(&mut self, repo_dir: &Path) -> Result<bool, Error> {
        let result = self.run_command_captured(
            &["git".to_string(), "rev-parse".to_string(), "--is-inside-work-tree".to_string()],
            Some(repo_dir),
            None,
        )?;
        Ok(result.returncode == 0 && result.stdout.trim() == "true")
    }

    // ── stats ──────────────────────────────────────────────────

    fn load_stats(&mut self) -> Result<StatsData, Error> {
        if let Some(cache) = &self.stats_cache {
            return Ok(cache.clone());
        }
        self.ensure_ssh_environment()?;
        let data = StatsData::load(&self.stats_path)?;
        self.stats_cache = Some(data.clone());
        Ok(data)
    }

    fn save_stats(&mut self, data: &StatsData) -> Result<(), Error> {
        self.stats_cache = Some(data.clone());
        data.save(&self.stats_path)?;
        try_chmod(&self.stats_path, 0o600);
        Ok(())
    }

    /// 统计快照（对应 Python `get_stats_snapshot`）。
    pub fn get_stats_snapshot(&mut self, aliases: &[String]) -> Result<HashMap<String, AccountStats>, Error> {
        let data = self.load_stats()?;
        Ok(data.snapshot(aliases))
    }

    /// 记录连接结果（对应 Python `record_connection_result`）。
    pub fn record_connection_result(
        &mut self,
        alias: &str,
        success: bool,
        message: &str,
    ) -> Result<(), Error> {
        let mut data = self.load_stats()?;
        let entry = data.entry(alias);
        entry.last_connection_ok = Some(success);
        entry.last_connection_at = now_text();
        entry.last_connection_message = message.to_string();
        self.save_stats(&data)
    }

    /// 记录同步成功（对应 Python `record_sync_success`）。
    pub fn record_sync_success(&mut self, alias: &str, repo_dir: &Path) -> Result<(), Error> {
        let mut data = self.load_stats()?;
        let entry = data.entry(alias);
        entry.last_sync_at = now_text();
        entry.last_sync_repo = repo_dir.display().to_string();
        self.save_stats(&data)
    }

    fn move_stats_key(&mut self, source_alias: &str, target_alias: &str) -> Result<(), Error> {
        if source_alias == target_alias {
            return Ok(());
        }
        let mut data = self.load_stats()?;
        let source = data.accounts.shift_remove(source_alias);
        if let Some(entry) = source {
            if !data.accounts.contains_key(target_alias) {
                data.accounts.insert(target_alias.to_string(), entry);
            } else {
                let mut merged = AccountStats::default();
                let target = data.accounts[target_alias].clone();
                merged.last_sync_at = target.last_sync_at.clone();
                merged.last_sync_repo = target.last_sync_repo.clone();
                if entry.last_connection_ok.is_some() {
                    merged.last_connection_ok = entry.last_connection_ok;
                    merged.last_connection_at = entry.last_connection_at.clone();
                    merged.last_connection_message = entry.last_connection_message.clone();
                } else {
                    merged.last_connection_ok = target.last_connection_ok;
                    merged.last_connection_at = target.last_connection_at.clone();
                    merged.last_connection_message = target.last_connection_message.clone();
                }
                data.accounts.insert(target_alias.to_string(), merged);
            }
        }
        self.save_stats(&data)
    }

    fn remove_stats_key(&mut self, alias: &str) -> Result<(), Error> {
        let mut data = self.load_stats()?;
        if data.accounts.shift_remove(alias).is_some() {
            self.save_stats(&data)?;
        }
        Ok(())
    }

    // ── 密钥管理 ───────────────────────────────────────────────

    /// 默认私钥路径（对应 Python `default_key_path`）。
    pub fn default_key_path(&self, alias: &str) -> PathBuf {
        let safe: String = alias
            .trim()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let safe = if safe.is_empty() {
            "account".to_string()
        } else {
            safe
        };
        self.ssh_dir.join(format!("id_rsa_{safe}"))
    }

    /// 生成密钥对（对应 Python `generate_key_pair`）。
    pub fn generate_key_pair(&mut self, key_path: Option<&Path>, email: &str) -> Result<PathBuf, Error> {
        let key_path = key_path
            .ok_or_else(|| Error::validation("缺少私钥路径，无法生成密钥"))?
            .to_path_buf();
        if let Some(parent) = key_path.parent() {
            mkdir_dir(parent)?;
        }
        if key_path.exists() {
            return Err(Error::validation(format!(
                "私钥已存在：{}",
                key_path.display()
            )));
        }

        let comment = if email.is_empty() {
            "git-account".to_string()
        } else {
            email.to_string()
        };
        let result = self.run_command_captured(
            &[
                "ssh-keygen".to_string(),
                "-t".to_string(),
                "rsa".to_string(),
                "-b".to_string(),
                "4096".to_string(),
                "-f".to_string(),
                key_path.display().to_string(),
                "-C".to_string(),
                comment,
                "-N".to_string(),
                "".to_string(),
            ],
            None,
            None,
        )?;
        if result.returncode != 0 {
            let message = result.stderr.trim();
            return Err(Error::runtime(if message.is_empty() {
                "ssh-keygen 执行失败".to_string()
            } else {
                message.to_string()
            }));
        }
        Ok(key_path)
    }

    /// 重新生成密钥对（对应 Python `regenerate_key_pair`）。
    pub fn regenerate_key_pair(&mut self, key_path: Option<&Path>, email: &str) -> Result<PathBuf, Error> {
        let key_path = key_path
            .ok_or_else(|| Error::validation("缺少私钥路径，无法重新生成密钥"))?
            .to_path_buf();
        self.delete_key_files(&key_path)?;
        self.generate_key_pair(Some(&key_path), email)
    }

    /// 读取公钥内容（对应 Python `read_public_key`）。
    pub fn read_public_key(&self, key_path: Option<&Path>) -> String {
        let Some(key_path) = key_path else {
            return String::new();
        };
        let public_key_path = PathBuf::from(format!("{}.pub", key_path.display()));
        if !public_key_path.exists() {
            return String::new();
        }
        fs::read_to_string(&public_key_path)
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    /// 公钥管理页 URL（对应 Python `get_ssh_key_management_url`）。
    pub fn get_ssh_key_management_url(&mut self, alias: &str) -> Result<Option<String>, Error> {
        let account = self.get_account(alias)?;
        let host = account.host_name.trim().to_lowercase();
        let alias_lower = account.alias.trim().to_lowercase();
        if host.is_empty() {
            return Ok(None);
        }

        if host == "github.com" || host.contains("github") || alias_lower.contains("github") {
            return Ok(Some(build_site_url(&host, "/settings/keys")?));
        }
        if host == "gitee.com" {
            return Ok(Some("https://gitee.com/profile/sshkeys".to_string()));
        }
        if host == "gitcode.com" || host == "gitcode.net" {
            return Ok(Some(
                "https://docs.gitcode.com/docs/help/home/user_center/security_management/ssh/".to_string(),
            ));
        }
        if host.contains("gitlab") || alias_lower.contains("gitlab") {
            return Ok(Some(build_site_url(&host, "/-/profile/keys")?));
        }
        if host.contains("gitea") || alias_lower.contains("gitea") {
            return Ok(Some(build_site_url(&host, "/user/settings/keys")?));
        }
        self.get_git_site_url(alias)
    }

    /// Git 站点 URL（对应 Python `get_git_site_url`）。
    pub fn get_git_site_url(&mut self, alias: &str) -> Result<Option<String>, Error> {
        let account = self.get_account(alias)?;
        let host = account.host_name.trim().to_lowercase();
        if host.is_empty() {
            return Ok(None);
        }
        Ok(Some(build_site_url(&host, "")?))
    }

    /// 清理 known_hosts 中该账号相关的旧指纹（对应 Python `clear_known_host_entries`）。
    pub fn clear_known_host_entries(&mut self, alias: &str) -> Result<Vec<String>, Error> {
        let account = self.get_account(alias)?;
        let mut targets = vec![account.alias.trim().to_string(), account.host_name.trim().to_string()];
        if account.port != "22" {
            targets.push(format!("[{}]:{}", account.alias.trim(), account.port));
            targets.push(format!("[{}]:{}", account.host_name.trim(), account.port));
        }

        let mut cleared = Vec::new();
        for target in targets {
            match self.run_command_captured(
                &[
                    "ssh-keygen".to_string(),
                    "-R".to_string(),
                    target.clone(),
                    "-f".to_string(),
                    self.known_hosts_path.display().to_string(),
                ],
                None,
                None,
            ) {
                Ok(result) => {
                    if result.returncode == 0 {
                        cleared.push(target);
                    }
                }
                // ssh-keygen 缺失时静默放弃（对应 Python `except FileNotFoundError: break`）
                Err(Error::CommandNotFound(_)) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(cleared)
    }

    // ── 私有工具 ───────────────────────────────────────────────

    fn normalize_account(&self, account: &Account) -> Result<Account, Error> {
        let alias = account.alias.trim().to_string();
        let host_name = account.host_name.trim().to_string();
        let ssh_user = account.ssh_user.trim().to_string();
        let git_name = account.git_name.trim().to_string();
        let git_email = account.git_email.trim().to_string();
        let mut port = account.port.trim().to_string();
        if port.is_empty() {
            port = "22".to_string();
        }

        if alias.is_empty() {
            return Err(Error::validation("Host 别名不能为空"));
        }
        if host_name.is_empty() {
            return Err(Error::validation("主机地址不能为空"));
        }
        if ssh_user.is_empty() {
            return Err(Error::validation("SSH 用户不能为空"));
        }
        if git_name.is_empty() {
            return Err(Error::validation("Git 用户名不能为空"));
        }
        if git_email.is_empty() {
            return Err(Error::validation("Git 邮箱不能为空"));
        }
        if !port.chars().all(|c| c.is_ascii_digit()) {
            return Err(Error::validation("SSH 端口必须是数字"));
        }

        let key_path = match &account.key_path {
            Some(path) => Some(path.clone()),
            None => Some(self.default_key_path(&alias)),
        };

        Ok(Account {
            alias,
            host_name,
            port,
            ssh_user,
            key_path,
            git_name,
            git_email,
            start: -1,
            end: -1,
            comment_start: -1,
        })
    }

    fn ensure_key_exists(&self, key_path: Option<&Path>) -> Result<(), Error> {
        match key_path {
            Some(path) if path.exists() => Ok(()),
            Some(path) => Err(Error::validation(format!("私钥不存在：{}", path.display()))),
            None => Err(Error::validation("私钥不存在")),
        }
    }

    fn delete_key_files(&self, key_path: &Path) -> Result<(), Error> {
        let public = PathBuf::from(format!("{}.pub", key_path.display()));
        for path in [public, key_path.to_path_buf()] {
            if path.exists() && path.is_file() {
                fs::remove_file(&path).map_err(|e| Error::Io {
                    context: format!("删除密钥失败：{}", path.display()),
                    source: e,
                })?;
            }
        }
        Ok(())
    }

    fn git_config_get(
        &mut self,
        scope: &str,
        key: &str,
        cwd: Option<&Path>,
    ) -> Result<Option<String>, Error> {
        let result = self.run_command_captured(
            &["git".to_string(), "config".to_string(), scope.to_string(), "--get".to_string(), key.to_string()],
            cwd,
            None,
        )?;
        if result.returncode != 0 {
            return Ok(None);
        }
        let value = result.stdout.trim();
        Ok(if value.is_empty() { None } else { Some(value.to_string()) })
    }

    fn git_config_set(&mut self, scope: &str, key: &str, value: &str, cwd: Option<&Path>) -> Result<(), Error> {
        let result = self.run_command_captured(
            &["git".to_string(), "config".to_string(), scope.to_string(), key.to_string(), value.to_string()],
            cwd,
            None,
        )?;
        if result.returncode != 0 {
            let message = result.stderr.trim();
            return Err(Error::runtime(if message.is_empty() {
                format!("设置 {key} 失败")
            } else {
                message.to_string()
            }));
        }
        Ok(())
    }

    fn get_repo_remote_url(&mut self, repo_dir: &Path) -> Result<String, Error> {
        let names = self.run_command_captured(
            &["git".to_string(), "remote".to_string()],
            Some(repo_dir),
            None,
        )?;
        if names.returncode != 0 {
            let message = names.stderr.trim();
            return Err(Error::runtime(if message.is_empty() {
                "无法读取 Git 远程配置".to_string()
            } else {
                message.to_string()
            }));
        }

        let mut remote_names: Vec<String> = names
            .stdout
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        if remote_names.is_empty() {
            return Err(Error::validation("当前仓库没有配置任何远程地址"));
        }

        let remote_name = if remote_names.iter().any(|name| name == "origin") {
            "origin".to_string()
        } else {
            remote_names.remove(0)
        };
        let url = self.run_command_captured(
            &["git".to_string(), "remote".to_string(), "get-url".to_string(), remote_name.clone()],
            Some(repo_dir),
            None,
        )?;
        if url.returncode != 0 {
            let message = url.stderr.trim();
            return Err(Error::runtime(if message.is_empty() {
                format!("无法读取远程地址：{remote_name}")
            } else {
                message.to_string()
            }));
        }
        Ok(url.stdout.trim().to_string())
    }

    fn resolve_account_for_remote_url(&mut self, remote_url: &str) -> Result<Account, Error> {
        let remote_host = extract_host_from_git_url(remote_url)
            .ok_or_else(|| Error::validation(format!("无法从远程地址识别账号：{remote_url}")))?;

        let accounts = self.list_accounts()?;
        if let Some(account) = accounts.iter().find(|account| account.alias == remote_host) {
            return Ok(account.clone());
        }

        let host_matches: Vec<&Account> = accounts
            .iter()
            .filter(|account| account.host_name == remote_host)
            .collect();
        match host_matches.len() {
            0 => Err(Error::validation(format!(
                "远程地址 {remote_url} 不属于当前工具管理的账号"
            ))),
            1 => Ok(host_matches[0].clone()),
            _ => Err(Error::validation(format!(
                "远程地址 {remote_url} 对应多个账号，请改用唯一 Host 别名"
            ))),
        }
    }

    /// 解析命令路径（带缓存，对应 Python `_resolve_command`）。
    fn resolve_command(&mut self, name: &str) -> Result<PathBuf, Error> {
        if let Some(cached) = self.command_cache.get(name) {
            return Ok(cached.clone());
        }

        if let Some(path) = platform::which(&env_getter, name) {
            self.command_cache.insert(name.to_string(), path.clone());
            return Ok(path);
        }

        let home = dirs::home_dir().unwrap_or_default();
        for candidate in platform::command_candidates(
            name,
            platform::current_platform_name(),
            &env_getter,
            &home,
        ) {
            if candidate.exists() {
                self.command_cache.insert(name.to_string(), candidate.clone());
                return Ok(candidate);
            }
        }

        Err(Error::command_not_found(missing_command_message(name)))
    }

    /// 执行命令并捕获输出（对应 Python `_run_command`，支持超时）。
    fn run_command_captured(
        &mut self,
        args: &[String],
        cwd: Option<&Path>,
        timeout: Option<Duration>,
    ) -> Result<CommandOutput, Error> {
        let program = self.resolve_command(&args[0])?;
        let mut command = crate::new_cmd(&program);
        command.args(&args[1..]);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        if args[0] == "git" {
            let mut env = std::env::vars().collect::<HashMap<String, String>>();
            let ssh_command = self
                .resolve_command("ssh")
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "ssh".to_string());
            env.insert(
                "GIT_SSH_COMMAND".to_string(),
                format!("\"{ssh_command}\" -F \"{}\"", self.ssh_config_path.display()),
            );
            command.envs(env);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|e| Error::Io {
            context: format!("启动命令失败：{}", args[0]),
            source: e,
        })?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let out_handle = thread::spawn(move || read_to_string(stdout));
        let err_handle = thread::spawn(move || read_to_string(stderr));

        let deadline = timeout.map(|t| Instant::now() + t);
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| Error::Io {
                context: "等待子进程失败".into(),
                source: e,
            })? {
                break status;
            }
            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::Timeout);
                }
            }
            thread::sleep(Duration::from_millis(50));
        };

        let output = CommandOutput {
            returncode: status.code().unwrap_or(-1),
            stdout: out_handle.join().unwrap_or_default(),
            stderr: err_handle.join().unwrap_or_default(),
        };
        Ok(output)
    }
}

fn read_to_string<R: Read>(reader: Option<R>) -> String {
    let mut buf = String::new();
    if let Some(mut reader) = reader {
        let _ = reader.read_to_string(&mut buf);
    }
    buf
}

/// 合并 stdout/stderr 输出（对应 Python `"\n".join(part for part in [stdout, stderr] if part)`）。
fn join_output(result: &CommandOutput) -> String {
    let mut parts = Vec::new();
    if !result.stdout.trim().is_empty() {
        parts.push(result.stdout.trim().to_string());
    }
    if !result.stderr.trim().is_empty() {
        parts.push(result.stderr.trim().to_string());
    }
    parts.join("\n")
}

/// SSH 成功判定标记（对应 Python `success_markers`）。
fn has_success_marker(output: &str) -> bool {
    let lowered = output.to_lowercase();
    [
        "successfully authenticated",
        "does not provide shell access",
        "welcome to gitee.com",
        "shell access is not supported",
    ]
    .iter()
    .any(|marker| lowered.contains(marker))
}

/// 构造 SSH 成功消息（对应 Python `_build_ssh_success_message`）。
fn build_ssh_success_message(account: &Account, raw_output: &str, prefix: Option<&mut String>) -> String {
    let output = if raw_output.trim().is_empty() {
        "未返回任何输出"
    } else {
        raw_output.trim()
    };
    let lowered = output.to_lowercase();

    let reason = if lowered.contains("does not provide shell access")
        || lowered.contains("shell access is not supported")
    {
        "认证成功，目标平台不提供 shell 登录，这属于正常现象。"
    } else if lowered.contains("welcome to gitee.com") {
        "认证成功，已连接到 Gitee。"
    } else if lowered.contains("successfully authenticated") {
        "认证成功，可以正常使用当前 SSH 账号进行 Git 操作。"
    } else {
        "SSH 连接测试通过。"
    };

    let message = format!(
        "{reason}\n目标别名：{}\n主机地址：{}\nSSH 用户：{}\n端口：{}\n\n原始输出：\n{output}",
        account.alias, account.host_name, account.ssh_user, account.port
    );

    match prefix {
        Some(prefix) => format!("{prefix}\n\n{message}"),
        None => message,
    }
}

/// 构造 SSH 失败消息（对应 Python `_build_ssh_failure_message`）。
fn build_ssh_failure_message(account: &Account, raw_output: &str) -> String {
    let output = if raw_output.trim().is_empty() {
        "未返回任何输出".to_string()
    } else {
        raw_output.trim().to_string()
    };
    let lowered = output.to_lowercase();
    let key_path_text = account
        .key_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let (reason, suggestions): (&str, Vec<String>) =
        if lowered.contains("permission denied") && lowered.contains("publickey") {
            (
                "公钥认证失败，服务器没有接受当前私钥。",
                vec![
                    "确认对应公钥已经添加到目标 Git 平台或服务器。".to_string(),
                    format!("确认当前账号使用的私钥路径正确：{key_path_text}"),
                    "如果最近更换过密钥，请重新上传新的公钥。".to_string(),
                ],
            )
        } else if lowered.contains("could not resolve hostname")
            || lowered.contains("name or service not known")
        {
            (
                "主机名解析失败，找不到目标主机。",
                vec![
                    format!("检查 HostName 是否填写正确：{}", account.host_name),
                    "检查本机 DNS 或网络连接是否正常。".to_string(),
                    "如果是内网地址，确认当前网络环境可访问。".to_string(),
                ],
            )
        } else if lowered.contains("connection timed out") || lowered.contains("operation timed out") {
            (
                "连接超时，目标主机没有在预期时间内响应。",
                vec![
                    format!("检查主机 {} 是否可达。", account.host_name),
                    format!("检查 SSH 端口是否正确：{}", account.port),
                    "确认服务器防火墙或安全组没有拦截当前连接。".to_string(),
                ],
            )
        } else if lowered.contains("connection refused") {
            (
                "连接被拒绝，目标端口没有接受 SSH 连接。",
                vec![
                    format!("确认 SSH 服务已经启动，端口为 {}。", account.port),
                    "确认没有填错主机地址或端口。".to_string(),
                    "检查服务器防火墙是否关闭了该端口。".to_string(),
                ],
            )
        } else if lowered.contains("host key verification failed") {
            (
                "主机指纹校验失败。",
                vec![
                    "目标主机指纹可能变化了，请检查 `known_hosts` 中的旧记录。".to_string(),
                    "确认你连接的是正确的服务器，避免中间人风险。".to_string(),
                    "确认后再清理对应的 `known_hosts` 记录并重试。".to_string(),
                ],
            )
        } else if lowered.contains("identity file") && lowered.contains("not accessible") {
            (
                "私钥文件不可访问。",
                vec![
                    format!("检查私钥路径是否存在：{key_path_text}"),
                    "确认当前用户对该私钥文件有读取权限。".to_string(),
                    "如果路径包含手动修改内容，建议重新选择私钥文件。".to_string(),
                ],
            )
        } else if lowered.contains("no route to host") {
            (
                "网络不可达，当前机器没有到目标主机的路由。",
                vec![
                    "检查网络连接、VPN 或代理配置。".to_string(),
                    "如果是内网服务器，确认已经接入对应网络。".to_string(),
                    format!("确认主机地址填写正确：{}", account.host_name),
                ],
            )
        } else {
            (
                "SSH 连接失败。",
                vec![
                    "检查主机地址、端口、SSH 用户和私钥路径是否正确。".to_string(),
                    "确认公钥已添加到目标 Git 平台或服务器。".to_string(),
                    "必要时在终端手动执行同样的 ssh 命令做进一步排查。".to_string(),
                ],
            )
        };

    let suggestion_text = suggestions
        .iter()
        .enumerate()
        .map(|(i, text)| format!("{}. {}", i + 1, text))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{reason}\n目标别名：{}\n主机地址：{}\nSSH 用户：{}\n端口：{}\n\n建议检查：\n{suggestion_text}\n\n原始输出：\n{output}",
        account.alias, account.host_name, account.ssh_user, account.port
    )
}

/// 构造站点 URL（对应 Python `_build_site_url`）。
fn build_site_url(host: &str, path: &str) -> Result<String, Error> {
    let clean_host = host.trim().trim_end_matches('/');
    if clean_host.is_empty() {
        return Err(Error::validation("主机地址不能为空"));
    }
    let bare_host = clean_host.trim_start_matches('[').trim_end_matches(']');
    let is_ip = bare_host.parse::<std::net::IpAddr>().is_ok();
    let scheme = if is_ip { "http" } else { "https" };
    let host_for_url = if is_ip && bare_host.contains(':') && !clean_host.starts_with('[') {
        format!("[{bare_host}]")
    } else {
        clean_host.to_string()
    };
    let suffix = if path.is_empty() || path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    Ok(format!("{scheme}://{host_for_url}{suffix}"))
}

/// 从 git 远程地址提取主机名（对应 Python `_extract_host_from_git_url`）。
fn extract_host_from_git_url(remote_url: &str) -> Option<String> {
    if let Some(rest) = remote_url.strip_prefix("ssh://") {
        let host_part = rest.split('/').next().unwrap_or("");
        let host = host_part.rsplit('@').next().unwrap_or("");
        let host = host.split(':').next().unwrap_or("");
        return Some(host.to_string());
    }
    let colon = remote_url.find(':')?;
    let before = &remote_url[..colon];
    let host = before.rsplit('@').next().unwrap_or("");
    Some(host.to_string())
}

/// 命令未找到的提示（对应 Python `_missing_command_message`）。
fn missing_command_message(name: &str) -> String {
    let display = match name {
        "git" => "Git",
        "ssh" => "OpenSSH 客户端",
        "ssh-keygen" => "ssh-keygen",
        _ => name,
    };
    let mut message = format!("未找到 `{name}` 命令，请先安装或启用 {display}。");
    if cfg!(windows) {
        message.push_str(" 可以检查当前用户 PATH、Scoop，或 Git for Windows 自带的工具目录。");
    }
    message
}

/// 创建目录（unix 下权限 0700，对应 Python `_mkdir_dir`）。
fn mkdir_dir(path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder
            .recursive(true)
            .create(path)
            .map_err(|e| Error::Io {
                context: format!("创建目录失败：{}", path.display()),
                source: e,
            })?;
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(path).map_err(|e| Error::Io {
            context: format!("创建目录失败：{}", path.display()),
            source: e,
        })?;
    }
    Ok(())
}

/// 设置权限（unix 下生效，Windows 忽略，对应 Python `_try_chmod`）。
fn try_chmod(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    let _ = (path, mode);
}

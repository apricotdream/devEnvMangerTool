//! DevEnvManager Tauri 后端。
//!
//! 策略：尽量复用 Windows 自带工具（curl.exe / tar.exe / powershell），
//! 减少第三方依赖与编译风险。检测在 Rust 内完成，网络探测与下载走 curl。

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

mod git_cmds;
pub mod gitcore;

// ---------------------------------------------------------------------------
// 数据结构（字段名对齐前端 camelCase）
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkProbe {
    pub name: String,
    pub url: String,
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodeEnvInfo {
    pub installed: bool,
    pub store_stub: bool,
    pub real_node_exe: Option<String>,
    pub node_version: Option<String>,
    pub npm_version: Option<String>,
    pub in_path: bool,
    pub node_dir_in_path: Option<String>,
    pub network: Vec<NetworkProbe>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryResult {
    pub ok: bool,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathResult {
    ok: bool,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallResult {
    ok: bool,
    version: String,
    dir: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RegistryTestResult {
    pub reachable: bool,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub throughput_kbs: Option<f64>,
    pub needs_auth: bool,
    pub auth_user: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    pub registry: String,
    #[serde(default)]
    pub auth_type: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountsFile {
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub accounts: Vec<Account>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledVersion {
    pub version: String,
    pub current: bool,
    pub size_mb: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionsInfo {
    pub nvm_detected: bool,
    pub install_dir: String,
    pub installed: Vec<InstalledVersion>,
    pub available: Vec<String>,
    pub current: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NvmInfo {
    pub detected: bool,
    pub dirs: Vec<String>,
    pub path_entries: Vec<String>,
}

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

fn run_cmd(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        }
    } else {
        Some(stdout)
    }
}

fn windows_apps_dir() -> Option<String> {
    let local = std::env::var("LOCALAPPDATA").ok()?;
    Some(format!("{}\\Microsoft\\WindowsApps", local))
}

fn is_zero_byte_stub(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() == 0)
        .unwrap_or(false)
}

pub fn user_npmrc_path() -> String {
    if let Ok(p) = std::env::var("NPM_CONFIG_USERCONFIG") {
        return p;
    }
    std::env::var("USERPROFILE")
        .map(|h| format!("{}\\{}", h, ".npmrc"))
        .unwrap_or_else(|_| ".npmrc".to_string())
}

fn node_install_root() -> String {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    format!("{}\\Programs\\nodejs", local)
}

/// 用 curl.exe 做 HEAD 探测，解析 http_code + time_total。
fn probe(name: &str, url: &str) -> NetworkProbe {
    let mut p = NetworkProbe {
        name: name.to_string(),
        url: url.to_string(),
        ok: false,
        latency_ms: None,
        error: None,
    };
    let out = run_cmd(
        "curl.exe",
        &[
            "-s",
            "-o",
            "NUL",
            "-w",
            "%{http_code} %{time_total}",
            "-I",
            "-m",
            "4",
            url,
        ],
    );
    match out {
        Some(s) => {
            let parts: Vec<&str> = s.trim().split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(code) = parts[0].parse::<u16>() {
                    if code < 500 {
                        p.ok = true;
                        p.latency_ms = parts[1]
                            .parse::<f64>()
                            .ok()
                            .map(|t| (t * 1000.0) as u64);
                    }
                }
            }
            if !p.ok {
                p.error = Some(s);
            }
        }
        None => p.error = Some("curl 执行失败".to_string()),
    }
    p
}

/// 从 npmmirror 二进制目录索引中解析最新 LTS（偶数主版本）。
pub fn latest_lts(html: &str) -> Option<String> {
    let re = regex::Regex::new(r"v(\d+)\.(\d+)\.(\d+)/").ok()?;
    let mut versions: Vec<(i32, i32, i32)> = Vec::new();
    for cap in re.captures_iter(html) {
        let major: i32 = cap.get(1)?.as_str().parse().ok()?;
        if major % 2 == 0 {
            let minor: i32 = cap.get(2)?.as_str().parse().ok()?;
            let patch: i32 = cap.get(3)?.as_str().parse().ok()?;
            versions.push((major, minor, patch));
        }
    }
    versions.sort_unstable();
    versions
        .last()
        .map(|(m, min, p)| format!("{}.{}.{}", m, min, p))
}

/// 单次 curl 探测，返回 (HTTP状态, 延迟ms, 吞吐KB/s)。
fn probe_curl(endpoint: &str, auth_args: &[&str]) -> (u16, Option<u64>, Option<f64>) {
    let mut args: Vec<&str> = vec![
        "-s",
        "-k", // 忽略自签名 HTTPS 证书（私有源常见）
        "-o",
        "NUL",
        "-w",
        "%{http_code} %{time_total} %{size_download}",
        "-L",
        "-m",
        "10",
    ];
    args.extend_from_slice(auth_args);
    args.push(endpoint);
    let out = run_cmd("curl.exe", &args);
    match out {
        Some(s) => {
            let parts: Vec<&str> = s.trim().split_whitespace().collect();
            if parts.len() >= 3 {
                let status = parts[0].parse::<u16>().unwrap_or(0);
                let time = parts[1].parse::<f64>().unwrap_or(0.0);
                let bytes = parts[2].parse::<f64>().unwrap_or(0.0);
                let throughput = if time > 0.0 && bytes > 0.0 {
                    Some(bytes / 1024.0 / time)
                } else {
                    None
                };
                (status, Some((time * 1000.0) as u64), throughput)
            } else {
                (0, None, None)
            }
        }
        None => (0, None, None),
    }
}

/// 统一 registry 探测：
/// 1) `/-/ping` 判定可达性（npm 标准健康检查，任何兼容源返回 200）
/// 2) 尝试拉取 `debug` 包测吞吐/验认证（私有源无此包时 404，不误判为不可达）
fn probe_registry_endpoint(registry_base: &str, auth_args: &[&str]) -> RegistryTestResult {
    let mut result = RegistryTestResult::default();

    let (ping_status, ping_latency, _) = probe_curl(&format!("{}/-/ping", registry_base), auth_args);
    if ping_status == 0 {
        result.reachable = false;
        result.error = Some("连接失败（无法建立连接或超时）".to_string());
        return result;
    }
    if ping_status == 401 || ping_status == 403 {
        result.reachable = true;
        result.needs_auth = true;
        result.error = Some(format!("HTTP {} 需要认证", ping_status));
        return result;
    }
    if !(200..400).contains(&ping_status) {
        result.reachable = false;
        result.error = Some(format!("HTTP {} 异常", ping_status));
        return result;
    }
    result.reachable = true;
    result.status = Some(ping_status);
    result.latency_ms = ping_latency;

    // 次要探测：真实拉包（吞吐/认证），404 不视为不可达
    let (pkg_status, _, pkg_throughput) = probe_curl(&format!("{}/debug", registry_base), auth_args);
    match pkg_status {
        200..=399 => result.throughput_kbs = pkg_throughput,
        401 | 403 => result.needs_auth = true,
        _ => { /* 源正常但无该包，忽略 */ }
    }
    result
}

/// 下载并解压指定版本的 Node 到 install root，emit 阶段回调。
pub fn install_specific(version: &str, emit: impl Fn(&str)) -> Result<String, String> {
    let binary_mirror = "https://registry.npmmirror.com/-/binary/node";
    let local = std::env::var("LOCALAPPDATA").map_err(|_| "缺少 LOCALAPPDATA".to_string())?;
    let parent = format!("{}\\Programs\\nodejs", local);
    let target = format!("{}\\v{}", parent, version);
    let zip_path = format!("{}\\node-v{}-win-x64.zip", parent, version);

    if !Path::new(&format!("{}\\node.exe", target)).exists() {
        let _ = std::fs::create_dir_all(&parent);
        let zip_url = format!("{}/v{}/node-v{}-win-x64.zip", binary_mirror, version, version);

        emit("downloading");
        run_cmd("curl.exe", &["-L", "-s", "-o", &zip_path, &zip_url]);
        if !Path::new(&zip_path).exists() {
            return Err("下载 Node 失败（curl 未生成文件）".to_string());
        }

        emit("extracting");
        let _ = run_cmd("tar.exe", &["-xf", &zip_path, "-C", &parent]);
        let extracted = format!("{}\\node-v{}-win-x64", parent, version);
        if !Path::new(&extracted).exists() {
            return Err("解压 Node 失败".to_string());
        }
        let _ = std::fs::rename(&extracted, &target);
        let _ = std::fs::remove_file(&zip_path);
    }
    Ok(target)
}

// ---------------------------------------------------------------------------
// 账号存储
// ---------------------------------------------------------------------------

fn accounts_path() -> String {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    format!("{}\\DevEnvManager\\accounts.json", appdata)
}

fn read_accounts() -> AccountsFile {
    std::fs::read_to_string(accounts_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_accounts(file: &AccountsFile) -> Result<(), String> {
    let path = accounts_path();
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// registry URL -> npmrc key，如 http://host:port/path/ -> //host:port/path/
fn npmrc_key(registry: &str) -> String {
    let trimmed = registry.trim();
    let without_proto = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    // npm 官方格式要求 key 末尾带斜杠，与 .npmrc 中已有行保持一致
    let with_slash = if without_proto.ends_with('/') {
        without_proto.to_string()
    } else {
        format!("{}/", without_proto)
    };
    format!("//{}", with_slash)
}

/// 用账号凭据对目标 registry 执行 npm whoami，验证发布认证。
/// 返回用户名（成功）或 None（失败）。通过临时 .npmrc 隔离，不改用户配置。
fn check_whoami(account: &Account) -> Option<String> {
    let temp_npmrc = std::env::temp_dir().join(format!("envmanager-whoami-{}.npmrc", account.id));
    let key = npmrc_key(&account.registry);
    let line = match account.auth_type.as_str() {
        "basic" => {
            let cred = format!(
                "{}:{}",
                account.username.as_deref().unwrap_or(""),
                account.password.as_deref().unwrap_or("")
            );
            let b64 = base64::engine::general_purpose::STANDARD.encode(cred.as_bytes());
            format!("{}:_auth={}", key, b64)
        }
        _ => format!(
            "{}:_authToken={}",
            key,
            account.token.as_deref().unwrap_or("")
        ),
    };
    let _ = std::fs::write(&temp_npmrc, format!("registry={}\n{}\n", account.registry, line));

    let out = Command::new("cmd")
        .args([
            "/d",
            "/c",
            "npm.cmd",
            "whoami",
            "--registry",
            account.registry.as_str(),
        ])
        .env("NPM_CONFIG_USERCONFIG", &temp_npmrc)
        .output();

    let _ = std::fs::remove_file(&temp_npmrc);

    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        _ => None,
    }
}

/// 把账号凭据写入全局 .npmrc（自动备份 .bak）。
fn write_npmrc_credential(account: &Account) -> Result<(), String> {
    let path = user_npmrc_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::copy(&path, format!("{}.bak", path));

    let key = npmrc_key(&account.registry);
    let line = match account.auth_type.as_str() {
        "basic" => {
            let cred = format!(
                "{}:{}",
                account.username.as_deref().unwrap_or(""),
                account.password.as_deref().unwrap_or("")
            );
            let b64 = base64::engine::general_purpose::STANDARD.encode(cred.as_bytes());
            format!("{}:_auth={}", key, b64)
        }
        _ => format!(
            "{}:_authToken={}",
            key,
            account.token.as_deref().unwrap_or("")
        ),
    };

    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
    let prefix = format!("{}:", key);
    lines.retain(|l| !l.trim_start().starts_with(&prefix));
    lines.push(line);
    std::fs::write(&path, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 版本管理辅助
// ---------------------------------------------------------------------------

/// 读取注册表中的 PATH（机器级 + 用户级，合并去重），不依赖过期的进程环境。
pub fn get_registry_path_entries() -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    for level in ["Machine", "User"] {
        let ps = format!("[Environment]::GetEnvironmentVariable('Path','{}')", level);
        if let Some(raw) = run_cmd("powershell", &["-NoProfile", "-Command", &ps]) {
            for entry in raw.split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()) {
                if !entries.contains(&entry) {
                    entries.push(entry);
                }
            }
        }
    }
    entries
}

pub fn detect_nvm() -> bool {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    for d in [format!("{}\\nvm", appdata), format!("{}\\nvm-windows", appdata)] {
        if Path::new(&d).exists() {
            return true;
        }
    }
    for dir in get_registry_path_entries() {
        let lower = dir.to_lowercase();
        if lower.contains("\\nvm\\")
            || lower.ends_with("\\nvm")
            || lower.contains("\\nvm-windows")
        {
            return true;
        }
        if Path::new(&dir).join("nvm.exe").exists() {
            return true;
        }
    }
    false
}

pub fn current_version_from_path() -> Option<String> {
    let root_str = node_install_root();
    let root = Path::new(&root_str);
    for dir in get_registry_path_entries() {
        let p = Path::new(&dir);
        if let Some(parent) = p.parent() {
            if parent == root {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if let Some(ver) = name.strip_prefix('v') {
                        if ver.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                            return Some(name.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// 检测环境当前实际使用的 Node 版本：优先查自管目录 PATH，否则读真实 node -v。
fn detect_current_node_version() -> Option<String> {
    if let Some(v) = current_version_from_path() {
        return Some(v);
    }
    if let Some(out) = run_cmd("node", &["-v"]) {
        let t = out.trim().to_string();
        if t.starts_with('v') {
            return Some(t);
        }
    }
    None
}

fn dir_size(path: &Path) -> Option<u64> {
    fn walk(p: &Path, total: &mut u64) {
        if let Ok(entries) = std::fs::read_dir(p) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    walk(&e.path(), total);
                } else if let Ok(m) = e.metadata() {
                    *total += m.len();
                }
            }
        }
    }
    let mut total = 0u64;
    walk(path, &mut total);
    Some(total)
}

fn fetch_available_versions() -> Vec<String> {
    let html = run_cmd(
        "curl.exe",
        &[
            "-s",
            "-m",
            "15",
            "https://registry.npmmirror.com/-/binary/node/",
        ],
    )
    .unwrap_or_default();
    let re = regex::Regex::new(r"v(\d+)\.(\d+)\.(\d+)/").unwrap();
    // 每个大版本只取最新 patch，列表从 ~200 个精简到 ~27 个
    let mut best: std::collections::HashMap<i32, (i32, i32)> = std::collections::HashMap::new();
    for cap in re.captures_iter(&html) {
        let major: i32 = cap[1].parse().unwrap_or(0);
        let minor: i32 = cap[2].parse().unwrap_or(0);
        let patch: i32 = cap[3].parse().unwrap_or(0);
        let e = best.entry(major).or_insert((0, 0));
        if (minor, patch) > *e {
            *e = (minor, patch);
        }
    }
    let mut majors: Vec<i32> = best.keys().cloned().collect();
    majors.sort_unstable();
    majors.reverse();
    majors
        .iter()
        .map(|m| {
            let (min, p) = best[m];
            format!("v{}.{}.{}", m, min, p)
        })
        .collect()
}

/// 读取用户级 PATH 条目。
fn get_user_path_entries() -> Vec<String> {
    run_cmd(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "[Environment]::GetEnvironmentVariable('Path','User')",
        ],
    )
    .map(|s| {
        s.split(';')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

/// 写回用户级 PATH 条目。
fn set_user_path_entries(entries: &[String]) -> Result<(), String> {
    let joined = entries.join(";");
    if joined.trim().is_empty() {
        return Err("拒绝清空用户 PATH（结果为空）".to_string());
    }
    let escaped = joined.replace('\'', "''");
    let ps = format!("[Environment]::SetEnvironmentVariable('Path', '{}', 'User')", escaped);
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

/// 把 target 目录写入用户 PATH（移除旧的 nodejs\v* 与 current 条目，target 置顶）。
fn update_user_path_for_node(target: &str) -> Result<(), String> {
    let ps = format!(
        "$t='{}'; $p=[Environment]::GetEnvironmentVariable('Path','User'); \
         $parts = $p -split ';' | Where-Object {{ $_ -notmatch 'nodejs\\\\v\\d' -and $_ -notmatch 'nodejs\\\\current' }}; \
         $new = ($t + ';' + ($parts -join ';')).TrimEnd(';'); \
         [Environment]::SetEnvironmentVariable('Path', $new, 'User')",
        target
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

// ---------------------------------------------------------------------------
// Tauri 命令：检测 / 镜像
// ---------------------------------------------------------------------------

/// 检测 Node 环境：安装状态 / 版本 / 商店假命令 / PATH / 网络连通性。
#[tauri::command]
fn detect_node() -> NodeEnvInfo {
    let mut info = NodeEnvInfo::default();

    if let Some(apps) = windows_apps_dir() {
        let stub = Path::new(&apps).join("node.exe");
        if stub.exists() && is_zero_byte_stub(&stub) {
            info.store_stub = true;
        }
    }

    let apps_norm = windows_apps_dir().map(|d| d.to_lowercase());
    // 读注册表 PATH（机器+用户），不依赖过期的进程环境
    for dir in get_registry_path_entries() {
        if let Some(apps) = &apps_norm {
            if dir.to_lowercase() == *apps {
                continue;
            }
        }
        let cand = Path::new(&dir).join("node.exe");
        if cand.exists() {
            info.real_node_exe = Some(cand.to_string_lossy().to_string());
            info.in_path = true;
            info.node_dir_in_path = Some(dir.clone());
            break;
        }
    }

    if let Some(exe) = &info.real_node_exe {
        if let Some(v) = run_cmd(exe, &["-v"]) {
            info.node_version = Some(v);
            info.installed = true;
        }
        if let Some(dir) = Path::new(exe).parent() {
            let npm = dir.join("npm.cmd");
            if npm.exists() {
                let npm_path = npm.to_string_lossy().to_string();
                if let Some(nv) = run_cmd("cmd", &["/d", "/c", &npm_path, "-v"]) {
                    info.npm_version = Some(nv);
                }
            }
        }
    }

    info.network = vec![
        probe("官方 npm", "https://registry.npmjs.org"),
        probe("淘宝 npmmirror", "https://registry.npmmirror.com"),
    ];
    info
}

/// 读取全局 .npmrc 中的 registry。
#[tauri::command]
fn read_registry() -> Result<Option<String>, String> {
    let path = user_npmrc_path();
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    for line in content.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("registry=") {
            return Ok(Some(v.trim().to_string()));
        }
    }
    Ok(None)
}

/// 写入 registry 到全局 .npmrc（自动备份 .bak）。
#[tauri::command]
fn set_registry(url: String) -> Result<RegistryResult, String> {
    let path = user_npmrc_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::copy(&path, format!("{}.bak", path));

    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
    let mut found = false;
    for line in lines.iter_mut() {
        if line.trim_start().starts_with("registry=") {
            *line = format!("registry={}", url);
            found = true;
        }
    }
    if !found {
        lines.push(format!("registry={}", url));
    }
    let content = lines.join("\n") + "\n";
    std::fs::write(&path, content).map_err(|e| e.to_string())?;

    Ok(RegistryResult { ok: true, path })
}

/// 将目录追加到用户级 PATH（通过 PowerShell，去重）。
#[tauri::command]
fn fix_path(dir: String) -> Result<PathResult, String> {
    let ps = format!(
        "$d='{}'; $p=[Environment]::GetEnvironmentVariable('Path','User'); \
         if(-not ($p -split ';' -contains $d)){{ [Environment]::SetEnvironmentVariable('Path', $p.TrimEnd(';')+';'+$d,'User') }}",
        dir
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(PathResult { ok: true, path: dir })
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

/// 测试 registry 连通性：真实拉取 {url}/debug 包元数据。
#[tauri::command]
fn test_registry(url: String) -> RegistryTestResult {
    let mut u = url.trim().to_string();
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        u = format!("http://{}", u);
    }
    while u.ends_with('/') {
        u.pop();
    }
    probe_registry_endpoint(&u, &[])
}

/// 从 npmmirror 下载安装最新 LTS Node，通过事件上报阶段进度。
#[tauri::command]
fn install_node(app: tauri::AppHandle) -> Result<InstallResult, String> {
    use tauri::Emitter;

    let _ = app.emit("install-progress", serde_json::json!({ "stage": "connecting" }));
    let index = run_cmd(
        "curl.exe",
        &[
            "-s",
            "-m",
            "15",
            "https://registry.npmmirror.com/-/binary/node/",
        ],
    )
    .ok_or("无法获取 Node 版本列表")?;
    let version = latest_lts(&index).ok_or("无法解析最新 LTS 版本")?;

    let dir = install_specific(&version, |stage| {
        let _ = app.emit("install-progress", serde_json::json!({ "stage": stage }));
    })?;
    let _ = app.emit("install-progress", serde_json::json!({ "stage": "done" }));
    Ok(InstallResult {
        ok: true,
        version,
        dir,
    })
}

// ---------------------------------------------------------------------------
// Tauri 命令：上传登录配置
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_accounts() -> AccountsFile {
    read_accounts()
}

#[tauri::command]
fn save_account(account: Account) -> Result<AccountsFile, String> {
    let mut file = read_accounts();
    if let Some(existing) = file.accounts.iter_mut().find(|a| a.id == account.id) {
        *existing = account;
    } else {
        file.accounts.push(account);
    }
    write_accounts(&file)?;
    Ok(file)
}

#[tauri::command]
fn delete_account(id: String) -> Result<AccountsFile, String> {
    let mut file = read_accounts();
    file.accounts.retain(|a| a.id != id);
    if file.active.as_deref() == Some(&id) {
        file.active = None;
    }
    write_accounts(&file)?;
    Ok(file)
}

#[tauri::command]
fn switch_account(id: String) -> Result<AccountsFile, String> {
    let mut file = read_accounts();
    let account = file
        .accounts
        .iter()
        .find(|a| a.id == id)
        .cloned()
        .ok_or("账号不存在")?;
    write_npmrc_credential(&account)?;
    file.active = Some(id);
    write_accounts(&file)?;
    Ok(file)
}

#[tauri::command]
fn test_account(id: String) -> RegistryTestResult {
    let file = read_accounts();
    let account = match file.accounts.iter().find(|a| a.id == id) {
        Some(a) => a.clone(),
        None => {
            let mut r = RegistryTestResult::default();
            r.error = Some("账号不存在".to_string());
            return r;
        }
    };

    let mut u = account.registry.trim().to_string();
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        u = format!("http://{}", u);
    }
    while u.ends_with('/') {
        u.pop();
    }

    let mut auth_args: Vec<String> = Vec::new();
    match account.auth_type.as_str() {
        "basic" => {
            let cred = format!(
                "{}:{}",
                account.username.as_deref().unwrap_or(""),
                account.password.as_deref().unwrap_or("")
            );
            auth_args.push("-u".to_string());
            auth_args.push(cred);
        }
        _ => {
            if let Some(t) = &account.token {
                auth_args.push("-H".to_string());
                auth_args.push(format!("Authorization: Bearer {}", t));
            }
        }
    }
    let arg_refs: Vec<&str> = auth_args.iter().map(|s| s.as_str()).collect();
    let mut result = probe_registry_endpoint(&u, &arg_refs);
    if result.reachable {
        result.auth_user = check_whoami(&account);
    }
    result
}

// ---------------------------------------------------------------------------
// Tauri 命令：Node 版本管理
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_versions() -> VersionsInfo {
    let install_dir = node_install_root();
    let current = detect_current_node_version();
    let mut installed = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&install_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(ver) = name.strip_prefix('v') {
                if ver.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    let is_cur = current.as_ref() == Some(&name);
                    let size_mb = dir_size(&entry.path()).map(|b| b / 1024 / 1024);
                    installed.push(InstalledVersion {
                        version: name.clone(),
                        current: is_cur,
                        size_mb,
                    });
                }
            }
        }
    }
    installed.sort_by(|a, b| b.version.cmp(&a.version));
    let available = fetch_available_versions();
    VersionsInfo {
        nvm_detected: detect_nvm(),
        install_dir,
        installed,
        available,
        current,
    }
}

#[tauri::command]
fn install_version(app: tauri::AppHandle, version: String) -> Result<InstallResult, String> {
    use tauri::Emitter;

    let _ = app.emit(
        "version-progress",
        serde_json::json!({ "stage": "connecting", "version": version }),
    );
    let dir = install_specific(&version, |stage| {
        let _ = app.emit(
            "version-progress",
            serde_json::json!({ "stage": stage, "version": version }),
        );
    })?;
    let _ = app.emit(
        "version-progress",
        serde_json::json!({ "stage": "done", "version": version }),
    );
    Ok(InstallResult {
        ok: true,
        version,
        dir,
    })
}

#[tauri::command]
fn switch_version(version: String) -> Result<String, String> {
    if detect_nvm() {
        return Err("检测到 nvm 管理器，请先卸载 nvm 后再使用版本切换".to_string());
    }
    let install_dir = node_install_root();
    let target = format!("{}\\{}", install_dir, version);
    if !Path::new(&format!("{}\\node.exe", target)).exists() {
        return Err(format!("版本 {} 尚未安装", version));
    }
    update_user_path_for_node(&target)?;
    Ok(target)
}

/// 探测 nvm 安装信息：目录 + PATH 条目（用于卸载预览）。
#[tauri::command]
fn nvm_info() -> NvmInfo {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let mut dirs: Vec<String> = Vec::new();
    for d in [
        format!("{}\\nvm", appdata),
        format!("{}\\nvm-windows", appdata),
        format!("{}\\nvm", local),
    ] {
        if Path::new(&d).exists() && !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    let mut path_entries: Vec<String> = Vec::new();

    // 扫描用户级 + 系统级 PATH 中的 nvm 条目
    for level in ["User", "Machine"] {
        let ps = format!("[Environment]::GetEnvironmentVariable('Path','{}')", level);
        if let Some(raw) = run_cmd("powershell", &["-NoProfile", "-Command", &ps]) {
            for entry in raw.split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()) {
                if entry.to_lowercase().contains("nvm") {
                    path_entries.push(entry.clone());
                    let p = Path::new(&entry);
                    if p.join("nvm.exe").exists() {
                        if !dirs.contains(&entry) {
                            dirs.push(entry.clone());
                        }
                    } else if let Some(parent) = p.parent() {
                        let parent_str = parent.to_string_lossy().to_string();
                        if parent.join("nvm.exe").exists() && !dirs.contains(&parent_str) {
                            dirs.push(parent_str);
                        }
                    }
                }
            }
        }
    }
    NvmInfo {
        detected: !dirs.is_empty() || !path_entries.is_empty(),
        dirs,
        path_entries,
    }
}

/// 触发提权（UAC）清理系统级 PATH 中的 nvm 条目。
fn cleanup_machine_path_elevated() -> Result<(), String> {
    let script = r#"$p = [Environment]::GetEnvironmentVariable('Path','Machine')
$parts = $p -split ';' | Where-Object { $_ -and ($_ -notmatch 'nvm') }
[Environment]::SetEnvironmentVariable('Path', ($parts -join ';'), 'Machine')"#;
    let temp_ps1 = std::env::temp_dir().join("envmanager-clean-nvm.ps1");
    std::fs::write(&temp_ps1, script).map_err(|e| e.to_string())?;

    let ps_cmd = format!(
        "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','{}' -Wait",
        temp_ps1.to_string_lossy()
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .output();
    let _ = std::fs::remove_file(&temp_ps1);

    match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// 带重试的删除：应对大目录 + 杀毒扫描导致的临时文件锁。
fn delete_path_robust(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let mut last_err = String::new();
    for attempt in 0..3 {
        let result = if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        match result {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = format!("{}: {}", path.display(), e);
                if attempt < 2 {
                    std::thread::sleep(std::time::Duration::from_millis(800));
                }
            }
        }
    }
    Err(last_err)
}

/// 卸载 nvm：删除 nvm 目录 + 清理用户 PATH + 提权清理系统 PATH。
#[tauri::command]
fn uninstall_nvm() -> Result<String, String> {
    let info = nvm_info();
    if !info.detected {
        return Err("未检测到 nvm，无需卸载".to_string());
    }

    // 1. 删除 nvm 目录（含所有 nvm 管理的 Node 版本），带重试
    for d in &info.dirs {
        delete_path_robust(Path::new(d))?;
    }

    // 2. 移除用户 PATH 中的 nvm 条目
    let current = get_user_path_entries();
    let remaining: Vec<String> = current
        .iter()
        .filter(|e| !e.to_lowercase().contains("nvm"))
        .cloned()
        .collect();
    set_user_path_entries(&remaining)?;

    // 3. 系统级 PATH 中的 nvm 条目需管理员，触发 UAC 提权清理
    let machine_path =
        run_cmd("powershell", &["-NoProfile", "-Command", "[Environment]::GetEnvironmentVariable('Path','Machine')"])
            .unwrap_or_default();
    let mut msg = format!("已卸载 nvm\n删除目录: {}", info.dirs.join(", "));
    if machine_path.to_lowercase().contains("nvm") {
        match cleanup_machine_path_elevated() {
            Ok(()) => msg.push_str("\n系统 PATH 已清理（管理员）"),
            Err(e) => msg.push_str(&format!("\n系统 PATH 清理需管理员，请在 UAC 中确认：{}", e)),
        }
    }
    Ok(msg)
}

#[tauri::command]
fn uninstall_version(version: String) -> Result<String, String> {
    let install_dir = node_install_root();
    let target = format!("{}\\{}", install_dir, version);
    if !Path::new(&target).exists() {
        return Err(format!("版本 {} 不存在", version));
    }
    if let Some(cur) = current_version_from_path() {
        if cur == version {
            return Err("不能卸载当前正在使用的版本".to_string());
        }
    }
    std::fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    Ok(target)
}

// ---------------------------------------------------------------------------
// 包管理器（pnpm / yarn / cnpm / pm2 / bun）
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PkgManagerInfo {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub bin_path: Option<String>,
    pub data_dir: Option<String>,
}

fn find_in_path_entries(name: &str, entries: &[String]) -> Option<String> {
    let exe = format!("{}.exe", name);
    let cmd = format!("{}.cmd", name);
    for dir in entries {
        let p = Path::new(dir);
        for cand in [p.join(&exe), p.join(&cmd), p.join(name)] {
            if cand.exists() {
                return Some(cand.to_string_lossy().to_string());
            }
        }
    }
    None
}

fn pkg_data_dir(name: &str, user: &str, local: &str) -> String {
    match name {
        "pnpm" => format!("{}\\pnpm\\store", local),
        "yarn" => format!("{}\\Yarn", local),
        "cnpm" => format!("{}\\.cnpmrc", user),
        "pm2" => format!("{}\\.pm2", user),
        "bun" => format!("{}\\.bun", user),
        _ => format!("{}\\.{}", user, name),
    }
}

/// 列出包管理器状态（已装/版本/位置/数据目录）。
#[tauri::command]
fn list_pkg_managers() -> Vec<PkgManagerInfo> {
    let user = std::env::var("USERPROFILE").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let tools = ["pnpm", "yarn", "cnpm", "pm2", "bun"];
    // PATH 只读一次，避免每工具重复跑 PowerShell
    let entries = get_registry_path_entries();
    let mut out = Vec::new();
    for name in tools {
        let bin = find_in_path_entries(name, &entries);
        let mut info = PkgManagerInfo {
            name: name.to_string(),
            installed: bin.is_some(),
            version: None,
            bin_path: bin.clone(),
            data_dir: Some(pkg_data_dir(name, &user, &local)),
        };
        if let Some(b) = &bin {
            // 版本命令：bun 用 --version，其余用 -v
            let flag = if name == "bun" { "--version" } else { "-v" };
            if let Some(v) = run_cmd(b, &[flag]) {
                info.version = Some(v.trim().to_string());
            }
        }
        out.push(info);
    }
    out
}

/// 安装/切换包管理器版本。npm 生态走 `npm i -g`，bun 走独立升级。
#[tauri::command]
fn install_pkg_manager(name: String, version: Option<String>) -> Result<String, String> {
    let ver = version.unwrap_or_else(|| "latest".to_string());
    if name == "bun" {
        let entries = get_registry_path_entries();
        if find_in_path_entries("bun", &entries).is_some() {
            let _ = run_cmd("bun", &["upgrade", "--to", &ver]);
            return Ok(format!("bun 升级请求已发送（目标 {}）", ver));
        }
        let _ = run_cmd(
            "powershell",
            &["-NoProfile", "-Command", "irm bun.sh/install.ps1 | iex"],
        );
        return Ok("bun 安装脚本已执行，请重开终端后验证".to_string());
    }

    let target = format!("{}@{}", name, ver);
    let out = Command::new("npm.cmd")
        .args(["install", "-g", &target])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(format!("已安装 {}", target))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

// ---------------------------------------------------------------------------
// 项目级管理（打开项目目录，解析 .nvmrc / package.json / 项目 .npmrc）
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub path: String,
    pub node_requirement: Option<String>,
    pub requirement_source: Option<String>,
    pub package_manager: Option<String>,
    pub registry: Option<String>,
    pub has_nvmrc: bool,
    pub has_package_json: bool,
    pub has_npmrc: bool,
    pub current_node: Option<String>,
    pub node_installed: bool,
}

fn resolve_nvmrc(content: &str) -> Option<String> {
    let t = content.trim();
    if t.is_empty() {
        return None;
    }
    // 完整版本 v18.20.0 / 18.20.0
    if let Some(v) = regex::Regex::new(r"^v?(\d+\.\d+\.\d+)$")
        .ok()
        .and_then(|re| re.captures(t))
        .map(|c| format!("v{}", &c[1]))
    {
        return Some(v);
    }
    // 裸主版本 18 -> 解析为该大版本最新
    if let Some(m) = regex::Regex::new(r"^(\d+)$")
        .ok()
        .and_then(|re| re.captures(t))
        .map(|c| c[1].parse::<i32>().unwrap_or(0))
    {
        let info = list_versions();
        let best = info
            .available
            .iter()
            .find(|v| v.starts_with(&format!("v{}.", m)));
        return best.cloned();
    }
    // lts / lts/* / node -> 最新 LTS
    if t == "lts" || t.starts_with("lts/") || t == "node" {
        let info = list_versions();
        // available 按大版本降序，第一个偶数主版本即最新 LTS
        return info
            .available
            .iter()
            .find(|v| {
                v.strip_prefix('v')
                    .and_then(|s| s.split('.').next())
                    .and_then(|m| m.parse::<i32>().ok())
                    .map(|m| m % 2 == 0)
                    .unwrap_or(false)
            })
            .cloned();
    }
    None
}

/// 解析项目目录，返回项目要求的 Node/包管理器/registry，并与全局对比。
#[tauri::command]
fn inspect_project(path: String) -> Result<ProjectInfo, String> {
    let p = Path::new(&path);
    if !p.is_dir() {
        return Err("路径不是有效目录".to_string());
    }

    let mut info = ProjectInfo {
        path: path.clone(),
        node_requirement: None,
        requirement_source: None,
        package_manager: None,
        registry: None,
        has_nvmrc: false,
        has_package_json: false,
        has_npmrc: false,
        current_node: None,
        node_installed: false,
    };

    // .nvmrc
    let nvmrc = p.join(".nvmrc");
    if nvmrc.exists() {
        info.has_nvmrc = true;
        if let Ok(content) = std::fs::read_to_string(&nvmrc) {
            info.node_requirement = resolve_nvmrc(&content);
            info.requirement_source = Some(".nvmrc".to_string());
        }
    }

    // package.json
    let pkg = p.join("package.json");
    if pkg.exists() {
        info.has_package_json = true;
        if let Ok(content) = std::fs::read_to_string(&pkg) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if info.node_requirement.is_none() {
                    if let Some(eng) = json.get("engines").and_then(|e| e.get("node")).and_then(|n| n.as_str()) {
                        // engines 可能是 ">=18" / "18.x" / "20" —— 取能解析的
                        if let Some(v) = resolve_nvmrc(eng) {
                            info.node_requirement = Some(v);
                            info.requirement_source = Some("package.json engines".to_string());
                        }
                    }
                }
                if let Some(pm) = json.get("packageManager").and_then(|m| m.as_str()) {
                    info.package_manager = Some(pm.to_string());
                }
            }
        }
    }

    // 项目 .npmrc
    let npmrc = p.join(".npmrc");
    if npmrc.exists() {
        info.has_npmrc = true;
        if let Ok(content) = std::fs::read_to_string(&npmrc) {
            for line in content.lines() {
                if let Some(v) = line.trim().strip_prefix("registry=") {
                    info.registry = Some(v.trim().to_string());
                    break;
                }
            }
        }
    }

    // 全局当前 node
    info.current_node = detect_current_node_version();

    // 要求的版本是否已安装
    if let Some(req) = &info.node_requirement {
        let root = node_install_root();
        info.node_installed = Path::new(&format!("{}\\{}\\node.exe", root, req)).exists();
    }

    Ok(info)
}

/// 应用项目配置：安装+切换 Node 版本，激活包管理器。
#[tauri::command]
fn apply_project(path: String) -> Result<String, String> {
    let info = inspect_project(path.clone())?;
    let mut msgs = Vec::new();

    // 1. Node 版本
    if let Some(req) = &info.node_requirement {
        if !info.node_installed {
            let dir = install_specific(req, |stage| {
                let _ = stage;
            })?;
            msgs.push(format!("已安装 Node {}", req));
            let _ = dir;
        }
        let target = format!("{}\\{}", node_install_root(), req);
        if Path::new(&format!("{}\\node.exe", target)).exists() {
            update_user_path_for_node(&target)?;
            msgs.push(format!("Node 已切换到 {}", req));
        }
    } else {
        msgs.push("未检测到项目 Node 版本要求（.nvmrc / engines）".to_string());
    }

    // 2. 包管理器（package.json packageManager -> corepack）
    if let Some(pm) = &info.package_manager {
        let _ = run_cmd("corepack", &["use", pm]);
        msgs.push(format!("corepack 已激活 {}", pm));
    }

    Ok(msgs.join("\n"))
}

// ---------------------------------------------------------------------------
// CLI API（无 GUI 版本复用，不含 #[tauri::command] 宏）
// ---------------------------------------------------------------------------

pub mod api {
    use crate::{NodeEnvInfo, RegistryResult, VersionsInfo};

    pub fn detect_node() -> NodeEnvInfo {
        crate::detect_node()
    }
    pub fn read_registry() -> Result<Option<String>, String> {
        crate::read_registry()
    }
    pub fn set_registry(url: String) -> Result<RegistryResult, String> {
        crate::set_registry(url)
    }
    pub fn list_versions() -> VersionsInfo {
        crate::list_versions()
    }
    pub fn switch_version(version: String) -> Result<String, String> {
        crate::switch_version(version)
    }
    pub fn uninstall_version(version: String) -> Result<String, String> {
        crate::uninstall_version(version)
    }
    pub fn install_specific(version: &str, emit: impl Fn(&str)) -> Result<String, String> {
        crate::install_specific(version, emit)
    }
    pub fn detect_nvm() -> bool {
        crate::detect_nvm()
    }
}

// ---------------------------------------------------------------------------
// 应用入口
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let service = gitcore::service::Service::new(None).expect("初始化 Git 服务失败");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(git_cmds::ServiceState(std::sync::Arc::new(std::sync::Mutex::new(service))))
        .invoke_handler(tauri::generate_handler![
            detect_node,
            read_registry,
            set_registry,
            fix_path,
            install_node,
            test_registry,
            list_accounts,
            save_account,
            delete_account,
            switch_account,
            test_account,
            list_versions,
            install_version,
            switch_version,
            uninstall_version,
            nvm_info,
            uninstall_nvm,
            list_pkg_managers,
            install_pkg_manager,
            inspect_project,
            apply_project,
            git_cmds::git_list_accounts,
            git_cmds::git_get_identity,
            git_cmds::git_get_stats,
            git_cmds::git_add_account,
            git_cmds::git_update_account,
            git_cmds::git_delete_account,
            git_cmds::git_test_connection,
            git_cmds::git_switch_identity,
            git_cmds::git_sync_repo,
            git_cmds::git_is_git_repo,
            git_cmds::git_generate_key,
            git_cmds::git_regenerate_key,
            git_cmds::git_read_public_key,
            git_cmds::git_get_key_url,
            git_cmds::git_default_key_path,
            git_cmds::git_home_dir,
            git_cmds::git_ssh_config_path,
            git_cmds::git_ssh_dir,
            git_cmds::git_open_in_manager,
            git_cmds::git_open_url,
            git_cmds::git_check_env
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

//! DevEnvManager Tauri 后端。
//!
//! 策略：尽量复用 Windows 自带工具（curl.exe / tar.exe / powershell），
//! 减少第三方依赖与编译风险。检测在 Rust 内完成，网络探测与下载走 curl。

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
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
    /// 连接失败源于证书不受信任（自签/私有CA），前端应提示用户是否信任。
    #[serde(default)]
    pub cert_issue: bool,
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
    /// 凭据作用域：None = 全局 ~/.npmrc；Some(项目目录) = 写入该项目的 .npmrc
    #[serde(default)]
    pub project_dir: Option<String>,
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

/// 同时返回 stdout 与 stderr（探测需要区分「请求结果」与「证书/连接错误」）。
fn run_cmd_both(program: &str, args: &[&str]) -> (String, String) {
    match Command::new(program).args(args).output() {
        Ok(out) => (
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ),
        Err(e) => (String::new(), e.to_string()),
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

struct CurlProbe {
    status: u16,
    latency_ms: Option<u64>,
    throughput_kbs: Option<f64>,
    error: Option<String>,
}

/// 单次 curl 探测。
/// TLS 信任策略：已通过指纹锚定信任的 host 使用 `--cacert <pem>`（锚定验证）；
/// 未信任的 https host 不再无脑 `-k`，自签/私有CA 会在此报错，交由上层判定并提示。
fn probe_curl(endpoint: &str, auth_args: &[&str]) -> CurlProbe {
    let mut args: Vec<String> = vec![
        "-sS".into(), // 静默进度，但保留错误输出到 stderr（证书/连接错误需靠它判定）
        "-o".into(),
        "NUL".into(),
        "-w".into(),
        "%{http_code} %{time_total} %{size_download}".into(),
        "-L".into(),
        "-m".into(),
        "10".into(),
    ];
    if let Some((host, port)) = url_host_port(endpoint) {
        if let Some(pem) = host_pem_path(&host, port) {
            args.push("--cacert".into());
            args.push(pem);
        }
    }
    args.extend(auth_args.iter().map(|s| s.to_string()));
    args.push(endpoint.to_string());
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (stdout, stderr) = run_cmd_both("curl.exe", &arg_refs);
    let parts: Vec<&str> = stdout.trim().split_whitespace().collect();
    if parts.len() >= 3 {
        if let Ok(status) = parts[0].parse::<u16>() {
            if status > 0 {
                let time = parts[1].parse::<f64>().unwrap_or(0.0);
                let bytes = parts[2].parse::<f64>().unwrap_or(0.0);
                let throughput = if time > 0.0 && bytes > 0.0 {
                    Some(bytes / 1024.0 / time)
                } else {
                    None
                };
                return CurlProbe {
                    status,
                    latency_ms: Some((time * 1000.0) as u64),
                    throughput_kbs: throughput,
                    error: None,
                };
            }
        }
    }
    // 失败：优先取 stderr（证书/连接错误），其次 stdout
    let err = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "curl 执行失败".to_string()
    };
    CurlProbe {
        status: 0,
        latency_ms: None,
        throughput_kbs: None,
        error: Some(err),
    }
}

/// 统一 registry 探测：
/// 1) `/-/ping` 判定可达性（npm 标准健康检查，任何兼容源返回 200）
/// 2) 尝试拉取 `debug` 包测吞吐/验认证（私有源无此包时 404，不误判为不可达）
fn probe_registry_endpoint(registry_base: &str, auth_args: &[&str]) -> RegistryTestResult {
    let mut result = RegistryTestResult::default();

    let ping = probe_curl(&format!("{}/-/ping", registry_base), auth_args);
    if ping.status == 0 {
        result.reachable = false;
        if let Some(e) = &ping.error {
            if is_cert_error(e) {
                result.error = Some("证书不受信任（自签/私有CA）".to_string());
                result.cert_issue = true;
            } else {
                result.error = Some(format!("连接失败：{}", e));
            }
        } else {
            result.error = Some("连接失败（无法建立连接或超时）".to_string());
        }
        return result;
    }
    if ping.status == 401 || ping.status == 403 {
        result.reachable = true;
        result.needs_auth = true;
        result.error = Some(format!("HTTP {} 需要认证", ping.status));
        return result;
    }
    if !(200..400).contains(&ping.status) {
        result.reachable = false;
        result.error = Some(format!("HTTP {} 异常", ping.status));
        return result;
    }
    result.reachable = true;
    result.status = Some(ping.status);
    result.latency_ms = ping.latency_ms;

    // 次要探测：真实拉包（吞吐/认证），404 不视为不可达
    let pkg = probe_curl(&format!("{}/debug", registry_base), auth_args);
    match pkg.status {
        200..=399 => result.throughput_kbs = pkg.throughput_kbs,
        401 | 403 => result.needs_auth = true,
        _ => { /* 源正常但无该包，忽略 */ }
    }
    result
}

// ---------------------------------------------------------------------------
// 证书信任（自签/私有CA → 指纹锚定，TOFU 知情信任）
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CertInfo {
    pub host: String,
    pub port: u16,
    pub subject: Option<String>,
    pub issuer: Option<String>,
    pub self_signed: bool,
    pub sha256: Option<String>,
    pub pem: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrustedCert {
    pub host: String,
    pub port: u16,
    pub sha256: String,
    pub pem_path: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrustedCertsFile {
    #[serde(default)]
    pub certs: Vec<TrustedCert>,
}

fn certs_dir() -> String {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    format!("{}\\DevEnvManager\\certs", appdata)
}

fn trusted_certs_path() -> String {
    format!("{}\\trusted.json", certs_dir())
}

fn read_trusted_certs() -> TrustedCertsFile {
    std::fs::read_to_string(trusted_certs_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_trusted_certs(file: &TrustedCertsFile) -> Result<(), String> {
    let dir = certs_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    std::fs::write(trusted_certs_path(), json).map_err(|e| e.to_string())
}

/// 从 registry 地址解析出 (host, port)，https 默认 443 / http 默认 80。
fn url_host_port(url: &str) -> Option<(String, u16)> {
    let u = url.trim();
    let rest = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    match authority.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && p.parse::<u16>().is_ok() => {
            Some((h.to_string(), p.parse().unwrap()))
        }
        _ => {
            let default = if u.starts_with("https://") { 443 } else { 80 };
            Some((authority.to_string(), default))
        }
    }
}

fn host_pem_path(host: &str, port: u16) -> Option<String> {
    read_trusted_certs().certs.into_iter().find_map(|c| {
        if c.host.eq_ignore_ascii_case(host) && c.port == port {
            Some(c.pem_path)
        } else {
            None
        }
    })
}

/// 用 PowerShell + SslStream 抓取远端证书（忽略校验），返回主体/签发者/SHA-256/PEM。
/// 纯 Windows 自带组件，无需额外依赖。
fn fetch_cert_info(registry: &str) -> Result<CertInfo, String> {
    let (host, port) = url_host_port(registry).ok_or_else(|| "无法解析 registry 地址".to_string())?;
    let script = format!(
        "$h='{}';$p={};$tcp=New-Object System.Net.Sockets.TcpClient($h,$p);$cb=[System.Net.Security.RemoteCertificateValidationCallback]{{$true}};$ssl=New-Object System.Net.Security.SslStream($tcp.GetStream(),$false,$cb);$ssl.AuthenticateAsClient($h);$c=New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($ssl.RemoteCertificate);$sha=($c.GetCertHash([System.Security.Cryptography.HashAlgorithmName]::SHA256)|ForEach-Object{{$_.ToString('x2')}}) -join '';$b64=[Convert]::ToBase64String($c.RawData);$lines=$b64 -split '(.{{64}})'|Where-Object{{$_}};$pem=@('-----BEGIN CERTIFICATE-----')+$lines+@('-----END CERTIFICATE-----') -join \"`n\";$ssl.Close();$tcp.Close();[PSCustomObject]@{{host=$h;port=$p;subject=$c.Subject;issuer=$c.Issuer;selfSigned=($c.Subject -eq $c.Issuer);sha256=$sha;pem=$pem}}|ConvertTo-Json -Compress",
        host, port
    );
    let out = run_cmd("powershell.exe", &["-NoProfile", "-NonInteractive", "-Command", &script])
        .ok_or_else(|| "无法启动 PowerShell 抓取证书".to_string())?;
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).map_err(|e| format!("解析证书信息失败: {}", e))?;
    Ok(CertInfo {
        host: host.clone(),
        port,
        subject: v["subject"].as_str().map(|s| s.to_string()),
        issuer: v["issuer"].as_str().map(|s| s.to_string()),
        self_signed: v["selfSigned"].as_bool().unwrap_or(false),
        sha256: v["sha256"].as_str().map(|s| s.to_string()),
        pem: v["pem"].as_str().map(|s| s.to_string()),
        error: None,
    })
}

/// 合并所有受信证书为 all.pem，并 setx NODE_EXTRA_CA_CERTS（供 npm/Node 追加信任）。
/// best-effort：失败不阻断接受流程。
fn sync_node_extra_ca(tf: &TrustedCertsFile) -> Result<(), String> {
    let dir = certs_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut merged = String::new();
    for c in &tf.certs {
        if let Ok(pem) = std::fs::read_to_string(&c.pem_path) {
            let t = pem.trim();
            if !t.is_empty() {
                merged.push_str(t);
                merged.push_str("\n\n");
            }
        }
    }
    // 始终写 all.pem（空文件也写，避免 NODE_EXTRA_CA_CERTS 指向不存在的路径导致 Node 启动报错）
    let all_path = format!("{}\\all.pem", dir);
    std::fs::write(&all_path, merged).map_err(|e| e.to_string())?;
    let _ = run_cmd("setx.exe", &["NODE_EXTRA_CA_CERTS", all_path.as_str()]);
    Ok(())
}

/// 判断 curl 报错文本是否源于证书不受信任。
fn is_cert_error(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("untrusted_root")
        || l.contains("certificate verify")
        || l.contains("self-signed")
        || l.contains("ssl certificate problem")
        || l.contains("unable to get local issuer")
        || l.contains("unable to verify the first certificate")
}

// ---------------------------------------------------------------------------
// 下载并解压指定版本的 Node 到 install root，emit 阶段回调。
// ---------------------------------------------------------------------------
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

/// 用账号凭据对目标 registry 调用 `GET /-/whoami`，验证发布认证。
/// 返回用户名（成功）或 None（失败）。纯 curl 实现，不依赖 npm 是否安装。
/// npm 官方 whoami 即请求该端点，返回 `{"username":"..."}`。
fn check_whoami(account: &Account) -> Option<String> {
    let mut u = account.registry.trim().to_string();
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        u = format!("http://{}", u);
    }
    while u.ends_with('/') {
        u.pop();
    }
    let whoami_url = format!("{}/-/whoami", u);

    let mut auth_args: Vec<String> = vec![
        "-sS".to_string(),
        "-m".to_string(),
        "10".to_string(),
        "-w".to_string(),
        "\n%{http_code}".to_string(),
    ];
    // 已指纹锚定信任的自签/私有CA host：用 --cacert 锚定验证，避免 whoami 因证书失败
    if let Some((host, port)) = url_host_port(&u) {
        if let Some(pem) = host_pem_path(&host, port) {
            auth_args.push("--cacert".to_string());
            auth_args.push(pem);
        }
    }
    match account.auth_type.as_str() {
        "basic" => {
            let cred = format!(
                "{}:{}",
                account.username.as_deref().unwrap_or(""),
                account.password.as_deref().unwrap_or("")
            );
            let b64 = base64::engine::general_purpose::STANDARD.encode(cred.as_bytes());
            auth_args.push("-H".to_string());
            auth_args.push(format!("Authorization: Basic {}", b64));
        }
        _ => {
            auth_args.push("-H".to_string());
            auth_args.push(format!(
                "Authorization: Bearer {}",
                account.token.as_deref().unwrap_or("")
            ));
        }
    }
    auth_args.push(whoami_url);
    let arg_refs: Vec<&str> = auth_args.iter().map(|s| s.as_str()).collect();

    let out = run_cmd("curl.exe", &arg_refs)?;
    // 输出 = 响应体 + "\n" + http_code
    let (body, code) = out.rsplit_once('\n')?;
    if code.trim() != "200" {
        return None;
    }
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    // 标准返回 {"username":"xxx"}；个别源可能直接返回用户名文本
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("username").and_then(|n| n.as_str()).map(|s| s.to_string()))
        .or_else(|| Some(body.to_string()))
}

/// 生成账号的 .npmrc 凭据行，如 `//host:port/path/:_authToken=...`。
fn credential_line(account: &Account) -> String {
    let key = npmrc_key(&account.registry);
    match account.auth_type.as_str() {
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
    }
}

/// 检查发布目标 registry 是否已有可用的 .npmrc 凭据（用户级 + 发布目录级）。
/// npmjs 官方只认 token（`_authToken`），basic（`_auth`）会被拒，因此对 npmjs 只认 token 行。
fn has_registry_credential(registry: &str, dir: &str) -> bool {
    let key = npmrc_key(registry);
    let is_npmjs = registry.contains("npmjs.org");
    let paths = vec![user_npmrc_path(), format!("{}/.npmrc", dir)];
    for p in paths {
        if let Ok(content) = std::fs::read_to_string(&p) {
            for line in content.lines() {
                let l = line.trim_start();
                if l.starts_with(&key) {
                    let is_token = l.contains("_authToken=");
                    let is_basic = l.contains("_auth=");
                    if is_token || (!is_npmjs && is_basic) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// 账号凭据应写入/读取的 .npmrc 路径：项目级账号写项目文件，否则写全局。
fn npmrc_path_for(account: &Account) -> String {
    match account.project_dir.as_deref() {
        Some(dir) if !dir.trim().is_empty() => format!("{}\\{}", dir.trim_end_matches('\\'), ".npmrc"),
        _ => user_npmrc_path(),
    }
}

/// 把账号凭据写入 .npmrc（自动备份 .bak）。
/// - 全局账号：只写凭据行，不动 `registry=`（避免污染所有项目的下载源）。
/// - 项目账号：同时把 `registry=` 设为该账号 registry，项目内完整切换。
fn write_npmrc_credential(account: &Account) -> Result<(), String> {
    let path = npmrc_path_for(account);
    if let Some(dir) = account.project_dir.as_deref() {
        if !dir.trim().is_empty() && !Path::new(dir).is_dir() {
            return Err(format!("项目目录不存在: {}", dir));
        }
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::copy(&path, format!("{}.bak", path));

    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();

    // 项目级：确保 registry=<account.registry>
    if account.project_dir.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false) {
        let reg_line = format!("registry={}", account.registry);
        let mut found = false;
        for l in lines.iter_mut() {
            if l.trim_start().starts_with("registry=") {
                *l = reg_line.clone();
                found = true;
            }
        }
        if !found {
            lines.push(reg_line);
        }
    }

    // 替换同 registry key 的旧凭据行，再写入新凭据
    let line = credential_line(account);
    let prefix = format!("{}:", npmrc_key(&account.registry));
    lines.retain(|l| !l.trim_start().starts_with(&prefix));
    lines.push(line);
    std::fs::write(&path, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

/// 从 .npmrc 中移除账号凭据（删除账号时清理残留）。
/// 全局文件移除凭据行；项目文件额外移除 `registry=`（若等于该账号 registry）。
/// 兼容历史遗留：项目级账号也会顺带清理全局文件中的同名凭据行。
fn remove_npmrc_credential(account: &Account) -> Result<(), String> {
    let mut targets = vec![(user_npmrc_path(), false)];
    if let Some(dir) = account.project_dir.as_deref() {
        if !dir.trim().is_empty() {
            targets.push((npmrc_path_for(account), true));
        }
    }
    for (path, remove_registry) in targets {
        let existing = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue, // 文件不存在视为已清理
        };
        let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
        let before = lines.len();
        let line = credential_line(account);
        lines.retain(|l| l.trim() != line.trim());
        if remove_registry {
            let reg_line = format!("registry={}", account.registry);
            lines.retain(|l| l.trim() != reg_line);
        }
        if lines.len() != before {
            let content = if lines.is_empty() {
                String::new()
            } else {
                lines.join("\n") + "\n"
            };
            std::fs::write(&path, content).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
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

// ---------------------------------------------------------------------------
// Tauri 命令：证书信任
// ---------------------------------------------------------------------------

/// 抓取指定 registry 的证书信息（用于「证书不受信任」提示弹窗）。
#[tauri::command]
fn probe_cert(registry: String) -> Result<CertInfo, String> {
    fetch_cert_info(&registry)
}

/// 接受并信任指定 registry 的证书：落盘 PEM + 记录指纹 + 合并 NODE_EXTRA_CA_CERTS。
#[tauri::command]
fn accept_cert(registry: String) -> Result<CertInfo, String> {
    let info = fetch_cert_info(&registry)?;
    let sha = info.sha256.clone().ok_or_else(|| "未获取到证书指纹".to_string())?;
    let pem = info.pem.clone().ok_or_else(|| "未获取到证书内容".to_string())?;

    let dir = certs_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let pem_path = format!("{}\\{}_{}.pem", dir, info.host, info.port);
    std::fs::write(&pem_path, &pem).map_err(|e| e.to_string())?;

    let mut tf = read_trusted_certs();
    tf.certs.retain(|c| !(c.host.eq_ignore_ascii_case(&info.host) && c.port == info.port));
    tf.certs.push(TrustedCert {
        host: info.host.clone(),
        port: info.port,
        sha256: sha,
        pem_path: pem_path.clone(),
    });
    write_trusted_certs(&tf)?;

    // npm/Node 侧：合并 all.pem + setx（best-effort）
    let _ = sync_node_extra_ca(&tf);
    Ok(info)
}

#[tauri::command]
fn list_trusted_certs() -> TrustedCertsFile {
    read_trusted_certs()
}

#[tauri::command]
fn remove_trusted_cert(host: String, port: u16) -> Result<TrustedCertsFile, String> {
    let mut tf = read_trusted_certs();
    let removed: Vec<TrustedCert> = tf
        .certs
        .iter()
        .filter(|c| c.host.eq_ignore_ascii_case(&host) && c.port == port)
        .cloned()
        .collect();
    tf.certs.retain(|c| !(c.host.eq_ignore_ascii_case(&host) && c.port == port));
    write_trusted_certs(&tf)?;
    for c in &removed {
        let _ = std::fs::remove_file(&c.pem_path);
    }
    let _ = sync_node_extra_ca(&tf);
    Ok(tf)
}

// ---------------------------------------------------------------------------
// Tauri 命令：包发布工作流（workspace 扫描 / 撞车预检 / npm publish）
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPackage {
    pub name: String,
    pub version: String,
    pub dir: String,
    pub has_dist: bool,
    pub is_private: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPackageCheck {
    pub exists: bool,
    pub versions: Vec<String>,
    pub error: Option<String>,
}

/// 发布失败时的操作提示（前端渲染为可点击链接）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishHint {
    pub text: String,
    pub url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub name: String,
    pub version: String,
    pub ok: bool,
    pub message: String,
    /// 发布因账号 2FA 需要一次性验证码（EOTP），前端应提示输入 OTP 后重试。
    #[serde(default)]
    pub needs_otp: bool,
    /// 可选操作提示（渲染为链接），如 npmjs 403 需要 bypass-2FA token。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<PublishHint>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeCaInfo {
    pub all_pem_exists: bool,
    pub env_set: bool,
    pub env_matches: bool,
}

/// 扫描 `packages/*` 与 `packages/*/*` 下的可发布包（跳过 node_modules / extension）。
fn find_package_jsons(root: &str) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    let packages_dir = Path::new(root).join("packages");
    let Ok(entries) = std::fs::read_dir(&packages_dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        // depth 1: packages/*/package.json
        let pj1 = p.join("package.json");
        if pj1.exists() {
            out.push(pj1);
        }
        // depth 2: packages/*/*/package.json
        if let Ok(subs) = std::fs::read_dir(&p) {
            for se in subs.flatten() {
                let sp = se.path();
                if sp.is_dir()
                    && se.file_name() != "node_modules"
                    && se.file_name() != "extension"
                {
                    let pj2 = sp.join("package.json");
                    if pj2.exists() {
                        out.push(pj2);
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[tauri::command]
fn list_publish_packages(project_dir: String) -> Result<Vec<PublishPackage>, String> {
    let root = Path::new(&project_dir);
    if !root.is_dir() {
        return Err("项目目录不存在".to_string());
    }
    let mut pkgs = Vec::new();
    for pj in find_package_jsons(&project_dir) {
        let Ok(content) = std::fs::read_to_string(&pj) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        let name = v["name"].as_str().unwrap_or("").to_string();
        let version = v["version"].as_str().unwrap_or("").to_string();
        if name.is_empty() || version.is_empty() {
            continue;
        }
        let dir = pj
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let has_dist = Path::new(&dir).join("dist").is_dir();
        let is_private = v["private"].as_bool().unwrap_or(false);
        pkgs.push(PublishPackage {
            name,
            version,
            dir,
            has_dist,
            is_private,
        });
    }
    Ok(pkgs)
}

/// 检查某个包在 registry 上是否已存在（版本撞车预检）。
#[tauri::command]
fn check_registry_package(registry: String, name: String) -> RegistryPackageCheck {
    let mut u = registry.trim().to_string();
    while u.ends_with('/') {
        u.pop();
    }
    let url = format!("{}/{}", u, name);
    let mut args: Vec<String> = vec![
        "-sS".into(),
        "-m".into(),
        "10".into(),
        "-w".into(),
        "\n%{http_code}".into(),
    ];
    if let Some((host, port)) = url_host_port(&u) {
        if let Some(pem) = host_pem_path(&host, port) {
            args.push("--cacert".into());
            args.push(pem);
        }
    }
    args.push(url);
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (stdout, _stderr) = run_cmd_both("curl.exe", &arg_refs);
    let (body, code) = stdout.rsplit_once('\n').unwrap_or((stdout.as_str(), "0"));
    match code.trim().parse::<u16>().unwrap_or(0) {
        200 => {
            let versions = match serde_json::from_str::<serde_json::Value>(body.trim()) {
                Ok(json) => json
                    .get("versions")
                    .and_then(|v| v.as_object())
                    .map(|o| o.keys().cloned().collect())
                    .unwrap_or_default(),
                Err(_) => vec![],
            };
            RegistryPackageCheck {
                exists: true,
                versions,
                error: None,
            }
        }
        404 => RegistryPackageCheck {
            exists: false,
            versions: vec![],
            error: None,
        },
        401 | 403 => RegistryPackageCheck {
            exists: false,
            versions: vec![],
            error: Some(format!("HTTP {} 需要认证", code.trim())),
        },
        c => RegistryPackageCheck {
            exists: false,
            versions: vec![],
            error: Some(format!("HTTP {} 检查失败", c)),
        },
    }
}

/// 在指定目录跑 npm 命令（自动注入 NODE_EXTRA_CA_CERTS 以信任已锚定的自签证书）。
fn run_npm_in(dir: &str, args: &[&str]) -> (i32, String) {
    let all_pem = format!("{}\\all.pem", certs_dir());
    let mut cmd = Command::new("npm.cmd");
    cmd.current_dir(dir).args(args);
    if Path::new(&all_pem).exists() {
        cmd.env("NODE_EXTRA_CA_CERTS", &all_pem);
    }
    match cmd.output() {
        Ok(o) => {
            let code = o.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            let combined = if stdout.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                format!("{}\n{}", stdout.trim(), stderr.trim())
            };
            (code, combined)
        }
        Err(e) => (-1, format!("npm 启动失败: {}", e)),
    }
}

/// 从版本号派生发布 dist-tag：预发布版本（如 `0.9.0-beta.2` → `beta`）取 prerelease
/// 标识符第一段；稳定版返回 `None`（用 npm 默认的 `latest`）。
fn prerelease_tag(version: &str) -> Option<String> {
    let pre = version.split_once('-')?.1.split('+').next()?;
    let tag = pre.split('.').next().unwrap_or(pre).trim();
    if tag.is_empty() {
        None
    } else {
        Some(tag.to_string())
    }
}

/// 批量发布：每个包 撞车预检 → （可选）构建 → npm publish。
/// `otp`：一次性验证码（账号 2FA 需要时由前端提示输入后带上重试）。
#[tauri::command]
fn publish_packages(
    packages: Vec<String>,
    registry: String,
    build: bool,
    otp: Option<String>,
) -> Result<Vec<PublishResult>, String> {
    let reg = registry.trim().to_string();
    if reg.is_empty() {
        return Err("请填写发布源 registry".to_string());
    }
    let is_npmjs = reg.contains("npmjs.org");
    let otp_arg = otp
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let mut results = Vec::new();
    for dir in &packages {
        let pj = Path::new(dir).join("package.json");
        let content = std::fs::read_to_string(&pj).map_err(|e| format!("{}: {}", dir, e))?;
        let v: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("{}: {}", dir, e))?;
        let name = v["name"].as_str().unwrap_or("").to_string();
        let version = v["version"].as_str().unwrap_or("").to_string();
        let display = if name.is_empty() {
            dir.clone()
        } else {
            format!("{}@{}", name, version)
        };

        // 0) npmjs 凭据预检：官方只认 token，无凭据时给可执行的提示
        if is_npmjs && !has_registry_credential(&reg, dir) {
            results.push(PublishResult {
                name: display,
                version: version.clone(),
                ok: false,
                needs_otp: false,
                message: "未检测到 npmjs 凭据。发布到 npmjs 官方需要 token：\n\
                    1. 「上传登录」页新建/编辑 npmjs 账号（认证类型 Token，registry=https://registry.npmjs.org）\n\
                    2. 保存后在该账号点「切换」，把 _authToken 写入 ~/.npmrc\n\
                    或直接执行：npm login --registry=https://registry.npmjs.org/"
                    .to_string(),
                hint: None,
            });
            continue;
        }

        // 1) 版本撞车预检
        let check = check_registry_package(reg.clone(), name.clone());
        if let Some(e) = &check.error {
            results.push(PublishResult {
                name: display,
                version: version.clone(),
                ok: false,
                needs_otp: false,
                message: format!("预检失败: {}", e),
                hint: None,
            });
            continue;
        }
        if check.exists && check.versions.iter().any(|v| v == &version) {
            results.push(PublishResult {
                name: display,
                version: version.clone(),
                ok: false,
                needs_otp: false,
                message: "版本已存在，跳过（撞车）".to_string(),
                hint: None,
            });
            continue;
        }

        // 2) 构建（可选）
        if build {
            let (code, out) = run_npm_in(dir, &["run", "build"]);
            if code != 0 {
                results.push(PublishResult {
                    name: display,
                    version: version.clone(),
                    ok: false,
                    needs_otp: false,
                    message: format!("构建失败:\n{}", out),
                    hint: None,
                });
                continue;
            }
        }

        // 3) 发布
        // 预发布版本（如 0.9.0-beta）必须显式指定 dist-tag，否则 npm 拒绝发布到默认 latest。
        let pub_tag = prerelease_tag(&version);
        let mut npm_args: Vec<String> = vec![
            "publish".into(),
            "--registry".into(),
            reg.clone(),
            "--fetch-retries".into(),
            "0".into(),
        ];
        if let Some(tag) = &pub_tag {
            npm_args.push("--tag".into());
            npm_args.push(tag.clone());
        }
        if let Some(otp) = &otp_arg {
            npm_args.push("--otp".into());
            npm_args.push(otp.clone());
        }
        let arg_refs: Vec<&str> = npm_args.iter().map(|s| s.as_str()).collect();
        let (code, out) = run_npm_in(dir, &arg_refs);
        if code == 0 {
            results.push(PublishResult {
                name: display,
                version,
                ok: true,
                needs_otp: false,
                message: match &pub_tag {
                    Some(t) => format!("发布成功（tag: {}）", t),
                    None => "发布成功".to_string(),
                },
                hint: None,
            });
        } else {
            let lower = out.to_lowercase();
            let eotp = lower.contains("eotp") || lower.contains("one-time password");
            // npmjs 官方 403：账号要求 2FA（可输入 OTP 发布），或需 bypass-2FA token。
            let npmjs_2fa = is_npmjs
                && (lower.contains("two-factor authentication")
                    || lower.contains("bypass 2fa")
                    || lower.contains("granular access token"));
            let needs_otp = eotp || npmjs_2fa;
            let (message, hint) = if npmjs_2fa {
                (
                    "发布被拒（403）：npmjs 要求两步验证（2FA）。\n\
                     ① 若账号已启用 2FA：在手机验证器查看 6 位验证码并输入，即可发布（推荐，符合 npmjs 明年强制 2FA 的方向）\n\
                     ② 若输入 OTP 后仍失败：说明当前 token 是 granular 类型，需在 npmjs 生成 Automation（bypass-2FA）token 并更新本账号\n\
                     ③ 更新后在「上传登录」点「切换」生效"
                        .to_string(),
                    Some(PublishHint {
                        text: "npmjs 2FA 与 Token 说明 ↗".to_string(),
                        url: "https://docs.npmjs.com/about-two-factor-authentication"
                            .to_string(),
                    }),
                )
            } else if eotp {
                (
                    "发布需要一次性验证码（账号 2FA）：请在手机验证器查看 OTP 并重新发布。".to_string(),
                    None,
                )
            } else {
                (format!("发布失败:\n{}", out), None)
            };
            results.push(PublishResult {
                name: display,
                version,
                ok: false,
                needs_otp,
                message,
                hint,
            });
        }
    }
    Ok(results)
}

/// 检查 npm/Node 侧证书信任环境（NODE_EXTRA_CA_CERTS 是否指向工具管理的 all.pem）。
#[tauri::command]
fn check_node_extra_ca() -> NodeCaInfo {
    let all_pem = format!("{}\\all.pem", certs_dir());
    let all_pem_exists = Path::new(&all_pem).exists();
    let env = std::env::var("NODE_EXTRA_CA_CERTS").unwrap_or_default();
    let env_set = !env.trim().is_empty();
    let env_matches = env.trim().trim_end_matches('\\').eq_ignore_ascii_case(&all_pem);
    NodeCaInfo {
        all_pem_exists,
        env_set,
        env_matches,
    }
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
    let id = account.id.clone();
    let is_active = file.active.as_deref() == Some(id.as_str());
    if let Some(existing) = file.accounts.iter_mut().find(|a| a.id == id) {
        // 作用域变更（全局<->项目、项目A->项目B）时，清理旧位置残留凭据
        if existing.project_dir != account.project_dir {
            let old = existing.clone();
            let _ = remove_npmrc_credential(&old);
        }
        *existing = account;
    } else {
        file.accounts.push(account);
    }
    // 编辑的是当前激活账号时，同步重写 .npmrc 凭据，
    // 否则保存了新 token 但 .npmrc 仍是旧 token，发布会继续用旧凭据失败。
    if is_active {
        let acc = file
            .accounts
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or("账号不存在")?;
        write_npmrc_credential(&acc)?;
    }
    write_accounts(&file)?;
    Ok(file)
}

#[tauri::command]
fn delete_account(id: String) -> Result<AccountsFile, String> {
    let mut file = read_accounts();
    let target = file.accounts.iter().find(|a| a.id == id).cloned();
    file.accounts.retain(|a| a.id != id);
    if file.active.as_deref() == Some(&id) {
        file.active = None;
    }
    write_accounts(&file)?;
    // 清理该账号留在 .npmrc 里的凭据（best-effort，失败不阻断删除）
    if let Some(acc) = target {
        let _ = remove_npmrc_credential(&acc);
    }
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

/// 项目内允许编辑的配置文件白名单。
const PROJECT_CONFIG_FILES: [&str; 3] = [".nvmrc", ".npmrc", "package.json"];

fn project_config_file_path(project: &str, name: &str) -> Result<PathBuf, String> {
    let p = Path::new(project);
    if !p.is_dir() {
        return Err("路径不是有效目录".to_string());
    }
    if !PROJECT_CONFIG_FILES.contains(&name) {
        return Err(format!("不允许编辑该文件: {}", name));
    }
    Ok(p.join(name))
}

/// 读取项目配置文件内容（仅限白名单文件）。
#[tauri::command]
fn read_project_file(path: String, name: String) -> Result<String, String> {
    let f = project_config_file_path(&path, &name)?;
    if !f.exists() {
        return Err(format!("文件不存在: {}", name));
    }
    std::fs::read_to_string(&f).map_err(|e| format!("读取失败: {}", e))
}

/// 写回项目配置文件内容（仅限白名单文件）。
#[tauri::command]
fn write_project_file(path: String, name: String, content: String) -> Result<String, String> {
    let f = project_config_file_path(&path, &name)?;
    std::fs::write(&f, &content).map_err(|e| format!("写入失败: {}", e))?;
    Ok(content)
}

// ---------------------------------------------------------------------------
// Python：检测 / 版本管理 / pip 镜像 / 项目
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PythonEnvInfo {
    pub installed: bool,
    pub store_stub: bool,
    pub real_python_exe: Option<String>,
    pub python_version: Option<String>,
    pub pip_version: Option<String>,
    pub in_path: bool,
    pub python_dir_in_path: Option<String>,
    pub foreign: Vec<ForeignPython>,
    pub network: Vec<NetworkProbe>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForeignPython {
    pub kind: String,
    pub path: String,
    pub version: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonVersionsInfo {
    pub py_launcher_detected: bool,
    pub install_dir: String,
    pub installed: Vec<InstalledVersion>,
    pub available: Vec<String>,
    pub current: Option<String>,
}

fn python_install_root() -> String {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    format!("{}\\Programs\\python", local)
}

fn parse_python_version(s: &str) -> Option<(i32, i32, i32)> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    let mut it = t.split('.');
    let major: i32 = it.next()?.parse().ok()?;
    let minor: i32 = it.next()?.parse().ok()?;
    let patch: i32 = it.next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn version_str(v: (i32, i32, i32)) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

/// 从 npmmirror python 目录索引解析可用版本：每个小版本取最新 patch。
fn fetch_available_python_versions() -> Vec<String> {
    let json = run_cmd(
        "curl.exe",
        &[
            "-s",
            "-m",
            "15",
            "https://registry.npmmirror.com/-/binary/python/",
        ],
    )
    .unwrap_or_default();
    let re = regex::Regex::new(r#""name":"(\d+)\.(\d+)\.(\d+)/""#).unwrap();
    let mut best: std::collections::HashMap<i32, (i32, i32)> = std::collections::HashMap::new();
    for cap in re.captures_iter(&json) {
        let major: i32 = cap[1].parse().unwrap_or(0);
        if major != 3 {
            continue;
        }
        let minor: i32 = cap[2].parse().unwrap_or(0);
        // 3.8 以下已不可用（python-build-standalone 最低 3.8，现代 pip 也不支持），过滤
        if minor < 8 {
            continue;
        }
        let patch: i32 = cap[3].parse().unwrap_or(0);
        let e = best.entry(minor).or_insert((0, 0));
        if patch > e.1 {
            *e = (minor, patch);
        }
    }
    let mut minors: Vec<i32> = best.keys().cloned().collect();
    minors.sort_unstable();
    minors.reverse();
    minors
        .iter()
        .map(|m| {
            let (min, p) = best[m];
            format!("3.{}.{}", min, p)
        })
        .collect()
}

fn latest_patch_for_minor(minor: i32) -> Option<String> {
    fetch_available_python_versions()
        .into_iter()
        .find(|v| parse_python_version(v).map(|(_, m, _)| m == minor).unwrap_or(false))
}

/// 当前自管 Python 版本：扫 PATH 中 parent 为 install_root 的目录（或 Scripts 子目录）。
fn current_python_version_from_path() -> Option<String> {
    let root_lower = python_install_root().to_lowercase();
    for dir in get_registry_path_entries() {
        let p = Path::new(&dir);
        if let Some(parent) = p.parent() {
            if parent.to_string_lossy().to_lowercase() == root_lower {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if parse_python_version(name).is_some() {
                        return Some(name.to_string());
                    }
                }
            }
        }
        if p.file_name().and_then(|n| n.to_str()) == Some("Scripts") {
            if let Some(parent) = p.parent() {
                if let Some(gp) = parent.parent() {
                    if gp.to_string_lossy().to_lowercase() == root_lower {
                        if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
                            if parse_python_version(name).is_some() {
                                return Some(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn detect_current_python_version() -> Option<String> {
    if let Some(v) = current_python_version_from_path() {
        return Some(v);
    }
    if let Some(out) = run_cmd("python", &["-V"]) {
        let t = out.trim();
        if let Some(v) = t.strip_prefix("Python ") {
            return Some(v.to_string());
        }
        if parse_python_version(t).is_some() {
            return Some(t.to_string());
        }
    }
    None
}

fn detect_py_launcher() -> bool {
    if run_cmd("py", &["--version"]).is_some() {
        return true;
    }
    if let Ok(root) = std::env::var("SystemRoot") {
        if Path::new(&format!("{}\\py.exe", root)).exists() {
            return true;
        }
    }
    false
}

/// 在 python-build-standalone 日期目录里从新到旧找目标版本的 tarball URL。
fn find_python_build_standalone(version: &str) -> Option<String> {
    let base = "https://registry.npmmirror.com/-/binary/python-build-standalone";
    let index = run_cmd("curl.exe", &["-s", "-m", "15", &format!("{}/", base)]).unwrap_or_default();
    let re = regex::Regex::new(r#""name":"(\d{8})/""#).unwrap();
    let mut dates: Vec<String> = re.captures_iter(&index).map(|c| c[1].to_string()).collect();
    dates.sort();
    dates.reverse();
    for date in dates {
        for variant in ["install_only", "install_only_stripped"] {
            let name = format!(
                "cpython-{}+{}-x86_64-pc-windows-msvc-{}.tar.gz",
                version, date, variant
            );
            let url = format!("{}/{}/{}", base, date, name);
            let code = run_cmd(
                "curl.exe",
                &["-s", "-o", "NUL", "-w", "%{http_code}", "-I", "-L", "-m", "6", url.as_str()],
            )
            .unwrap_or_default();
            if code.trim() == "200" {
                return Some(url);
            }
        }
    }
    None
}

/// 下载并安装指定版本的 Python 到 install root。
/// 优先 npmmirror 完整 zip（3.11.9+），老版本回退 python-build-standalone。
pub fn install_python_specific(version: &str, emit: impl Fn(&str)) -> Result<String, String> {
    let ver = version.trim().strip_prefix('v').unwrap_or(version.trim()).to_string();
    let local = std::env::var("LOCALAPPDATA").map_err(|_| "缺少 LOCALAPPDATA".to_string())?;
    let parent = format!("{}\\Programs\\python", local);
    let target = format!("{}\\{}", parent, ver);

    if Path::new(&format!("{}\\python.exe", target)).exists() {
        return Ok(target);
    }
    let _ = std::fs::create_dir_all(&parent);

    // 路径 A：npmmirror 官方重打包完整 zip（带 pip，python -m pip 可用）
    emit("downloading");
    let zip_url = format!(
        "https://registry.npmmirror.com/-/binary/python/{}/python-{}-amd64.zip",
        ver, ver
    );
    let zip_path = format!("{}\\python-{}-amd64.zip", parent, ver);
    let dl_ok = Command::new("curl.exe")
        .args(["-f", "-L", "-s", "-o", &zip_path, &zip_url])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if dl_ok && Path::new(&zip_path).exists() {
        emit("extracting");
        let _ = std::fs::create_dir_all(&target);
        let _ = run_cmd("tar.exe", &["-xf", &zip_path, "-C", &target]);
        let _ = std::fs::remove_file(&zip_path);
        if Path::new(&format!("{}\\python.exe", target)).exists() {
            return Ok(target);
        }
    }
    let _ = std::fs::remove_file(&zip_path);

    // 路径 B：python-build-standalone 兜底（老版本）
    emit("connecting");
    if let Some(tar_url) = find_python_build_standalone(&ver) {
        emit("downloading");
        let tar_path = format!("{}\\python-{}.tar.gz", parent, ver);
        let dl_ok = Command::new("curl.exe")
            .args(["-f", "-L", "-s", "-o", &tar_path, &tar_url])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if dl_ok && Path::new(&tar_path).exists() {
            emit("extracting");
            let _ = std::fs::remove_dir_all(&target);
            let _ = std::fs::create_dir_all(&target);
            let _ = run_cmd("tar.exe", &["-xf", &tar_path, "-C", &target]);
            let _ = std::fs::remove_file(&tar_path);
            // python-build-standalone 顶层是 python/ 目录，摊平到 target
            let nested = Path::new(&target).join("python");
            if nested.exists() && !Path::new(&format!("{}\\python.exe", target)).exists() {
                if let Ok(entries) = std::fs::read_dir(&nested) {
                    for e in entries.flatten() {
                        let _ = std::fs::rename(&e.path(), Path::new(&target).join(e.file_name()));
                    }
                }
                let _ = std::fs::remove_dir_all(&nested);
            }
            if Path::new(&format!("{}\\python.exe", target)).exists() {
                return Ok(target);
            }
        }
        let _ = std::fs::remove_file(&tar_path);
    }
    Err(format!(
        "下载 Python {} 失败（npmmirror zip 与 python-build-standalone 均不可用）",
        ver
    ))
}

/// 把 target 与 target\Scripts 写入用户 PATH（移除旧的 python\3.* 条目，置顶）。
fn update_user_path_for_python(target: &str) -> Result<(), String> {
    let ps = format!(
        "$t='{}'; $s='{}\\Scripts'; $p=[Environment]::GetEnvironmentVariable('Path','User'); \
         $parts = $p -split ';' | Where-Object {{ $_ -notmatch 'Programs\\\\python\\\\3\\.' }}; \
         $new = ($t + ';' + $s + ';' + ($parts -join ';')).TrimEnd(';'); \
         [Environment]::SetEnvironmentVariable('Path', $new, 'User')",
        target, target
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

/// 读取 pip 配置文件（%APPDATA%\pip\pip.ini 或 PIP_CONFIG_FILE）。
fn pip_ini_path() -> String {
    if let Ok(p) = std::env::var("PIP_CONFIG_FILE") {
        return p;
    }
    std::env::var("APPDATA")
        .map(|a| format!("{}\\pip\\pip.ini", a))
        .unwrap_or_else(|_| "pip.ini".to_string())
}

#[tauri::command]
fn detect_python() -> PythonEnvInfo {
    let mut info = PythonEnvInfo::default();

    if let Some(apps) = windows_apps_dir() {
        for name in ["python.exe", "python3.exe"] {
            let stub = Path::new(&apps).join(name);
            if stub.exists() && is_zero_byte_stub(&stub) {
                info.store_stub = true;
            }
        }
    }

    let apps_norm = windows_apps_dir().map(|d| d.to_lowercase());
    for dir in get_registry_path_entries() {
        if let Some(apps) = &apps_norm {
            if dir.to_lowercase() == *apps {
                continue;
            }
        }
        let cand = Path::new(&dir).join("python.exe");
        if cand.exists() {
            info.real_python_exe = Some(cand.to_string_lossy().to_string());
            info.in_path = true;
            info.python_dir_in_path = Some(dir.clone());
            break;
        }
    }

    if let Some(exe) = &info.real_python_exe {
        if let Some(v) = run_cmd(exe, &["-V"]) {
            let t = v.trim();
            info.python_version = Some(
                t.strip_prefix("Python ")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| t.to_string()),
            );
            info.installed = true;
        }
        if let Some(pv) = run_cmd(exe, &["-m", "pip", "--version"]) {
            info.pip_version = Some(pv.trim().to_string());
        }
    }

    // 外来安装：官方安装器 / py launcher / conda（只报告，不干预）
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(format!("{}\\Programs\\Python", local)) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("Python3") {
                let exe = e.path().join("python.exe");
                if exe.exists() {
                    let v = run_cmd(exe.to_string_lossy().as_ref(), &["-V"])
                        .map(|s| s.trim().to_string());
                    info.foreign.push(ForeignPython {
                        kind: "official-install".to_string(),
                        path: exe.to_string_lossy().to_string(),
                        version: v,
                    });
                }
            }
        }
    }
    if let Some(v) = run_cmd("py", &["-3", "-V"]) {
        info.foreign.push(ForeignPython {
            kind: "py-launcher".to_string(),
            path: "py -3".to_string(),
            version: Some(v.trim().to_string()),
        });
    }
    if let Some(v) = run_cmd("conda", &["--version"]) {
        info.foreign.push(ForeignPython {
            kind: "conda".to_string(),
            path: "conda".to_string(),
            version: Some(v.trim().to_string()),
        });
    }

    info.network = vec![
        probe("官方 PyPI", "https://pypi.org/simple/"),
        probe("清华 TUNA", "https://pypi.tuna.tsinghua.edu.cn/simple/"),
        probe("阿里云", "https://mirrors.aliyun.com/pypi/simple/"),
    ];
    info
}

#[tauri::command]
fn list_python_versions() -> PythonVersionsInfo {
    let install_dir = python_install_root();
    let current = detect_current_python_version();
    let mut installed = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&install_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if parse_python_version(&name).is_some() {
                let is_cur = current.as_deref() == Some(name.as_str());
                let size_mb = dir_size(&entry.path()).map(|b| b / 1024 / 1024);
                installed.push(InstalledVersion {
                    version: name.clone(),
                    current: is_cur,
                    size_mb,
                });
            }
        }
    }
    installed.sort_by(|a, b| {
        let pa = parse_python_version(&a.version).unwrap_or((0, 0, 0));
        let pb = parse_python_version(&b.version).unwrap_or((0, 0, 0));
        pb.cmp(&pa)
    });
    let available = fetch_available_python_versions();
    PythonVersionsInfo {
        py_launcher_detected: detect_py_launcher(),
        install_dir,
        installed,
        available,
        current,
    }
}

#[tauri::command]
fn install_python_version(app: tauri::AppHandle, version: String) -> Result<InstallResult, String> {
    use tauri::Emitter;

    let _ = app.emit(
        "python-version-progress",
        serde_json::json!({ "stage": "connecting", "version": version }),
    );
    let dir = install_python_specific(&version, |stage| {
        let _ = app.emit(
            "python-version-progress",
            serde_json::json!({ "stage": stage, "version": version }),
        );
    })?;
    let _ = app.emit(
        "python-version-progress",
        serde_json::json!({ "stage": "done", "version": version }),
    );
    Ok(InstallResult {
        ok: true,
        version,
        dir,
    })
}

#[tauri::command]
fn switch_python_version(version: String) -> Result<String, String> {
    let install_dir = python_install_root();
    let target = format!("{}\\{}", install_dir, version);
    if !Path::new(&format!("{}\\python.exe", target)).exists() {
        return Err(format!("版本 {} 尚未安装", version));
    }
    update_user_path_for_python(&target)?;
    Ok(target)
}

#[tauri::command]
fn uninstall_python_version(version: String) -> Result<String, String> {
    let install_dir = python_install_root();
    let target = format!("{}\\{}", install_dir, version);
    if !Path::new(&target).exists() {
        return Err(format!("版本 {} 不存在", version));
    }
    if let Some(cur) = current_python_version_from_path() {
        if cur == version {
            return Err("不能卸载当前正在使用的版本".to_string());
        }
    }
    std::fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
    Ok(target)
}

/// 读取 pip 全局 index-url。
#[tauri::command]
fn read_pip_index() -> Result<Option<String>, String> {
    let path = pip_ini_path();
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut in_global = false;
    for line in content.lines() {
        let t = line.trim();
        if t == "[global]" {
            in_global = true;
            continue;
        }
        if t.starts_with('[') {
            in_global = false;
            continue;
        }
        if in_global {
            if let Some(v) = t
                .strip_prefix("index-url=")
                .or_else(|| t.strip_prefix("index-url ="))
            {
                return Ok(Some(v.trim().to_string()));
            }
        }
    }
    Ok(None)
}

/// 写入 pip 全局 index-url（[global] 段，自动备份 .bak）。
#[tauri::command]
fn set_pip_index(url: String) -> Result<RegistryResult, String> {
    let path = pip_ini_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::copy(&path, format!("{}.bak", path));

    let mut out: Vec<String> = Vec::new();
    let mut global_idx: Option<usize> = None;
    for line in existing.lines() {
        let t = line.trim();
        if t == "[global]" {
            global_idx = Some(out.len());
        }
        if t.starts_with("index-url") {
            continue;
        }
        out.push(line.to_string());
    }
    let url_line = format!("index-url = {}", url);
    match global_idx {
        Some(i) => {
            out.insert(i + 1, url_line);
        }
        None => {
            if !out.is_empty() && !out.iter().any(|l| l.trim().starts_with('[')) {
                out.insert(0, String::new());
                out.insert(0, "[global]".to_string());
                out.push(url_line);
            } else {
                out.push(String::new());
                out.push("[global]".to_string());
                out.push(url_line);
            }
        }
    }
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, out.join("\n") + "\n").map_err(|e| e.to_string())?;
    Ok(RegistryResult { ok: true, path })
}

/// PyPI 统一探测：GET {base}/simple/ 判可达，拉取 pip 包页测吞吐/验认证。
fn probe_pypi_endpoint(registry_base: &str, auth_args: &[&str]) -> RegistryTestResult {
    let mut result = RegistryTestResult::default();

    let ping = probe_curl(&format!("{}/simple/", registry_base), auth_args);
    if ping.status == 0 {
        result.reachable = false;
        if let Some(e) = &ping.error {
            if is_cert_error(e) {
                result.error = Some("证书不受信任（自签/私有CA）".to_string());
                result.cert_issue = true;
            } else {
                result.error = Some(format!("连接失败：{}", e));
            }
        } else {
            result.error = Some("连接失败（无法建立连接或超时）".to_string());
        }
        return result;
    }
    if ping.status == 401 || ping.status == 403 {
        result.reachable = true;
        result.needs_auth = true;
        result.error = Some(format!("HTTP {} 需要认证", ping.status));
        return result;
    }
    if !(200..400).contains(&ping.status) {
        result.reachable = false;
        result.error = Some(format!("HTTP {} 异常", ping.status));
        return result;
    }
    result.reachable = true;
    result.status = Some(ping.status);
    result.latency_ms = ping.latency_ms;

    let pkg = probe_curl(&format!("{}/simple/pip/", registry_base), auth_args);
    match pkg.status {
        200..=399 => result.throughput_kbs = pkg.throughput_kbs,
        401 | 403 => result.needs_auth = true,
        _ => { /* 源正常但包页缺失，忽略 */ }
    }
    result
}

#[tauri::command]
fn test_pypi(url: String) -> RegistryTestResult {
    let mut u = url.trim().to_string();
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        u = format!("http://{}", u);
    }
    while u.ends_with('/') {
        u.pop();
    }
    probe_pypi_endpoint(&u, &[])
}

// ---- Python 项目 ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonProjectInfo {
    pub path: String,
    pub python_requirement: Option<String>,
    pub requirement_source: Option<String>,
    pub pkg_tool: Option<String>,
    pub index: Option<String>,
    pub has_python_version_file: bool,
    pub has_pyproject_toml: bool,
    pub has_runtime_txt: bool,
    pub has_pipfile: bool,
    pub current_python: Option<String>,
    pub python_installed: bool,
}

fn resolve_python_req(raw: &str) -> Option<String> {
    let t = raw.trim();
    if let Some(v) = parse_python_version(t) {
        return Some(version_str(v));
    }
    if let Some(v) = regex::Regex::new(r"^(\d+)\.(\d+)$")
        .ok()
        .and_then(|re| re.captures(t))
    {
        let minor: i32 = v[2].parse().unwrap_or(0);
        return latest_patch_for_minor(minor);
    }
    if regex::Regex::new(r"^3(\.x)?$")
        .ok()
        .and_then(|re| re.captures(t))
        .is_some()
    {
        return fetch_available_python_versions().first().cloned();
    }
    None
}

/// 解析 pyproject.toml requires-python：">=3.11" / ">=3.11,<4" / "^3.11" 等，取最低版本。
fn parse_requires_python(spec: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?:>=|==|~=|~=|\^)?\s*(\d+\.\d+(?:\.\d+)?)").ok()?;
    let cap = re.captures(spec)?;
    let v = cap[1].to_string();
    if let Some((maj, min, pat)) = parse_python_version(&v) {
        if pat == 0 {
            latest_patch_for_minor(min)
        } else {
            Some(version_str((maj, min, pat)))
        }
    } else {
        None
    }
}

#[tauri::command]
fn inspect_python_project(path: String) -> Result<PythonProjectInfo, String> {
    let p = Path::new(&path);
    if !p.is_dir() {
        return Err("路径不是有效目录".to_string());
    }

    let mut info = PythonProjectInfo {
        path: path.clone(),
        python_requirement: None,
        requirement_source: None,
        pkg_tool: None,
        index: None,
        has_python_version_file: false,
        has_pyproject_toml: false,
        has_runtime_txt: false,
        has_pipfile: false,
        current_python: detect_current_python_version(),
        python_installed: false,
    };

    let pv = p.join(".python-version");
    if pv.exists() {
        info.has_python_version_file = true;
        if let Ok(c) = std::fs::read_to_string(&pv) {
            if let Some(req) = resolve_python_req(&c) {
                info.python_requirement = Some(req);
                info.requirement_source = Some(".python-version".to_string());
            }
        }
    }

    let pyproj = p.join("pyproject.toml");
    if pyproj.exists() {
        info.has_pyproject_toml = true;
        if let Ok(c) = std::fs::read_to_string(&pyproj) {
            if info.python_requirement.is_none() {
                let re = regex::Regex::new(r#"(?m)^requires-python\s*=\s*"([^"]+)""#).unwrap();
                if let Some(cap) = re.captures(&c) {
                    if let Some(req) = parse_requires_python(&cap[1]) {
                        info.python_requirement = Some(req);
                        info.requirement_source = Some("pyproject.toml requires-python".to_string());
                    }
                }
            }
            if info.pkg_tool.is_none() {
                for tool in ["uv", "poetry", "pipenv", "hatchling", "setuptools"] {
                    if c.contains(&format!("[tool.{}]", tool)) {
                        info.pkg_tool = Some(tool.to_string());
                        break;
                    }
                }
            }
            if info.index.is_none() {
                let re2 = regex::Regex::new(r#"(?m)^index\s*=\s*"([^"]+)""#).unwrap();
                if let Some(cap) = re2.captures(&c) {
                    info.index = Some(cap[1].to_string());
                }
            }
        }
    }

    let rt = p.join("runtime.txt");
    if rt.exists() {
        info.has_runtime_txt = true;
        if info.python_requirement.is_none() {
            if let Ok(c) = std::fs::read_to_string(&rt) {
                let t = c.trim();
                let t = t.strip_prefix("python-").unwrap_or(t);
                if let Some(req) = resolve_python_req(t) {
                    info.python_requirement = Some(req);
                    info.requirement_source = Some("runtime.txt".to_string());
                }
            }
        }
    }

    let pf = p.join("Pipfile");
    if pf.exists() {
        info.has_pipfile = true;
        if info.python_requirement.is_none() {
            if let Ok(c) = std::fs::read_to_string(&pf) {
                let re = regex::Regex::new(r#"(?m)^python_version\s*=\s*"([^"]+)""#).unwrap();
                if let Some(cap) = re.captures(&c) {
                    if let Some(req) = resolve_python_req(&cap[1]) {
                        info.python_requirement = Some(req);
                        info.requirement_source = Some("Pipfile".to_string());
                    }
                }
            }
        }
    }

    if let Some(req) = &info.python_requirement {
        let root = python_install_root();
        info.python_installed = Path::new(&format!("{}\\{}\\python.exe", root, req)).exists();
    }

    Ok(info)
}

/// 应用项目配置：安装+切换要求的 Python 版本。
#[tauri::command]
fn apply_python_project(path: String) -> Result<String, String> {
    let info = inspect_python_project(path.clone())?;
    let mut msgs = Vec::new();
    if let Some(req) = &info.python_requirement {
        if !info.python_installed {
            let _dir = install_python_specific(req, |_| {})?;
            msgs.push(format!("已安装 Python {}", req));
        }
        let target = format!("{}\\{}", python_install_root(), req);
        if Path::new(&format!("{}\\python.exe", target)).exists() {
            update_user_path_for_python(&target)?;
            msgs.push(format!("Python 已切换到 {}", req));
        }
    } else {
        msgs.push("未检测到项目 Python 版本要求（.python-version / pyproject.toml / runtime.txt / Pipfile）".to_string());
    }
    Ok(msgs.join("\n"))
}

// ---- Python 包管理器 ----

fn python_pkg_data_dir(name: &str, user: &str, local: &str) -> String {
    match name {
        "pip" => format!("{}\\pip\\cache", local),
        "uv" => format!("{}\\uv", local),
        "poetry" => format!("{}\\pypoetry", user),
        "pipenv" => format!("{}\\pipenv", local),
        "conda" => format!("{}\\miniconda3", user),
        _ => format!("{}\\.{}", user, name),
    }
}

#[tauri::command]
fn list_python_pkg_managers() -> Vec<PkgManagerInfo> {
    let user = std::env::var("USERPROFILE").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let tools = ["pip", "uv", "poetry", "pipenv", "conda"];
    let entries = get_registry_path_entries();
    let mut out = Vec::new();
    for name in tools {
        let bin = find_in_path_entries(name, &entries);
        let mut info = PkgManagerInfo {
            name: name.to_string(),
            installed: bin.is_some(),
            version: None,
            bin_path: bin.clone(),
            data_dir: Some(python_pkg_data_dir(name, &user, &local)),
        };
        if let Some(b) = &bin {
            if let Some(v) = run_cmd(b, &["--version"]) {
                info.version = Some(v.trim().to_string());
            }
        } else if name == "pip" {
            if let Some(v) = run_cmd("python", &["-m", "pip", "--version"]) {
                info.installed = true;
                info.version = Some(v.trim().to_string());
                info.bin_path = Some("python -m pip".to_string());
            }
        }
        out.push(info);
    }
    out
}

#[tauri::command]
fn install_python_pkg_manager(name: String, version: Option<String>) -> Result<String, String> {
    let ver = version.unwrap_or_else(|| "latest".to_string());
    match name.as_str() {
        "pip" => Err("pip 随 Python 内置，无需单独安装".to_string()),
        "conda" => Err("conda 需安装 Miniconda，暂不支持自动安装".to_string()),
        _ => {
            let target = if ver == "latest" {
                name.clone()
            } else {
                format!("{}=={}", name, ver)
            };
            let out = Command::new("python")
                .args(["-m", "pip", "install", &target])
                .output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(format!("已安装 {}", target))
            } else {
                Err(String::from_utf8_lossy(&out.stderr).to_string())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CLI API（无 GUI 版本复用，不含 #[tauri::command] 宏）
// ---------------------------------------------------------------------------

pub mod api {
    use crate::{
        NodeEnvInfo, PythonEnvInfo, PythonProjectInfo, PythonVersionsInfo, RegistryResult,
        RegistryTestResult, VersionsInfo,
    };

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

    // ---- Python ----
    pub fn detect_python() -> PythonEnvInfo {
        crate::detect_python()
    }
    pub fn read_pip_index() -> Result<Option<String>, String> {
        crate::read_pip_index()
    }
    pub fn set_pip_index(url: String) -> Result<RegistryResult, String> {
        crate::set_pip_index(url)
    }
    pub fn test_pypi(url: String) -> RegistryTestResult {
        crate::test_pypi(url)
    }
    pub fn list_python_versions() -> PythonVersionsInfo {
        crate::list_python_versions()
    }
    pub fn switch_python_version(version: String) -> Result<String, String> {
        crate::switch_python_version(version)
    }
    pub fn uninstall_python_version(version: String) -> Result<String, String> {
        crate::uninstall_python_version(version)
    }
    pub fn install_python_specific(version: &str, emit: impl Fn(&str)) -> Result<String, String> {
        crate::install_python_specific(version, emit)
    }
    pub fn inspect_python_project(path: String) -> Result<PythonProjectInfo, String> {
        crate::inspect_python_project(path)
    }
    pub fn apply_python_project(path: String) -> Result<String, String> {
        crate::apply_python_project(path)
    }
    pub fn list_python_pkg_managers() -> Vec<crate::PkgManagerInfo> {
        crate::list_python_pkg_managers()
    }
    pub fn install_python_pkg_manager(
        name: String,
        version: Option<String>,
    ) -> Result<String, String> {
        crate::install_python_pkg_manager(name, version)
    }
}

/// 列出各模块的配置文件/目录（Node / Python / Git / 工具自身），供前端「打开配置目录」使用。
#[tauri::command]
fn list_config_dirs() -> Vec<serde_json::Value> {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let appdata = std::env::var("APPDATA").unwrap_or_default();

    let mut items: Vec<serde_json::Value> = Vec::new();
    let mut add = |name: &str, path: String| {
        let exists = std::path::Path::new(&path).exists();
        items.push(serde_json::json!({ "name": name, "path": path, "exists": exists }));
    };

    // Node.js
    add("npm 全局配置 (.npmrc)", format!("{}\\.npmrc", home));
    add("npm 全局目录 (prefix)", format!("{}\\npm", appdata));
    add("Node 版本安装目录", node_install_root());
    add("nvm 安装目录", format!("{}\\nvm", appdata));
    // Python
    add("pip 配置 (pip.ini)", format!("{}\\pip\\pip.ini", appdata));
    add("Python 版本安装目录", python_install_root());
    // Git
    add("SSH 密钥目录 (.ssh)", format!("{}\\.ssh", home));
    add("Git 全局配置 (.gitconfig)", format!("{}\\.gitconfig", home));
    // 工具自身
    add("DevEnvManager 配置目录", format!("{}\\DevEnvManager", appdata));

    items
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
            probe_cert,
            accept_cert,
            list_trusted_certs,
            remove_trusted_cert,
            list_publish_packages,
            check_registry_package,
            publish_packages,
            check_node_extra_ca,
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
            read_project_file,
            write_project_file,
            detect_python,
            read_pip_index,
            set_pip_index,
            test_pypi,
            list_python_versions,
            install_python_version,
            switch_python_version,
            uninstall_python_version,
            inspect_python_project,
            apply_python_project,
            list_python_pkg_managers,
            install_python_pkg_manager,
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
            git_cmds::git_check_env,
            list_config_dirs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

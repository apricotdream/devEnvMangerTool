//! Go 环境管理：核心由 GUI 与 CLI 共用，下载和解压复用系统工具。
use crate::runtime::{atomic_write, checked_output};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Deserialize)]
struct Release {
    version: String,
    stable: bool,
    files: Vec<Asset>,
}
#[derive(Clone, Deserialize)]
struct Asset {
    filename: String,
    os: String,
    arch: String,
    sha256: String,
    kind: String,
}

/// 一个请求对应一个动作，避免 GUI 与 CLI 各自实现一套安装流程。
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub action: String,
    pub version: Option<String>,
    pub path: Option<String>,
    pub url: Option<String>,
    pub private: Option<String>,
    pub name: Option<String>,
}

pub fn install_root() -> PathBuf {
    #[cfg(target_os = "linux")]
    { crate::linux::install_root().join("go") }
    #[cfg(not(target_os = "linux"))]
    { dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Programs").join("DevEnvManager").join("go") }
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

fn normalize_version(raw: &str) -> Result<String, String> {
    let raw = raw.trim().trim_start_matches("go");
    if !regex::Regex::new(r"^\d+\.\d+(?:\.\d+)?$").unwrap().is_match(raw) {
        return Err("Go 版本格式应为 1.26.1 或 go1.26.1".to_string());
    }
    Ok(format!("go{raw}"))
}

fn version_key(raw: &str) -> (u64, u64, u64) {
    let mut parts = raw.trim_start_matches("go").split('.').map(|p| p.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

fn selected_version() -> Option<String> {
    let value = std::fs::read_to_string(install_root().join("current")).ok()?;
    let version = normalize_version(value.trim()).ok()?;
    install_root().join(&version).join("bin").join(executable_name("go")).is_file().then_some(version)
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let name = executable_name(name);
    #[cfg(windows)]
    let paths = crate::get_registry_path_entries().into_iter().map(PathBuf::from).collect::<Vec<_>>();
    #[cfg(not(windows))]
    let paths = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    paths.into_iter().map(|p| p.join(&name)).find(|p| p.is_file())
}

fn same_executable(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn go_executable() -> Result<PathBuf, String> {
    selected_version().map(|v| install_root().join(v).join("bin").join(executable_name("go")))
        .or_else(|| find_in_path("go")).ok_or_else(|| "未安装 Go，请先安装版本".to_string())
}

fn go_output(args: &[&str], cwd: Option<&Path>, local: bool) -> Result<String, String> {
    let exe = go_executable()?;
    let mut command = crate::new_cmd(&exe);
    command.args(args).env_remove("GOROOT");
    if local { command.env("GOTOOLCHAIN", "local"); }
    // 新安装的 Go/bin 立即供子进程使用，不等待重开应用。
    let mut paths = vec![exe.parent().unwrap().to_path_buf()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
    command.env("PATH", std::env::join_paths(paths).map_err(|e| e.to_string())?);
    if let Some(path) = cwd { command.current_dir(path); }
    checked_output(&mut command)
}

fn environment() -> Result<Value, String> {
    let mut env: Value = serde_json::from_str(&go_output(&["env", "-json", "GOROOT", "GOPATH", "GOBIN", "GOPROXY", "GOPRIVATE", "GOSUMDB", "GOENV"], None, true)?)
        .map_err(|e| e.to_string())?;
    // 检测禁用自动下载，但界面必须显示真实配置，而不是检测进程的临时 local。
    let saved = env["GOENV"].as_str().and_then(|path| config_value(Path::new(path), "GOTOOLCHAIN"));
    let defaults = env["GOROOT"].as_str().and_then(|root| config_value(&Path::new(root).join("go.env"), "GOTOOLCHAIN"));
    env["GOTOOLCHAIN"] = json!(std::env::var("GOTOOLCHAIN").ok().filter(|s| !s.is_empty()).or(saved).or(defaults).unwrap_or_else(|| "local".into()));
    Ok(env)
}

fn config_value(path: &Path, key: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()?.lines().find_map(|line| {
        line.trim().strip_prefix(&format!("{key}=")).filter(|value| !value.is_empty()).map(str::to_string)
    })
}

fn status() -> Result<Value, String> {
    let path = match go_executable() {
        Ok(path) => path,
        Err(_) => return Ok(json!({"installed":false,"installDir":install_root(),"env":{}})),
    };
    let version = go_output(&["version"], None, true)?;
    let shadowed = find_in_path("go").filter(|found| !same_executable(found, &path));
    Ok(json!({"installed":true,"version":version,"path":path,"env":environment()?,
        "managed":path.starts_with(install_root()),"installDir":install_root(),
        "shadowedPath":shadowed,
        "gorootOverride":std::env::var("GOROOT").ok(),
        "proxyOverride":std::env::var("GOPROXY").ok()}))
}

fn releases() -> Result<Vec<Release>, String> {
    type ReleaseCache = Mutex<Option<(Instant, Vec<Release>)>>;
    static CACHE: OnceLock<ReleaseCache> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Mutex::default).lock().map_err(|e| e.to_string())?;
    if let Some((time, items)) = &*cache {
        if time.elapsed() < Duration::from_secs(300) { return Ok(items.clone()); }
    }
    let mut response = Err("无法获取 Go 版本列表".to_string());
    for url in ["https://golang.google.cn/dl/?mode=json&include=all", "https://go.dev/dl/?mode=json&include=all"] {
        response = checked_output(crate::new_cmd(crate::curl_bin()).args([
            "-fLSs", "--connect-timeout", "5", "--max-time", "20", url
        ]));
        if response.is_ok() { break; }
    }
    let text = response?;
    let mut items: Vec<Release> = serde_json::from_str(&text).map_err(|e| format!("Go 版本列表解析失败：{e}"))?;
    items.retain(|r| r.stable && normalize_version(&r.version).is_ok());
    items.sort_by_key(|r| std::cmp::Reverse(version_key(&r.version)));
    *cache = Some((Instant::now(), items.clone()));
    Ok(items)
}

fn platform() -> Result<(&'static str, &'static str), String> {
    let os = match std::env::consts::OS { "windows" => "windows", "linux" => "linux", _ => return Err("Go 托管安装目前支持 Windows / Linux / WSL".into()) };
    let arch = match std::env::consts::ARCH { "x86_64" => "amd64", "aarch64" => "arm64", "x86" => "386", _ => return Err("不支持当前 CPU 架构".into()) };
    Ok((os, arch))
}

fn asset<'a>(release: &'a Release, os: &str, arch: &str) -> Option<&'a Asset> {
    release.files.iter().find(|f| f.os == os && f.arch == arch && f.kind == "archive")
}

fn versions() -> Result<Value, String> {
    let current = selected_version();
    let root = install_root();
    let mut installed = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let version = entry.file_name().to_string_lossy().to_string();
            if normalize_version(&version).is_ok() && entry.path().join("bin").join(executable_name("go")).is_file() {
                installed.push(json!({"version":version,"current":current.as_ref()==Some(&version)}));
            }
        }
    }
    installed.sort_by_key(|v| std::cmp::Reverse(version_key(v["version"].as_str().unwrap())));
    let (os, arch) = platform()?;
    let mut available = Vec::new();
    let mut error = None;
    match releases() {
        Ok(items) => {
            let mut seen = std::collections::HashSet::new();
            for release in items {
                let key = version_key(&release.version);
                if asset(&release, os, arch).is_some() && seen.insert((key.0, key.1)) {
                    available.push(release.version);
                    if available.len() == 12 { break; }
                }
            }
        }
        Err(e) => error = Some(e),
    }
    Ok(json!({"installed":installed,"available":available,"current":current,"installDir":root,"error":error}))
}

fn install(raw: Option<&str>, emit: &impl Fn(&str)) -> Result<String, String> {
    let releases = releases()?;
    let (os, arch) = platform()?;
    let version = match raw.filter(|v| !v.is_empty()) {
        Some(v) => normalize_version(v)?,
        None => releases.iter().find(|r| asset(r, os, arch).is_some()).ok_or("没有适合当前平台的 Go 版本")?.version.clone(),
    };
    let release = releases.iter().find(|r| r.version == version).ok_or("该稳定版本不存在")?;
    let asset = asset(release, os, arch).ok_or("该版本不支持当前平台")?;
    let root = install_root();
    let target = root.join(&version);
    if target.join("bin").join(executable_name("go")).is_file() {
        return switch_version(&version);
    }
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let staging = root.join(format!(".install-{version}-{}", std::process::id()));
    if staging.exists() { std::fs::remove_dir_all(&staging).map_err(|e| e.to_string())?; }
    std::fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        emit("downloading");
        let archive = staging.join(&asset.filename);
        let mut downloaded = Err("无法下载 Go 安装包".to_string());
        for base in ["https://golang.google.cn/dl", "https://registry.npmmirror.com/-/binary/go"] {
            downloaded = checked_output(crate::new_cmd(crate::curl_bin()).args(["-fLSs", "--connect-timeout", "8", "--max-time", "900", "-o"])
                .arg(&archive).arg(format!("{base}/{}", asset.filename)));
            if downloaded.is_ok() { break; }
        }
        downloaded?;
        emit("verifying");
        verify_sha256(&archive, &asset.sha256)?;
        emit("extracting");
        checked_output(crate::new_cmd(if cfg!(windows) { "tar.exe" } else { "tar" }).arg("-xf").arg(&archive).arg("-C").arg(&staging))?;
        let unpacked = staging.join("go");
        checked_output(crate::new_cmd(unpacked.join("bin").join(executable_name("go"))).arg("version").env("GOTOOLCHAIN", "local").env_remove("GOROOT"))?;
        // 只在完整校验、解压、运行成功后暴露为已安装版本。
        std::fs::rename(&unpacked, &target).map_err(|e| format!("写入 Go 安装目录失败：{e}"))?;
        let switched = switch_version(&version)?;
        emit("done");
        Ok(format!("已安装 {version}；{switched}"))
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result
}

fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let size = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if size == 0 { break; }
        hasher.update(&buffer[..size]);
    }
    if format!("{:x}", hasher.finalize()) != expected.to_lowercase() { return Err("Go 安装包 SHA256 校验失败，请重新下载".into()); }
    Ok(())
}

fn switch_version(raw: &str) -> Result<String, String> {
    let version = normalize_version(raw)?;
    let target = install_root().join(&version);
    if !target.join("bin").join(executable_name("go")).is_file() { return Err(format!("{version} 尚未安装")); }
    #[cfg(windows)] {
        let root = install_root();
        let mut entries = read_user_path()?;
        entries.retain(|p| !Path::new(p).starts_with(&root));
        entries.insert(0, target.join("bin").to_string_lossy().to_string());
        crate::set_user_path_entries(&entries)?;
    }
    #[cfg(target_os = "linux")] {
        let bin = crate::linux::local_bin();
        std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
        // 仅覆盖本工具的软链，保留用户自己的命令。
        for name in ["go", "gofmt"] { check_managed_link(&bin.join(name))?; }
        for name in ["go", "gofmt"] {
            let dest = bin.join(name);
            if std::fs::symlink_metadata(&dest).is_ok() { std::fs::remove_file(&dest).map_err(|e| e.to_string())?; }
            std::os::unix::fs::symlink(target.join("bin").join(name), dest).map_err(|e| e.to_string())?;
        }
        crate::linux::ensure_local_bin_in_path();
    }
    atomic_write(&install_root().join("current"), version.as_bytes())?;
    let executable = target.join("bin").join(executable_name("go"));
    if let Some(shadowed) = find_in_path("go").filter(|path| !same_executable(path, &executable)) {
        return Ok(format!("本工具已切换到 {version}。终端 PATH 仍优先命中 {}，请调整该外部安装的 PATH 顺序后重开终端。", shadowed.display()));
    }
    Ok(format!("已切换到 {version}；新终端生效"))
}

#[cfg(windows)]
fn read_user_path() -> Result<Vec<String>, String> {
    let value = checked_output(crate::new_cmd("powershell").args(["-NoProfile", "-Command", "[Environment]::GetEnvironmentVariable('Path','User')"]))?;
    Ok(value.split(';').filter(|s| !s.trim().is_empty()).map(str::to_string).collect())
}

#[cfg(target_os = "linux")]
fn check_managed_link(path: &Path) -> Result<(), String> {
    if std::fs::symlink_metadata(path).is_ok() && !std::fs::read_link(path).map(|p| p.starts_with(install_root())).unwrap_or(false) {
        return Err(format!("{} 已存在且不属于本工具，请先移走后再切换", path.display()));
    }
    Ok(())
}

fn uninstall(raw: &str) -> Result<String, String> {
    let version = normalize_version(raw)?;
    let target = install_root().join(&version);
    if !target.is_dir() { return Err("该托管版本不存在".into()); }
    if selected_version().as_deref() == Some(&version) {
        #[cfg(windows)] {
            let mut entries = read_user_path()?;
            entries.retain(|p| !Path::new(p).eq(&target.join("bin")));
            // 已成功读取 PATH，允许卸载唯一托管条目后结果为空。
            let escaped = entries.join(";").replace('\'', "''");
            checked_output(crate::new_cmd("powershell").args(["-NoProfile", "-Command", &format!("[Environment]::SetEnvironmentVariable('Path','{escaped}','User')")]))?;
        }
        #[cfg(target_os = "linux")]
        for name in ["go", "gofmt"] {
            let dest = crate::linux::local_bin().join(name);
            if std::fs::read_link(&dest).map(|p| p.starts_with(&target)).unwrap_or(false) {
                std::fs::remove_file(dest).map_err(|e| e.to_string())?;
            }
        }
        std::fs::remove_file(install_root().join("current")).map_err(|e| e.to_string())?;
    }
    std::fs::remove_dir_all(target).map_err(|e| format!("删除失败，请关闭占用 Go 的进程后重试：{e}"))?;
    Ok(format!("已卸载 {version}"))
}

fn proxy_settings(url: &str, private: Option<&str>) -> Result<Value, String> {
    if url.trim().is_empty() || url.contains(['\n', '\r']) { return Err("请输入 GOPROXY 地址".into()); }
    // 允许 Go 原生的逗号/竖线回退语法及 direct/off。
    for item in url.split([',', '|']) {
        if item != "direct" && item != "off" && !item.starts_with("https://") && !item.starts_with("http://") {
            return Err("代理地址需为 http(s) URL、direct 或 off".into());
        }
    }
    let mut args = vec!["env".to_string(), "-w".into(), format!("GOPROXY={}", url.trim())];
    if let Some(private) = private { args.push(format!("GOPRIVATE={}", private.trim())); }
    go_output(&args.iter().map(String::as_str).collect::<Vec<_>>(), None, true)?;
    Ok(json!({"message":"Go 代理配置已保存","env":environment()?,"proxyOverride":std::env::var("GOPROXY").ok()}))
}

fn proxy_test(url: &str) -> Result<Value, String> {
    let mut results = Vec::new();
    for base in url.split([',', '|']) {
        let base = base.trim();
        if base == "direct" || base == "off" { continue; }
        if !base.starts_with("https://") && !base.starts_with("http://") { return Err("请输入有效代理 URL".into()); }
        let endpoint = format!("{}/golang.org/x/text/@v/list", base.trim_end_matches('/'));
        let start = Instant::now();
        let result = checked_output(crate::new_cmd(crate::curl_bin()).args(["-fLSs", "--connect-timeout", "5", "--max-time", "12", "-o", crate::null_dev(), &endpoint]));
        results.push(json!({"url":base,"ok":result.is_ok(),"latencyMs":start.elapsed().as_millis(),"error":result.err()}));
    }
    if results.is_empty() { return Err("direct/off 不提供代理测速，请填写代理 URL".into()); }
    Ok(json!(results))
}

const TOOLS: [(&str, &str); 3] = [
    ("gopls", "golang.org/x/tools/gopls"),
    ("dlv", "github.com/go-delve/delve/cmd/dlv"),
    ("goimports", "golang.org/x/tools/cmd/goimports"),
];

fn tool_bin(env: &Value) -> Result<PathBuf, String> {
    if let Some(path) = env["GOBIN"].as_str().filter(|s| !s.is_empty()) { return Ok(PathBuf::from(path)); }
    let paths = env["GOPATH"].as_str().ok_or("无法读取 GOPATH")?;
    std::env::split_paths(paths).next().map(|p| p.join("bin")).ok_or_else(|| "GOPATH 为空".into())
}

fn tools() -> Result<Value, String> {
    let env = environment()?;
    let bin = tool_bin(&env)?;
    let items = TOOLS.iter().map(|(name, package)| {
        let candidate = bin.join(executable_name(name));
        let path = if candidate.is_file() { Some(candidate) } else { find_in_path(name) };
        let version = path.as_ref().and_then(|p| go_output(&["version", "-m", &p.to_string_lossy()], None, true).ok())
            .and_then(|text| text.lines().find_map(|l| {
                let parts = l.split_whitespace().collect::<Vec<_>>();
                (parts.first() == Some(&"mod")).then(|| parts.get(2).copied().unwrap_or("").to_string())
            }));
        json!({"name":name,"package":package,"installed":path.is_some(),"path":path,"version":version,"dataDir":bin})
    }).collect::<Vec<_>>();
    Ok(json!(items))
}

fn install_tool(name: &str, version: Option<&str>) -> Result<String, String> {
    let package = TOOLS.iter().find(|(n, _)| *n == name).ok_or("不支持该 Go 工具")?.1;
    let version = version.filter(|s| !s.is_empty()).unwrap_or("latest");
    if !regex::Regex::new(r"^(latest|v?\d+\.\d+\.\d+(?:[-+][\w.]+)?)$").unwrap().is_match(version) { return Err("工具版本应为 latest 或 v0.20.0 等完整版本".into()); }
    let version = if version == "latest" || version.starts_with('v') { version.to_string() } else { format!("v{version}") };
    go_output(&["install", &format!("{package}@{version}")], None, false)?;
    let bin = tool_bin(&environment()?)?;
    #[cfg(windows)] {
        let mut entries = read_user_path()?;
        if !entries.iter().any(|p| Path::new(p) == bin) {
            entries.push(bin.to_string_lossy().to_string());
            crate::set_user_path_entries(&entries)?;
        }
    }
    #[cfg(target_os = "linux")]
    {
        // 工具默认在 ~/go/bin，写入 shell PATH，不挪动用户已有工具。
        crate::linux::ensure_bin_in_path(&bin)?;
    }
    Ok(format!("已安装 {name}@{version}，目录：{}；新终端生效", bin.display()))
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    path: String,
    module: Option<String>,
    go_version: Option<String>,
    toolchain: Option<String>,
    source: String,
    required_version: Option<String>,
}

fn parse_project(text: &str, project: &mut Project) {
    for line in text.lines() {
        let mut words = line.split("//").next().unwrap_or("").split_whitespace();
        match (words.next(), words.next()) {
            (Some("module"), Some(value)) => project.module = Some(value.trim_matches('"').into()),
            (Some("go"), Some(value)) => project.go_version = Some(value.into()),
            (Some("toolchain"), Some(value)) => project.toolchain = Some(value.into()),
            _ => {}
        }
    }
}

fn inspect_project(path: &str) -> Result<Project, String> {
    let root = Path::new(path).canonicalize().map_err(|e| format!("项目目录无效：{e}"))?;
    if !root.is_dir() { return Err("请选择项目目录".into()); }
    let mut project = Project { path: root.to_string_lossy().to_string(), ..Default::default() };
    let module = root.join("go.mod");
    if module.is_file() {
        parse_project(&std::fs::read_to_string(&module).map_err(|e| e.to_string())?, &mut project);
        project.source = "go.mod".into();
    }
    // Go 优先采用当前目录及父目录的 go.work；尊重 GOWORK=off 或显式路径。
    let workspace = match std::env::var("GOWORK").ok().filter(|s| !s.is_empty()) {
        Some(value) if value == "off" => None,
        Some(value) => Some(PathBuf::from(value)),
        None => root.ancestors().map(|p| p.join("go.work")).find(|p| p.is_file()),
    };
    if let Some(workspace) = workspace {
        let text = std::fs::read_to_string(&workspace).map_err(|e| format!("读取 go.work 失败：{e}"))?;
        project.go_version = None;
        project.toolchain = None;
        parse_project(&text, &mut project);
        project.source = workspace.to_string_lossy().to_string();
    }
    if project.source.is_empty() { return Err("未找到 go.mod / go.work，请选择 Go 项目目录".into()); }
    project.required_version = project_requirement(project.go_version.as_deref(), project.toolchain.as_deref())?;
    Ok(project)
}

fn project_requirement(go: Option<&str>, toolchain: Option<&str>) -> Result<Option<String>, String> {
    let minimum = go.map(normalize_version).transpose()?;
    let suggested = toolchain.filter(|s| *s != "default").map(normalize_version).transpose()?;
    Ok(minimum.into_iter().chain(suggested).max_by_key(|v| version_key(v)))
}

fn desired_project_version(project: &Project, setting: &str) -> Result<(Option<String>, bool), String> {
    let minimum = project.go_version.as_deref().map(normalize_version).transpose()?;
    let (base, mode) = setting.split_once('+').unwrap_or((setting, ""));
    let pinned = base.starts_with("go").then(|| normalize_version(base)).transpose()?;
    if let Some(pin) = &pinned {
        if mode.is_empty() {
            if minimum.as_ref().map(|v| version_key(v) > version_key(pin)).unwrap_or(false) {
                return Err(format!("GOTOOLCHAIN={setting} 固定的版本低于项目要求，请先调整该配置"));
            }
            return Ok((pinned, true));
        }
    }
    let suggested = if setting == "local" || project.toolchain.as_deref() == Some("default") {
        minimum
    } else { project.required_version.clone() };
    Ok((suggested.into_iter().chain(pinned).max_by_key(|v| version_key(v)), false))
}

fn apply_project(path: &str, emit: &impl Fn(&str)) -> Result<String, String> {
    let project = inspect_project(path)?;
    let mut message = String::new();
    let config = environment().ok();
    let toolchain = std::env::var("GOTOOLCHAIN").ok().or_else(|| config.as_ref().and_then(|e| e["GOTOOLCHAIN"].as_str().map(str::to_string))).unwrap_or_else(|| "auto".into());
    let (required, exact) = desired_project_version(&project, &toolchain)?;
    if let Some(required) = required {
        let items = releases()?;
        // go 1.N 表示语言最低版本，选该系列最新补丁；精确补丁要求也不能降级。
        let key = version_key(&required);
        let (os, arch) = platform()?;
        let release = items.iter().find(|r| {
            let candidate = version_key(&r.version);
            (if exact { candidate == key } else { candidate >= key && candidate.0 == key.0 && candidate.1 == key.1 }) && asset(r, os, arch).is_some()
        }).ok_or("没有满足项目要求的平台版本")?;
        message.push_str(&install(Some(&release.version), emit)?);
        message.push('\n');
    }
    emit("dependencies");
    let output = go_output(&["mod", "download"], Some(Path::new(&project.path)), true)?;
    Ok(format!("{message}项目依赖已下载\n{output}"))
}

pub fn execute(request: Request, emit: impl Fn(&str)) -> Result<Value, String> {
    static MUTATION: Mutex<()> = Mutex::new(());
    let action = request.action.as_str();
    let _guard = if matches!(action, "install" | "switch" | "uninstall" | "fix-path" | "proxy-set" | "tool-install" | "project-apply") {
        Some(MUTATION.lock().map_err(|e| e.to_string())?)
    } else { None };
    let version = request.version.as_deref().unwrap_or("");
    let path = request.path.as_deref().unwrap_or("");
    match action {
        "status" => status(),
        "versions" => versions(),
        "install" => install(request.version.as_deref(), &emit).map(|s| json!({"message":s})),
        "switch" => switch_version(version).map(|s| json!({"message":s})),
        "uninstall" => uninstall(version).map(|s| json!({"message":s})),
        "fix-path" => {
            let version = selected_version().ok_or("没有已选中的托管 Go 版本，请先安装或切换版本")?;
            switch_version(&version).map(|s| json!({"message":s}))
        }
        "proxy" => environment(),
        "proxy-set" => proxy_settings(request.url.as_deref().unwrap_or(""), request.private.as_deref()),
        "test" => proxy_test(request.url.as_deref().unwrap_or("https://proxy.golang.org")),
        "tools" => tools(),
        "tool-install" => install_tool(request.name.as_deref().unwrap_or(""), request.version.as_deref()).map(|s| json!({"message":s})),
        "project" => serde_json::to_value(inspect_project(path)?).map_err(|e| e.to_string()),
        "project-apply" => apply_project(path, &emit).map(|s| json!({"message":s})),
        _ => Err(format!("未知 Go 操作：{action}")),
    }
}

#[cfg(feature = "gui")]
#[tauri::command]
pub async fn go_command(app: tauri::AppHandle, request: Request) -> Result<Value, String> {
    use tauri::Emitter;
    tauri::async_runtime::spawn_blocking(move || execute(request, |stage| {
        let _ = app.emit("go-progress", json!({"stage":stage}));
    })).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_paths_cannot_escape_install_root() {
        for value in ["../go1.2", "1.2/../other", "1.2; command", "1.2.3-beta"] { assert!(normalize_version(value).is_err()); }
        assert_eq!(normalize_version("1.26.1").unwrap(), "go1.26.1");
        assert!(version_key("go1.10.1") > version_key("go1.9.9"));
    }
    #[test]
    fn project_parses_minimum_and_suggested_toolchain() {
        let mut project = Project::default();
        parse_project("module example.com/app\ngo 1.23.0 // minimum\ntoolchain go1.24.2\n", &mut project);
        assert_eq!(project.module.as_deref(), Some("example.com/app"));
        assert_eq!(project_requirement(project.go_version.as_deref(), project.toolchain.as_deref()).unwrap().as_deref(), Some("go1.24.2"));
        assert_eq!(project_requirement(Some("1.25"), Some("default")).unwrap().as_deref(), Some("go1.25"));
        assert_eq!(project_requirement(Some("1.25"), Some("go1.24.1")).unwrap().as_deref(), Some("go1.25"));
        project.required_version = Some("go1.24.2".into());
        assert_eq!(desired_project_version(&project, "local").unwrap(), (Some("go1.23.0".into()), false));
        assert_eq!(desired_project_version(&project, "go1.24.1").unwrap(), (Some("go1.24.1".into()), true));
        assert!(desired_project_version(&project, "go1.22.1").is_err());
        assert_eq!(desired_project_version(&project, "go1.25.0+auto").unwrap(), (Some("go1.25.0".into()), false));
    }
    #[test]
    fn sha256_rejects_corrupted_download() {
        let path = std::env::temp_dir().join(format!("devenv-go-checksum-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        assert!(verify_sha256(&path, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").is_ok());
        assert!(verify_sha256(&path, "invalid").is_err());
        std::fs::remove_file(path).unwrap();
    }
}

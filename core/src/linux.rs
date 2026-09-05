//! Linux 后端：自管 tarball 安装 + `~/.local/bin` 软链切换 + shell rc 注入。
//!
//! 仅在 Linux 目标编译（WSL 等价，无特例）：
//! - 安装根：`$XDG_DATA_HOME/devenv`（回退 `~/.local/share/devenv`）
//! - 切换机制：逐二进制软链进 `~/.local/bin`（Q5）
//! - PATH 持久化：`~/.local/bin` 缺失时幂等注入 shell rc（Q6）
//! - 不索要 root、不碰系统包管理器（Git 缺失只给安装命令提示，Q8）

use crate::{
    ForeignPython, InstalledVersion, NetworkProbe, NodeEnvInfo, PkgManagerInfo, ProjectInfo,
    PythonEnvInfo, PythonProjectInfo, PythonVersionsInfo, VersionsInfo,
};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 路径（XDG + $HOME 回退，Q11）
// ---------------------------------------------------------------------------

pub fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

pub fn data_home() -> PathBuf {
    match std::env::var("XDG_DATA_HOME") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => home_dir().join(".local").join("share"),
    }
}

pub fn config_home() -> PathBuf {
    match std::env::var("XDG_CONFIG_HOME") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => home_dir().join(".config"),
    }
}

/// 自管安装根：`$XDG_DATA_HOME/devenv`。
pub fn install_root() -> PathBuf {
    data_home().join("devenv")
}

/// 软链目录 `~/.local/bin`。
pub fn local_bin() -> PathBuf {
    home_dir().join(".local").join("bin")
}

pub fn node_install_root() -> PathBuf {
    install_root().join("node")
}

pub fn python_install_root() -> PathBuf {
    install_root().join("python")
}

/// npmrc：`NPM_CONFIG_USERCONFIG` 优先，回退 `~/.npmrc`。
pub fn npmrc_path() -> PathBuf {
    if let Ok(p) = std::env::var("NPM_CONFIG_USERCONFIG") {
        return PathBuf::from(p);
    }
    home_dir().join(".npmrc")
}

/// pip 配置：`PIP_CONFIG_FILE` 优先，回退 `$XDG_CONFIG_HOME/pip/pip.conf`。
pub fn pip_config_path() -> PathBuf {
    if let Ok(p) = std::env::var("PIP_CONFIG_FILE") {
        return PathBuf::from(p);
    }
    config_home().join("pip").join("pip.conf")
}

// ---------------------------------------------------------------------------
// 架构 / libc 探测（Q7：x64/arm64 × gnu/musl）
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arch {
    Amd64,
    Arm64,
    X86,
    Arm,
    Unknown,
}

pub fn arch() -> Arch {
    match std::env::consts::ARCH {
        "x86_64" => Arch::Amd64,
        "aarch64" => Arch::Arm64,
        "x86" => Arch::X86,
        "arm" => Arch::Arm,
        _ => Arch::Unknown,
    }
}

impl Arch {
    /// Node 官方资产段（node-v22.14.0-linux-<x64|arm64|x86|armv7l>）。
    pub fn node_segment(&self) -> Option<&'static str> {
        match self {
            Arch::Amd64 => Some("x64"),
            Arch::Arm64 => Some("arm64"),
            Arch::X86 => Some("x86"),
            Arch::Arm => Some("armv7l"),
            Arch::Unknown => None,
        }
    }

    /// python-build-standalone 资产段（cpython-3.12.8+20250101-x86_64-unknown-linux-gnu）。
    pub fn pbs_segment(&self) -> Option<&'static str> {
        match self {
            Arch::Amd64 => Some("x86_64"),
            Arch::Arm64 => Some("aarch64"),
            Arch::X86 => Some("i686"),
            Arch::Arm => Some("armv7l"),
            Arch::Unknown => None,
        }
    }
}

/// `gnu` / `musl`（按 musl loader 文件是否存在判定，无需 `ldd`）。
pub fn libc_kind() -> &'static str {
    for loader in ["/lib/ld-musl-x86_64.so.1", "/lib/ld-musl-aarch64.so.1"] {
        if Path::new(loader).exists() {
            return "musl";
        }
    }
    "gnu"
}

/// 目标三元组后缀：`linux-x64` / `linux-arm64-musl` 等。
fn linux_platform_segment() -> Result<String, String> {
    let arch = arch().node_segment().ok_or("不支持的 CPU 架构（仅支持 x64 / arm64 / x86 / arm）")?;
    let libc = libc_kind();
    Ok(if libc == "musl" {
        format!("{}-{}", arch, libc)
    } else {
        arch.to_string()
    })
}

// ---------------------------------------------------------------------------
// 命令执行（复用 crate 的 new_cmd，Linux 用无后缀命令）
// ---------------------------------------------------------------------------

pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

pub fn which_first(names: &[&str]) -> Option<PathBuf> {
    names.iter().find_map(|n| find_in_path(n))
}

fn run_cmd_out(program: &str, args: &[&str]) -> (String, String) {
    match crate::new_cmd(program).args(args).output() {
        Ok(out) => (
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ),
        Err(e) => (String::new(), e.to_string()),
    }
}

fn run_cmd(program: &str, args: &[&str]) -> Option<String> {
    let (stdout, stderr) = run_cmd_out(program, args);
    if stdout.is_empty() {
        if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        }
    } else {
        Some(stdout)
    }
}

fn curl_download(url: &str, dest: &Path) -> bool {
    crate::new_cmd("curl")
        .args(["-f", "-L", "-s", "-o"])
        .arg(dest)
        .arg(url)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn probe_url(name: &str, url: &str) -> NetworkProbe {
    let mut p = NetworkProbe {
        name: name.to_string(),
        url: url.to_string(),
        ok: false,
        latency_ms: None,
        error: None,
    };
    if let Some(s) = run_cmd(
        "curl",
        &["-s", "-o", "/dev/null", "-w", "%{http_code} %{time_total}", "-m", "4", url],
    ) {
        let parts: Vec<&str> = s.trim().split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(code) = parts[0].parse::<u16>() {
                if code < 500 {
                    p.ok = true;
                    p.latency_ms = parts[1].parse::<f64>().ok().map(|t| (t * 1000.0) as u64);
                }
            }
        }
        if !p.ok {
            p.error = Some(s);
        }
    } else {
        p.error = Some("curl 执行失败".to_string());
    }
    p
}

// ---------------------------------------------------------------------------
// 软链切换（Q5）：扫描版本 bin/ 下全部可执行文件，链接/撤销到 ~/.local/bin
// ---------------------------------------------------------------------------

/// 可执行文件判断：unix 下 `st_mode & 0o111`。Linux 无 libc 依赖时用
/// `std::os::unix::fs::PermissionsExt` 读取 mode。
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// 把某版本 bin/ 下全部可执行文件软链到 `~/.local/bin`（同名非软链文件跳过，避免覆盖用户自己的程序）。
pub fn link_version_bins(install_dir: &Path) -> Result<(), String> {
    let bin_dir = install_dir.join("bin");
    let link_dir = local_bin();
    std::fs::create_dir_all(&link_dir).map_err(|e| format!("创建 {} 失败：{}", link_dir.display(), e))?;

    let entries = std::fs::read_dir(&bin_dir).map_err(|_| format!("版本目录 {} 缺少 bin/", install_dir.display()))?;
    for entry in entries.flatten() {
        let src = entry.path();
        if !is_executable(&src) {
            continue;
        }
        let name = entry.file_name();
        let dst = link_dir.join(&name);
        match std::fs::symlink_metadata(&dst) {
            Ok(meta) if meta.file_type().is_symlink() => {
                // 软链：删除后重指（可能是旧版本）
                let _ = std::fs::remove_file(&dst);
            }
            Ok(_) => continue, // 用户自己的真实文件，不覆盖
            Err(_) => {}
        }
        std::os::unix::fs::symlink(&src, &dst)
            .map_err(|e| format!("创建软链 {} -> {} 失败：{}", dst.display(), src.display(), e))?;
    }
    Ok(())
}

/// 撤销指向某版本（install_dir 的 bin/ 或子路径）的软链。仅删软链，不碰用户文件。
pub fn unlink_version_bins(install_dir: &Path) -> Result<(), String> {
    let bin_dir = install_dir.join("bin");
    let link_dir = local_bin();
    if let Ok(entries) = std::fs::read_dir(&link_dir) {
        for entry in entries.flatten() {
            let dst = entry.path();
            if let Ok(target) = std::fs::read_link(&dst) {
                if target.starts_with(&bin_dir) || target == bin_dir {
                    let _ = std::fs::remove_file(&dst);
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// PATH 持久化（Q6）：~/.local/bin 不在 PATH 时幂等注入 shell rc
// ---------------------------------------------------------------------------

fn path_has_local_bin() -> bool {
    let bin = local_bin().to_string_lossy().to_string();
    std::env::var("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.to_string_lossy() == bin))
        .unwrap_or(false)
}

const RC_MARKER: &str = "# >>> devenv >>>";
const RC_APPEND_BASH: &str = "export PATH=\"$HOME/.local/bin:$PATH\"";
const RC_APPEND_ZSH: &str = "export PATH=\"$HOME/.local/bin:$PATH\"";
const RC_FISH_FILE: &str = "set -gx PATH \"$HOME/.local/bin\" $PATH";

fn append_rc_block(path: &Path, content: &[&str]) -> Result<(), String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    if text.contains(RC_MARKER) {
        return Ok(()); // 幂等
    }
    let mut out = text.trim_end().to_string();
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(RC_MARKER);
    out.push('\n');
    for line in content {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("# <<< devenv <<<\n");
    std::fs::write(path, out).map_err(|e| format!("写入 {} 失败：{}", path.display(), e))
}

/// 若 `~/.local/bin` 不在 PATH，注入到已存在的 bash/zsh rc 或 fish conf.d（自动创建）。
/// 返回实际注入的 shell 名列表（空 = 无需注入）。
pub fn ensure_local_bin_in_path() -> Vec<String> {
    if path_has_local_bin() {
        return Vec::new();
    }
    let mut touched = Vec::new();
    let home = home_dir();

    for (rc_name, append) in [(".bashrc", RC_APPEND_BASH), (".zshrc", RC_APPEND_ZSH)] {
        let rc = home.join(rc_name);
        if rc.exists() && append_rc_block(&rc, &[append]).is_ok() {
            touched.push(rc_name.trim_start_matches('.').to_string());
        }
    }

    // fish：conf.d 目录按约定自动 source，不存在则创建整个目录
    if touched.is_empty() {
        let conf = config_home().join("fish").join("conf.d");
        if std::fs::create_dir_all(&conf).is_ok() {
            let f = conf.join("devenv.fish");
            if !f.exists()
                && std::fs::write(&f, format!("{RC_FISH_FILE}\n")).is_ok()
            {
                touched.push("fish".to_string());
            }
        }
    }
    touched
}

// ---------------------------------------------------------------------------
// Git 缺失提示（Q8）：识别常见包管理器，给精确命令，不替跑 sudo
// ---------------------------------------------------------------------------

pub fn git_install_hint() -> String {
    let ssh_pkg = if find_in_path("dnf").is_some() {
        "openssh-clients"
    } else if find_in_path("pacman").is_some() || find_in_path("zypper").is_some() {
        "openssh"
    } else {
        // apt-get / apk
        "openssh-client"
    };
    install_hint_for(&["git", ssh_pkg])
}

/// 识别常见包管理器，给出安装 `pkgs` 的精确命令（不替跑 sudo）。Q8。
pub fn install_hint_for(pkgs: &[&str]) -> String {
    let (pm, args) = if find_in_path("apt-get").is_some() {
        ("apt-get", "install -y")
    } else if find_in_path("dnf").is_some() {
        ("dnf", "install -y")
    } else if find_in_path("pacman").is_some() {
        ("pacman", "-S --noconfirm")
    } else if find_in_path("zypper").is_some() {
        ("zypper", "install -y")
    } else if find_in_path("apk").is_some() {
        ("apk", "add")
    } else {
        return "未检测到 apt/dnf/pacman/zypper/apk，请使用发行版包管理器安装".to_string();
    };
    // 发行版间 SSH 客户端包名不一（openssh-client / openssh-clients / openssh），
    // 由调用方传入正确组合；此处按调用方给定的包名原样拼接。
    format!("sudo {pm} {args} {}", pkgs.join(" "))
}

/// 已检测到的包管理器（命令名 + 安装参数），用于 `git versions` 列出可装包。
pub fn package_manager() -> Option<(&'static str, &'static str)> {
    [
        ("apt-get", "install -y"),
        ("dnf", "install -y"),
        ("pacman", "-S --noconfirm"),
        ("zypper", "install -y"),
        ("apk", "add"),
    ]
    .into_iter()
    .find(|(pm, _)| find_in_path(pm).is_some())
}

/// 检测 Git / OpenSSH：安装状态、版本、可执行路径。Git 由系统包管理器托管，
/// 本工具不代为下载安装（Linux 上 Git 强依赖系统库/Perl，且需 root），只给精确安装命令。
pub fn detect_git() -> crate::GitEnvInfo {
    use crate::gitcore::platform;
    let env: &dyn Fn(&str) -> Option<String> = &|k| std::env::var(k).ok();
    let mut info = crate::GitEnvInfo::default();
    info.ssh_installed = platform::which(env, "ssh").is_some();
    if let Some(exe) = find_in_path("git") {
        info.path = Some(exe.to_string_lossy().to_string());
        if let Some(out) = run_cmd("git", &["--version"]) {
            info.version = Some(out);
            info.installed = true;
        }
    }
    info
}

// ---------------------------------------------------------------------------
// Node：检测 / 安装 / 切换 / 卸载
// ---------------------------------------------------------------------------

fn node_bins_dir(install_root: &Path) -> PathBuf {
    install_root.join("bin")
}

fn node_current_from_link() -> Option<String> {
    // .../node/v22.14.0/bin/node -> 取版本段 v22.14.0
    let target = std::fs::read_link(local_bin().join("node")).ok()?;
    let ver = target.parent()?.parent()?.file_name()?.to_string_lossy().to_string();
    if ver.chars().next()? == 'v' {
        let rest = &ver[1..];
        if rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return Some(ver);
        }
    }
    None
}

/// 从 npmmirror node 索引解析每个大版本最新实现（复用 Windows 端的正则纯逻辑）。
fn fetch_available_node_versions() -> Vec<String> {
    let html = run_cmd(
        "curl",
        &["-s", "-m", "15", "https://registry.npmmirror.com/-/binary/node/"],
    )
    .unwrap_or_default();
    let re = regex::Regex::new(r"v(\d+)\.(\d+)\.(\d+)/").unwrap();
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

pub fn detect_node() -> NodeEnvInfo {
    let mut info = NodeEnvInfo::default();
    if let Some(exe) = find_in_path("node") {
        info.real_node_exe = Some(exe.to_string_lossy().to_string());
        info.in_path = true;
        info.node_dir_in_path = Some(exe.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default());
        if let Some(v) = run_cmd(&exe.to_string_lossy(), &["--version"]) {
            info.node_version = Some(v.trim_matches('v').to_string());
        }
        if let Some(npm) = find_in_path("npm") {
            if let Some(v) = run_cmd(&npm.to_string_lossy(), &["--version"]) {
                info.npm_version = Some(v.trim().to_string());
            }
        }
    }
    info.installed = info.real_node_exe.is_some();
    info.network = vec![
        probe_url("官方 npm", "https://registry.npmjs.org"),
        probe_url("淘宝 npmmirror", "https://registry.npmmirror.com"),
    ];
    info
}

pub fn list_versions() -> VersionsInfo {
    let install_dir = node_install_root();
    let current = node_current_from_link();
    let mut installed = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&install_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(ver) = name.strip_prefix('v') {
                if ver.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    let is_cur = current.as_ref() == Some(&name);
                    let size_mb = crate::dir_size(&entry.path()).map(|b| b / 1024 / 1024);
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
    VersionsInfo {
        nvm_detected: false,
        install_dir: install_dir.to_string_lossy().to_string(),
        installed,
        available: fetch_available_node_versions(),
        current,
    }
}

/// 组装 Node 资产名：`node-v{ver}-linux-{arch}{-musl}.tar.gz`（先试 .tar.xz，若 404）。
fn node_asset(ver: &str) -> Result<(String, String), String> {
    let platform = linux_platform_segment()?;
    let base = format!("https://registry.npmmirror.com/-/binary/node/v{ver}/");
    for ext in ["tar.gz", "tar.xz"] {
        let name = format!("node-v{ver}-linux-{platform}.{ext}");
        let url = format!("{base}{name}");
        if let Some(code) = run_cmd("curl", &["-s", "-o", "/dev/null", "-w", "%{http_code}", "-I", "-L", "-m", "6", url.as_str()]) {
            if code.trim() == "200" {
                return Ok((name, url));
            }
        }
    }
    Err(format!("v{ver} 在 npmmirror 未找到 linux-{platform} 资产"))
}

pub fn install_node(version: &str, emit: impl Fn(&str)) -> Result<String, String> {
    // 目录名统一 v{ver}（与 list_versions 扫描约定一致，对齐 Windows 版）
    let vnum = version.trim().trim_start_matches('v').to_string();
    let dir_name = format!("v{vnum}");
    let root = node_install_root();
    let target = root.join(&dir_name);
    if node_bins_dir(&target).join("node").exists() {
        return Ok(target.to_string_lossy().to_string());
    }
    std::fs::create_dir_all(&root).map_err(|e| format!("创建 {} 失败：{}", root.display(), e))?;

    let (name, url) = node_asset(&vnum)?;
    emit("downloading");
    let archive = root.join(&name);
    if !curl_download(&url, &archive) {
        return Err(format!("下载失败（网络或代理问题）：{url}"));
    }
    emit("extracting");
    std::fs::create_dir_all(&target).map_err(|e| format!("创建 {} 失败：{e:?}", target.display()))?;
    // --strip-components=1：去掉顶层 node-v{ver}-linux-{arch}/ 目录
    let archive_s = archive.to_string_lossy();
    let target_s = target.to_string_lossy();
    let ok = crate::new_cmd("tar")
        .args(["-xf", archive_s.as_ref(), "-C", target_s.as_ref(), "--strip-components=1"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let _ = std::fs::remove_file(&archive);
    if !ok {
        return Err("解压失败：tar 无法读取下载的压缩包".to_string());
    }
    if !node_bins_dir(&target).join("node").exists() {
        return Err("解压后未找到 bin/node".to_string());
    }
    link_version_bins(&target)?;
    ensure_local_bin_in_path();
    Ok(target.to_string_lossy().to_string())
}

pub fn switch_node(version: String) -> Result<String, String> {
    let vnum = version.trim().trim_start_matches('v').to_string();
    let target = node_install_root().join(format!("v{vnum}"));
    if !node_bins_dir(&target).join("node").exists() {
        return Err(format!("版本 {} 尚未安装", version));
    }
    link_version_bins(&target)?;
    let touched = ensure_local_bin_in_path();
    if !touched.is_empty() {
        eprintln!("已将 ~/.local/bin 写入 {} 的配置，请重开终端后生效", touched.join("/"));
    }
    Ok(target.to_string_lossy().to_string())
}

pub fn uninstall_node(version: String) -> Result<String, String> {
    let vnum = version.trim().trim_start_matches('v').to_string();
    let target = node_install_root().join(format!("v{vnum}"));
    if !target.exists() {
        return Err(format!("版本 {} 不存在", version));
    }
    unlink_version_bins(&target)?;
    std::fs::remove_dir_all(&target).map_err(|e| format!("删除 {} 失败：{e:?}", target.display()))?;
    Ok(target.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Python：检测 / 安装 / 切换 / 卸载
// ---------------------------------------------------------------------------

/// `pip 25.0 (python 3.x)` → `25.0`；非 pip 格式（报错文本）返回 None。
fn parse_pip_version(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.trim().split_whitespace().collect();
    let v = *parts.get(1)?;
    if !v.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    Some(v.to_string())
}

fn parse_python_ver_part(s: &str) -> Option<(i32, i32, i32)> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    let mut it = t.split('.');
    let major: i32 = it.next()?.parse().ok()?;
    let minor: i32 = it.next()?.parse().ok()?;
    let patch: i32 = it.next()?.parse().ok()?;
    Some((major, minor, patch))
}

pub fn detect_python() -> PythonEnvInfo {
    let mut info = PythonEnvInfo::default();
    let mut foreign: Vec<ForeignPython> = Vec::new();

    if let Some(exe) = find_in_path("python3") {
        info.real_python_exe = Some(exe.to_string_lossy().to_string());
        info.in_path = true;
        info.python_dir_in_path = Some(exe.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default());
        if let Some(v) = run_cmd(&exe.to_string_lossy(), &["-V"]) {
            if let Some(p) = v.trim().strip_prefix("Python ") {
                info.python_version = Some(p.to_string());
            }
        }
        // pip 优先取同目录 pip3，再回退 python3 -m pip；输出须形如 "pip 25.0 ..."，否则视为无 pip
        let pip = exe.parent().map(|d| d.join("pip3")).filter(|p| p.is_file());
        if let Some(pip) = pip {
            if let Some(v) = parse_pip_version(&run_cmd(&pip.to_string_lossy(), &["--version"]).unwrap_or_default()) {
                info.pip_version = Some(v);
            }
        } else if let Some(v) = parse_pip_version(&run_cmd(&exe.to_string_lossy(), &["-m", "pip", "--version"]).unwrap_or_default()) {
            info.pip_version = Some(v);
        }
    }

    // 系统自带 python 列作外来安装；自管目录下其他版本也列出
    if info.real_python_exe.is_none() {
        if let Some(system) = find_in_path("python3") {
            let v = run_cmd(&system.to_string_lossy(), &["-V"])
                .and_then(|s| s.trim().strip_prefix("Python ").map(String::from));
            foreign.push(ForeignPython {
                kind: "system".to_string(),
                path: system.to_string_lossy().to_string(),
                version: v,
            });
        }
    }
    info.foreign = foreign;
    info.installed = info.real_python_exe.is_some();
    info.network = vec![
        probe_url("官方 PyPI", "https://pypi.org/simple/"),
        probe_url("清华 PyPI 镜像", "https://pypi.tuna.tsinghua.edu.cn/simple/"),
    ];
    info
}

/// 从 npmmirror python 索引解析可用版本（3.8+，每小版本最新）。
fn fetch_available_python_versions() -> Vec<String> {
    let json = run_cmd("curl", &["-s", "-m", "15", "https://registry.npmmirror.com/-/binary/python/"]).unwrap_or_default();
    let re = regex::Regex::new(r#""name":"(\d+)\.(\d+)\.(\d+)/""#).unwrap();
    let mut best: std::collections::HashMap<(i32, i32), i32> = std::collections::HashMap::new();
    for cap in re.captures_iter(&json) {
        let major: i32 = cap[1].parse().unwrap_or(0);
        if major != 3 {
            continue;
        }
        let minor: i32 = cap[2].parse().unwrap_or(0);
        if minor < 8 {
            continue;
        }
        let patch: i32 = cap[3].parse().unwrap_or(0);
        let e = best.entry((major, minor)).or_insert(0);
        if patch > *e {
            *e = patch;
        }
    }
    let mut keys: Vec<(i32, i32)> = best.keys().cloned().collect();
    keys.sort_unstable();
    keys.reverse();
    keys.iter()
        .map(|(maj, min)| format!("{}.{}.{}", maj, min, best[&(*maj, *min)]))
        .collect()
}

/// python-build-standalone：`cpython-{ver}+{date}-{arch}-unknown-linux-{libc}-install_only.tar.gz`。
fn find_python_build_standalone(version: &str) -> Option<String> {
    let base = "https://registry.npmmirror.com/-/binary/python-build-standalone";
    let index = run_cmd("curl", &["-s", "-m", "15", &format!("{base}/")]).unwrap_or_default();
    let re = regex::Regex::new(r#""name":"(\d{8})/""#).unwrap();
    let mut dates: Vec<String> = re.captures_iter(&index).map(|c| c[1].to_string()).collect();
    dates.sort();
    dates.reverse();
    let arch = arch().pbs_segment()?;
    let libc = libc_kind();
    for date in dates {
        for variant in ["install_only", "install_only_stripped"] {
            let name = format!("cpython-{version}+{date}-{arch}-unknown-linux-{libc}-{variant}.tar.gz");
            let url = format!("{base}/{date}/{name}");
            if let Some(code) = run_cmd("curl", &["-s", "-o", "/dev/null", "-w", "%{http_code}", "-I", "-L", "-m", "6", url.as_str()]) {
                if code.trim() == "200" {
                    return Some(url);
                }
            }
        }
    }
    None
}

pub fn install_python(version: &str, emit: impl Fn(&str)) -> Result<String, String> {
    let ver = version.trim().trim_start_matches('v').to_string();
    let root = python_install_root();
    let target = root.join(&ver);
    if target.join("bin").join("python3").exists() {
        return Ok(target.to_string_lossy().to_string());
    }
    std::fs::create_dir_all(&root).map_err(|e| format!("创建 {} 失败：{}", root.display(), e))?;

    emit("connecting");
    let url = find_python_build_standalone(&ver).ok_or(format!("版本 {ver} 未找到 linux 资产（python-build-standalone），请确认版本存在"))?;
    emit("downloading");
    let archive = root.join(format!("python-{ver}.tar.gz"));
    if !curl_download(&url, &archive) {
        return Err(format!("下载失败（网络或代理问题）：{url}"));
    }
    emit("extracting");
    let _ = std::fs::remove_dir_all(&target);
    std::fs::create_dir_all(&target).map_err(|e| format!("创建 {} 失败：{e:?}", target.display()))?;
    // --strip-components=1：去掉顶层 python/ 目录
    let archive_s = archive.to_string_lossy();
    let target_s = target.to_string_lossy();
    let ok = crate::new_cmd("tar")
        .args(["-xf", archive_s.as_ref(), "-C", target_s.as_ref(), "--strip-components=1"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let _ = std::fs::remove_file(&archive);
    if !ok {
        return Err("解压失败：tar 无法读取下载的压缩包".to_string());
    }
    // install_only 顶层是 python/，再摊一层（--strip 后应为 python3 在 bin/，此处兜底）
    let nested = target.join("python");
    if nested.exists() && !target.join("bin").join("python3").exists() {
        if let Ok(entries) = std::fs::read_dir(&nested) {
            for e in entries.flatten() {
                let _ = std::fs::rename(e.path(), target.join(e.file_name()));
            }
        }
        let _ = std::fs::remove_dir_all(&nested);
    }
    if !target.join("bin").join("python3").exists() {
        return Err("解压后未找到 bin/python3".to_string());
    }
    link_version_bins(&target)?;
    ensure_local_bin_in_path();
    Ok(target.to_string_lossy().to_string())
}

/// 当前自管 python 版本（读 ~/.local/bin/python3 软链目标）。
fn python_current_from_link() -> Option<String> {
    let link = local_bin().join("python3");
    let target = std::fs::read_link(&link).ok()?;
    let ver = target.parent()?.parent()?.file_name()?.to_string_lossy().to_string();
    parse_python_ver_part(&ver).map(|_| ver)
}

pub fn list_python_versions() -> PythonVersionsInfo {
    let install_dir = python_install_root();
    let current = python_current_from_link();
    let mut installed = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&install_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if parse_python_ver_part(&name).is_some() {
                let is_cur = current.as_deref() == Some(name.as_str());
                let size_mb = crate::dir_size(&entry.path()).map(|b| b / 1024 / 1024);
                installed.push(InstalledVersion {
                    version: name.clone(),
                    current: is_cur,
                    size_mb,
                });
            }
        }
    }
    installed.sort_by(|a, b| parse_python_ver_part(&b.version).unwrap_or((0, 0, 0)).cmp(&parse_python_ver_part(&a.version).unwrap_or((0, 0, 0))));
    PythonVersionsInfo {
        py_launcher_detected: false,
        install_dir: install_dir.to_string_lossy().to_string(),
        installed,
        available: fetch_available_python_versions(),
        current,
    }
}

pub fn switch_python(version: String) -> Result<String, String> {
    let ver = version.trim().trim_start_matches('v').to_string();
    let target = python_install_root().join(&ver);
    if !target.join("bin").join("python3").exists() {
        return Err(format!("版本 {} 尚未安装", version));
    }
    link_version_bins(&target)?;
    let touched = ensure_local_bin_in_path();
    if !touched.is_empty() {
        eprintln!("已将 ~/.local/bin 写入 {} 的配置，请重开终端后生效", touched.join("/"));
    }
    Ok(target.to_string_lossy().to_string())
}

pub fn uninstall_python(version: String) -> Result<String, String> {
    let ver = version.trim().trim_start_matches('v').to_string();
    let target = python_install_root().join(&ver);
    if !target.exists() {
        return Err(format!("版本 {} 不存在", version));
    }
    unlink_version_bins(&target)?;
    std::fs::remove_dir_all(&target).map_err(|e| format!("删除 {} 失败：{e:?}", target.display()))?;
    Ok(target.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// doctor 数据（Q12）：CLI 展示用
// ---------------------------------------------------------------------------

/// `~/.local/bin` 是否已在 PATH。
pub fn local_bin_in_path() -> bool {
    path_has_local_bin()
}

/// 是否存在 devenv 注入的 rc 标记（bash/zsh）或 fish conf.d 文件。
pub fn rc_marker_exists() -> bool {
    let home = home_dir();
    [home.join(".bashrc"), home.join(".zshrc")]
        .iter()
        .any(|p| std::fs::read_to_string(p).map(|t| t.contains(RC_MARKER)).unwrap_or(false))
        || config_home().join("fish").join("conf.d").join("devenv.fish").exists()
}

/// 悬空软链检测：~/.local/bin 下指向不存在的目标的软链列表。
pub fn dangling_symlinks() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(local_bin()) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Ok(target) = std::fs::read_link(&p) {
                if !target.exists() {
                    out.push(p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// bun：独立单二进制运行时（npmmirror zip，不依赖 node）
// ---------------------------------------------------------------------------

/// bun 托管安装根：`$XDG_DATA_HOME/devenv/bun/<version>/bun`。
fn bun_install_root() -> PathBuf {
    install_root().join("bun")
}

/// `~/.bun`（或 `$BUN_INSTALL`）—— bun 自身的全局/缓存数据目录。
fn bun_data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("BUN_INSTALL") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    home_dir().join(".bun")
}

/// bun 资产 arch 段：gnu → `bun-linux-x64.zip`；musl → `bun-linux-x64-musl.zip`。
fn bun_asset_name() -> Result<String, String> {
    let seg = match arch() {
        Arch::Amd64 => "x64",
        Arch::Arm64 => "aarch64",
        _ => return Err("bun 仅提供 x64 / arm64 Linux 资产".to_string()),
    };
    Ok(if libc_kind() == "musl" {
        format!("bun-linux-{seg}-musl.zip")
    } else {
        format!("bun-linux-{seg}.zip")
    })
}

/// 从 npmmirror bun 索引解析最高 `bun-v<x.y.z>` 版本（返回纯版本号 x.y.z）。
fn fetch_latest_bun_version() -> Option<String> {
    let html = run_cmd(
        "curl",
        &["-s", "-m", "15", "https://registry.npmmirror.com/-/binary/bun/"],
    )?;
    let re = regex::Regex::new(r"bun-v(\d+)\.(\d+)\.(\d+)/").ok()?;
    let mut best: Option<(i32, i32, i32)> = None;
    for cap in re.captures_iter(&html) {
        let v = (
            cap[1].parse().unwrap_or(0),
            cap[2].parse().unwrap_or(0),
            cap[3].parse().unwrap_or(0),
        );
        if best.map(|b| v > b).unwrap_or(true) {
            best = Some(v);
        }
    }
    best.map(|(a, b, c)| format!("{a}.{b}.{c}"))
}

/// 组装 bun 资产 URL：`…/-/binary/bun/bun-v<ver>/<asset>`，HEAD 探 200。
fn bun_asset_url(version: &str) -> Result<String, String> {
    let name = bun_asset_name()?;
    let url = format!(
        "https://registry.npmmirror.com/-/binary/bun/bun-v{version}/{name}"
    );
    let code = run_cmd(
        "curl",
        &["-s", "-o", "/dev/null", "-w", "%{http_code}", "-I", "-L", "-m", "6", &url],
    )
    .ok_or_else(|| format!("探测 bun 资产失败：{url}"))?;
    if code.trim() == "200" {
        Ok(url)
    } else {
        Err(format!("bun v{version} 未找到 linux 资产（HTTP {code}）"))
    }
}

pub fn bun_info() -> PkgManagerInfo {
    let bin = find_in_path("bun");
    let bin_path = bin.as_ref().map(|p| p.to_string_lossy().to_string());
    let mut info = PkgManagerInfo {
        name: "bun".to_string(),
        installed: bin.is_some(),
        version: None,
        bin_path: bin_path.clone(),
        data_dir: Some(bun_data_dir().to_string_lossy().to_string()),
    };
    if let Some(b) = &bin_path {
        if let Some(v) = run_cmd(b, &["--version"]) {
            info.version = Some(v.trim().to_string());
        }
    }
    info
}

/// 安装/升级 bun。`version` 为 None 时装最新版。zip 扁平单文件 `bun`，
/// 直接软链 `~/.local/bin/bun → <版本目录>/bun`（不走 link_version_bins，它硬编码 bin/）。
/// 在版本目录里定位 bun 二进制：zip 可能扁平（`bun`）或带顶层目录（`bun-linux-x64/bun`）。
fn find_bun_bin(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("bun");
    if direct.is_file() {
        return Some(direct);
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let cand = e.path().join("bun");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

pub fn install_bun(version: Option<&str>, emit: impl Fn(&str)) -> Result<String, String> {
    let ver = match version {
        Some(v) => v.trim().trim_start_matches('v').to_string(),
        None => fetch_latest_bun_version().ok_or("无法解析 bun 最新版本（网络问题）")?,
    };
    let root = bun_install_root();
    let target = root.join(&ver);

    if let Some(existing) = find_bun_bin(&target) {
        link_bun(&existing)?;
        return Ok(format!("bun {ver} 已安装，已切换软链"));
    }

    // unzip 是解压 zip 的前提（GNU tar 读不了 zip）
    if find_in_path("unzip").is_none() {
        return Err(format!(
            "解压 bun 需要 unzip，请先安装：{}",
            install_hint_for(&["unzip"])
        ));
    }

    let url = bun_asset_url(&ver)?;
    std::fs::create_dir_all(&root).map_err(|e| format!("创建 {} 失败：{e}", root.display()))?;
    emit("downloading");
    let archive = root.join(format!("bun-{ver}.zip"));
    if !curl_download(&url, &archive) {
        return Err(format!("下载失败（网络或代理问题）：{url}"));
    }
    // zip 魔数 PK\x03\x04 校验
    let head = std::fs::read(&archive).map(|b| b.get(..4).map(|s| s.to_vec()).unwrap_or_default()).unwrap_or_default();
    if head != b"PK\x03\x04" {
        let _ = std::fs::remove_file(&archive);
        return Err("下载内容不是合法 zip（可能是错误页）".to_string());
    }
    emit("extracting");
    let _ = std::fs::remove_dir_all(&target);
    std::fs::create_dir_all(&target).map_err(|e| format!("创建 {} 失败：{e}", target.display()))?;
    let archive_s = archive.to_string_lossy();
    let target_s = target.to_string_lossy();
    let ok = crate::new_cmd("unzip")
        .args(["-o", archive_s.as_ref(), "-d", target_s.as_ref()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let _ = std::fs::remove_file(&archive);
    if !ok {
        return Err("解压失败：unzip 无法读取下载压缩包".to_string());
    }
    let bun_bin = find_bun_bin(&target).ok_or("解压后未找到 bun 可执行文件")?;
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&bun_bin, std::fs::Permissions::from_mode(0o755));
    link_bun(&bun_bin)?;
    Ok(format!("bun {ver} 安装完成"))
}

/// 软链 `~/.local/bin/bun` → 托管 bun 二进制（覆盖旧软链，不碰用户真实文件）。
fn link_bun(bun_bin: &Path) -> Result<(), String> {
    let link_dir = local_bin();
    std::fs::create_dir_all(&link_dir).map_err(|e| format!("创建 {} 失败：{e}", link_dir.display()))?;
    let dst = link_dir.join("bun");
    match std::fs::symlink_metadata(&dst) {
        Ok(meta) if meta.file_type().is_symlink() => {
            let _ = std::fs::remove_file(&dst);
        }
        Ok(_) => return Err("~/.local/bin/bun 已存在且不是软链，拒绝覆盖".to_string()),
        Err(_) => {}
    }
    std::os::unix::fs::symlink(bun_bin, &dst)
        .map_err(|e| format!("创建软链 {} -> {} 失败：{e}", dst.display(), bun_bin.display()))?;
    ensure_local_bin_in_path();
    Ok(())
}

pub fn upgrade_bun() -> Result<String, String> {
    // 优先用 bun 自带升级；不可用（未装/旧版）则重下最新版覆盖。
    if find_in_path("bun").is_some() {
        if let Some(out) = run_cmd("bun", &["upgrade"]) {
            return Ok(format!("bun 自带升级已执行：{out}"));
        }
    }
    install_bun(None, |_| {})
}

pub fn uninstall_bun() -> Result<String, String> {
    let link = local_bin().join("bun");
    if let Ok(meta) = std::fs::symlink_metadata(&link) {
        if meta.file_type().is_symlink() {
            let _ = std::fs::remove_file(&link);
        }
    }
    let root = bun_install_root();
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|e| format!("删除 {} 失败：{e}", root.display()))?;
    }
    Ok("已卸载托管 bun（软链与版本目录已清除；~/.bun 数据目录保留，如需可手动删除）".to_string())
}

// ---------------------------------------------------------------------------
// Python 包管理器：状态探测 + pip --user 安装（落到共享 ~/.local/bin，跨 python 版本不丢）
// ---------------------------------------------------------------------------

fn python_pkg_data_dir(name: &str) -> PathBuf {
    match name {
        "pip" => home_dir().join(".cache").join("pip"),
        "uv" => data_home().join("uv"),
        "poetry" => config_home().join("pypoetry"),
        "pipenv" => data_home().join("virtualenvs"),
        "conda" => home_dir().join("miniconda3"),
        _ => data_home().join(name),
    }
}

pub fn list_python_pkg_managers() -> Vec<PkgManagerInfo> {
    let tools: &[(&str, &[&str])] = &[
        ("pip", &["pip", "pip3"]),
        ("uv", &["uv"]),
        ("poetry", &["poetry"]),
        ("pipenv", &["pipenv"]),
        ("conda", &["conda"]),
    ];
    let mut out = Vec::new();
    for &(name, candidates) in tools {
        let bin = which_first(candidates);
        let bin_path = bin.as_ref().map(|p| p.to_string_lossy().to_string());
        let mut info = PkgManagerInfo {
            name: name.to_string(),
            installed: bin.is_some(),
            version: None,
            bin_path: bin_path.clone(),
            data_dir: Some(python_pkg_data_dir(name).to_string_lossy().to_string()),
        };
        if let Some(b) = &bin_path {
            if let Some(v) = run_cmd(b, &["--version"]) {
                info.version = Some(v.trim().to_string());
            }
        } else if name == "pip" {
            // pip 常以模块形式存在：python3 -m pip --version
            if let Some(py) = which_first(&["python3", "python"]) {
                if let Some(v) = parse_pip_version(
                    &run_cmd(&py.to_string_lossy(), &["-m", "pip", "--version"]).unwrap_or_default(),
                ) {
                    info.installed = true;
                    info.version = Some(v);
                    info.bin_path = Some("python3 -m pip".to_string());
                }
            }
        }
        out.push(info);
    }
    out
}

pub fn install_python_pkg_manager(name: String, version: Option<String>) -> Result<String, String> {
    let ver = version.unwrap_or_else(|| "latest".to_string());
    match name.as_str() {
        "pip" => return Err("pip 随 Python 内置，无需单独安装".to_string()),
        "conda" => return Err("conda 需安装 Miniconda，暂不支持自动安装".to_string()),
        _ => {}
    }
    let py = which_first(&["python3", "python"])
        .ok_or_else(|| "未找到 python3/python，无法用 pip 安装".to_string())?;
    let target = if ver == "latest" {
        name.clone()
    } else {
        format!("{}=={}", name, ver.trim_start_matches('v'))
    };
    // --user：装到共享 ~/.local/bin（真实文件），切换/卸载 python 版本后仍在
    let py_s = py.to_string_lossy().to_string();
    let out = crate::new_cmd(&py_s)
        .args(["-m", "pip", "install", "--user", &target])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        ensure_local_bin_in_path();
        Ok(format!("已安装 {target}（--user → ~/.local/bin）"))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// ---------------------------------------------------------------------------
// Node 项目：.nvmrc / package.json 解析 → 一键装切
// ---------------------------------------------------------------------------

/// `.nvmrc` / engines.node 解析为可安装的具体版本（v 前缀）。裸主版本/lts 取可用列表里的最佳匹配。
fn resolve_node_req(raw: &str, available: &[String]) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(c) = regex::Regex::new(r"^v?(\d+\.\d+\.\d+)$").ok().and_then(|re| re.captures(t)) {
        return Some(format!("v{}", &c[1]));
    }
    if let Some(c) = regex::Regex::new(r"^v?(\d+)$").ok().and_then(|re| re.captures(t)) {
        let major: i32 = c[1].parse().unwrap_or(0);
        return available.iter().find(|v| v.starts_with(&format!("v{major}."))).cloned();
    }
    if t == "lts" || t.starts_with("lts/") || t == "node" {
        return available
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
    // engines 形如 ">=18" / "18.x" / "^20.1.0"：抓第一个数字段
    if let Some(c) = regex::Regex::new(r"(\d+)(?:\.(\d+))?(?:\.(\d+))?").ok().and_then(|re| re.captures(t)) {
        if let (Some(maj), Some(min), Some(pat)) = (c.get(1), c.get(2), c.get(3)) {
            return Some(format!("v{}.{}.{}", maj.as_str(), min.as_str(), pat.as_str()));
        }
        if let Some(maj) = c.get(1) {
            let major: i32 = maj.as_str().parse().unwrap_or(0);
            return available.iter().find(|v| v.starts_with(&format!("v{major}."))).cloned();
        }
    }
    None
}

pub fn inspect_node_project(path: String) -> Result<ProjectInfo, String> {
    let p = Path::new(&path);
    if !p.is_dir() {
        return Err("路径不是有效目录".to_string());
    }
    let versions = list_versions();
    let mut info = ProjectInfo {
        path: path.clone(),
        node_requirement: None,
        requirement_source: None,
        package_manager: None,
        registry: None,
        has_nvmrc: false,
        has_package_json: false,
        has_npmrc: false,
        current_node: versions.current.clone(),
        node_installed: false,
    };

    let nvmrc = p.join(".nvmrc");
    if nvmrc.exists() {
        info.has_nvmrc = true;
        if let Ok(c) = std::fs::read_to_string(&nvmrc) {
            info.node_requirement = resolve_node_req(&c, &versions.available);
            info.requirement_source = Some(".nvmrc".to_string());
        }
    }

    let pkg = p.join("package.json");
    if pkg.exists() {
        info.has_package_json = true;
        if let Ok(c) = std::fs::read_to_string(&pkg) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&c) {
                if info.node_requirement.is_none() {
                    if let Some(eng) = json.get("engines").and_then(|e| e.get("node")).and_then(|n| n.as_str()) {
                        info.node_requirement = resolve_node_req(eng, &versions.available);
                        if info.node_requirement.is_some() {
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

    let npmrc = p.join(".npmrc");
    if npmrc.exists() {
        info.has_npmrc = true;
        if let Ok(c) = std::fs::read_to_string(&npmrc) {
            for line in c.lines() {
                if let Some(v) = line.trim().strip_prefix("registry=") {
                    info.registry = Some(v.trim().to_string());
                    break;
                }
            }
        }
    }

    if let Some(req) = &info.node_requirement {
        let vnum = req.trim_start_matches('v');
        info.node_installed = node_install_root().join(format!("v{vnum}")).join("bin").join("node").exists();
    }
    Ok(info)
}

pub fn apply_node_project(path: String) -> Result<String, String> {
    let info = inspect_node_project(path)?;
    let mut msgs = Vec::new();
    if let Some(req) = &info.node_requirement {
        if !info.node_installed {
            install_node(req, |_| {})?;
            msgs.push(format!("已安装 Node {req}"));
        }
        switch_node(req.clone())?;
        msgs.push(format!("Node 已切换到 {req}"));
    } else {
        msgs.push("未检测到项目 Node 版本要求（.nvmrc / engines）".to_string());
    }
    // corepack 仅在存在时尝试（缺失则跳过，不强制安装）
    if let Some(pm) = &info.package_manager {
        if find_in_path("corepack").is_some() {
            let _ = run_cmd("corepack", &["use", pm]);
            msgs.push(format!("corepack 已激活 {pm}"));
        } else {
            msgs.push(format!("检测到 packageManager={pm}，但未安装 corepack，已跳过"));
        }
    }
    Ok(msgs.join("\n"))
}

// ---------------------------------------------------------------------------
// Python 项目：.python-version / pyproject.toml / runtime.txt / Pipfile → 一键装切
// ---------------------------------------------------------------------------

fn parse_py_ver_tuple(s: &str) -> Option<(i32, i32, i32)> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    let mut it = t.split('.');
    Some((
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next().and_then(|p| p.parse().ok()).unwrap_or(0),
    ))
}

/// 把要求解析为具体 3 段版本。裸 minor（3.11）/requires-python 取已安装可用列表里该 minor 最新补丁。
fn resolve_python_req(raw: &str, available: &[String]) -> Option<String> {
    let t = raw.trim();
    if let Some(v) = parse_py_ver_tuple(t) {
        if v.2 != 0 {
            return Some(format!("{}.{}.{}", v.0, v.1, v.2));
        }
        // x.y.0 / x.y → 取该 minor 最新补丁
        return available
            .iter()
            .find(|s| s.starts_with(&format!("{}.{}.", v.0, v.1)))
            .cloned()
            .or_else(|| Some(format!("{}.{}", v.0, v.1)));
    }
    if t == "3" || t == "3.x" {
        return available.first().cloned();
    }
    // requires-python：">=3.11" / "^3.11" / ">=3.11,<4"
    if let Some(c) = regex::Regex::new(r"(\d+)\.(\d+)(?:\.(\d+))?").ok().and_then(|re| re.captures(t)) {
        let maj: i32 = c[1].parse().unwrap_or(3);
        let min: i32 = c[2].parse().unwrap_or(0);
        if let Some(pat) = c.get(3) {
            return Some(format!("{maj}.{min}.{}", pat.as_str()));
        }
        return available
            .iter()
            .find(|s| s.starts_with(&format!("{maj}.{min}.")))
            .cloned();
    }
    None
}

pub fn inspect_python_project(path: String) -> Result<PythonProjectInfo, String> {
    let p = Path::new(&path);
    if !p.is_dir() {
        return Err("路径不是有效目录".to_string());
    }
    let versions = list_python_versions();
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
        current_python: versions.current.clone(),
        python_installed: false,
    };

    let pv = p.join(".python-version");
    if pv.exists() {
        info.has_python_version_file = true;
        if let Ok(c) = std::fs::read_to_string(&pv) {
            info.python_requirement = resolve_python_req(&c, &versions.available);
            if info.python_requirement.is_some() {
                info.requirement_source = Some(".python-version".to_string());
            }
        }
    }

    let pyproj = p.join("pyproject.toml");
    if pyproj.exists() {
        info.has_pyproject_toml = true;
        if let Ok(c) = std::fs::read_to_string(&pyproj) {
            if info.python_requirement.is_none() {
                if let Some(cap) = regex::Regex::new(r#"(?m)^requires-python\s*=\s*"([^"]+)""#)
                    .ok()
                    .and_then(|re| re.captures(&c))
                {
                    info.python_requirement = resolve_python_req(&cap[1], &versions.available);
                    if info.python_requirement.is_some() {
                        info.requirement_source = Some("pyproject.toml requires-python".to_string());
                    }
                }
            }
            for tool in ["uv", "poetry", "pipenv", "hatchling", "setuptools"] {
                if c.contains(&format!("[tool.{tool}]")) {
                    info.pkg_tool = Some(tool.to_string());
                    break;
                }
            }
            if let Some(cap) = regex::Regex::new(r#"(?m)^index\s*=\s*"([^"]+)""#)
                .ok()
                .and_then(|re| re.captures(&c))
            {
                info.index = Some(cap[1].to_string());
            }
        }
    }

    let rt = p.join("runtime.txt");
    if rt.exists() {
        info.has_runtime_txt = true;
        if info.python_requirement.is_none() {
            if let Ok(c) = std::fs::read_to_string(&rt) {
                let t = c.trim().strip_prefix("python-").unwrap_or(c.trim());
                info.python_requirement = resolve_python_req(t, &versions.available);
                if info.python_requirement.is_some() {
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
                if let Some(cap) = regex::Regex::new(r#"(?m)^python_version\s*=\s*"([^"]+)""#)
                    .ok()
                    .and_then(|re| re.captures(&c))
                {
                    info.python_requirement = resolve_python_req(&cap[1], &versions.available);
                    if info.python_requirement.is_some() {
                        info.requirement_source = Some("Pipfile".to_string());
                    }
                }
            }
        }
    }

    if let Some(req) = &info.python_requirement {
        info.python_installed = python_install_root().join(req).join("bin").join("python3").exists();
    }
    Ok(info)
}

pub fn apply_python_project(path: String) -> Result<String, String> {
    let info = inspect_python_project(path)?;
    let mut msgs = Vec::new();
    if let Some(req) = &info.python_requirement {
        if !info.python_installed {
            install_python(req, |_| {})?;
            msgs.push(format!("已安装 Python {req}"));
        }
        switch_python(req.clone())?;
        msgs.push(format!("Python 已切换到 {req}"));
    } else {
        msgs.push("未检测到项目 Python 版本要求（.python-version / pyproject.toml / runtime.txt / Pipfile）".to_string());
    }
    Ok(msgs.join("\n"))
}

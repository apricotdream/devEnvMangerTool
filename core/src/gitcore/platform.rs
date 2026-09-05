//! 平台适配（对应 Python `platform_utils.py`）。
//!
//! - [`command_candidates`]：Windows / Scoop / Git-for-Windows 路径发现
//! - [`fallback_ssh_dir`]：各平台 SSH 目录回退规则
//! - [`open_in_file_manager`]：在文件管理器中打开路径
//! - [`which`] / [`which_first`]：PATH 查找可执行文件
//! - [`preferred_font_families`]：各平台中文字体栈

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::gitcore::error::Error;

/// 环境变量读取器。生产环境传 `|k| std::env::var(k).ok()`，测试可注入固定 map。
pub type Env = dyn Fn(&str) -> Option<String>;

/// 应用名（对应 Python `APP_NAME`）。
pub const APP_NAME: &str = "git-env-gui";

/// 当前平台名，返回 Python `sys.platform` 风格的值：`win32` / `darwin` / `linux` / `unknown`。
pub fn current_platform_name() -> &'static str {
    if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}

/// SSH 目录回退规则（对应 Python `fallback_ssh_dir`）。
pub fn fallback_ssh_dir(
    platform_name: &str,
    env: &Env,
    home: &Path,
    temp_dir: &Path,
) -> PathBuf {
    if platform_name == "win32" {
        if let Some(local_app_data) = env("LOCALAPPDATA") {
            return PathBuf::from(local_app_data).join(APP_NAME).join("ssh");
        }
        return temp_dir.join(APP_NAME).join("ssh");
    }

    if platform_name == "darwin" {
        return home
            .join("Library")
            .join("Application Support")
            .join(APP_NAME)
            .join("ssh");
    }

    if platform_name.starts_with("linux") {
        if let Some(xdg_data_home) = env("XDG_DATA_HOME") {
            return PathBuf::from(xdg_data_home).join(APP_NAME).join("ssh");
        }
        return home.join(".local").join("share").join(APP_NAME).join("ssh");
    }

    temp_dir.join(APP_NAME).join("ssh")
}

/// 命令候选路径（对应 Python `command_candidates`）。
pub fn command_candidates(name: &str, platform_name: &str, env: &Env, home: &Path) -> Vec<PathBuf> {
    if platform_name == "win32" {
        windows_command_candidates(name, env, home)
    } else {
        unix_command_candidates(name, platform_name, env, home)
    }
}

fn windows_command_candidates(name: &str, env: &Env, home: &Path) -> Vec<PathBuf> {
    let executable = format!("{name}.exe");
    let local_app_data = env("LOCALAPPDATA").map(PathBuf::from);
    let program_files = env("ProgramFiles").map(PathBuf::from);
    let program_files_x86 = env("ProgramFiles(x86)").map(PathBuf::from);
    let windows_dir = env("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));

    let mut candidates = vec![home.join("scoop").join("shims").join(&executable)];

    let git_roots = [
        local_app_data.map(|p| p.join("Programs").join("Git")),
        program_files.map(|p| p.join("Git")),
        program_files_x86.map(|p| p.join("Git")),
    ];

    let suffixes: Vec<PathBuf> = if name == "git" {
        vec![PathBuf::from("cmd").join(&executable), PathBuf::from("bin").join(&executable)]
    } else {
        vec![
            PathBuf::from("usr").join("bin").join(&executable),
            PathBuf::from("bin").join(&executable),
        ]
    };

    for root in git_roots.into_iter().flatten() {
        for suffix in &suffixes {
            candidates.push(root.join(suffix));
        }
    }

    if name == "ssh" || name == "ssh-keygen" {
        candidates.push(windows_dir.join("System32").join("OpenSSH").join(&executable));
    }

    dedupe_paths(candidates)
}

fn unix_command_candidates(name: &str, platform_name: &str, env: &Env, home: &Path) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(path_env) = env("PATH") {
        for item in std::env::split_paths(&path_env) {
            if !item.as_os_str().is_empty() {
                roots.push(item);
            }
        }
    }

    if platform_name == "darwin" {
        roots.extend(
            [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/usr/bin",
                "/bin",
            ]
            .map(PathBuf::from),
        );
    } else {
        roots.extend(
            [
                home.join(".local").join("bin"),
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/usr/bin"),
                PathBuf::from("/bin"),
                PathBuf::from("/usr/sbin"),
                PathBuf::from("/sbin"),
            ],
        );
    }

    dedupe_paths(roots.into_iter().map(|root| root.join(name)).collect())
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            out.push(path);
        }
    }
    out
}

/// 在 PATH 中查找可执行文件（对应 Python `shutil.which` 的简化版，支持 `.exe` 后缀）。
pub fn which(env: &Env, name: &str) -> Option<PathBuf> {
    let path = env("PATH")?;
    let exe_suffix = if cfg!(windows) { ".exe" } else { "" };
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(format!("{name}{exe_suffix}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// 依次查找多个可执行文件，返回第一个命中（对应 Python `shutil_which_first`）。
pub fn which_first(env: &Env, names: &[&str]) -> Option<PathBuf> {
    for name in names {
        if let Some(path) = which(env, name) {
            return Some(path);
        }
    }
    None
}

/// 默认仓库目录选择（对应 Python `default_repo_dir`）。
pub fn default_repo_dir(
    cwd: &Path,
    home: &Path,
    saved_repo_dir: Option<&Path>,
    is_git_repo: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    if let Some(saved) = saved_repo_dir {
        if is_git_repo(saved) {
            return saved.to_path_buf();
        }
    }
    if is_git_repo(cwd) {
        return cwd.to_path_buf();
    }
    home.to_path_buf()
}

/// 各平台首选中文字体族（对应 Python `preferred_font_families`）。
pub fn preferred_font_families(platform_name: &str) -> Vec<String> {
    if platform_name == "win32" {
        return ["Microsoft YaHei UI", "Segoe UI", "Arial"]
            .map(String::from)
            .to_vec();
    }
    if platform_name == "darwin" {
        return [
            "PingFang SC",
            "Heiti SC",
            "STHeiti",
            "Hiragino Sans GB",
            "Songti SC",
            "Kaiti SC",
            "SF Pro Text",
            "Helvetica Neue",
        ]
        .map(String::from)
        .to_vec();
    }
    [
        "Microsoft YaHei",
        "Microsoft YaHei UI",
        "DengXian",
        "SimSun",
        "SimHei",
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "WenQuanYi Micro Hei",
        "WenQuanYi Zen Hei",
        "Source Han Sans SC",
        "Noto Sans CJK TC",
        "AR PL UMing CN",
        "AR PL UKai CN",
        "Droid Sans Fallback",
        "Ubuntu",
        "Liberation Sans",
        "Noto Sans",
        "DejaVu Sans",
        "Sans Serif",
    ]
    .map(String::from)
    .to_vec()
}

/// 在文件管理器中打开路径（对应 Python `open_in_file_manager`）。
pub fn open_in_file_manager(path: &Path, select: bool) -> Result<(), Error> {
    let resolved = path;
    if !resolved.exists() {
        return Err(Error::PathNotFound(resolved.to_path_buf()));
    }

    if cfg!(windows) {
        if select && resolved.is_file() {
            return spawn_explorer(&format!("/select,{}", resolved.display()));
        }
        let target = if resolved.is_dir() {
            resolved.to_path_buf()
        } else {
            resolved.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        return spawn_explorer(&target.display().to_string());
    }

    if cfg!(target_os = "macos") {
        if select {
            return spawn_command("open", &["-R", &resolved.display().to_string()]);
        }
        let target = if resolved.is_dir() {
            resolved.display().to_string()
        } else {
            resolved
                .parent()
                .map(Path::display)
                .unwrap_or_else(|| resolved.display())
                .to_string()
        };
        return spawn_command("open", &[&target]);
    }

    // Linux
    let target = if resolved.is_dir() {
        resolved.to_path_buf()
    } else {
        resolved.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let opener_env: &Env = &|k| std::env::var(k).ok();
    let opener = which_first(opener_env, &["xdg-open", "gio"]);
    match opener {
        Some(gio) if gio.file_name().map(|n| n == "gio").unwrap_or(false) => {
            spawn_command(&gio.display().to_string(), &["open", &target.display().to_string()])
        }
        Some(xdg) => spawn_command(&xdg.display().to_string(), &[&target.display().to_string()]),
        None => Err(Error::command_not_found(
            "未找到可用的 Linux 文件管理器启动命令：xdg-open 或 gio",
        )),
    }
}

/// 在系统默认浏览器中打开 URL（无 opener 插件依赖）。
pub fn open_url(url: &str) -> Result<(), Error> {
    if cfg!(windows) {
        return spawn_command("cmd", &["/c", "start", "", url]);
    }
    if cfg!(target_os = "macos") {
        return spawn_command("open", &[url]);
    }
    let opener_env: &Env = &|k| std::env::var(k).ok();
    match which_first(opener_env, &["xdg-open"]) {
        Some(xdg) => spawn_command(&xdg.display().to_string(), &[url]),
        None => Err(Error::command_not_found("未找到 xdg-open")),
    }
}

fn spawn_explorer(args: &str) -> Result<(), Error> {
    let mut child = crate::new_cmd("explorer.exe")
        .arg(args)
        .spawn()
        .map_err(|e| Error::Io {
            context: "启动 explorer.exe 失败".into(),
            source: e,
        })?;
    // explorer 是异步 GUI 进程，直接释放
    let _ = child.wait();
    Ok(())
}

fn spawn_command(program: &str, args: &[&str]) -> Result<(), Error> {
    crate::new_cmd(program)
        .args(args)
        .spawn()
        .map_err(|e| Error::Io {
            context: format!("启动 {program} 失败"),
            source: e,
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_from(map: &HashMap<String, String>) -> impl Fn(&str) -> Option<String> + 'static {
        let owned = map.clone();
        move |k: &str| owned.get(k).cloned()
    }

    #[test]
    fn fallback_ssh_dir_windows_uses_local_appdata() {
        let map = HashMap::from([("LOCALAPPDATA".to_string(), r"C:\Users\u\AppData\Local".to_string())]);
        let getter = env_from(&map);
        let env: &Env = &getter;
        assert_eq!(
            fallback_ssh_dir("win32", env, Path::new(r"C:\Users\u"), Path::new(r"C:\Temp")),
            PathBuf::from(r"C:\Users\u\AppData\Local\git-env-gui\ssh")
        );
    }

    #[test]
    fn fallback_ssh_dir_windows_falls_back_to_temp() {
        let map = HashMap::new();
        let getter = env_from(&map);
        let env: &Env = &getter;
        assert_eq!(
            fallback_ssh_dir("win32", env, Path::new(r"C:\Users\u"), Path::new(r"C:\Temp")),
            PathBuf::from(r"C:\Temp\git-env-gui\ssh")
        );
    }

    #[test]
    fn fallback_ssh_dir_linux_uses_xdg_data_home() {
        let map = HashMap::from([("XDG_DATA_HOME".to_string(), "/home/u/.local/share".to_string())]);
        let getter = env_from(&map);
        let env: &Env = &getter;
        assert_eq!(
            fallback_ssh_dir("linux", env, Path::new("/home/u"), Path::new("/tmp")),
            PathBuf::from("/home/u/.local/share/git-env-gui/ssh")
        );
    }

    #[test]
    fn fallback_ssh_dir_darwin_uses_application_support() {
        let map = HashMap::new();
        let getter = env_from(&map);
        let env: &Env = &getter;
        assert_eq!(
            fallback_ssh_dir("darwin", env, Path::new("/Users/u"), Path::new("/tmp")),
            PathBuf::from("/Users/u/Library/Application Support/git-env-gui/ssh")
        );
    }

    #[test]
    fn command_candidates_windows_checks_scoop_and_git_for_windows() {
        let map = HashMap::from([
            ("LOCALAPPDATA".to_string(), r"C:\Users\u\AppData\Local".to_string()),
            ("ProgramFiles".to_string(), r"C:\Program Files".to_string()),
            ("ProgramFiles(x86)".to_string(), r"C:\Program Files (x86)".to_string()),
            ("WINDIR".to_string(), r"C:\Windows".to_string()),
        ]);
        let getter = env_from(&map);
        let env: &Env = &getter;
        let git = command_candidates("git", "win32", env, Path::new(r"C:\Users\u"));
        assert!(git.contains(&PathBuf::from(r"C:\Users\u\scoop\shims\git.exe")));
        assert!(git.contains(&PathBuf::from(r"C:\Program Files\Git\cmd\git.exe")));
        assert!(git.contains(&PathBuf::from(r"C:\Program Files\Git\bin\git.exe")));

        let ssh = command_candidates("ssh", "win32", env, Path::new(r"C:\Users\u"));
        assert!(ssh.contains(&PathBuf::from(r"C:\Windows\System32\OpenSSH\ssh.exe")));
    }

    #[test]
    fn command_candidates_unix_uses_home_local_bin() {
        let map = HashMap::from([("PATH".to_string(), "/usr/local/bin:/usr/bin".to_string())]);
        let getter = env_from(&map);
        let env: &Env = &getter;
        let git = command_candidates("git", "linux", env, Path::new("/home/u"));
        assert!(git.contains(&PathBuf::from("/home/u/.local/bin/git")));
        assert!(git.contains(&PathBuf::from("/usr/local/bin/git")));
        assert!(git.contains(&PathBuf::from("/usr/bin/git")));
    }

    #[test]
    fn default_repo_dir_priority() {
        let is_repo = |p: &Path| p.file_name().map(|n| n == "repo").unwrap_or(false);
        // 1. saved 是 repo → 优先 saved
        assert_eq!(
            default_repo_dir(
                Path::new("/cwd"),
                Path::new("/home/u"),
                Some(Path::new("/repo")),
                &is_repo,
            ),
            PathBuf::from("/repo")
        );
        // 2. saved 不是 repo，但 cwd 是 repo → cwd
        assert_eq!(
            default_repo_dir(
                Path::new("/repo"),
                Path::new("/home/u"),
                Some(Path::new("/not-repo")),
                &is_repo,
            ),
            PathBuf::from("/repo")
        );
        // 3. 都不是 → home
        assert_eq!(
            default_repo_dir(
                Path::new("/cwd"),
                Path::new("/home/u"),
                None,
                &is_repo,
            ),
            PathBuf::from("/home/u")
        );
    }

    #[test]
    fn preferred_font_families_windows() {
        let fonts = preferred_font_families("win32");
        assert!(fonts.first().unwrap().contains("Microsoft YaHei"));
    }
}

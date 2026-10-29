//! 桌面应用更新：复用 Gitee 安装包与 SHA256SUMS，不接管开发环境的版本。
#![cfg_attr(not(feature = "gui"), allow(dead_code))]
use crate::runtime::checked_output;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[cfg(feature = "gui")]
use std::sync::OnceLock;

const REPO: &str = "apricotdream/devEnvMangerTool";

#[derive(Clone, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(skip)]
    github: bool,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Clone)]
struct Candidate {
    version: String,
    installer: Asset,
    checksums: Asset,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub available: bool,
    pub can_install: bool,
    pub notes: String,
    pub published_at: Option<String>,
    pub release_url: Option<String>,
    pub message: Option<String>,
}

struct Prepared {
    path: PathBuf,
    checksum: String,
    version: String,
}

#[derive(Default)]
struct UpdateState {
    candidate: Option<Candidate>,
    prepared: Option<Prepared>,
}

static STATE: Mutex<UpdateState> = Mutex::new(UpdateState { candidate: None, prepared: None });
static DOWNLOAD: Mutex<()> = Mutex::new(());

fn version(raw: &str) -> Option<semver::Version> {
    semver::Version::parse(raw.strip_prefix('v').unwrap_or(raw)).ok()
}

#[cfg(test)]
fn select_release(releases: Vec<Release>, current: &str, arch: &str) -> Result<(UpdateInfo, Option<Candidate>), String> {
    select_release_for(releases, current, arch, false)
}

fn select_release_for(releases: Vec<Release>, current: &str, arch: &str, cli: bool) -> Result<(UpdateInfo, Option<Candidate>), String> {
    let current_semver = version(current).ok_or("当前应用版本格式不正确")?;
    let latest = releases.into_iter().filter_map(|release| {
        let parsed = version(&release.tag_name)?;
        // 正式版只提示正式更新；beta 安装可以更新至 beta 或更高正式版。
        if current_semver.pre.is_empty() && !parsed.pre.is_empty() { return None; }
        Some((parsed, release))
    }).max_by(|a, b| a.0.cmp(&b.0));
    let mut info = UpdateInfo {
        current_version: current.into(), latest_version: None, available: false, can_install: false,
        notes: String::new(), published_at: None, release_url: None, message: None,
    };
    let Some((latest_version, release)) = latest else {
        info.message = Some("发布源暂未提供可用版本".into());
        return Ok((info, None));
    };
    info.latest_version = Some(latest_version.to_string());
    info.notes = release.body.unwrap_or_default();
    info.published_at = release.created_at;
    let host = if release.github { "github.com" } else { "gitee.com" };
    info.release_url = Some(format!("https://{host}/{REPO}/releases/tag/{}", release.tag_name));
    info.available = latest_version > current_semver;
    if !info.available { return Ok((info, None)); }
    let installer_name = if cli {
        if cfg!(windows) { format!("devenv-cli-{latest_version}-windows-{arch}.zip") }
        else { format!("devenv-{}-unknown-linux-musl-{latest_version}.tar.gz", std::env::consts::ARCH) }
    } else { format!("DevEnvManager_{latest_version}_{arch}-setup.exe") };
    let asset_prefix = format!("https://{host}/{REPO}/releases/download/{}/", release.tag_name);
    let installer = release.assets.iter().find(|a| a.name == installer_name && a.browser_download_url == format!("{asset_prefix}{installer_name}"));
    let manifest = if cli && !cfg!(windows) { format!("SHA256SUMS-linux-{}.txt", std::env::consts::ARCH) }
        else { format!("SHA256SUMS-windows-{arch}.txt") };
    let checksums = [manifest.as_str(), "SHA256SUMS.txt"].into_iter().find_map(|name|
        release.assets.iter().find(|a| a.name == name && a.browser_download_url == format!("{asset_prefix}{name}")));
    let candidate = match (installer, checksums) {
        (Some(installer), Some(checksums)) => Some(Candidate { version: latest_version.to_string(), installer: installer.clone(), checksums: checksums.clone() }),
        _ => { info.message = Some("新版安装包或校验文件尚未就绪，可稍后手动检查".into()); None }
    };
    info.can_install = (cli || cfg!(windows)) && candidate.is_some();
    if !cli && !cfg!(windows) { info.message = Some("应用内安装更新目前仅支持 Windows 桌面版".into()); }
    Ok((info, candidate))
}

pub fn check(current: &str) -> Result<UpdateInfo, String> { check_for(current, false) }
pub fn check_cli(current: &str) -> Result<UpdateInfo, String> { check_for(current, true) }

fn check_for(current: &str, cli: bool) -> Result<UpdateInfo, String> {
    let mut releases = Vec::new();
    let mut errors = Vec::new();
    for (api, github) in [("https://gitee.com/api/v5", false), ("https://api.github.com", true)] {
        let result = (|| -> Result<Vec<Release>, String> {
            let mut page = 1;
            let mut all = Vec::new();
            loop {
                let url = format!("{api}/repos/{REPO}/releases?per_page=100&page={page}&sort=created_at&direction=desc");
                let text = checked_output(crate::new_cmd(crate::curl_bin()).args([
                    "-fLSs", "--connect-timeout", "5", "--max-time", "15", "--proto", "=https", "--proto-redir", "=https",
                    "-H", "User-Agent: DevEnvManager", &url,
                ]))?;
                let mut batch: Vec<Release> = serde_json::from_str(&text).map_err(|e| format!("发布信息解析失败：{e}"))?;
                let done = batch.len() < 100;
                for item in &mut batch { item.github = github; }
                all.extend(batch);
                if done { break; }
                page += 1;
            }
            Ok(all)
        })();
        match result { Ok(found) => releases.extend(found), Err(error) => errors.push(error) }
    }
    if releases.is_empty() && !errors.is_empty() { return Err(format!("检查更新失败，请检查网络后手动重试：{}", errors.join("；"))); }
    let arch = match std::env::consts::ARCH { "x86_64" => "x64", "aarch64" => "arm64", _ => "unsupported" };
    let (info, candidate) = select_release_for(releases, current, arch, cli)?;
    STATE.lock().map_err(|e| e.to_string())?.candidate = candidate;
    Ok(info)
}

fn parse_checksum(text: &str, name: &str) -> Result<String, String> {
    for line in text.lines() {
        let line = line.trim_start_matches('\u{feff}').trim();
        let Some((hash, file)) = line.split_once(char::is_whitespace) else { continue; };
        if file.trim().trim_start_matches('*') == name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(hash.to_ascii_lowercase());
        }
    }
    Err("校验文件中未找到该安装包的 SHA256，已停止更新".into())
}

fn verify(path: &Path, expected: &str) -> Result<(), String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 { break; }
        hasher.update(&buffer[..count]);
    }
    if format!("{:x}", hasher.finalize()) != expected { return Err("安装包校验失败，请重新下载".into()); }
    Ok(())
}

fn download(selected: &str, emit: impl Fn(&str, u64)) -> Result<String, String> {
    let _download = DOWNLOAD.try_lock().map_err(|_| "更新任务正在进行，请勿重复操作")?;
    let candidate = STATE.lock().map_err(|e| e.to_string())?.candidate.clone().filter(|c| c.version == selected).ok_or("更新信息已变化，请重新检查")?;
    emit("checksums", 0);
    let text = checked_output(crate::new_cmd(crate::curl_bin()).args([
        "-fLSs", "--connect-timeout", "5", "--max-time", "20", "--proto", "=https", "--proto-redir", "=https", &candidate.checksums.browser_download_url,
    ]))?;
    let checksum = parse_checksum(&text, &candidate.installer.name)?;
    let directory = dirs::cache_dir().ok_or("无法确定更新缓存目录")?.join("DevEnvManager").join("updates").join(selected);
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let destination = directory.join(&candidate.installer.name);
    let partial = directory.join(format!("{}.part", candidate.installer.name));
    emit("downloading", 0);
    let result = (|| {
        use std::process::Stdio;
        let mut child = crate::new_cmd(crate::curl_bin()).args([
            "-fLSs", "--connect-timeout", "8", "--max-time", "900", "--speed-limit", "1024", "--speed-time", "30",
            "--proto", "=https", "--proto-redir", "=https", "-o",
        ]).arg(&partial).arg(&candidate.installer.browser_download_url).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
        loop {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() { break; }
            emit("downloading", std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0));
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() { return Err(format!("下载安装包失败：{}", String::from_utf8_lossy(&output.stderr))); }
        emit("verifying", 0);
        verify(&partial, &checksum)?;
        if destination.exists() { std::fs::remove_file(&destination).map_err(|e| e.to_string())?; }
        std::fs::rename(&partial, &destination).map_err(|e| e.to_string())?;
        STATE.lock().map_err(|e| e.to_string())?.prepared = Some(Prepared { path: destination, checksum, version: selected.into() });
        emit("ready", 0);
        Ok("安装包已下载并通过校验，点击「退出并安装」完成更新".into())
    })();
    if result.is_err() { let _ = std::fs::remove_file(partial); }
    result
}

/// CLI 与 GUI 共用发布选择、下载和 SHA256 校验。
pub fn download_cli(selected: &str, emit: impl Fn(&str, u64)) -> Result<PathBuf, String> {
    download(selected, emit)?;
    Ok(STATE.lock().map_err(|e| e.to_string())?.prepared.as_ref().ok_or("下载未完成")?.path.clone())
}

pub fn install_cli(selected: &str) -> Result<PathBuf, String> {
    let _download = DOWNLOAD.try_lock().map_err(|_| "更新任务正在进行")?;
    let state = STATE.lock().map_err(|e| e.to_string())?;
    let prepared = state.prepared.as_ref().filter(|p| p.version == selected).ok_or("请先下载更新")?;
    verify(&prepared.path, &prepared.checksum)?;
    let directory = prepared.path.parent().unwrap().join("cli-extracted");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    // archive 已通过官方发布的散列校验，解压路径只用于本次更新。
    checked_output(crate::new_cmd("tar").arg("-xf").arg(&prepared.path).arg("-C").arg(&directory))?;
    #[cfg(windows)]
    let binary = directory.join("devenv.exe");
    #[cfg(not(windows))]
    let binary = directory.join(format!("devenv-{}-unknown-linux-musl-{selected}", std::env::consts::ARCH)).join("devenv");
    if !binary.is_file() { return Err("更新包中缺少 devenv 可执行文件".into()); }
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    // Windows 不覆盖正在映射的 exe，先保留可回滚备份；Unix 原子替换运行文件。
    #[cfg(windows)] {
        let backup = current.with_file_name(format!("devenv-{}.backup.exe", std::process::id()));
        std::fs::rename(&current, &backup).map_err(|e| format!("无法备份当前 CLI：{e}"))?;
        if let Err(e) = std::fs::copy(&binary, &current) {
            let _ = std::fs::rename(&backup, &current);
            return Err(format!("更新失败，已尝试恢复原程序：{e}"));
        }
    }
    #[cfg(not(windows))] {
        crate::runtime::atomic_write(&current, &std::fs::read(&binary).map_err(|e| e.to_string())?)?;
    }
    Ok(current)
}

#[cfg(feature = "gui")]
#[tauri::command]
pub fn app_version(app: tauri::AppHandle) -> String { app.package_info().version.to_string() }

#[cfg(feature = "gui")]
#[tauri::command]
pub async fn check_app_update(app: tauri::AppHandle, automatic: Option<bool>) -> Result<UpdateInfo, String> {
    static AUTOMATIC: OnceLock<Result<UpdateInfo, String>> = OnceLock::new();
    let current = app.package_info().version.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        if automatic.unwrap_or(false) {
            AUTOMATIC.get_or_init(|| check(&current)).clone()
        } else { check(&current) }
    }).await.map_err(|e| e.to_string())?
}

#[cfg(feature = "gui")]
#[tauri::command]
pub async fn download_app_update(app: tauri::AppHandle, version: String) -> Result<String, String> {
    use tauri::Emitter;
    tauri::async_runtime::spawn_blocking(move || download(&version, |stage, bytes| {
        let _ = app.emit("app-update-progress", serde_json::json!({ "stage":stage, "bytes":bytes }));
    })).await.map_err(|e| e.to_string())?
}

#[cfg(feature = "gui")]
#[tauri::command]
pub async fn install_app_update(app: tauri::AppHandle, version: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)] {
            let _download = DOWNLOAD.try_lock().map_err(|_| "更新下载仍在进行")?;
            let mut state = STATE.lock().map_err(|e| e.to_string())?;
            let prepared = state.prepared.as_ref().filter(|p| p.version == version).ok_or("请先下载安装包")?;
            verify(&prepared.path, &prepared.checksum)?;
            // NSIS 显示正常安装向导；进程成功启动后才关闭应用。
            crate::new_cmd(&prepared.path).spawn().map_err(|e| format!("启动安装器失败：{e}"))?;
            state.prepared = None;
            app.exit(0);
            Ok(())
        }
        #[cfg(not(windows))] { let _ = (app, version); Err("应用内更新安装目前仅支持 Windows".into()) }
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> Release {
        let v = tag.trim_start_matches('v');
        let prefix = format!("https://gitee.com/{REPO}/releases/download/{tag}/");
        let installer = format!("DevEnvManager_{v}_x64-setup.exe");
        Release { tag_name:tag.into(), github:false, body:None, created_at:None, assets:vec![
            Asset { name:installer.clone(), browser_download_url:format!("{prefix}{installer}") },
            Asset { name:"SHA256SUMS.txt".into(), browser_download_url:format!("{prefix}SHA256SUMS.txt") },
        ] }
    }
    #[test]
    fn chooses_semver_not_api_order_and_never_downgrades() {
        let (info, candidate) = select_release(vec![release("v0.2.9-beta"), release("v0.2.10-beta"), release("v0.2.8-beta")], "0.2.8-beta", "x64").unwrap();
        assert!(info.available);
        assert_eq!(candidate.unwrap().version, "0.2.10-beta");
        assert!(!select_release(vec![release("v0.2.8-beta")], "0.2.9-beta", "x64").unwrap().0.available);
    }
    #[test]
    fn stable_channel_ignores_beta_and_assets_must_match_architecture() {
        let (info, _) = select_release(vec![release("v0.3.0-beta"), release("v0.2.9")], "0.2.8", "x64").unwrap();
        assert_eq!(info.latest_version.as_deref(), Some("0.2.9"));
        assert!(select_release(vec![release("v0.2.9")], "0.2.8", "arm64").unwrap().1.is_none());
    }
    #[test]
    fn cli_uses_its_platform_archive_and_prefers_platform_checksum() {
        let mut item = release("v0.2.10-beta");
        let archive = if cfg!(windows) { "devenv-cli-0.2.10-beta-windows-x64.zip".into() }
            else { format!("devenv-{}-unknown-linux-musl-0.2.10-beta.tar.gz", std::env::consts::ARCH) };
        let manifest = if cfg!(windows) { "SHA256SUMS-windows-x64.txt".into() }
            else { format!("SHA256SUMS-linux-{}.txt", std::env::consts::ARCH) };
        for name in [archive, manifest.clone()] {
            item.assets.push(Asset { browser_download_url:format!("https://gitee.com/{REPO}/releases/download/v0.2.10-beta/{name}"), name });
        }
        let (info, candidate) = select_release_for(vec![item], "0.2.9-beta", "x64", true).unwrap();
        assert!(info.can_install);
        assert_eq!(candidate.unwrap().checksums.name, manifest);
    }
    #[test]
    fn github_assets_must_stay_in_the_expected_repository() {
        let mut item = release("v0.2.10-beta");
        item.github = true;
        assert!(select_release_for(vec![item.clone()], "0.2.9-beta", "x64", false).unwrap().1.is_none());
        for asset in &mut item.assets {
            asset.browser_download_url = asset.browser_download_url.replace("gitee.com", "github.com");
        }
        let (info, candidate) = select_release_for(vec![item], "0.2.9-beta", "x64", false).unwrap();
        assert!(candidate.is_some());
        assert!(info.release_url.unwrap().starts_with("https://github.com/apricotdream/devEnvMangerTool/"));
    }
    #[test]
    fn checksums_match_exact_filename_and_reject_invalid_hashes() {
        let hash = "a".repeat(64);
        assert_eq!(parse_checksum(&format!("\u{feff}{hash}  installer.exe\n"), "installer.exe").unwrap(), hash);
        assert!(parse_checksum(&format!("{hash}  another.exe"), "installer.exe").is_err());
        assert!(parse_checksum("wrong  installer.exe", "installer.exe").is_err());
    }
    #[test]
    fn installer_rejects_corrupted_content() {
        let path = std::env::temp_dir().join(format!("devenv-update-verify-{}", std::process::id()));
        std::fs::write(&path, "abc").unwrap();
        assert!(verify(&path, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").is_ok());
        assert!(verify(&path, &"0".repeat(64)).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[ignore = "联网验证真实发布包；仅下载校验，不运行安装器"]
    fn live_release_download_verifies_checksum() {
        let info = check("0.0.0-beta").unwrap();
        assert!(info.available);
        let selected = info.latest_version.unwrap();
        download(&selected, |_, _| {}).unwrap();
        let state = STATE.lock().unwrap();
        let prepared = state.prepared.as_ref().unwrap();
        assert_eq!(prepared.version, selected);
        verify(&prepared.path, &prepared.checksum).unwrap();
    }
}

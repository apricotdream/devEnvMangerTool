use std::path::Path;
use std::process::Command;

pub fn checked_output(command: &mut Command) -> Result<String, String> {
    let output = command.output().map_err(|e| format!("启动命令失败：{e}"))?;
    if !output.status.success() {
        return Err(format!("命令执行失败（{}）：{}{}", output.status,
            String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 同目录落盘后原子替换；写入失败时保留原文件。
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("文件缺少父目录")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = parent.join(format!(".devenv-{}-{}.tmp", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e| e.to_string())?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::metadata(path).map(|m| m.permissions())
                .unwrap_or_else(|_| std::fs::Permissions::from_mode(0o600));
            file.set_permissions(permissions).map_err(|e| e.to_string())?;
        }
        file.write_all(content).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
        drop(file);
        #[cfg(windows)] {
            use std::os::windows::ffi::OsStrExt;
            #[link(name = "kernel32")]
            extern "system" { fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32; }
            let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1 | 8) } == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        #[cfg(not(windows))]
        std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temp); }
    result
}

pub fn version_key(raw: &str) -> semver::Version {
    semver::Version::parse(raw.trim_start_matches('v')).unwrap_or_else(|_| semver::Version::new(0, 0, 0))
}

/// 两个平台共用解析，支持 npm 常用范围；不把不认识的表达式猜成版本。
pub fn resolve_node_requirement(raw: &str, available: &[String]) -> Option<String> {
    let raw = raw.trim().trim_start_matches('v');
    if let Ok(exact) = semver::Version::parse(raw) { return Some(format!("v{exact}")); }
    if raw == "lts" || raw == "lts/*" || raw.starts_with("lts/") {
        let index = checked_output(crate::new_cmd(crate::curl_bin()).args([
            "-fsSL", "--connect-timeout", "5", "--max-time", "15", "https://nodejs.org/dist/index.json"
        ])).ok()?;
        let entries: Vec<serde_json::Value> = serde_json::from_str(&index).ok()?;
        let name = raw.strip_prefix("lts/").filter(|s| *s != "*");
        return entries.iter().filter(|e| e["lts"].is_string())
            .filter(|e| name.map(|n| e["lts"].as_str().unwrap_or("").eq_ignore_ascii_case(n)).unwrap_or(true))
            .filter_map(|e| e["version"].as_str()).max_by_key(|v| version_key(v)).map(str::to_string);
    }
    if raw == "node" { return available.iter().max_by_key(|v| version_key(v)).cloned(); }
    let comparators = regex::Regex::new(r"(>=|<=|>|<|=|\^|~)?\s*(\d+(?:\.(?:\d+|x|X|\*)){0,2}|\*)").ok()?;
    let mut matches = Vec::new();
    for branch in raw.split("||") {
        let normalized = comparators.replace_all(branch, "$1$2");
        let normalized = normalized.split_whitespace().collect::<Vec<_>>().join(", ");
        let Ok(requirement) = semver::VersionReq::parse(&normalized) else { return None; };
        matches.extend(available.iter().filter(|v| requirement.matches(&version_key(v))).cloned());
    }
    matches.into_iter().max_by_key(|v| version_key(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_respect_upper_bounds_and_alternatives() {
        let versions = ["v18.20.8", "v20.19.0", "v22.1.0"].map(str::to_string);
        assert_eq!(resolve_node_requirement(">=18 <22", &versions).as_deref(), Some("v20.19.0"));
        assert_eq!(resolve_node_requirement("18.x || ^22", &versions).as_deref(), Some("v22.1.0"));
        assert_eq!(resolve_node_requirement("invalid", &versions), None);
        assert!(version_key("v22.1.0") > version_key("v9.0.0"));
    }
    #[test]
    fn atomic_write_replaces_existing_content() {
        let root = std::env::temp_dir().join(format!("devenv-atomic-test-{}", std::process::id()));
        let path = root.join("accounts.json");
        atomic_write(&path, b"old").unwrap();
        atomic_write(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        std::fs::remove_dir_all(root).unwrap();
    }
}

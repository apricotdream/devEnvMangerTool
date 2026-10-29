//! npm 发布使用真实 npm 配置优先级，避免手工扫描 .npmrc 误判凭据。
use crate::{PublishHint, RegistryPackageCheck};
use std::process::{Command, Stdio};

pub(crate) fn is_official(registry: &str) -> bool {
    matches!(registry.trim().trim_end_matches('/').to_ascii_lowercase().as_str(),
        "https://registry.npmjs.org" | "https://registry.npmjs.org:443")
}

pub(crate) fn command(dir: &str) -> Command {
    #[cfg(windows)]
    let (program, entries) = {
        let entries = crate::get_registry_path_entries();
        let npm = entries.iter().map(|p| std::path::Path::new(p).join("npm.cmd")).find(|p| p.is_file());
        (npm.unwrap_or_else(|| std::path::PathBuf::from(crate::npm_bin())), entries)
    };
    #[cfg(not(windows))]
    let program = std::path::PathBuf::from(crate::npm_bin());
    let mut command = crate::new_cmd(&program);
    command.current_dir(dir).stdin(Stdio::null());
    #[cfg(windows)] {
        let mut paths = program.parent().filter(|p| !p.as_os_str().is_empty()).map(|p| vec![p.to_path_buf()]).unwrap_or_default();
        paths.extend(entries.into_iter().map(std::path::PathBuf::from));
        if let Ok(path) = std::env::join_paths(paths) { command.env("PATH", path); }
    }
    let cert = std::path::Path::new(&crate::certs_dir()).join("all.pem");
    if cert.is_file() { command.env("NODE_EXTRA_CA_CERTS", cert); }
    command
}

pub(crate) fn check_metadata(dir: &str, registry: &str, name: &str) -> RegistryPackageCheck {
    let result = command(dir).args(["view", name, "versions", "--json", "--registry", registry,
        "--fetch-retries", "0", "--fetch-timeout", "15000"]).output();
    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                match serde_json::from_str::<serde_json::Value>(&stdout) {
                    Ok(value) => RegistryPackageCheck { exists:true, versions:match value {
                        serde_json::Value::String(v) => vec![v],
                        serde_json::Value::Array(items) => items.into_iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
                        _ => vec![],
                    }, error:None },
                    Err(e) => RegistryPackageCheck { exists:false, versions:vec![], error:Some(format!("npm 版本响应解析失败：{e}")) },
                }
            } else if stdout.contains("E404") || stderr.contains("E404") {
                RegistryPackageCheck { exists:false, versions:vec![], error:None }
            } else {
                RegistryPackageCheck { exists:false, versions:vec![], error:Some(format!("{stderr}\n{stdout}")) }
            }
        }
        Err(e) => RegistryPackageCheck { exists:false, versions:vec![], error:Some(format!("启动 npm 失败：{e}")) },
    }
}

pub(crate) fn arguments(registry: &str, version: &str, otp: Option<&str>, dry_run: bool, access: Option<&str>) -> Result<Vec<String>, String> {
    if access.is_some_and(|a| !["public", "restricted"].contains(&a)) { return Err("发布可见性只支持 public / restricted".into()); }
    let mut args = vec!["publish".into(), "--registry".into(), registry.into(), "--fetch-retries".into(), "0".into(), "--fetch-timeout".into(), "60000".into()];
    if let Some(tag) = crate::prerelease_tag(version) {
        // 纯数字 prerelease 不能作为 npm dist-tag。
        args.extend(["--tag".into(), if tag.chars().all(|c| c.is_ascii_digit()) { "next".into() } else { tag }]);
    }
    if dry_run { args.push("--dry-run".into()); }
    if let Some(otp) = otp.filter(|s| !s.trim().is_empty()) { args.extend(["--otp".into(), otp.trim().into()]); }
    if let Some(access) = access { args.extend(["--access".into(), access.into()]); }
    Ok(args)
}

/// 只在明确收到 OTP challenge 时询问验证码；保留原始输出，便于定位权限与网络错误。
pub(crate) fn failure(registry: &str, output: &str) -> (String, bool, Option<PublishHint>) {
    let lower = output.to_ascii_lowercase();
    let needs_otp = lower.contains("eotp") || lower.contains("one-time password is required") || lower.contains("one-time pass");
    let official = is_official(registry);
    let (advice, url) = if official && lower.contains("e_stage_required") {
        ("当前 Token 仅允许暂存发布，不能直接 npm publish。请使用 npm 的 staged publishing 工作流，或换用具备直接发布权限的凭据。", "https://docs.npmjs.com/about-access-tokens/")
    } else if needs_otp {
        ("发布需要一次性验证码，请输入当前有效的 OTP；验证码将只用于重试需要验证的包。", "https://docs.npmjs.com/accessing-npm-using-2fa/")
    } else if official && (lower.contains("granular access token") || lower.contains("bypass 2fa") || lower.contains("two-factor authentication")) {
        ("请检查 granular Token 是否有效、是否拥有目标包的写入及直接发布权限，并按包的 2FA 策略配置。登录成功不代表拥有发布权限。", "https://docs.npmjs.com/about-access-tokens/")
    } else if lower.contains("e401") || lower.contains("eneedauth") || lower.contains("eauth") {
        ("凭据缺失或已过期，请重新登录或更新 Token，并确认发布项目使用了对应的 npm 配置。", "https://docs.npmjs.com/creating-and-viewing-access-tokens/")
    } else if official && (lower.contains("e402") || lower.contains("payment required")) {
        ("请检查 scoped 包的发布可见性；如果要发布公开包，可选择「公开」。私有包需要相应的 npm 账户权益。", "https://docs.npmjs.com/creating-and-publishing-scoped-public-packages/")
    } else if lower.contains("e403") {
        ("发布被拒，请检查包名归属、版本是否已存在、账号权限及 Token 权限。", "https://docs.npmjs.com/cli/v11/commands/npm-publish/")
    } else {
        ("发布失败，请根据 npm 原始错误检查项目配置与网络。", "https://docs.npmjs.com/cli/v11/commands/npm-publish/")
    };
    (format!("{advice}\n\nnpm 原始输出：\n{output}"), needs_otp, Some(PublishHint { text:"查看 npm 官方说明 ↗".into(), url:url.into() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn passes_otp_dry_run_tag_and_access_without_changing_default_visibility() {
        let args = arguments("https://registry.npmjs.org", "1.0.0-beta.1", Some("123456"), true, Some("public")).unwrap();
        for pair in [["--otp", "123456"], ["--tag", "beta"], ["--access", "public"]] { assert!(args.windows(2).any(|v| v == pair)); }
        assert!(args.contains(&"--dry-run".to_string()));
        assert!(!arguments("https://registry.npmjs.org", "1.0.0", None, false, None).unwrap().contains(&"--access".to_string()));
        assert!(arguments("x", "1.0.0", None, false, Some("unexpected")).is_err());
    }
    #[test]
    fn token_permission_errors_do_not_trigger_otp_retry() {
        assert!(!failure("https://registry.npmjs.org", "E403 requires two-factor authentication or granular access token with bypass 2fa").1);
        assert!(failure("https://registry.npmjs.org", "EOTP This operation requires a one-time password").1);
        assert!(!failure("https://registry.npmjs.org", "E_STAGE_REQUIRED").1);
        assert!(!is_official("https://registry.npmjs.org.attacker.example"));
    }
    #[test]
    #[ignore = "需要本机 npm；只执行临时 workspace 包的 dry-run，不上传"]
    fn real_npm_workspace_dry_run_packs_the_selected_package() {
        let root = std::env::temp_dir().join(format!("devenv-publish-smoke-{}", std::process::id()));
        let package = root.join("packages/demo");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(root.join("package.json"), r#"{"name":"devenv-smoke-root","private":true,"workspaces":["packages/*"]}"#).unwrap();
        std::fs::write(package.join("package.json"), r#"{"name":"@devenv-local-check/publish-smoke","version":"0.0.0-beta.1","main":"index.js"}"#).unwrap();
        std::fs::write(package.join("index.js"), "module.exports = 1;\n").unwrap();
        let config = root.join("empty.npmrc");
        std::fs::write(&config, "").unwrap();
        let args = arguments("https://registry.npmjs.org", "0.0.0-beta.1", Some("123456"), true, Some("public")).unwrap();
        let output = command(&package.to_string_lossy()).args(args).args(["--json", "--ignore-scripts"])
            .env("NPM_CONFIG_USERCONFIG", &config).env("NPM_CONFIG_CACHE", root.join("cache")).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        // npm 11 按包名返回结果映射，旧版 npm 返回单个对象。
        let value = if parsed["name"].is_string() { &parsed } else { &parsed["@devenv-local-check/publish-smoke"] };
        assert_eq!(value["name"], "@devenv-local-check/publish-smoke");
        assert_eq!(value["version"], "0.0.0-beta.1");
        let scanned = crate::list_publish_packages(root.to_string_lossy().into()).unwrap();
        assert_eq!(scanned.len(), 1); // 无版本号的私有 workspace 根目录不作为包。
        let results = crate::publish_packages_impl(vec![package.to_string_lossy().into()],
            "https://registry.npmjs.org".into(), false, None, Some(true), Some("public".into())).unwrap();
        assert!(results[0].ok, "{}", results[0].message);
        assert_eq!(results[0].message, "演练成功（未上传）");
        std::fs::remove_dir_all(root).unwrap();
    }
}

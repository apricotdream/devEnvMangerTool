use clap::Subcommand;

#[derive(Subcommand)]
pub enum UpdateCmd {
    /// 只检查最新 CLI 版本
    Check,
    /// 下载并校验 CLI 更新，不安装
    Download,
    /// 下载校验后替换当前 CLI，保留 Windows 原程序备份
    Install,
}

pub fn run(action: Option<UpdateCmd>) {
    let action = action.unwrap_or_else(|| {
        match super::pick("CLI 更新", &["检查更新", "下载更新", "安装更新"], |s| s.to_string()) {
            Some(&"下载更新") => UpdateCmd::Download,
            Some(&"安装更新") => UpdateCmd::Install,
            _ => UpdateCmd::Check,
        }
    });
    let result = (|| -> Result<(), String> {
        use devenv_core::app_update;
        let info = app_update::check_cli(env!("CARGO_PKG_VERSION"))?;
        println!("当前 {}，最新 {}", info.current_version, info.latest_version.as_deref().unwrap_or("暂无"));
        if let Some(message) = &info.message { println!("{message}"); }
        if !info.available || matches!(action, UpdateCmd::Check) { return Ok(()); }
        if !info.can_install { return Err("当前平台的更新包尚未就绪".into()); }
        let version = info.latest_version.ok_or("缺少最新版本")?;
        let path = app_update::download_cli(&version, |stage, _| {
            // 仅显示阶段，避免脚本输出被每 250ms 的下载进度淹没。
            if stage != "downloading" { println!("{stage}"); }
        })?;
        println!("已通过 SHA256 校验：{}", path.display());
        if matches!(action, UpdateCmd::Install) {
            println!("已更新：{}；下次运行生效", app_update::install_cli(&version)?.display());
        }
        Ok(())
    })();
    if let Err(error) = result { super::fail(error); }
}

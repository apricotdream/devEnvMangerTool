//! DevEnvManager CLI（无 GUI 版本）。
//!
//! 用法示例：
//!   devenv node status
//!   devenv node versions
//!   devenv node install v26.7.0
//!   devenv git accounts
//!   devenv git key gitee.com

use clap::{Parser, Subcommand};
use std::path::Path;

#[derive(Parser)]
#[command(name = "devenv", about = "DevEnvManager - 开发环境管理 CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Node 相关操作
    #[command(subcommand)]
    Node(NodeCmd),
    /// Python 相关操作
    #[command(subcommand)]
    Python(PythonCmd),
    /// Git 相关操作
    #[command(subcommand)]
    Git(GitCmd),
}

#[derive(Subcommand)]
enum NodeCmd {
    /// 检测 Node 环境状态
    Status,
    /// 查看或设置 npm registry（devenv node registry [set <url>]）
    Registry { set: Option<String> },
    /// 列出已安装和可用版本
    Versions,
    /// 安装指定版本（devenv node install v26.7.0）
    Install { version: String },
    /// 切换版本
    Switch { version: String },
    /// 卸载版本
    Uninstall { version: String },
}

#[derive(Subcommand)]
enum PythonCmd {
    /// 检测 Python 环境状态
    Status,
    /// 查看或设置 pip 镜像（devenv python registry [set <url>]）
    Registry { set: Option<String> },
    /// 列出已安装和可用版本
    Versions,
    /// 安装指定版本（devenv python install 3.12.4）
    Install { version: String },
    /// 切换版本
    Switch { version: String },
    /// 卸载版本
    Uninstall { version: String },
}

#[derive(Subcommand)]
enum GitCmd {
    /// 列出 Git 账号
    Accounts,
    /// 当前 Git 身份
    Identity,
    /// 切换全局身份
    Switch { alias: String },
    /// 显示指定账号的公钥
    Key { alias: String },
    /// 测试 SSH 连接
    Test { alias: String },
    /// 同步仓库（git pull --ff-only）
    Sync { repo: String },
}

fn fail(msg: String) -> ! {
    eprintln!("[错误] {}", msg);
    std::process::exit(1);
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Node(cmd) => run_node(cmd),
        Command::Python(cmd) => run_python(cmd),
        Command::Git(cmd) => run_git(cmd),
    }
}

// ── Node ─────────────────────────────────────────────────────────

fn run_node(cmd: NodeCmd) {
    match cmd {
        NodeCmd::Status => {
            let info = dev_env_manager::api::detect_node();
            println!("Node 环境体检:");
            println!("  安装状态 : {}", if info.installed { "已安装 ✓" } else { "未安装 ✗" });
            println!("  Node 版本: {}", info.node_version.as_deref().unwrap_or("-"));
            println!("  npm 版本 : {}", info.npm_version.as_deref().unwrap_or("-"));
            println!("  PATH     : {}", if info.in_path { "已配置 ✓" } else { "未配置 ✗" });
            if let Some(p) = &info.real_node_exe {
                println!("  可执行文件: {}", p);
            }
            if info.store_stub {
                println!("  ⚠ 检测到商店假命令（node 可能打开微软商店）");
            }
            println!("  网络连通性:");
            for n in &info.network {
                let s = if n.ok {
                    format!("可达 · {}ms", n.latency_ms.unwrap_or(0))
                } else {
                    format!("不可达 · {}", n.error.as_deref().unwrap_or(""))
                };
                println!("    {}: {}", n.name, s);
            }
        }
        NodeCmd::Registry { set } => match set {
            Some(url) => match dev_env_manager::api::set_registry(url.clone()) {
                Ok(r) => println!("registry 已设置为 {}\n写入: {}", url, r.path),
                Err(e) => fail(format!("设置 registry 失败: {}", e)),
            },
            None => match dev_env_manager::api::read_registry() {
                Ok(Some(r)) => println!("当前 registry: {}", r),
                Ok(None) => println!("当前 registry: 未设置"),
                Err(e) => fail(format!("读取失败: {}", e)),
            },
        },
        NodeCmd::Versions => {
            let info = dev_env_manager::api::list_versions();
            println!("安装目录: {}", info.install_dir);
            println!("当前版本: {}", info.current.as_deref().unwrap_or("未设置"));
            if info.nvm_detected {
                println!("⚠ 检测到 nvm，切换需先卸载 nvm");
            }
            println!("\n已安装版本:");
            if info.installed.is_empty() {
                println!("  （无）");
            }
            for v in &info.installed {
                let cur = if v.current { "  <- 当前" } else { "" };
                println!("  {}{}{}", v.version, if let Some(mb) = v.size_mb { format!("  ({}MB)", mb) } else { String::new() }, cur);
            }
            println!("\n可用版本（每大版本最新）:");
            for v in info.available.iter().take(20) {
                println!("  {}", v);
            }
        }
        NodeCmd::Install { version } => {
            println!("安装 Node {} ...", version);
            match dev_env_manager::api::install_specific(&version, |stage| {
                println!("  [{}]", stage);
            }) {
                Ok(dir) => println!("✓ 安装完成: {}", dir),
                Err(e) => fail(format!("安装失败: {}", e)),
            }
        }
        NodeCmd::Switch { version } => match dev_env_manager::api::switch_version(version.clone()) {
            Ok(dir) => {
                println!("✓ 已切换到 {}（{}）", version, dir);
                println!("请重开终端使 PATH 生效");
            }
            Err(e) => fail(e),
        },
        NodeCmd::Uninstall { version } => match dev_env_manager::api::uninstall_version(version.clone()) {
            Ok(dir) => println!("✓ 已卸载 {}（{}）", version, dir),
            Err(e) => fail(e),
        },
    }
}

// ── Python ──────────────────────────────────────────────────────

fn run_python(cmd: PythonCmd) {
    match cmd {
        PythonCmd::Status => {
            let info = dev_env_manager::api::detect_python();
            println!("Python 环境体检:");
            println!("  安装状态 : {}", if info.installed { "已安装 ✓" } else { "未安装 ✗" });
            println!("  Python   : {}", info.python_version.as_deref().unwrap_or("-"));
            println!("  pip      : {}", info.pip_version.as_deref().unwrap_or("-"));
            println!("  PATH     : {}", if info.in_path { "已配置 ✓" } else { "未配置 ✗" });
            if let Some(p) = &info.real_python_exe {
                println!("  可执行文件: {}", p);
            }
            if info.store_stub {
                println!("  ⚠ 检测到商店假命令（python 可能打开微软商店）");
            }
            for f in &info.foreign {
                println!(
                    "  外来安装 : [{}] {} {}",
                    f.kind,
                    f.path,
                    f.version.as_deref().unwrap_or("")
                );
            }
            println!("  网络连通性:");
            for n in &info.network {
                let s = if n.ok {
                    format!("可达 · {}ms", n.latency_ms.unwrap_or(0))
                } else {
                    format!("不可达 · {}", n.error.as_deref().unwrap_or(""))
                };
                println!("    {}: {}", n.name, s);
            }
        }
        PythonCmd::Registry { set } => match set {
            Some(url) => match dev_env_manager::api::set_pip_index(url.clone()) {
                Ok(r) => println!("pip index 已设置为 {}\n写入: {}", url, r.path),
                Err(e) => fail(format!("设置 pip 镜像失败: {}", e)),
            },
            None => match dev_env_manager::api::read_pip_index() {
                Ok(Some(r)) => println!("当前 pip index: {}", r),
                Ok(None) => println!("当前 pip index: 未设置"),
                Err(e) => fail(format!("读取失败: {}", e)),
            },
        },
        PythonCmd::Versions => {
            let info = dev_env_manager::api::list_python_versions();
            println!("安装目录: {}", info.install_dir);
            println!("当前版本: {}", info.current.as_deref().unwrap_or("未设置"));
            if info.py_launcher_detected {
                println!("ℹ 检测到 py launcher（官方安装器），与自管版本可共存");
            }
            println!("\n已安装版本:");
            if info.installed.is_empty() {
                println!("  （无）");
            }
            for v in &info.installed {
                let cur = if v.current { "  <- 当前" } else { "" };
                println!(
                    "  {}{}{}",
                    v.version,
                    if let Some(mb) = v.size_mb {
                        format!("  ({}MB)", mb)
                    } else {
                        String::new()
                    },
                    cur
                );
            }
            println!("\n可用版本（每小版本最新）:");
            for v in info.available.iter().take(20) {
                println!("  {}", v);
            }
        }
        PythonCmd::Install { version } => {
            println!("安装 Python {} ...", version);
            match dev_env_manager::api::install_python_specific(&version, |stage| {
                println!("  [{}]", stage);
            }) {
                Ok(dir) => println!("✓ 安装完成: {}", dir),
                Err(e) => fail(format!("安装失败: {}", e)),
            }
        }
        PythonCmd::Switch { version } => match dev_env_manager::api::switch_python_version(version.clone()) {
            Ok(dir) => {
                println!("✓ 已切换到 {}（{}）", version, dir);
                println!("请重开终端使 PATH 生效");
            }
            Err(e) => fail(e),
        },
        PythonCmd::Uninstall { version } => match dev_env_manager::api::uninstall_python_version(version.clone()) {
            Ok(dir) => println!("✓ 已卸载 {}（{}）", version, dir),
            Err(e) => fail(e),
        },
    }
}

// ── Git ──────────────────────────────────────────────────────────

fn git_service() -> dev_env_manager::gitcore::service::Service {
    match dev_env_manager::gitcore::service::Service::new(None) {
        Ok(s) => s,
        Err(e) => fail(format!("Git 服务初始化失败: {}", e)),
    }
}

fn run_git(cmd: GitCmd) {
    let mut svc = git_service();
    match cmd {
        GitCmd::Accounts => match svc.list_accounts() {
            Ok(accounts) => {
                if accounts.is_empty() {
                    println!("暂无 Git 账号");
                    return;
                }
                println!("Git 账号（{} 个）:", accounts.len());
                for a in &accounts {
                    println!(
                        "  {:<24} -> {}:{}  ({} <{}>)  [{}]",
                        a.alias,
                        a.host_name,
                        a.port,
                        a.git_name,
                        a.git_email,
                        a.key_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "-".to_string())
                    );
                }
            }
            Err(e) => fail(format!("读取账号失败: {}", e)),
        },
        GitCmd::Identity => match svc.get_current_identity(false, None) {
            Ok((name, email)) => {
                println!("全局 Git 身份:");
                println!("  name : {}", name.as_deref().unwrap_or("未设置"));
                println!("  email: {}", email.as_deref().unwrap_or("未设置"));
            }
            Err(e) => fail(format!("读取身份失败: {}", e)),
        },
        GitCmd::Switch { alias } => match svc.switch_identity(&alias, false, None) {
            Ok(a) => println!("✓ 已切换全局身份为「{}」（{} <{}>）", a.alias, a.git_name, a.git_email),
            Err(e) => fail(format!("切换失败: {}", e)),
        },
        GitCmd::Key { alias } => match svc.list_accounts() {
            Ok(accounts) => {
                let acc = accounts.iter().find(|a| a.alias == alias);
                match acc {
                    Some(a) => {
                        let pub_key = svc.read_public_key(a.key_path.as_deref());
                        if pub_key.is_empty() {
                            println!("「{}」无公钥（请先生成密钥）", alias);
                        } else {
                            println!("「{}」SSH 公钥:\n{}", alias, pub_key);
                        }
                    }
                    None => fail(format!("账号「{}」不存在", alias)),
                }
            }
            Err(e) => fail(format!("读取账号失败: {}", e)),
        },
        GitCmd::Test { alias } => match svc.test_connection(&alias) {
            Ok((success, output)) => {
                println!("SSH 连接测试「{}」:", alias);
                println!("  结果: {}", if success { "成功 ✓" } else { "失败 ✗" });
                if !output.is_empty() {
                    println!("  {}", output.lines().next().unwrap_or(""));
                }
            }
            Err(e) => fail(format!("测试失败: {}", e)),
        },
        GitCmd::Sync { repo } => {
            let path = Path::new(&repo);
            match svc.sync_repo(path) {
                Ok((account, output)) => {
                    println!("✓ 仓库同步完成（账号 {}）", account.alias);
                    println!("{}", output.lines().take(5).collect::<Vec<_>>().join("\n"));
                }
                Err(e) => fail(format!("同步失败: {}", e)),
            }
        }
    }
}

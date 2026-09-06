//! DevEnvManager CLI（无 GUI 版本）。
//!
//! 用法示例：
//!   devenv node status
//!   devenv node versions
//!   devenv node install          # 不带版本号 → 列出可用版本，输入数字选择
//!   devenv node switch           # 不带版本号 → 列出已装版本，输入数字选择
//!   devenv git accounts
//!   devenv git add               # 交互式新增账号（逐项提示，密钥可自动生成）
//!   devenv git key gitee.com
//!
//! 凡是「选择版本 / 账号」的命令，省略参数即进入数字选择菜单；也可直接带参数用于脚本。

use clap::{Parser, Subcommand};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

// ── 交互式输入（数字选择菜单）──────────────────────────────────────

/// 读取一行输入（去掉首尾空白）。非 TTY / EOF 时返回 None。
fn prompt_line(prompt: &str) -> Option<String> {
    print!("{}", prompt);
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    if handle.read_line(&mut line).ok()? == 0 {
        return None;
    }
    Some(line.trim().to_string())
}

/// 列出候选项并读取一个数字选择，返回所选项的引用。
/// `prompt` 中会自动追加「(输入序号)」。输入 q / 空 / 越界均放弃。
fn pick<'a, T>(prompt: &str, items: &'a [T], label: impl Fn(&T) -> String) -> Option<&'a T> {
    if items.is_empty() {
        eprintln!("（无可选项）");
        return None;
    }
    for (i, item) in items.iter().enumerate() {
        println!("  [{}] {}", i + 1, label(item));
    }
    let raw = prompt_line(&format!("{}（输入序号，q 取消）: ", prompt))?;
    if raw.is_empty() || raw.eq_ignore_ascii_case("q") {
        return None;
    }
    match raw.parse::<usize>() {
        Ok(n) if n >= 1 && n <= items.len() => Some(&items[n - 1]),
        _ => {
            eprintln!("无效序号: {}", raw);
            None
        }
    }
}

/// 命令行参数为纯数字时，解析为列表序号（1 起）。返回 0-based 下标；非数字或越界返回 None。
fn arg_to_index(arg: &str, len: usize) -> Option<usize> {
    arg.parse::<usize>()
        .ok()
        .filter(|&n| n >= 1 && n <= len)
        .map(|n| n - 1)
}

/// 解析 Git 账号选择参数：纯数字 → 按 `git accounts` 列表序号；否则当作别名原样返回；
/// 省略参数 → 进入数字选择菜单。
fn resolve_alias(
    svc: &mut devenv_core::gitcore::service::Service,
    arg: Option<String>,
) -> Option<String> {
    match arg {
        None => pick_account(svc).map(|a| a.alias),
        Some(a) => {
            if a.chars().all(|c| c.is_ascii_digit()) {
                let accounts = svc.list_accounts().ok()?;
                match arg_to_index(&a, accounts.len()) {
                    Some(i) => Some(accounts[i].alias.clone()),
                    None => {
                        eprintln!("序号超出范围（共 {} 个账号）", accounts.len());
                        None
                    }
                }
            } else {
                Some(a)
            }
        }
    }
}

#[derive(Parser)]
#[command(name = "devenv", about = "DevEnvManager - 开发环境管理 CLI（不带参数进入数字菜单）", version)]
struct Cli {
    /// 省略子命令时进入交互式数字菜单；子命令（node/python/git/doctor）仅供脚本使用
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Node.js 相关操作（省略动作进入数字菜单）
    Node {
        #[command(subcommand)]
        action: Option<NodeCmd>,
    },
    /// Python 相关操作（省略动作进入数字菜单）
    Python {
        #[command(subcommand)]
        action: Option<PythonCmd>,
    },
    /// Git 相关操作（省略动作进入数字菜单）
    Git {
        #[command(subcommand)]
        action: Option<GitCmd>,
    },
    /// 环境体检：工具链 / PATH / 软链 / shell 配置，给出可操作修复提示
    Doctor,
}

#[derive(Subcommand, Clone)]
enum NodeCmd {
    /// 检测 Node 环境状态
    Status,
    /// 查看或设置 npm registry（devenv node registry [set <url>]）
    Registry { set: Option<String> },
    /// 列出已安装和可用版本
    Versions,
    /// 安装版本（参数可填版本号 v26.7.0、或列表序号 3；省略则数字选择）
    Install { version: Option<String> },
    /// 切换版本（参数可填版本号或已装列表序号；省略则数字选择）
    Switch { version: Option<String> },
    /// 卸载版本（参数可填版本号或已装列表序号；省略则数字选择）
    Uninstall { version: Option<String> },
    /// 项目管理：解析 .nvmrc / package.json 并一键装切 Node（参数为项目目录，默认当前目录）
    Project { path: Option<String> },
    /// npm 镜像测速（参数为 registry URL，省略则测当前/常用源）
    Test { url: Option<String> },
    /// bun 运行时：状态 / 安装 / 升级 / 卸载（省略动作进入数字菜单）
    Bun {
        #[command(subcommand)]
        action: Option<BunAction>,
    },
    /// npm 发布账号：扫描 ~/.npmrc 已有凭据并纳入管理 / 切换
    Accounts,
    /// 发布 npm 包：扫描 packages/ → dry-run 预检 → 确认后真发
    Publish {
        /// 项目根目录（默认当前目录）
        #[arg(long)]
        dir: Option<String>,
        /// 发布源 registry（默认 https://registry.npmjs.org）
        #[arg(long)]
        registry: Option<String>,
        /// 发布前执行 npm run build
        #[arg(long)]
        build: bool,
        /// 跳过 y/N 确认（脚本用；仍会先 dry-run）
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Clone)]
enum BunAction {
    /// 查看 bun 安装状态
    Status,
    /// 安装 / 升级 bun（参数可填版本号，省略装最新）
    Install { version: Option<String> },
    /// 升级 bun 到最新
    Upgrade,
    /// 卸载托管 bun
    Uninstall,
}

#[derive(Subcommand, Clone)]
enum PythonCmd {
    /// 检测 Python 环境状态
    Status,
    /// 查看或设置 pip 镜像（devenv python registry [set <url>]）
    Registry { set: Option<String> },
    /// 列出已安装和可用版本
    Versions,
    /// 安装版本（参数可填版本号 3.12.4、或列表序号；省略则数字选择）
    Install { version: Option<String> },
    /// 切换版本（参数可填版本号或已装列表序号；省略则数字选择）
    Switch { version: Option<String> },
    /// 卸载版本（参数可填版本号或已装列表序号；省略则数字选择）
    Uninstall { version: Option<String> },
    /// 项目管理：解析 .python-version / pyproject.toml 等并一键装切 Python（参数为项目目录）
    Project { path: Option<String> },
    /// pip 镜像测速（参数为 index URL，省略则测当前/常用源）
    Test { url: Option<String> },
    /// Python 包管理器：列出 / 安装 pip·uv·poetry·pipenv（装到共享 ~/.local/bin）
    Pkg {
        #[command(subcommand)]
        action: Option<PyPkgAction>,
    },
}

#[derive(Subcommand, Clone)]
enum PyPkgAction {
    /// 列出 pip / uv / poetry / pipenv / conda 状态
    List,
    /// 安装包管理器（参数为名称，省略数字选择；--version 指定版本）
    Install {
        name: Option<String>,
        #[arg(long)]
        version: Option<String>,
    },
}

#[derive(Subcommand, Clone)]
enum GitCmd {
    /// 检测 Git / OpenSSH 安装状态、版本与路径
    Status,
    /// 列出 Git 账号
    Accounts,
    /// 当前 Git 身份
    Identity,
    /// 交互式新增 Git 账号（逐项提示，可自动生成 SSH 密钥）
    Add,
    /// 交互式编辑账号（参数为别名或账号列表序号；省略则数字选择）
    Edit { alias: Option<String> },
    /// 删除账号（参数为别名或账号列表序号；省略则数字选择；--purge 连密钥一起删）
    Delete {
        alias: Option<String>,
        /// 同时删除该账号的私钥/公钥文件
        #[arg(long)]
        purge: bool,
    },
    /// 为账号生成 SSH 密钥对（参数为别名或序号；已存在报错，--force 重新生成）
    GenKey {
        alias: Option<String>,
        /// 覆盖已存在的密钥（重新生成）
        #[arg(long)]
        force: bool,
    },
    /// 切换全局身份（参数为别名或账号列表序号；省略则数字选择）
    Switch { alias: Option<String> },
    /// 显示账号公钥（参数为别名或账号列表序号；省略则数字选择）
    Key { alias: Option<String> },
    /// 测试 SSH 连接（参数为别名或账号列表序号；省略则数字选择）
    Test { alias: Option<String> },
    /// 同步仓库（git pull --ff-only）
    Sync { repo: String },
    /// 列出可安装的 Git 版本（Windows：镜像最新稳定版；Linux：系统包管理器）
    Versions,
    /// 安装 Git（Windows：托管安装 MinGit，可带序号直接选版本；Linux：打印包管理器命令）
    Install {
        /// 直接指定 `git versions` 列表序号（省略则数字选择）
        n: Option<usize>,
    },
    /// 卸载本工具托管的 MinGit（仅 Windows）
    Uninstall,
}

fn fail(msg: String) -> ! {
    eprintln!("[错误] {}", msg);
    std::process::exit(1);
}

/// 顶部模块菜单项：(名称, 说明)。
fn module_menu() -> [(&'static str, &'static str); 4] {
    [
        ("Node.js", "环境体检 / 版本安装切换 / npm 镜像"),
        ("Python", "环境体检 / 版本安装切换 / pip 镜像"),
        ("Git", "环境体检 / 账号与密钥 / 身份切换 / 仓库同步"),
        ("环境体检 doctor", "工具链 / PATH / 软链 / shell 配置 一键体检"),
    ]
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        // 无子命令 → 顶部数字菜单
        None => loop {
            println!("\n==== DevEnvManager · 请选择要管理的环境 ====");
            match pick("", &module_menu(), |(name, desc)| format!("{:<16} {}", name, desc)) {
                Some(("Node.js", _)) => {
                    pick_node_action();
                }
                Some(("Python", _)) => {
                    pick_python_action();
                }
                Some(("Git", _)) => {
                    pick_git_action();
                }
                Some(("环境体检 doctor", _)) => run_doctor(),
                Some(_) => unreachable!(),
                None => break, // q / 非交互
            }
        },
        Some(Command::Node { action: None }) => {
            pick_node_action();
        }
        Some(Command::Python { action: None }) => {
            pick_python_action();
        }
        Some(Command::Git { action: None }) => {
            pick_git_action();
        }
        Some(Command::Node { action: Some(cmd) }) => run_node(cmd),
        Some(Command::Python { action: Some(cmd) }) => run_python(cmd),
        Some(Command::Git { action: Some(cmd) }) => run_git(cmd),
        Some(Command::Doctor) => run_doctor(),
    }
}

/// 一个动作菜单项：键（英文子命令）+ 中文说明；执行时按需向用户收集参数。
type MenuItem = (&'static str, &'static str);

fn node_action_menu() -> [MenuItem; 12] {
    [
        ("status", "环境体检"),
        ("versions", "查看已装 / 可用版本"),
        ("install", "安装版本（数字选择）"),
        ("switch", "切换版本（数字选择）"),
        ("uninstall", "卸载版本（数字选择）"),
        ("registry", "查看 / 设置 npm 镜像源"),
        ("project", "项目管理（.nvmrc / package.json 一键装切）"),
        ("test", "npm 镜像测速"),
        ("bun", "bun 运行时（状态 / 安装 / 升级 / 卸载）"),
        ("accounts", "npm 发布账号（扫描 ~/.npmrc 凭据）"),
        ("publish", "发布 npm 包（dry-run 预检后确认）"),
        ("back", "返回上一级"),
    ]
}

fn python_action_menu() -> [MenuItem; 10] {
    [
        ("status", "环境体检"),
        ("versions", "查看已装 / 可用版本"),
        ("install", "安装版本（数字选择）"),
        ("switch", "切换版本（数字选择）"),
        ("uninstall", "卸载版本（数字选择）"),
        ("registry", "查看 / 设置 pip 镜像源"),
        ("project", "项目管理（.python-version / pyproject 一键装切）"),
        ("test", "pip 镜像测速"),
        ("pkg", "Python 包管理器（uv / poetry / pipenv）"),
        ("back", "返回上一级"),
    ]
}

fn git_action_menu() -> [MenuItem; 14] {
    [
        ("status", "环境体检（Git / OpenSSH）"),
        ("accounts", "查看账号列表"),
        ("add", "新增账号（交互式，自动生成密钥）"),
        ("switch", "切换身份（数字选择）"),
        ("key", "查看公钥（数字选择）"),
        ("test", "测试 SSH 连接（数字选择）"),
        ("gen-key", "生成 / 重新生成密钥"),
        ("edit", "编辑账号"),
        ("delete", "删除账号"),
        ("identity", "查看当前身份"),
        ("sync", "同步仓库（git pull）"),
        ("versions", "查看可安装 Git 版本"),
        ("install", "安装 Git（Linux 打印包管理器命令 / Win 托管安装）"),
        ("back", "返回上一级"),
    ]
}

/// 在动作菜单上反复选择，直到返回上一级或取消。
fn menu_loop(
    title: &str,
    items: &[MenuItem],
    mut run: impl FnMut(&str),
) {
    loop {
        println!("\n{} — 请选择操作:", title);
        match pick("", items, |(_, desc)| desc.to_string()) {
            Some((key, _)) if *key == "back" || key.is_empty() => return,
            Some((key, _)) => run(key),
            None => return,
        }
    }
}

fn pick_node_action() -> NodeCmd {
    let mut chosen = NodeCmd::Status;
    menu_loop("Node.js", &node_action_menu(), |key| {
        chosen = match key {
            "status" => NodeCmd::Status,
            "versions" => NodeCmd::Versions,
            "install" => NodeCmd::Install { version: None },
            "switch" => NodeCmd::Switch { version: None },
            "uninstall" => NodeCmd::Uninstall { version: None },
            "registry" => NodeCmd::Registry { set: ask_registry_url() },
            "project" => NodeCmd::Project { path: None },
            "test" => NodeCmd::Test { url: None },
            "bun" => NodeCmd::Bun { action: None },
            "accounts" => NodeCmd::Accounts,
            "publish" => NodeCmd::Publish { dir: None, registry: None, build: false, yes: false },
            _ => return,
        };
        run_node(chosen.clone());
    });
    chosen
}

fn pick_python_action() -> PythonCmd {
    let mut chosen = PythonCmd::Status;
    menu_loop("Python", &python_action_menu(), |key| {
        chosen = match key {
            "status" => PythonCmd::Status,
            "versions" => PythonCmd::Versions,
            "install" => PythonCmd::Install { version: None },
            "switch" => PythonCmd::Switch { version: None },
            "uninstall" => PythonCmd::Uninstall { version: None },
            "registry" => PythonCmd::Registry { set: ask_registry_url() },
            "project" => PythonCmd::Project { path: None },
            "test" => PythonCmd::Test { url: None },
            "pkg" => PythonCmd::Pkg { action: None },
            _ => return,
        };
        run_python(chosen.clone());
    });
    chosen
}

fn pick_git_action() -> GitCmd {
    let mut chosen = GitCmd::Accounts;
    menu_loop("Git", &git_action_menu(), |key| {
        chosen = match key {
            "status" => GitCmd::Status,
            "accounts" => GitCmd::Accounts,
            "add" => GitCmd::Add,
            "switch" => GitCmd::Switch { alias: None },
            "key" => GitCmd::Key { alias: None },
            "test" => GitCmd::Test { alias: None },
            "gen-key" => GitCmd::GenKey { alias: None, force: false },
            "edit" => GitCmd::Edit { alias: None },
            "delete" => GitCmd::Delete { alias: None, purge: false },
            "identity" => GitCmd::Identity,
            "sync" => match ask_sync_repo() {
                Some(repo) => GitCmd::Sync { repo },
                None => return,
            },
            "versions" => GitCmd::Versions,
            "install" => GitCmd::Install { n: None },
            _ => return,
        };
        run_git(chosen.clone());
    });
    chosen
}

/// 询问镜像源 URL（直接回车 = 仅查看）。
fn ask_registry_url() -> Option<String> {
    match prompt_line("输入镜像源 URL（直接回车 = 仅查看当前）: ") {
        Some(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

/// 询问要同步的仓库目录（直接回车 / q 取消）。
fn ask_sync_repo() -> Option<String> {
    match prompt_line("输入仓库目录路径（q 取消）: ") {
        Some(s) if !s.is_empty() && !s.eq_ignore_ascii_case("q") => Some(s),
        _ => None,
    }
}

// ── Node ─────────────────────────────────────────────────────────

fn run_node(cmd: NodeCmd) {
    match cmd {
        NodeCmd::Status => {
            let info = devenv_core::api::detect_node();
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
            Some(url) => match devenv_core::api::set_registry(url.clone()) {
                Ok(r) => println!("registry 已设置为 {}\n写入: {}", url, r.path),
                Err(e) => fail(format!("设置 registry 失败: {}", e)),
            },
            None => match devenv_core::api::read_registry() {
                Ok(Some(r)) => println!("当前 registry: {}", r),
                Ok(None) => println!("当前 registry: 未设置"),
                Err(e) => fail(format!("读取失败: {}", e)),
            },
        },
        NodeCmd::Versions => {
            let info = devenv_core::api::list_versions();
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
            let version = match version {
                // 参数为纯数字 → 按 `node versions` 可用列表序号；否则当作版本号
                Some(v) if v.chars().all(|c| c.is_ascii_digit()) => {
                    let info = devenv_core::api::list_versions();
                    let n = info.available.len().min(20);
                    match arg_to_index(&v, n) {
                        Some(i) => info.available[i].clone(),
                        None => fail(format!("序号 {v} 超出范围（可选 1..{n}，先看 `devenv node versions`）")),
                    }
                }
                Some(v) => v,
                None => {
                    let info = devenv_core::api::list_versions();
                    let n = info.available.len().min(20);
                    println!("可用 Node 版本（每大版本最新，前 {}）:", n);
                    match pick("选择要安装的版本", &info.available[..n], |v| v.clone()) {
                        Some(v) => v.clone(),
                        None => fail("已取消".to_string()),
                    }
                }
            };
            println!("安装 Node {} ...", version);
            match devenv_core::api::install_specific(&version, |stage| {
                println!("  [{}]", stage);
            }) {
                Ok(dir) => println!("✓ 安装完成: {}", dir),
                Err(e) => fail(format!("安装失败: {}", e)),
            }
        }
        NodeCmd::Switch { version } => {
            let version = match version {
                Some(v) if v.chars().all(|c| c.is_ascii_digit()) => {
                    let info = devenv_core::api::list_versions();
                    let installed: Vec<String> = info.installed.iter().map(|x| x.version.clone()).collect();
                    match arg_to_index(&v, installed.len()) {
                        Some(i) => installed[i].clone(),
                        None => fail(format!("序号 {v} 超出范围（已装 {} 个，先看 `devenv node versions`）", installed.len())),
                    }
                }
                Some(v) => v,
                None => {
                    let info = devenv_core::api::list_versions();
                    let installed: Vec<String> = info.installed.iter().map(|v| v.version.clone()).collect();
                    println!("已安装的 Node 版本:");
                    match pick("选择要切换到的版本", &installed, |v| v.clone()) {
                        Some(v) => v.clone(),
                        None => fail("已取消".to_string()),
                    }
                }
            };
            match devenv_core::api::switch_version(version.clone()) {
                Ok(dir) => {
                    println!("✓ 已切换到 {}（{}）", version, dir);
                    println!("请重开终端使 PATH 生效");
                }
                Err(e) => fail(e),
            }
        }
        NodeCmd::Uninstall { version } => {
            let version = match version {
                Some(v) if v.chars().all(|c| c.is_ascii_digit()) => {
                    let info = devenv_core::api::list_versions();
                    let installed: Vec<String> = info.installed.iter().map(|x| x.version.clone()).collect();
                    match arg_to_index(&v, installed.len()) {
                        Some(i) => installed[i].clone(),
                        None => fail(format!("序号 {v} 超出范围（已装 {} 个，先看 `devenv node versions`）", installed.len())),
                    }
                }
                Some(v) => v,
                None => {
                    let info = devenv_core::api::list_versions();
                    let installed: Vec<String> = info.installed.iter().map(|v| v.version.clone()).collect();
                    println!("已安装的 Node 版本:");
                    match pick("选择要卸载的版本", &installed, |v| v.clone()) {
                        Some(v) => v.clone(),
                        None => fail("已取消".to_string()),
                    }
                }
            };
            match devenv_core::api::uninstall_version(version.clone()) {
                Ok(dir) => println!("✓ 已卸载 {}（{}）", version, dir),
                Err(e) => fail(e),
            }
        }
        NodeCmd::Project { path } => run_node_project(path),
        NodeCmd::Test { url } => run_node_test(url),
        NodeCmd::Bun { action } => run_bun(action),
        NodeCmd::Accounts => run_npm_accounts(),
        NodeCmd::Publish { dir, registry, build, yes } => run_npm_publish(dir, registry, build, yes),
    }
}

// ── Node：项目 / 测速 / bun / 发布账号 / 发布 ─────────────────────

/// 询问项目目录（默认当前目录）。
fn ask_project_dir() -> String {
    match prompt_line("项目目录（直接回车 = 当前目录）: ") {
        Some(s) if !s.is_empty() => s,
        _ => ".".to_string(),
    }
}

fn run_node_project(path: Option<String>) {
    let dir = path.unwrap_or_else(ask_project_dir);
    let info = match devenv_core::api::inspect_project(dir.clone()) {
        Ok(i) => i,
        Err(e) => fail(format!("解析项目失败: {}", e)),
    };
    println!("项目: {}", info.path);
    println!("  .nvmrc      : {}", if info.has_nvmrc { "有" } else { "无" });
    println!("  package.json: {}", if info.has_package_json { "有" } else { "无" });
    println!("  .npmrc      : {}", if info.has_npmrc { "有" } else { "无" });
    match (&info.node_requirement, &info.requirement_source) {
        (Some(req), Some(src)) => println!("  要求 Node   : {}（来自 {}）", req, src),
        (Some(req), None) => println!("  要求 Node   : {}", req),
        _ => println!("  要求 Node   : 未声明"),
    }
    if let Some(pm) = &info.package_manager {
        println!("  包管理器    : {}", pm);
    }
    if let Some(r) = &info.registry {
        println!("  项目 registry: {}", r);
    }
    println!("  当前 Node   : {}", info.current_node.as_deref().unwrap_or("未安装/未切换"));
    if info.node_requirement.is_some() {
        println!("  要求版本已装: {}", if info.node_installed { "是 ✓" } else { "否（需安装）" });
        if !prompt_yes("是否应用（安装/切换到要求版本）？", true) {
            println!("已取消");
            return;
        }
        match devenv_core::api::apply_project(dir) {
            Ok(msg) => println!("✓ {}", msg.replace('\n', "\n  ")),
            Err(e) => fail(format!("应用失败: {}", e)),
        }
    }
}

fn run_node_test(url: Option<String>) {
    // 无 URL：测当前 registry + 常用源
    let mut targets: Vec<String> = Vec::new();
    if let Some(u) = url {
        targets.push(u);
    } else {
        if let Ok(Some(cur)) = devenv_core::api::read_registry() {
            targets.push(cur);
        }
        for u in ["https://registry.npmmirror.com", "https://registry.npmjs.org"] {
            if !targets.iter().any(|t| t == u) {
                targets.push(u.to_string());
            }
        }
    }
    for u in targets {
        let r = devenv_core::api::test_registry(u.clone());
        if r.reachable {
            println!(
                "  [✓] {:<40} HTTP {} · {}ms{}",
                u,
                r.status.unwrap_or(0),
                r.latency_ms.unwrap_or(0),
                if r.needs_auth { " · 需认证" } else { "" }
            );
        } else {
            println!(
                "  [✗] {:<40} {} ({})",
                u,
                r.error.as_deref().unwrap_or("不可达"),
                if r.cert_issue { "证书不受信任" } else { "" }
            );
        }
    }
}

fn bun_menu() -> [MenuItem; 5] {
    [
        ("status", "查看 bun 状态"),
        ("install", "安装 / 升级 bun（可指定版本）"),
        ("upgrade", "升级 bun 到最新"),
        ("uninstall", "卸载托管 bun"),
        ("back", "返回上一级"),
    ]
}

fn run_bun(action: Option<BunAction>) {
    match action {
        Some(a) => run_bun_action(a),
        None => menu_loop("bun 运行时", &bun_menu(), |key| {
            let a = match key {
                "status" => BunAction::Status,
                "install" => BunAction::Install { version: None },
                "upgrade" => BunAction::Upgrade,
                "uninstall" => BunAction::Uninstall,
                _ => return,
            };
            run_bun_action(a);
        }),
    }
}

fn run_bun_action(action: BunAction) {
    use devenv_core::api;
    match action {
        BunAction::Status => {
            let info = api::bun_info();
            println!("bun 状态:");
            println!("  安装: {}", if info.installed { "已安装 ✓" } else { "未安装 ✗" });
            println!("  版本: {}", info.version.as_deref().unwrap_or("-"));
            println!("  路径: {}", info.bin_path.as_deref().unwrap_or("-"));
            println!("  数据目录: {}", info.data_dir.as_deref().unwrap_or("-"));

            // WSL 下：shell 里的 bun 可能是 Windows 版透过 interop 过来的（跨边界、读 Windows 证书库）。
            if let Some(ip) = api::bun_interop_shadow() {
                println!();
                println!("⚠  检测到当前 shell 的 bun 是 WSL interop 过来的 Windows 版：");
                println!("    {}", ip);
                println!("    它跑在 Windows 侧：写 node_modules 是 Windows 进程跨边界（慢、权限/换行容易出问题），");
                println!("    且使用 Windows 证书库，信任不到 WSL 里配置的私有 CA。");
                println!("    工具未托管 Linux 原生 bun，`devenv node bun install` 可解决。");
                if !info.installed {
                    match pick("是否安装 Linux 原生 bun？", &["安装 Linux 原生 bun", "暂不处理"], |s| s.to_string()) {
                        Some(opt) if *opt == "安装 Linux 原生 bun" => {
                            println!("安装 bun latest ...");
                            match api::install_bun(None, |stage| println!("  [{}]", stage)) {
                                Ok(msg) => println!("✓ {}", msg),
                                Err(e) => fail(format!("安装失败: {}", e)),
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        BunAction::Install { version } => {
            let version = match version {
                Some(v) => Some(v),
                None => match prompt_line("bun 版本（直接回车 = 最新）: ") {
                    Some(s) if !s.is_empty() => Some(s),
                    _ => None,
                },
            };
            println!("安装 bun {} ...", version.as_deref().unwrap_or("latest"));
            match api::install_bun(version, |stage| println!("  [{}]", stage)) {
                Ok(msg) => println!("✓ {}", msg),
                Err(e) => fail(format!("安装失败: {}", e)),
            }
        }
        BunAction::Upgrade => match api::upgrade_bun() {
            Ok(msg) => println!("✓ {}", msg),
            Err(e) => fail(format!("升级失败: {}", e)),
        },
        BunAction::Uninstall => {
            if !prompt_yes("确认卸载托管 bun？", false) {
                println!("已取消");
                return;
            }
            match api::uninstall_bun() {
                Ok(msg) => println!("✓ {}", msg),
                Err(e) => fail(format!("卸载失败: {}", e)),
            }
        }
    }
}

/// npm 发布账号：扫描 ~/.npmrc 已有凭据，纳入管理 / 切换激活。
fn run_npm_accounts() {
    use devenv_core::api;
    let managed = api::list_accounts();
    println!("已管理账号（{} 个，激活: {}）:",
        managed.accounts.len(),
        managed.active.as_deref().unwrap_or("无"));
    for a in &managed.accounts {
        let act = if managed.active.as_deref() == Some(a.id.as_str()) { "  <- 激活" } else { "" };
        println!("  [{}] {}  ({}){}", a.id, a.name, a.registry, act);
    }

    let creds = api::scan_npmrc_credentials();
    if creds.is_empty() {
        println!("\n~/.npmrc 未发现外部凭据。");
        println!("请先在终端执行 `npm login --registry=<源>`，或手动编辑 ~/.npmrc 写入 _authToken。");
        return;
    }
    println!("\n~/.npmrc 中的外部凭据（{} 个，未纳入管理）:", creds.len());
    for (i, c) in creds.iter().enumerate() {
        println!(
            "  [{}] {}  [{}] {}{}",
            i + 1,
            c.registry,
            c.auth_type,
            if c.reachable { "可达 ✓" } else { "不可达/未验证" },
            c.username.as_deref().map(|u| format!(" user={u}")).unwrap_or_default()
        );
    }
    match prompt_line("\n输入序号纳入管理（q 取消）: ") {
        Some(raw) if !raw.is_empty() && !raw.eq_ignore_ascii_case("q") => {
            if let Ok(n) = raw.parse::<usize>() {
                if n >= 1 && n <= creds.len() {
                    let reg = creds[n - 1].registry.clone();
                    match api::import_npmrc_credential(reg.clone()) {
                        Ok(f) => {
                            println!("✓ 已纳入管理: {}（共 {} 个账号）", reg, f.accounts.len());
                            // 若只有这一个账号，提示可切换
                            if let Some(a) = f.accounts.last() {
                                if prompt_yes(&format!("切换激活到该账号（{}）？", a.name), true) {
                                    match api::switch_account(a.id.clone()) {
                                        Ok(_) => println!("✓ 已切换激活（_authToken 写入 ~/.npmrc 全局）"),
                                        Err(e) => eprintln!("切换失败: {}", e),
                                    }
                                }
                            }
                        }
                        Err(e) => fail(format!("纳入管理失败: {}", e)),
                    }
                    return;
                }
            }
            eprintln!("无效序号: {}", raw);
        }
        _ => println!("已取消"),
    }
}

/// 读取根目录单包（package.json 直接在 dir 下）。共享扫描器只看 packages/ 子目录，
/// CLI 这里补上单包场景。
fn read_root_package(root: &str) -> Option<devenv_core::PublishPackage> {
    let pj = Path::new(root).join("package.json");
    let content = std::fs::read_to_string(&pj).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    let name = v["name"].as_str()?.to_string();
    let version = v["version"].as_str()?.to_string();
    if name.is_empty() || version.is_empty() {
        return None;
    }
    let dir = Path::new(root).canonicalize().unwrap_or_else(|_| PathBuf::from(root));
    Some(devenv_core::PublishPackage {
        name,
        version,
        dir: dir.to_string_lossy().to_string(),
        has_dist: dir.join("dist").is_dir(),
        is_private: v["private"].as_bool().unwrap_or(false),
    })
}

/// 发布 npm 包：扫描 → dry-run → 确认 → 真发。
fn run_npm_publish(dir: Option<String>, registry: Option<String>, build: bool, yes: bool) {
    use devenv_core::api;
    let root = dir.unwrap_or_else(|| ".".to_string());
    let mut pkgs = match api::list_publish_packages(root.clone()) {
        Ok(p) => p,
        Err(e) => fail(format!("扫描包失败: {}", e)),
    };
    // CLI 额外支持单包（根目录 package.json）；共享扫描器只看 monorepo 的 packages/ 子目录。
    if let Some(root_pkg) = read_root_package(&root) {
        if !pkgs.iter().any(|p| p.dir == root_pkg.dir) {
            pkgs.insert(0, root_pkg);
        }
    }
    if pkgs.is_empty() {
        fail("未找到可发布的包（需要含 name/version 的 package.json，或 packages/* 子目录）".to_string());
    }
    println!("发现 {} 个包:", pkgs.len());
    for (i, p) in pkgs.iter().enumerate() {
        println!(
            "  [{}] {}@{}  {}{}",
            i + 1,
            p.name,
            p.version,
            if p.is_private { "[private] " } else { "" },
            if p.has_dist { "有 dist" } else { "无 dist" }
        );
    }
    // 选择要发布的包（默认全部非 private）
    let chosen: Vec<String> = match prompt_line("输入要发布的序号（逗号分隔，直接回车 = 全部非 private）: ") {
        Some(s) if !s.is_empty() => s
            .split([',', ' '])
            .filter_map(|t| t.trim().parse::<usize>().ok())
            .filter(|n| *n >= 1 && *n <= pkgs.len())
            .map(|n| pkgs[n - 1].dir.clone())
            .collect(),
        _ => pkgs.iter().filter(|p| !p.is_private).map(|p| p.dir.clone()).collect(),
    };
    if chosen.is_empty() {
        fail("没有选中任何可发布的包（private 包需显式选序号）".to_string());
    }

    let reg = registry.unwrap_or_else(|| {
        match prompt_line("发布源 registry（直接回车 = https://registry.npmjs.org）: ") {
            Some(s) if !s.is_empty() => s,
            _ => "https://registry.npmjs.org".to_string(),
        }
    });
    let build = build || prompt_yes("发布前执行 npm run build？", false);

    // 1) dry-run 预检
    println!("\n— 先执行 dry-run 预检 —");
    match api::publish_packages(chosen.clone(), reg.clone(), build, None, Some(true)) {
        Ok(results) => {
            let mut all_ok = true;
            for r in &results {
                let mark = if r.ok { "✓" } else { "✗" };
                for (i, line) in r.message.lines().enumerate() {
                    if i == 0 {
                        println!("  {} {}: {}", mark, r.name, line);
                    } else {
                        println!("      {}", line);
                    }
                }
                if !r.ok {
                    all_ok = false;
                }
            }
            if !all_ok {
                fail("dry-run 存在失败项，已中止（未真实发布）".to_string());
            }
        }
        Err(e) => fail(format!("dry-run 失败: {}", e)),
    }

    // 2) 确认后真发
    if !yes && !prompt_yes(&format!("\n确认真实发布 {} 个包到 {}？此操作不可逆！", chosen.len(), reg), false) {
        println!("已取消（未发布）");
        return;
    }
    publish_for_real(chosen, reg, build, None);
}

/// 真实发布；遇 needs_otp 提示输入 OTP 后重试。
fn publish_for_real(packages: Vec<String>, reg: String, build: bool, otp: Option<String>) {
    use devenv_core::api;
    match api::publish_packages(packages.clone(), reg.clone(), build, otp.clone(), Some(false)) {
        Ok(results) => {
            let mut need_otp: Vec<String> = Vec::new();
            for r in &results {
                let mark = if r.ok { "✓" } else { "✗" };
                for (i, line) in r.message.lines().enumerate() {
                    if i == 0 {
                        println!("  {} {}: {}", mark, r.name, line);
                    } else {
                        println!("      {}", line);
                    }
                }
                if r.needs_otp {
                    need_otp.push(r.name.clone());
                }
                if let Some(h) = &r.hint {
                    println!("      提示: {} ({})", h.text, h.url);
                }
            }
            if !need_otp.is_empty() && otp.is_none() {
                match prompt_line("检测到 2FA，输入 OTP 验证码（q 取消）: ") {
                    Some(code) if !code.is_empty() && !code.eq_ignore_ascii_case("q") => {
                        publish_for_real(packages, reg, build, Some(code));
                    }
                    _ => println!("已取消（未发布需 OTP 的包）"),
                }
            }
        }
        Err(e) => fail(format!("发布失败: {}", e)),
    }
}

// ── Python ──────────────────────────────────────────────────────

fn run_python(cmd: PythonCmd) {
    match cmd {
        PythonCmd::Status => {
            let info = devenv_core::api::detect_python();
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
            Some(url) => match devenv_core::api::set_pip_index(url.clone()) {
                Ok(r) => println!("pip index 已设置为 {}\n写入: {}", url, r.path),
                Err(e) => fail(format!("设置 pip 镜像失败: {}", e)),
            },
            None => match devenv_core::api::read_pip_index() {
                Ok(Some(r)) => println!("当前 pip index: {}", r),
                Ok(None) => println!("当前 pip index: 未设置"),
                Err(e) => fail(format!("读取失败: {}", e)),
            },
        },
        PythonCmd::Versions => {
            let info = devenv_core::api::list_python_versions();
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
            let version = match version {
                // 参数为纯数字 → 按 `python versions` 可用列表序号；否则当作版本号
                Some(v) if v.chars().all(|c| c.is_ascii_digit()) => {
                    let info = devenv_core::api::list_python_versions();
                    let n = info.available.len().min(20);
                    match arg_to_index(&v, n) {
                        Some(i) => info.available[i].clone(),
                        None => fail(format!("序号 {v} 超出范围（可选 1..{n}，先看 `devenv python versions`）")),
                    }
                }
                Some(v) => v,
                None => {
                    let info = devenv_core::api::list_python_versions();
                    let n = info.available.len().min(20);
                    println!("可用 Python 版本（每小版本最新，前 {}）:", n);
                    match pick("选择要安装的版本", &info.available[..n], |v| v.clone()) {
                        Some(v) => v.clone(),
                        None => fail("已取消".to_string()),
                    }
                }
            };
            println!("安装 Python {} ...", version);
            match devenv_core::api::install_python_specific(&version, |stage| {
                println!("  [{}]", stage);
            }) {
                Ok(dir) => println!("✓ 安装完成: {}", dir),
                Err(e) => fail(format!("安装失败: {}", e)),
            }
        }
        PythonCmd::Switch { version } => {
            let version = match version {
                Some(v) if v.chars().all(|c| c.is_ascii_digit()) => {
                    let info = devenv_core::api::list_python_versions();
                    let installed: Vec<String> = info.installed.iter().map(|x| x.version.clone()).collect();
                    match arg_to_index(&v, installed.len()) {
                        Some(i) => installed[i].clone(),
                        None => fail(format!("序号 {v} 超出范围（已装 {} 个，先看 `devenv python versions`）", installed.len())),
                    }
                }
                Some(v) => v,
                None => {
                    let info = devenv_core::api::list_python_versions();
                    let installed: Vec<String> = info.installed.iter().map(|v| v.version.clone()).collect();
                    println!("已安装的 Python 版本:");
                    match pick("选择要切换到的版本", &installed, |v| v.clone()) {
                        Some(v) => v.clone(),
                        None => fail("已取消".to_string()),
                    }
                }
            };
            match devenv_core::api::switch_python_version(version.clone()) {
                Ok(dir) => {
                    println!("✓ 已切换到 {}（{}）", version, dir);
                    println!("请重开终端使 PATH 生效");
                }
                Err(e) => fail(e),
            }
        }
        PythonCmd::Uninstall { version } => {
            let version = match version {
                Some(v) if v.chars().all(|c| c.is_ascii_digit()) => {
                    let info = devenv_core::api::list_python_versions();
                    let installed: Vec<String> = info.installed.iter().map(|x| x.version.clone()).collect();
                    match arg_to_index(&v, installed.len()) {
                        Some(i) => installed[i].clone(),
                        None => fail(format!("序号 {v} 超出范围（已装 {} 个，先看 `devenv python versions`）", installed.len())),
                    }
                }
                Some(v) => v,
                None => {
                    let info = devenv_core::api::list_python_versions();
                    let installed: Vec<String> = info.installed.iter().map(|v| v.version.clone()).collect();
                    println!("已安装的 Python 版本:");
                    match pick("选择要卸载的版本", &installed, |v| v.clone()) {
                        Some(v) => v.clone(),
                        None => fail("已取消".to_string()),
                    }
                }
            };
            match devenv_core::api::uninstall_python_version(version.clone()) {
                Ok(dir) => println!("✓ 已卸载 {}（{}）", version, dir),
                Err(e) => fail(e),
            }
        }
        PythonCmd::Project { path } => run_python_project(path),
        PythonCmd::Test { url } => run_python_test(url),
        PythonCmd::Pkg { action } => run_py_pkg(action),
    }
}

// ── Python：项目 / 测速 / 包管理器 ───────────────────────────────

fn run_python_project(path: Option<String>) {
    let dir = path.unwrap_or_else(ask_project_dir);
    let info = match devenv_core::api::inspect_python_project(dir.clone()) {
        Ok(i) => i,
        Err(e) => fail(format!("解析项目失败: {}", e)),
    };
    println!("项目: {}", info.path);
    println!("  .python-version: {}", if info.has_python_version_file { "有" } else { "无" });
    println!("  pyproject.toml : {}", if info.has_pyproject_toml { "有" } else { "无" });
    println!("  runtime.txt    : {}", if info.has_runtime_txt { "有" } else { "无" });
    println!("  Pipfile        : {}", if info.has_pipfile { "有" } else { "无" });
    match (&info.python_requirement, &info.requirement_source) {
        (Some(req), Some(src)) => println!("  要求 Python    : {}（来自 {}）", req, src),
        (Some(req), None) => println!("  要求 Python    : {}", req),
        _ => println!("  要求 Python    : 未声明"),
    }
    if let Some(t) = &info.pkg_tool {
        println!("  打包工具       : {}", t);
    }
    if let Some(i) = &info.index {
        println!("  项目 index     : {}", i);
    }
    println!("  当前 Python    : {}", info.current_python.as_deref().unwrap_or("未安装/未切换"));
    if info.python_requirement.is_some() {
        println!("  要求版本已装   : {}", if info.python_installed { "是 ✓" } else { "否（需安装）" });
        if !prompt_yes("是否应用（安装/切换到要求版本）？", true) {
            println!("已取消");
            return;
        }
        match devenv_core::api::apply_python_project(dir) {
            Ok(msg) => println!("✓ {}", msg.replace('\n', "\n  ")),
            Err(e) => fail(format!("应用失败: {}", e)),
        }
    }
}

fn run_python_test(url: Option<String>) {
    let mut targets: Vec<String> = Vec::new();
    if let Some(u) = url {
        targets.push(u);
    } else {
        if let Ok(Some(cur)) = devenv_core::api::read_pip_index() {
            targets.push(cur);
        }
        for u in ["https://pypi.tuna.tsinghua.edu.cn/simple", "https://pypi.org/simple"] {
            if !targets.iter().any(|t| t == u) {
                targets.push(u.to_string());
            }
        }
    }
    for u in targets {
        let r = devenv_core::api::test_pypi(u.clone());
        if r.reachable {
            println!(
                "  [✓] {:<45} HTTP {} · {}ms",
                u,
                r.status.unwrap_or(0),
                r.latency_ms.unwrap_or(0)
            );
        } else {
            println!("  [✗] {:<45} {}", u, r.error.as_deref().unwrap_or("不可达"));
        }
    }
}

fn py_pkg_menu() -> [MenuItem; 3] {
    [
        ("list", "查看包管理器状态"),
        ("install", "安装 uv / poetry / pipenv（到共享 ~/.local/bin）"),
        ("back", "返回上一级"),
    ]
}

fn run_py_pkg(action: Option<PyPkgAction>) {
    match action {
        Some(a) => run_py_pkg_action(a),
        None => menu_loop("Python 包管理器", &py_pkg_menu(), |key| {
            let a = match key {
                "list" => PyPkgAction::List,
                "install" => PyPkgAction::Install { name: None, version: None },
                _ => return,
            };
            run_py_pkg_action(a);
        }),
    }
}

fn run_py_pkg_action(action: PyPkgAction) {
    use devenv_core::api;
    match action {
        PyPkgAction::List => {
            let tools = api::list_python_pkg_managers();
            println!("Python 包管理器:");
            for t in &tools {
                println!(
                    "  [{}] {:<7} {:<10} {}",
                    if t.installed { "✓" } else { "✗" },
                    t.name,
                    t.version.as_deref().unwrap_or("-"),
                    t.bin_path.as_deref().unwrap_or("-")
                );
            }
        }
        PyPkgAction::Install { name, version } => {
            // 可装：uv / poetry / pipenv（pip 内置、conda 需 Miniconda，后端会拒绝）
            let installable = ["uv", "poetry", "pipenv"];
            let name = match name {
                Some(n) => n,
                None => {
                    println!("可安装的包管理器:");
                    match pick("选择要安装的工具", &installable, |n| n.to_string()) {
                        Some(n) => n.to_string(),
                        None => fail("已取消".to_string()),
                    }
                }
            };
            let version = match version {
                Some(v) => Some(v),
                None => match prompt_line("版本（直接回车 = 最新）: ") {
                    Some(s) if !s.is_empty() => Some(s),
                    _ => None,
                },
            };
            println!("安装 {} {} ...", name, version.as_deref().unwrap_or("latest"));
            match api::install_python_pkg_manager(name.clone(), version) {
                Ok(msg) => println!("✓ {}", msg),
                Err(e) => fail(format!("安装失败: {}", e)),
            }
        }
    }
}

// ── Git ──────────────────────────────────────────────────────────

fn git_service() -> devenv_core::gitcore::service::Service {
    match devenv_core::gitcore::service::Service::new(None) {
        Ok(s) => s,
        Err(e) => {
            #[cfg(target_os = "linux")]
            eprintln!("\n安装提示: {}", devenv_core::linux::git_install_hint());
            fail(format!("Git 服务初始化失败: {}", e));
        }
    }
}

/// 列出账号并读取一个数字选择，返回所选账号（克隆）。
fn pick_account(svc: &mut devenv_core::gitcore::service::Service) -> Option<devenv_core::gitcore::account::Account> {
    let accounts = svc.list_accounts().ok()?;
    if accounts.is_empty() {
        println!("暂无 Git 账号，请先用 `devenv git add` 新增");
        return None;
    }
    println!("Git 账号（{} 个）:", accounts.len());
    pick("选择账号", &accounts, |a| {
        format!("{:<20} {}  ({} <{}>)", a.alias, a.host_name, a.git_name, a.git_email)
    })
    .cloned()
}

/// 交互式收集账号字段。`existing` 为 Some 时进入编辑模式（回车保留原值）。
fn prompt_account(
    existing: Option<&devenv_core::gitcore::account::Account>,
) -> Option<devenv_core::gitcore::account::Account> {
    use devenv_core::gitcore::account::Account;
    // 编辑模式下在提示里展示当前值（回车保留）；新增模式无默认展示。
    let d = |v: &str| if existing.is_some() { format!(" [{}]", v) } else { String::new() };
    let base = existing.cloned().unwrap_or_else(|| Account {
        alias: String::new(),
        host_name: String::new(),
        port: "22".to_string(),
        ssh_user: "git".to_string(),
        key_path: None,
        git_name: String::new(),
        git_email: String::new(),
        start: -1,
        end: -1,
        comment_start: -1,
    });

    let ask = |prompt: String, cur: &str| -> Option<String> {
        let line = prompt_line(&prompt)?;
        Some(if line.is_empty() { cur.to_string() } else { line })
    };

    let alias = ask(format!("Host 别名（如 github-work）{}: ", d(&base.alias)), &base.alias)?;
    let host_name = ask(format!("主机地址 HostName（如 github.com）{}: ", d(&base.host_name)), &base.host_name)?;
    let ssh_user = ask(format!("SSH 用户{}: ", d(&base.ssh_user)), &base.ssh_user)?;
    let port = ask(format!("SSH 端口{}: ", d(&base.port)), &base.port)?;
    let git_name = ask(format!("Git 用户名 user.name{}: ", d(&base.git_name)), &base.git_name)?;
    let git_email = ask(format!("Git 邮箱 user.email{}: ", d(&base.git_email)), &base.git_email)?;

    if alias.is_empty() || host_name.is_empty() || git_name.is_empty() || git_email.is_empty() {
        eprintln!("别名 / 主机地址 / 用户名 / 邮箱不能为空");
        return None;
    }

    // 私钥路径：留空用默认 ~/.ssh/id_rsa_<alias>（由 service 归一化）。
    let existing_key = base.key_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    let key_prompt = if existing.is_some() && !existing_key.is_empty() {
        format!("私钥路径{}（留空保留）: ", d(&existing_key))
    } else {
        format!("私钥路径（留空用默认 ~/.ssh/id_rsa_{}）: ", alias)
    };
    let key_input = ask(key_prompt, &existing_key)?;
    let key_path = if key_input.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(key_input))
    };

    Some(Account {
        alias,
        host_name,
        port: if port.is_empty() { "22".to_string() } else { port },
        ssh_user: if ssh_user.is_empty() { "git".to_string() } else { ssh_user },
        key_path,
        git_name,
        git_email,
        start: -1,
        end: -1,
        comment_start: -1,
    })
}

/// 询问是否生成 SSH 密钥（默认是）。
fn prompt_yes(prompt: &str, default_yes: bool) -> bool {
    let hint = if default_yes { " [Y/n] " } else { " [y/N] " };
    match prompt_line(&format!("{}{}", prompt, hint)) {
        Some(line) if line.is_empty() => default_yes,
        Some(line) => matches!(line.chars().next(), Some(c) if c == 'y' || c == 'Y'),
        None => false,
    }
}

fn run_git(cmd: GitCmd) {
    let mut svc = git_service();
    match cmd {
        GitCmd::Status => {
            let info = devenv_core::api::detect_git();
            println!("Git 环境体检:");
            println!("  Git    : {}", if info.installed {
                format!("已安装 ✓  {}", info.version.as_deref().unwrap_or(""))
            } else { "未安装 ✗".to_string() });
            if let Some(p) = &info.path {
                println!("           路径: {}", p);
            }
            println!("  OpenSSH: {}", if info.ssh_installed { "已安装 ✓" } else { "未安装 ✗" });
            #[cfg(not(target_os = "linux"))]
            if info.managed {
                println!("  托管   : 本工具托管的 MinGit（可用 `devenv git uninstall` 卸载）");
            }
            if !info.installed {
                #[cfg(target_os = "linux")]
                {
                    println!("\n安装提示（Linux 由系统包管理器托管，需 sudo，本工具不代为执行）:");
                    println!("  {}", devenv_core::linux::git_install_hint());
                }
                #[cfg(not(target_os = "linux"))]
                println!("\n可执行 `devenv git install` 由本工具托管安装 MinGit（免管理员）。");
            }
        }
        GitCmd::Accounts => match svc.list_accounts() {
            Ok(accounts) => {
                if accounts.is_empty() {
                    println!("暂无 Git 账号（用 `devenv git add` 新增）");
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
        GitCmd::Add => {
            let account = match prompt_account(None) {
                Some(a) => a,
                None => fail("已取消新增账号".to_string()),
            };
            let mut gen_key = prompt_yes("是否自动生成 SSH 密钥对（rsa 4096）？", true);
            // SSH 账号必须有可用私钥：用户选择不生成、但目标路径又没有现成密钥时，
            // 直接代生成（否则 service 会因私钥缺失而拒绝写入账号）。
            if !gen_key {
                let kp = account
                    .key_path
                    .clone()
                    .unwrap_or_else(|| svc.default_key_path(&account.alias));
                if !kp.exists() {
                    println!("目标私钥 {} 不存在，改为自动生成。", kp.display());
                    gen_key = true;
                }
            }
            match svc.add_account(&account, gen_key) {
                Ok(a) => {
                    println!("✓ 已新增账号「{}」（{} <{}>）", a.alias, a.git_name, a.git_email);
                    if let Some(kp) = &a.key_path {
                        println!("  私钥: {}", kp.display());
                    }
                    after_account_write(&mut svc, &a.alias);
                }
                Err(e) => fail(format!("新增账号失败: {}", e)),
            }
        }
        GitCmd::Edit { alias } => {
            let source = match resolve_alias(&mut svc, alias) {
                Some(a) => a,
                None => fail("已取消".to_string()),
            };
            let existing = match svc.get_account(&source) {
                Ok(a) => a,
                Err(e) => fail(format!("账号「{}」不存在: {}", source, e)),
            };
            println!("编辑账号「{}」（直接回车保留原值）:", source);
            let account = match prompt_account(Some(&existing)) {
                Some(a) => a,
                None => fail("已取消编辑".to_string()),
            };
            let gen_key = prompt_yes("是否重新生成 SSH 密钥对（覆盖旧密钥）？", false);
            match svc.update_account(&source, &account, gen_key) {
                Ok(a) => {
                    println!("✓ 已更新账号「{}」", a.alias);
                    after_account_write(&mut svc, &a.alias);
                }
                Err(e) => fail(format!("更新账号失败: {}", e)),
            }
        }
        GitCmd::Delete { alias, purge } => {
            let alias = match resolve_alias(&mut svc, alias) {
                Some(a) => a,
                None => fail("已取消".to_string()),
            };
            let purge = if purge {
                true
            } else {
                prompt_yes(&format!("同时删除账号「{}」的私钥/公钥文件？", alias), false)
            };
            match svc.delete_account(&alias, purge) {
                Ok(()) => println!("✓ 已删除账号「{}」{}", alias, if purge { "（含密钥文件）" } else { "" }),
                Err(e) => fail(format!("删除失败: {}", e)),
            }
        }
        GitCmd::GenKey { alias, force } => {
            let alias = match resolve_alias(&mut svc, alias) {
                Some(a) => a,
                None => fail("已取消".to_string()),
            };
            let account = match svc.get_account(&alias) {
                Ok(a) => a,
                Err(e) => fail(format!("账号「{}」不存在: {}", alias, e)),
            };
            let key_path = account.key_path.clone().unwrap_or_else(|| svc.default_key_path(&alias));
            let exists = key_path.exists();
            if exists && !force {
                eprintln!("私钥已存在: {}（加 --force 可重新生成）", key_path.display());
                std::process::exit(1);
            }
            let result = if exists {
                svc.regenerate_key_pair(Some(&key_path), &account.git_email)
            } else {
                svc.generate_key_pair(Some(&key_path), &account.git_email)
            };
            match result {
                Ok(p) => {
                    println!("✓ 已{}密钥: {}", if exists { "重新生成" } else { "生成" }, p.display());
                    after_account_write(&mut svc, &alias);
                }
                Err(e) => fail(format!("生成密钥失败: {}", e)),
            }
        }
        GitCmd::Switch { alias } => {
            let alias = match resolve_alias(&mut svc, alias) {
                Some(a) => a,
                None => fail("已取消".to_string()),
            };
            match svc.switch_identity(&alias, false, None) {
                Ok(a) => println!("✓ 已切换全局身份为「{}」（{} <{}>）", a.alias, a.git_name, a.git_email),
                Err(e) => fail(format!("切换失败: {}", e)),
            }
        }
        GitCmd::Key { alias } => {
            let alias = match resolve_alias(&mut svc, alias) {
                Some(a) => a,
                None => fail("已取消".to_string()),
            };
            match svc.get_account(&alias) {
                Ok(a) => {
                    let pub_key = svc.read_public_key(a.key_path.as_deref());
                    if pub_key.is_empty() {
                        println!("「{}」无公钥（可执行 `devenv git gen-key {}` 生成）", alias, alias);
                    } else {
                        println!("「{}」SSH 公钥:\n{}", alias, pub_key);
                        if let Ok(Some(url)) = svc.get_ssh_key_management_url(&alias) {
                            println!("\n添加到平台: {}", url);
                        }
                    }
                }
                Err(e) => fail(format!("账号「{}」不存在: {}", alias, e)),
            }
        }
        GitCmd::Test { alias } => {
            let alias = match resolve_alias(&mut svc, alias) {
                Some(a) => a,
                None => fail("已取消".to_string()),
            };
            match svc.test_connection(&alias) {
                Ok((success, output)) => {
                    println!("SSH 连接测试「{}」:", alias);
                    println!("  结果: {}", if success { "成功 ✓" } else { "失败 ✗" });
                    if !output.is_empty() {
                        println!("  {}", output.lines().next().unwrap_or(""));
                    }
                }
                Err(e) => fail(format!("测试失败: {}", e)),
            }
        }
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
        GitCmd::Versions => run_git_versions(),
        GitCmd::Install { n } => run_git_install(n),
        GitCmd::Uninstall => run_git_uninstall(),
    }
}

/// 新增/生成密钥后：展示公钥与平台添加地址，提醒上传。
fn after_account_write(svc: &mut devenv_core::gitcore::service::Service, alias: &str) {
    let Ok(account) = svc.get_account(alias) else { return };
    let pub_key = svc.read_public_key(account.key_path.as_deref());
    if !pub_key.is_empty() {
        println!("  公钥:\n  {}", pub_key);
    }
    if let Ok(Some(url)) = svc.get_ssh_key_management_url(alias) {
        println!("  请将公钥添加到平台: {}", url);
    }
}

/// `git versions`：Windows 列镜像最新稳定版；Linux 列系统包管理器安装方式。
fn run_git_versions() {
    #[cfg(target_os = "linux")]
    {
        println!("Linux 上 Git 由系统包管理器托管（本工具不代为下载安装）:");
        match devenv_core::linux::package_manager() {
            Some((pm, args)) => {
                println!("  检测到包管理器: {pm}");
                println!("  安装/升级 Git + OpenSSH:");
                println!("    sudo {pm} {args} git {}",
                    if pm == "dnf" { "openssh-clients" }
                    else if pm == "pacman" || pm == "zypper" { "openssh" }
                    else { "openssh-client" });
            }
            None => println!("  未检测到 apt/dnf/pacman/zypper/apk，请用发行版包管理器安装 git 与 openssh。"),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        println!("可安装的 Git for Windows（MinGit，镜像最新稳定版）:");
        let releases = devenv_core::api::git_versions();
        if releases.is_empty() {
            println!("  （获取版本列表失败，请检查网络）");
            return;
        }
        for (i, r) in releases.iter().enumerate() {
            println!("  [{}] Git {}  ({})", i + 1, r.version, r.tag);
        }
        println!("\n执行 `devenv git install` 后按数字选择安装。");
    }
}

/// `git install [序号]`：Windows 托管安装 MinGit；Linux 仅打印包管理器命令。
fn run_git_install(n: Option<usize>) {
    #[cfg(target_os = "linux")]
    {
        let _ = n;
        let info = devenv_core::api::detect_git();
        if info.installed {
            println!("Git 已安装: {}", info.version.as_deref().unwrap_or(""));
            return;
        }
        println!("Linux 上 Git 由系统包管理器托管，需 sudo，本工具不代为执行。");
        println!("请手动运行:\n  {}", devenv_core::linux::git_install_hint());
    }
    #[cfg(not(target_os = "linux"))]
    {
        let info = devenv_core::api::detect_git();
        if info.installed && info.managed {
            println!("已安装本工具托管的 MinGit: {}", info.version.as_deref().unwrap_or(""));
        }
        let releases = devenv_core::api::git_versions();
        if releases.is_empty() {
            fail("获取 Git 版本列表失败（检查网络）".to_string());
        }
        // 带序号直接选；否则进入数字菜单
        let chosen = match n {
            Some(i) if i >= 1 && i <= releases.len() => releases[i - 1].clone(),
            Some(i) => fail(format!("序号 {i} 超出范围（共 {} 个版本）", releases.len())),
            None => {
                println!("选择要安装的 Git 版本:");
                match pick("选择版本", &releases, |r| format!("Git {}  ({})", r.version, r.tag)) {
                    Some(r) => r.clone(),
                    None => fail("已取消".to_string()),
                }
            }
        };
        println!("安装 Git {} ...", chosen.version);
        match devenv_core::api::install_git(&chosen.version, &chosen.tag, |stage| {
            let label = match stage {
                "connecting" => "连接镜像",
                "downloading" => "下载 MinGit",
                "extracting" => "解压",
                "done" => "完成",
                _ => stage,
            };
            println!("  [{}]", label);
        }) {
            Ok(dir) => {
                println!("✓ 安装完成: {}", dir);
                println!("已写入用户 PATH，新开终端即可使用 git。");
            }
            Err(e) => fail(format!("安装失败: {}", e)),
        }
    }
}

/// `git uninstall`：仅 Windows 支持卸载托管 MinGit。
fn run_git_uninstall() {
    #[cfg(target_os = "linux")]
    {
        println!("Linux 上 Git 由系统包管理器托管，卸载请用：");
        match devenv_core::linux::package_manager() {
            Some(("apt-get", _)) => println!("  sudo apt-get remove git"),
            Some(("dnf", _)) => println!("  sudo dnf remove git"),
            Some(("pacman", _)) => println!("  sudo pacman -R git"),
            Some(("zypper", _)) => println!("  sudo zypper remove git"),
            Some(("apk", _)) => println!("  sudo apk del git"),
            _ => println!("  请使用发行版包管理器卸载 git。"),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        match devenv_core::api::uninstall_git() {
            Ok(dir) => println!("✓ 已卸载托管 MinGit（{}）", dir),
            Err(e) => fail(format!("卸载失败: {}", e)),
        }
    }
}

// ── Doctor（Q12）：环境体检，Linux 特有项 + 跨平台工具链 ─────────────

#[cfg(target_os = "linux")]
fn run_doctor() {
    use devenv_core::api;
    use devenv_core::linux::{dangling_symlinks, find_in_path, git_install_hint, install_hint_for, local_bin_in_path, rc_marker_exists};

    println!("DevEnvManager 环境体检 (Linux)");
    let mut issues: Vec<String> = Vec::new();

    // 1. 基础工具链
    println!("\n基础工具:");
    for name in ["git", "ssh", "curl", "tar"] {
        match find_in_path(name) {
            Some(p) => println!("  [✓] {name:<4} {}", p.display()),
            None => {
                println!("  [✗] {name:<4} 未安装");
                let hint = match name {
                    "git" => git_install_hint(),
                    "ssh" => install_hint_for(&["openssh-client"]),
                    _ => install_hint_for(&[name, "openssh-client"]),
                };
                issues.push(format!("安装 {name}：{hint}"));
            }
        }
    }

    // 2. 自管工具链
    println!("\n自管工具链:");
    let node = api::detect_node();
    let node_in_lb = devenv_core::linux::local_bin().join("node").exists();
    if node.installed {
        println!(
            "  [✓] node     {}  ({})",
            node.node_version.as_deref().unwrap_or("-"),
            node.real_node_exe.as_deref().unwrap_or("-")
        );
    } else if node_in_lb {
        println!("  [✗] node     已安装于 ~/.local/bin 但 PATH 未生效（重开终端）");
        issues.push("令 node 生效：重开终端，或 source ~/.bashrc / ~/.zshrc".to_string());
    } else {
        println!("  [✗] node     未安装（可执行: devenv node install <版本>）");
        issues.push("安装 Node：devenv node install v22.23.2".to_string());
    }
    let py = api::detect_python();
    let py_in_lb = devenv_core::linux::local_bin().join("python3").exists();
    if py.installed {
        println!(
            "  [✓] python3  {}  ({})",
            py.python_version.as_deref().unwrap_or("-"),
            py.real_python_exe.as_deref().unwrap_or("-")
        );
    } else if py_in_lb {
        println!("  [✗] python3  已安装于 ~/.local/bin 但 PATH 未生效（重开终端）");
        issues.push("令 python3 生效：重开终端，或 source ~/.bashrc / ~/.zshrc".to_string());
    } else {
        println!("  [✗] python3  未安装（可执行: devenv python install 3.13.15）");
        issues.push("安装 Python：devenv python install 3.13.15".to_string());
    }

    // 3. PATH / shell 配置
    println!("\nPATH 与 shell 配置:");
    if local_bin_in_path() {
        println!("  [✓] ~/.local/bin 已在 PATH");
    } else {
        println!("  [✗] ~/.local/bin 不在 PATH（重启终端或 source ~/.bashrc / ~/.zshrc）");
        issues.push("令 ~/.local/bin 生效：重开终端，或执行 source ~/.bashrc".to_string());
    }
    if rc_marker_exists() {
        println!("  [✓] shell 配置已注入 devenv PATH 块");
    } else if !local_bin_in_path() {
        println!("  [✗] shell 配置未注入（执行一次: devenv node switch <已装版本>）");
    }

    // 4. 软链健康
    let dangling = dangling_symlinks();
    if dangling.is_empty() {
        println!("  [✓] ~/.local/bin 软链无悬空");
    } else {
        println!("  [✗] 悬空软链: {}", dangling.join(", "));
        issues.push("清理悬空软链：删除 ~/.local/bin/{".to_string() + &dangling.join(", ") + "}");
    }

    if issues.is_empty() {
        println!("\n待办事项: 无，环境健康 ✓");
    } else {
        println!("\n待办事项（{} 项）:", issues.len());
        for (i, it) in issues.iter().enumerate() {
            println!("  {}. {}", i + 1, it);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn run_doctor() {
    use devenv_core::{api, gitcore::platform};
    println!("DevEnvManager 环境体检");
    println!("  （本命令面向 Linux 优化；Windows 请使用 GUI「环境诊断」页）\n");
    let env = |k: &str| std::env::var(k).ok();
    for name in ["git", "ssh"] {
        match platform::which(&env, name) {
            Some(p) => println!("  [✓] {name:<4} {}", p.display()),
            None => println!("  [✗] {name:<4} 未安装"),
        }
    }
    let node = api::detect_node();
    println!(
        "  [{}] node    {}",
        if node.installed { "✓" } else { "✗" },
        node.node_version.as_deref().unwrap_or("-")
    );
    let py = api::detect_python();
    println!(
        "  [{}] python  {}",
        if py.installed { "✓" } else { "✗" },
        py.python_version.as_deref().unwrap_or("-")
    );
}

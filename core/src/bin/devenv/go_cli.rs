use clap::Subcommand;
use devenv_core::go::{execute, Request};

#[derive(Subcommand, Clone)]
pub enum GoCmd {
    /// Go 环境诊断
    Status,
    /// 已安装及可用版本
    Versions,
    /// 安装并切换；省略版本时数字选择
    Install { version: Option<String> },
    /// 切换托管版本
    Switch { version: Option<String> },
    /// 卸载托管版本
    Uninstall { version: Option<String> },
    /// 修复托管版本 PATH
    FixPath,
    /// 查看代理；set <url> 设置 GOPROXY，--private 设置 GOPRIVATE
    Proxy {
        #[command(subcommand)]
        action: Option<ProxyCmd>,
    },
    /// 模块代理测速
    Test { url: Option<String> },
    /// 开发工具管理
    Tools {
        #[command(subcommand)]
        action: Option<ToolCmd>,
    },
    /// 查看项目要求；--apply 安装切换并下载依赖
    Project {
        path: Option<String>,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand, Clone)]
pub enum ProxyCmd {
    Set {
        url: String,
        #[arg(long)]
        private: Option<String>,
    },
}

#[derive(Subcommand, Clone)]
pub enum ToolCmd {
    List,
    Install {
        name: Option<String>,
        #[arg(long)]
        version: Option<String>,
    },
}

fn select_version(action: &str, arg: Option<String>) -> Option<String> {
    if let Some(arg) = &arg {
        if !arg.chars().all(|c| c.is_ascii_digit()) { return Some(arg.clone()); }
    }
    let data = execute(Request { action: "versions".into(), ..Default::default() }, |_| {})
        .unwrap_or_else(|e| super::fail(e));
    if let Some(error) = data["error"].as_str() { eprintln!("可用版本查询失败：{error}"); }
    let key = if action == "install" { "available" } else { "installed" };
    let versions = data[key].as_array().unwrap().iter().filter_map(|v| {
        v.as_str().or_else(|| v["version"].as_str()).map(str::to_string)
    }).collect::<Vec<_>>();
    if let Some(arg) = arg {
        return super::arg_to_index(&arg, versions.len()).map(|i| versions[i].clone());
    }
    super::pick("选择 Go 版本", &versions, Clone::clone).cloned()
}

pub fn run(action: Option<GoCmd>) {
    let Some(action) = action else {
        loop {
            let actions = ["环境诊断", "版本列表", "安装版本", "切换版本", "卸载版本", "代理配置", "代理测速", "工具管理", "项目管理", "修复 PATH"];
            let Some(choice) = super::pick("Go 环境管理", &actions, |s| s.to_string()) else { return; };
            let action = match *choice {
                "环境诊断" => GoCmd::Status,
                "版本列表" => GoCmd::Versions,
                "安装版本" => GoCmd::Install { version: None },
                "切换版本" => GoCmd::Switch { version: None },
                "卸载版本" => GoCmd::Uninstall { version: None },
                "代理配置" => {
                    let url = super::prompt_line("GOPROXY（留空查看）：").unwrap_or_default();
                    GoCmd::Proxy { action: if url.is_empty() { None } else { Some(ProxyCmd::Set { url, private: super::prompt_line("GOPRIVATE（留空清除）：") }) } }
                }
                "代理测速" => GoCmd::Test { url: super::prompt_line("代理 URL（留空测官方源）：").filter(|s| !s.is_empty()) },
                "工具管理" => {
                    let items = ["列出工具", "安装工具"];
                    let Some(item) = super::pick("工具操作", &items, |s| s.to_string()) else { continue; };
                    GoCmd::Tools { action: Some(if *item == "安装工具" { ToolCmd::Install { name: None, version: None } } else { ToolCmd::List }) }
                }
                "项目管理" => GoCmd::Project { path: Some(super::ask_project_dir()), apply: false },
                _ => GoCmd::FixPath,
            };
            run(Some(action));
        }
    };
    let mut request = Request::default();
    let version_action = match &action { GoCmd::Install { .. } => "install", GoCmd::Switch { .. } => "switch", _ => "uninstall" };
    match action {
        GoCmd::Status => request.action = "status".into(),
        GoCmd::Versions => request.action = "versions".into(),
        GoCmd::FixPath => request.action = "fix-path".into(),
        GoCmd::Install { version } | GoCmd::Switch { version } | GoCmd::Uninstall { version } => {
            request.action = version_action.into();
            request.version = select_version(&request.action, version);
            if request.version.is_none() { return; }
            if request.action == "uninstall" && !super::prompt_yes(&format!("卸载 {}？", request.version.as_deref().unwrap()), false) { return; }
        }
        GoCmd::Proxy { action: None } => request.action = "proxy".into(),
        GoCmd::Proxy { action: Some(ProxyCmd::Set { url, private }) } => {
            request.action = "proxy-set".into(); request.url = Some(url); request.private = private;
        }
        GoCmd::Test { url } => { request.action = "test".into(); request.url = url; }
        GoCmd::Tools { action: None | Some(ToolCmd::List) } => request.action = "tools".into(),
        GoCmd::Tools { action: Some(ToolCmd::Install { name, version }) } => {
            request.action = "tool-install".into();
            request.name = name.or_else(|| super::pick("选择工具", &["gopls", "dlv", "goimports"], |s| s.to_string()).map(|s| s.to_string()));
            if request.name.is_none() { return; }
            request.version = version;
        }
        GoCmd::Project { path, apply } => {
            request.action = if apply { "project-apply" } else { "project" }.into();
            request.path = Some(path.unwrap_or_else(|| std::env::current_dir().unwrap().to_string_lossy().to_string()));
        }
    }
    let inspect = request.action == "project";
    let project_path = request.path.clone();
    match execute(request, |stage| eprintln!("Go：{}", match stage {
        "downloading" => "下载中", "verifying" => "校验安装包", "extracting" => "解压中", "dependencies" => "下载项目依赖", "done" => "完成", _ => stage
    })) {
        Ok(value) => {
            if let Some(message) = value["message"].as_str() { println!("{message}"); }
            else { println!("{}", serde_json::to_string_pretty(&value).unwrap()); }
            if inspect && super::prompt_yes("按项目要求安装切换 Go 并下载依赖？", false) {
                run(Some(GoCmd::Project { path: project_path, apply: true }));
            }
        }
        Err(e) => super::fail(e),
    }
}

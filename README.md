# DevEnvManager · 开发环境管理工具

一款本地桌面 GUI 工具，统一管理开发环境：**Node.js 环境体检/版本切换/镜像源/发布账号**，以及 **Git 多账号/SSH 密钥/身份切换/仓库同步**。

基于 **Rust + Tauri 2** 构建，前端使用原生 HTML/CSS/JS（无构建工具依赖）。

## 功能

### Node.js
| 模块 | 说明 |
|---|---|
| 环境诊断 | 检测 Node/npm 安装状态、版本、PATH、商店假命令、网络连通性 |
| 镜像源 | 切换 npm registry（官方 / npmmirror / 自定义），测试连接（含自签名 HTTPS） |
| Node 版本 | 安装/切换/卸载版本（每大版本取最新），nvm 检测与守卫，卸载 nvm 引导 |
| 上传登录 | 管理 npm registry 账号（token / basic），多账号切换，whoami 认证验证 |

### Git
| 模块 | 说明 |
|---|---|
| 账号管理 | 读写 SSH config，管理多平台 Git 账号（GitHub/Gitee/GitLab 等） |
| SSH 密钥 | 生成/查看公钥，一键复制，直达平台密钥管理页 |
| 身份切换 | 一键切换全局或仓库级 Git 身份 |
| 仓库同步 | 按远程地址匹配账号，`git pull --ff-only` |

## 技术栈

- **桌面框架**：[Tauri 2](https://tauri.app)
- **后端**：Rust（`tauri`、`serde`、`regex`、`base64`、`thiserror`、`dirs`、`chrono`、`indexmap`）
- **前端**：原生 HTML/CSS/JS（无 React/Vite，无构建工具）
- **主题**：紫色深色（默认）/ 明亮，可切换并持久化

## 环境要求

- **Windows 10/11**（当前优先支持；Git 模块的 core 为跨平台设计）
- Rust 工具链（MSVC）+ 已安装（可用 `setup-toolchain.cmd` 一键安装）
- Git + OpenSSH（Git 模块使用，缺失时应用内提示安装引导）

## 构建与运行

```bash
# 开发运行（自动编译并启动）
start.cmd

# 或手动：
cd src-tauri
cargo build          # 编译
cargo run            # 调试运行
cargo tauri build    # 打包安装包（需 tauri-cli）
```

> 首次编译需下载依赖，耗时较长。`start.cmd` 会自动检查工具链、增量编译并启动。

## 项目结构

```
├── src/                  # 前端（HTML/CSS/JS）
│   ├── index.html        # 主页面（Node 4 Tab + Git 4 Tab）
│   ├── styles.css        # 样式（双主题 CSS 变量）
│   ├── main.js           # 前端逻辑
│   └── backend.js        # Tauri invoke 桥接层
├── src-tauri/            # Rust 后端
│   ├── src/lib.rs        # 命令注册 + Node/镜像/账号/版本命令
│   ├── src/git_cmds.rs   # Git 命令层（来自 gitAccessTool）
│   └── src/gitcore/      # Git core 逻辑（多平台）
├── start.cmd             # 一键启动脚本
├── setup-toolchain.cmd   # 工具链安装脚本（Rust + MSVC）
└── docs/screenshot.png   # 界面截图
```

## 截图

![DevEnvManager 主界面](docs/screenshot.png)

## 许可证

[Apache License 2.0](LICENSE)

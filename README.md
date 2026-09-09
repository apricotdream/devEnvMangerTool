# DevEnvManager · 开发环境管理工具

一款本地桌面 GUI 工具，统一管理开发环境：**Node.js 环境体检/版本切换/镜像源/包管理器/发布账号/项目级管理/包发布**、**Python 版本管理/pip 镜像/包管理器**，以及 **Git 环境检测/一键安装·卸载·版本管理/多账号/SSH 密钥/身份切换/仓库同步**。附赠 `devenv` CLI（无 GUI 版本），并支持 **Linux / WSL2 非 GUI 环境**的完整命令使用。

基于 **Rust + Tauri 2** 构建（workspace：核心逻辑在零 GUI 依赖的 `devenv-core`，Tauri 为薄壳），前端使用原生 HTML/CSS/JS（无构建工具依赖）。

> **当前版本：v0.2.5-beta**。普通用户**无需安装 Rust**，直接下载预编译安装包即可使用。

## 快速开始（免 Rust 环境）

到 [Gitee Releases](https://gitee.com/apricotdream/devEnvMangerTool/releases) 下载最新版：

| 文件 | 用途 |
|---|---|
| `DevEnvManager_0.2.5-beta_x64-setup.exe` | 桌面 GUI（Windows），双击安装后从开始菜单启动 |
| `devenv-cli-0.2.5-beta-windows-x64.zip` | 命令行工具（Windows，内含 `devenv.exe`），解压到任意目录后将其加入 PATH |
| `devenv-*-unknown-linux-*.tar.gz` + `install.sh` | 命令行工具（Linux / WSL，见下「Linux / WSL 使用」），无 GUI 依赖，免装 Rust |

使用过程中，**无需预装 Rust / Node / Python / Git**——工具会自行检测，缺失时在应用内一键下载并托管安装（Git 走 npmmirror 镜像的 MinGit，免管理员权限）；需要 Git Bash / 右键菜单时也可一键打开官网完整安装包。

### 系统要求

- **Windows 10 / 11**（GUI 端）
- **WebView2 运行时**：Win11 自带；Win10 若缺失，安装器会自动引导安装
- 首次运行若出现 SmartScreen「未知发布者」提示：点击「更多信息」→「仍要运行」即可（暂无代码签名证书）

## Linux / WSL 使用

`devenv` CLI 在 Linux / WSL2 上无需 Rust、无需 root、无 GUI 依赖，安装时**免装 Rust**（预编译静态二进制）。

### 安装

**方式一 · curl 一键安装（推荐）**

一条命令直接拉取安装脚本并执行，自动匹配架构（x64/arm64）与 libc（gnu/musl，musl 静态包可在 gnu 上回退），无需手动下载：

```bash
# 安装最新版（自动选最新含 Linux 附件的 release）
curl -fsSL https://gitee.com/apricotdream/devEnvMangerTool/raw/main/install.sh | bash

# 安装指定版本（把版本号作为参数传给脚本，注意 bash -s --）
curl -fsSL https://gitee.com/apricotdream/devEnvMangerTool/raw/main/install.sh | bash -s -- 0.2.5-beta
```

> 也可从 [Gitee Releases](https://gitee.com/apricotdream/devEnvMangerTool/releases) 下载 `install.sh` 附件后本地执行（效果相同；当前目录若已有 `devenv-*.tar.gz` 会优先本地安装）：
>
> ```bash
> bash install.sh              # 安装最新版
> bash install.sh 0.2.5-beta   # 安装指定版本
> ```

**方式二 · 源码安装**（需 Rust 工具链；无系统 gcc 也能构建，用 musl 静态目标 + rust-lld 自包含链接）：

```bash
git clone https://gitee.com/apricotdream/devEnvMangerTool
cd devEnvMangerTool
export RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes"
cargo build -p devenv-core --bin devenv --release --target x86_64-unknown-linux-musl
install -m755 target/x86_64-unknown-linux-musl/release/devenv ~/.local/bin/
source ~/.bashrc   # ~/.local/bin 进 PATH（devenv 也会自动幂等注入）
```

### 日常使用

```bash
devenv doctor                      # 环境体检：工具链/PATH/软链/shell 配置 + 可操作修复提示（推荐首次先跑）

# Node（自管安装，多版本并存）
devenv node status                 # 环境体检
devenv node versions               # 已装 + 可用版本
devenv node install v26.8.1        # 安装（官方 tarball，自动选择 x64/arm64 × gnu/musl）
devenv node switch v24.20.0        # 切换（软链指到目标版本，立即生效）
devenv node uninstall v24.20.0
devenv node registry set https://registry.npmmirror.com   # npm 镜像

# Python（python-build-standalone，自带 pip）
devenv python status / versions
devenv python install 3.12.8
devenv python switch 3.12.8 / uninstall 3.12.8
devenv python registry set https://pypi.tuna.tsinghua.edu.cn/simple

# Git（与 Windows 同账号体系：~/.ssh + ~/.gitconfig；省略参数走数字选择）
devenv git status                   # Git/OpenSSH 体检，缺失时打印 sudo 安装命令
devenv git add                      # 交互式新增账号（自动生成 SSH 密钥、给出平台添加地址）
devenv git accounts / identity
devenv git switch                   # 数字选择账号切换身份
devenv git key                      # 数字选择账号显示公钥
devenv git test                     # 数字选择账号做 SSH 连接测试
devenv git edit / delete / gen-key  # 编辑 / 删除 / 重新生成密钥
devenv git sync <仓库目录>          # git pull --ff-only
```

### 工作方式（Linux）

| 项 | 说明 |
|---|---|
| 安装目录 | `$XDG_DATA_HOME/devenv`（回退 `~/.local/share/devenv`）：`node/<v版本>/`、`python/<版本>/` |
| 软链目录 | `~/.local/bin`（Node/Python 可执行逐项软链，切换=重指，卸载=删目录+撤软链） |
| PATH | `~/.local/bin` 不在 PATH 时**自动幂等注入** `~/.bashrc` / `~/.zshrc`（`# >>> devenv >>>` 标记块）或 fish `conf.d/devenv.fish` |
| npm / pip 配置 | `~/.npmrc`（`NPM_CONFIG_USERCONFIG` 优先）；`$XDG_CONFIG_HOME/pip/pip.conf`（`PIP_CONFIG_FILE` 优先） |
| Git 缺失 | 不托管安装：识别 apt/dnf/pacman/zypper/apk，`doctor` 打印精确安装命令（含 openssh），不索要 root |
| WSL | 按原生 Linux 处理：管理 Linux 侧工具链，不跨 interop 接管 Windows 侧 |

## 功能

### Node.js
| 模块 | 说明 |
|---|---|
| 环境诊断 | 检测 Node/npm 安装状态、版本、PATH、商店假命令、网络连通性；安装完成后自动写 PATH 并自动重新检测 |
| 镜像源 | 切换 npm registry（官方 / npmmirror / 自定义），测试连接（含自签名 HTTPS；内网源自动跳过 schannel 吊销检查） |
| Node 版本 | 安装/切换/卸载版本（每大版本取最新），安装后自动加入用户 PATH，nvm 检测与守卫，卸载 nvm 引导 |
| 上传登录 | 管理 npm registry 账号（token / basic），多账号切换，whoami 认证验证；新增账号 registry 留空即默认公网 npmjs；**自动扫描全局 `~/.npmrc` 中已有的外部账号**（npm login / 其它工具写入），实时检测连通性，一键「纳入管理」 |
| 包管理器 | 管理 pnpm / yarn / cnpm / pm2 / bun：安装/切换版本、**卸载（连带清理数据目录）**、**切换数据目录（robocopy 迁移数据并改指向：pm2 `PM2_HOME`、bun `BUN_INSTALL`、yarn `YARN_CACHE_FOLDER`、pnpm store-dir）**，显示安装位置与实际数据目录；bun 另支持**「装到 WSL」目标**（`devenv node bun install-wsl` / GUI「装到 WSL」：把 Linux 版 bun 装进所选 WSL 发行版的 `~/.bun/bin`，走 npmmirror 镜像） |
| 项目管理 | 打开项目目录，解析 `.nvmrc` / `package.json` / 项目 `.npmrc` 要求，一键应用 Node 版本与包管理器 |
| 发布 | 扫描 `packages/` 下可发布包（支持 monorepo），发布源默认提供「公网 npmjs」选项，点发布时自动扫描 `~/.npmrc` 凭据，按账号发布到指定 registry，发布前可选构建 |
| 证书信任 | 探测自签名 HTTPS 证书，一键信任并写入 Node extra CA，管理已信任证书 |

### Python
| 模块 | 说明 |
|---|---|
| 环境诊断 | 检测 Python/pip 安装状态、版本、PATH、商店假命令、外来安装（官方安装器 / py launcher / conda），网络连通性 |
| pip 镜像 | 切换 pip index（官方 / 清华 / 阿里云 / 自定义），测试连接（PyPI `{base}/simple/` 探测） |
| Python 版本 | 安装/切换/卸载版本（每小版本取最新）；混合源：npmmirror 完整 zip（3.11.9+）优先，老版本回退 python-build-standalone |
| 包管理器 | 管理 pip / uv / poetry / pipenv / conda |
| 项目管理 | 解析 `.python-version` / `pyproject.toml` / `runtime.txt` / `Pipfile`，一键应用 Python 版本 |

### Git
| 模块 | 说明 |
|---|---|
| 环境诊断 | 检测 Git/OpenSSH 安装状态、版本、可执行路径（含不在 PATH 的 Git for Windows）；缺失时一键**托管安装 MinGit**（npmmirror 镜像、zip 解压到用户目录、免 UAC、自动写 PATH），支持**安装/切换版本**（最新 3 个稳定版）与**卸载**；另提供官网完整安装包（含 Git Bash/右键菜单）兜底入口。仅卸载本工具托管的 MinGit，不触碰系统独立安装的 Git |
| 账号管理 | 读写 SSH config，管理多平台 Git 账号（GitHub/Gitee/GitLab 等）；列表提供切换/测试/SSH密钥/编辑/删除，「测试」以全屏加载 + 结果弹窗展示 SSH 连接成败与输出 |
| SSH 密钥 | 不再单独占标签页：点账号的「SSH密钥」弹出悬浮框，查看私钥路径/公钥与 clone 指引，一键复制公钥、直达平台密钥管理页、生成密钥 |
| 身份切换 | 一键切换全局或仓库级 Git 身份 |
| 仓库同步 | 按远程地址匹配账号，`git pull --ff-only` |

### CLI（无 GUI）
```bash
devenv doctor                       # 环境体检：工具链/PATH/软链/shell 配置 + 修复提示

devenv node status                  # Node 环境体检
devenv node registry                # 查看 registry
devenv node registry set <url>      # 切换镜像源
devenv node versions                # 已装 + 可用版本
devenv node install v26.7.0         # 安装版本
devenv node switch v26.7.0          # 切换版本
devenv node uninstall v26.7.0       # 卸载版本
devenv node project [dir]           # 项目管理：解析 .nvmrc/package.json 一键装切 Node
devenv node test [url]              # npm 镜像测速（省略 url 测当前源 + 常用源）
devenv node bun status              # bun 运行时状态
devenv node bun install [ver]       # 安装/升级 bun（独立二进制，npmmirror zip；省略 ver 装最新）
devenv node bun install-wsl [ver]   # 把 Linux 版 bun 装进所选 WSL 发行版（Windows 侧，装进其 ~/.bun/bin）
devenv node bun upgrade             # 升级 bun 到最新
devenv node bun uninstall           # 卸载托管 bun
devenv node accounts                # 扫描 ~/.npmrc 已有发布凭据，纳入管理 / 切换
devenv node publish [--dir D] [--registry R] [--build] [--yes]
                                    # 发布 npm 包：扫描 → dry-run 预检 → y/N 确认后真发

devenv python status                # Python 环境体检
devenv python registry              # 查看 pip index
devenv python registry set <url>    # 切换 pip 镜像
devenv python versions              # 已装 + 可用版本
devenv python install 3.12.4        # 安装版本
devenv python switch 3.12.4         # 切换版本
devenv python uninstall 3.12.4      # 卸载版本
devenv python project [dir]         # 项目管理：解析 .python-version/pyproject 等一键装切 Python
devenv python test [url]            # pip 镜像测速
devenv python pkg list              # 列出 pip/uv/poetry/pipenv/conda 状态
devenv python pkg install [name]    # 安装 uv/poetry/pipenv（pip --user → 共享 ~/.local/bin）

devenv git status                   # Git / OpenSSH 体检（版本、路径；缺失给安装命令）
devenv git versions                 # 可安装版本（Win 镜像版；Linux 给包管理器命令）
devenv git install                  # Win 托管安装 MinGit；Linux 打印 sudo 安装命令
devenv git uninstall                # 卸载本工具托管的 MinGit（仅 Win）
devenv git accounts                 # 列出 Git 账号
devenv git identity                 # 当前身份
devenv git add                      # 交互式新增账号（逐项提示，可自动生成 SSH 密钥）
devenv git edit [alias]             # 交互式编辑账号（省略 alias 则数字选择）
devenv git delete [alias] [--purge] # 删除账号（--purge 连密钥文件一起删）
devenv git gen-key [alias] [--force]# 生成 / 重新生成 SSH 密钥对
devenv git switch [alias]           # 切换身份（省略 alias 则数字选择）
devenv git key [alias]              # 显示公钥 + 平台添加地址（省略则数字选择）
devenv git test [alias]             # SSH 连接测试（省略则数字选择）
devenv git sync <repo>              # 同步仓库
```

> **全数字交互菜单（免记命令）**：直接运行 `devenv`（不带任何参数）即进入数字菜单——先选环境（`[1] Node.js  [2] Python  [3] Git  [4] doctor`），再选操作（体检 / 安装 / 切换 / 账号 …），全程只需输入序号，`返回上一级` 或 `q` 退出；`devenv node`、`devenv python`、`devenv git` 则直接进入该模块的操作菜单。适合日常交互使用。上面的英文子命令（`status`/`install`/`switch`…）保留给脚本与自动化。
>
> **数字选择**：在命令行里，凡是「选版本 / 选账号」的参数位置直接填**数字**即按 `versions`/`accounts` 列表序号选择（如 `devenv node install 3`、`devenv git switch 2`、`devenv git key 1`）；省略该参数则进入交互数字菜单（`[1] [2] …`，`q` 取消）；也仍可填完整版本号 / 别名（如 `devenv node install v22.23.2`、`devenv git switch gitee`）。序号以对应 `devenv node/python versions`、`devenv git accounts` 的列表为准。
>
> **Git 安装 / 账号写入在 Linux 的差异**：Git 由系统包管理器托管（强依赖系统库、需 root），`devenv git install` 在 Linux 上只打印对应的 `sudo apt/dnf/… install` 命令，不代为执行；账号增删改、SSH 密钥生成、身份切换等与 Windows 完全一致（写 `~/.ssh/config` + `~/.ssh/id_rsa_<别名>`）。
>
> **Linux 上的 Node/Python 对齐项**：`node project` / `python project` 解析项目版本声明并自动装切；`node test` / `python test` 用 curl 探测镜像延迟。**bun** 在 Linux/WSL 由本工具托管（npmmirror 下载 zip → `unzip` 解压 → 装入 **`~/.bun/bin/bun`**，需系统已装 `unzip`，缺失时给出 `sudo apt/dnf/… install unzip` 提示），独立于 node；Windows 侧可 `devenv node bun install-wsl` / GUI「装到 WSL」，把 Linux 版 bun 装进所选 WSL 发行版。**Python 包管理器**（uv/poetry/pipenv）通过 `pip install --user` 装到**共享 `~/.local/bin`（真实文件）**，切换/卸载 Python 版本后仍然可用。**npm 发布**复用 `~/.npmrc` 已有凭据（先 `npm login`，再 `devenv node accounts` 纳入管理），发布前**强制 `--dry-run` 预检**、确认后才真发（`--yes` 跳过确认）。

## 技术栈

- **架构**：Rust workspace——`devenv-core`（`core/`，跨平台、**零 tauri 依赖**，Node/Python/Git 全部逻辑 + `devenv` CLI）+ `src-tauri`（Tauri GUI 薄壳，Windows）
- **桌面框架**：[Tauri 2](https://tauri.app)（仅 GUI 壳使用；CLI 构建不链接 webkit/gtk，可在无桌面 Linux/服务器上运行）
- **后端**：Rust（`serde`、`regex`、`base64`、`thiserror`、`dirs`、`chrono`、`indexmap`、`clap`）
- **前端**：原生 HTML/CSS/JS（无 React/Vite，无构建工具）
- **主题**：紫色深色（默认）/ 明亮，可切换并持久化

## 源码构建（开发者）

> 普通用户无需本节，直接使用上方「快速开始」的预编译安装包即可。

### 环境要求

- **Windows 10/11**（GUI 端；GUI 需 MSVC 工具链，可用 `setup-toolchain.cmd` 一键安装，约 5~7GB / 20~40 分钟）
- **Linux / WSL2**（CLI 端）：`devenv` 全部功能可用（Git 账号/密钥/同步、Node/Python 版本自管安装（tarball + `~/.local/bin` 软链，无需 root）、npm/pip 镜像、`devenv doctor`）。构建 CLI 只需 Rust 工具链（musl 静态目标自带 rust-lld，**无需系统 gcc**）；直接从 release 安装则连 Rust 都不需要
- Git + OpenSSH（Git 模块使用，缺失时 `doctor` 按发行版打印安装命令）

### 构建与运行

```bash
# 开发运行（自动编译并启动）
start.cmd

# 或手动（workspace 根目录）：
cargo build -p dev-env-manager   # 编译 GUI 壳（仅 Windows）
cargo run -p dev-env-manager     # 调试运行 GUI
cargo build -p devenv-core --bin devenv   # 编译 CLI（Windows / Linux 均可，Linux 见上「Linux / WSL 使用」）
```

> 首次编译需下载依赖，耗时较长。`start.cmd` 会自动检查工具链、增量编译并启动。

### 打包发布（发版）

发版构建在本地执行：先安装 `tauri-cli`，设置 Gitee 访问令牌，再运行根目录的 `publish-release.cmd`。脚本会读取 `Cargo.toml` 版本号，自动完成冲突检查、release 构建、收集产物到 `dist/`（NSIS 安装包 + `devenv` CLI 便携 zip + SHA256SUMS.txt）、在 `origin/main` 上打 tag，并调用 Gitee API 创建预发布 release、上传全部附件。

```bash
cargo install tauri-cli --locked
# 或：npm install -g @tauri-apps/cli

# Windows cmd（token 需在同一窗口设置）：
set GITEE_TOKEN=你的_gitee_private_token
publish-release.cmd
```

**Linux CLI 发布**：在 WSL2/Linux 上运行 `publish-release.sh`（同样读 `core/Cargo.toml` 版本号），构建 musl 静态 `devenv`，产出 `devenv-<target>-<tag>.tar.gz` + `install.sh` + `SHA256SUMS.txt` 并上传 Gitee release。注意 `.cmd` 与 `.sh` 都会各自建 tag/release，**完整发布（Win+Linux）时先跑 `publish-release.cmd` 建 release，再把 Linux 三件套传到同一个 release**（不要紧接着跑 `.sh`，否则因 tag/release 已存在而中止）。用户安装：

```bash
# Linux / WSL：curl 一键安装（自动匹配架构 x64/arm64、libc gnu/musl 与最新版本）
curl -fsSL https://gitee.com/apricotdream/devEnvMangerTool/raw/main/install.sh | bash
curl -fsSL https://gitee.com/apricotdream/devEnvMangerTool/raw/main/install.sh | bash -s -- 0.2.5-beta  # 指定版本
```

> 发版前需先把改动合并到 `main`（脚本基于 `origin/main` 打 tag）。版本号在 `core/Cargo.toml`、`src-tauri/Cargo.toml` 与 `src-tauri/tauri.conf.json` 三处同步递增；tag 命名为 `v<版本号>`（如 `v0.2.1-beta`），已存在的 tag 会被脚本拒绝。

## 项目结构

```
├── src/                  # 前端（HTML/CSS/JS）
│   ├── index.html        # 主页面（Node 6 Tab + Git 4 子 Tab（含环境诊断） + Python 5 Tab；SSH 密钥改为账号列表弹窗）
│   ├── styles.css        # 样式（双主题 CSS 变量）
│   ├── main.js           # 前端逻辑
│   ├── backend.js        # Tauri invoke 桥接层
│   └── logo.png          # 侧栏 Logo（应用图标由 src-tauri/icons/ 生成）
├── core/                 # devenv-core：跨平台、零 GUI 依赖的核心（Rust workspace 成员）
│   ├── src/lib.rs        # Node/Python/镜像/账号/版本/包管理器/项目/发布逻辑 + Linux 分派
│   ├── src/linux.rs      # Linux 后端：XDG 路径、arch/libc 探测、tarball 安装、~/.local/bin 软链、shell rc 注入
│   ├── src/bin/devenv.rs # CLI 入口（含 doctor）
│   ├── src/git_cmds.rs   # Git 命令层（来自 gitAccessTool）
│   └── src/gitcore/      # Git core 逻辑（多平台）
├── src-tauri/            # Tauri GUI 壳（仅 Windows，薄壳转调 devenv-core）
├── start.cmd             # 一键启动脚本
├── publish-release.sh    # Linux CLI 发布脚本（WSL/Linux 运行）
├── install.sh            # Linux CLI 安装脚本（release 附件）
├── setup-toolchain.cmd   # 工具链安装脚本（Rust + MSVC）
├── publish-release.cmd   # 一键发版脚本（构建 + 打 tag + 建 Gitee release 传包）
└── docs/screenshot.png   # 界面截图
```

## 更新日志

### v0.2.5-beta
- **Linux CLI 与桌面端功能对齐（8 项）**：本轮把桌面端已有、CLI 缺失的能力补齐到 Linux/WSL `devenv`。
- **Node 项目管理**：`devenv node project [dir]` 解析 `.nvmrc` / `package.json`（engines、packageManager）/ 项目 `.npmrc`，一键安装并切换到要求的 Node 版本（corepack 缺失时跳过）。
- **Python 项目管理**：`devenv python project [dir]` 解析 `.python-version` / `pyproject.toml`（requires-python、tool）/ `runtime.txt` / `Pipfile`，一键装切 Python。
- **镜像测速**：`devenv node test [url]`（npm registry）、`devenv python test [url]`（PyPI `simple/`），探测可达性、HTTP 状态与延迟；省略 URL 测当前源 + 常用源。
- **Python 包管理器**：`devenv python pkg list|install`，管理 pip / uv / poetry / pipenv / conda；uv/poetry/pipenv 通过 `pip install --user` 装到**共享 `~/.local/bin`（真实文件）**，切换/卸载 Python 版本后仍可用。
- **bun 托管**：`devenv node bun status|install|upgrade|uninstall`——独立二进制（不依赖 node），从 npmmirror 下载 zip、`unzip` 解压后装入官方布局 `~/.bun/bin/bun` 并幂等注入 PATH（缺 unzip 给出安装提示），gnu/musl × x64/arm64 自动匹配；安装/卸载自动清理旧版（≤v0.2.5）`~/.local/bin/bun` 托管软链，避免其遮盖新布局。
- **npm 发布账号（CLI）**：`devenv node accounts` 扫描 `~/.npmrc` 已有凭据（npm login 写入）、检测连通性，一键「纳入管理」并切换激活（不提供粘贴 token 向导）。
- **npm 包发布（CLI）**：`devenv node publish` 扫描根包与 `packages/`（monorepo），**发布前强制 `--dry-run` 预检**，y/N 确认后才真发（`--yes` 跳过确认），遇 2FA 提示输入 OTP。
- 共享层去 Windows 硬编码（`npm`/`curl`/`/dev/null`/证书与账号路径跨平台），Windows 桌面端行为不变。

### v0.2.4-beta
- **Linux / WSL 完整支持（CLI）**：拆分为 Rust workspace——`devenv-core`（`core/`，跨平台、零 GUI/webkit/gtk 依赖）+ `src-tauri`（Tauri 薄壳，仅 Windows）。`devenv` CLI 在无桌面 Linux/服务器/WSL2 上可用。
- **Node / Python 全功能移植到 Linux**：官方 tarball 自管安装到 `$XDG_DATA_HOME/devenv`，多版本并存，通过 `~/.local/bin` 软链切换、免 root；`~/.local/bin` 不在 PATH 时幂等注入 bash/zsh/fish；`devenv doctor` 体检工具链/PATH/软链/shell 配置。
- **Linux 分发**：musl 静态自包含二进制（无需系统 gcc，rust-lld 链接），提供 `install.sh` 一键安装（自动匹配 x64/arm64 × gnu/musl，本地 tarball 优先）+ `publish-release.sh` 发版脚本。
- **Git 模块补齐（CLI）**：新增 `git add/edit/delete/gen-key` 账号写入与密钥生成、`git status` 体检、`git versions/install/uninstall`（Windows 托管安装 MinGit；Linux 打印对应包管理器命令、不替跑 sudo）。
- **全数字交互菜单**：直接运行 `devenv`（或 `devenv node/python/git`）进入数字菜单，选环境→选操作→选版本/账号全程只需输入序号；选择类参数也可直接填数字（如 `devenv node install 3`、`devenv git switch 2`）；英文子命令保留给脚本。

### v0.2.3-beta
- **Git 环境诊断（新 Tab）**：进入 Git 页默认显示「环境诊断」，检测 Git / OpenSSH 的安装状态、版本与可执行路径（含不在 PATH 中的 Git for Windows），支持「重新检测」。
- **Git 一键安装 / 卸载 / 版本管理**：缺失时从 npmmirror 镜像托管安装 MinGit（zip 解压到用户目录、免 UAC、自动写 PATH）；可在最新 3 个稳定版中选择安装/更新；支持卸载本工具托管的 MinGit（自动撤出 PATH，不触碰系统独立安装的完整 Git for Windows）。另提供官网完整安装包（含 Git Bash / 右键菜单）兜底入口。
- 修复 re-spin 版本（如 `v2.55.0.windows.5`）MinGit 文件名带第 4 段版本号导致下载 404 的问题：改为从镜像目录索引解析真实安装包文件名。
- 切换左侧导航标签时自动清空控制台日志。

### v0.2.2-beta
- Git 模块交互优化：SSH 密钥改为账号弹窗、测试连接结果反馈、导航对勾跟随。

## 截图

![DevEnvManager 主界面](docs/screenshot.png)

## 许可证

[Apache License 2.0](LICENSE)

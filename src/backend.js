/**
 * 数据层：前端统一通过 window.Backend 调用能力。
 *
 * 当前处于「前端预览」阶段，所有函数返回模拟数据（带延迟）。
 * 接入 Tauri 后，把 _invoke 里的 mock 分支替换为真实的
 * window.__TAURI__.core.invoke(cmd, args)，命令名与 Rust 端保持一致：
 *   detect_node / read_registry / set_registry / install_node / fix_path
 */
(function () {
  "use strict";

  const IS_TAURI = typeof window !== "undefined" && "__TAURI__" in window;

  function _delay(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  async function _invoke(cmd, args) {
    if (IS_TAURI) {
      try {
        return await window.__TAURI__.core.invoke(cmd, args);
      } catch (e) {
        // Tauri 把后端 `Err(String)` 序列化为字符串 reject，没有 .message；
        // 统一包成 Error，避免上层 `e.message` 一律得到 undefined。
        if (e instanceof Error) throw e;
        throw new Error(typeof e === "string" ? e : JSON.stringify(e));
      }
    }
    throw new Error(`[模拟模式] 命令 ${cmd} 未接入 Tauri`);
  }

  /* ---------------- 模拟数据 ---------------- */

  const MOCK_ENV = {
    installed: true,
    storeStub: false,
    realNodeExe: "D:\\soft\\nvm\\nodejs\\node.exe",
    nodeVersion: "v22.20.0",
    npmVersion: "10.9.3",
    inPath: true,
    nodeDirInPath: "D:\\soft\\nvm\\nodejs",
    network: [
      { name: "官方 npm", url: "https://registry.npmjs.org", ok: true, latencyMs: 741 },
      { name: "淘宝 npmmirror", url: "https://registry.npmmirror.com", ok: true, latencyMs: 121 },
    ],
  };

  const MOCK_REGISTRY = "https://npm.example.invalid/";

  /* ---------------- 后端 API ---------------- */

  const STAGE_TEXT = {
    connecting: "连接镜像源…",
    downloading: "下载中…",
    extracting: "解压中…",
    done: "完成",
  };

  // 浏览器预览只操作内存，界面操作可演示，真实功能通过 Tauri 调用。
  const goMock = {
    current: "go1.26.1", installed: ["go1.26.1"],
    available: ["go1.26.1", "go1.25.7", "go1.24.13"],
    env: { GOROOT: "C:\\DevEnvManager\\go\\go1.26.1", GOPATH: "C:\\Users\\dev\\go", GOBIN: "", GOPROXY: "https://proxy.golang.org,direct", GOPRIVATE: "", GOSUMDB: "sum.golang.org", GOENV: "C:\\Users\\dev\\AppData\\Roaming\\go\\env", GOTOOLCHAIN: "auto" },
    tools: new Set(),
  };
  function goPreview(r) {
    const root = "C:\\DevEnvManager\\go";
    switch (r.action) {
      case "status": return { installed: !!goMock.current, managed: !!goMock.current, version: goMock.current ? `go version ${goMock.current} windows/amd64` : null, path: goMock.current ? `${root}\\${goMock.current}\\bin\\go.exe` : null, env: { ...goMock.env, GOROOT: goMock.current ? `${root}\\${goMock.current}` : "" }, installDir: root };
      case "versions": return { current: goMock.current, installed: goMock.installed.map((version) => ({ version, current: version === goMock.current })), available: goMock.available, installDir: root };
      case "install": {
        const version = r.version || goMock.available[0];
        if (!/^(?:go)?\d+\.\d+(?:\.\d+)?$/.test(version)) throw new Error("请输入有效 Go 版本");
        const normalized = version.startsWith("go") ? version : "go" + version;
        if (!goMock.installed.includes(normalized)) goMock.installed.push(normalized);
        goMock.current = normalized;
        return { message: `已安装并切换到 ${normalized}（预览）` };
      }
      case "switch": goMock.current = r.version; return { message: `已切换到 ${r.version}（预览）` };
      case "uninstall": goMock.installed = goMock.installed.filter((v) => v !== r.version); if (goMock.current === r.version) goMock.current = null; return { message: `已卸载 ${r.version}（预览）` };
      case "fix-path": return { message: "Go PATH 已修复（预览）" };
      case "proxy": return { ...goMock.env };
      case "proxy-set": goMock.env.GOPROXY = r.url; goMock.env.GOPRIVATE = r.private || ""; return { message: "代理配置已保存（预览）" };
      case "test": return [{ url: r.url, ok: true, latencyMs: 68 }];
      case "tools": return ["gopls", "dlv", "goimports"].map((name) => ({ name, installed: goMock.tools.has(name), version: goMock.tools.has(name) ? "预览版本" : null, path: goMock.tools.has(name) ? `C:\\Users\\dev\\go\\bin\\${name}.exe` : null, dataDir: "C:\\Users\\dev\\go\\bin" }));
      case "tool-install": goMock.tools.add(r.name); return { message: `已安装 ${r.name}（预览）` };
      case "project": return { path: r.path, module: "example.com/demo", source: "go.mod", goVersion: "1.25.0", toolchain: "go1.26.1", requiredVersion: "go1.26.1" };
      case "project-apply": return { message: "项目 Go 环境已应用，依赖已下载（预览）" };
      default: throw new Error("未知 Go 预览操作");
    }
  }

  window.Backend = {
    IS_TAURI,

    async appVersion() {
      return IS_TAURI ? _invoke("app_version") : "0.2.10-beta";
    },
    async checkAppUpdate(automatic = false) {
      if (IS_TAURI) return _invoke("check_app_update", { automatic });
      await _delay(200);
      const mode = new URLSearchParams(window.location.search).get("updatePreview");
      if (mode === "error") throw new Error("预览：网络连接失败，请手动重试");
      const available = mode === "available";
      return { currentVersion:"0.2.10-beta", latestVersion:available ? "0.2.11-beta" : "0.2.10-beta", available, canInstall:available,
        notes:"改进环境管理与发布体验。\n修复已知问题。", releaseUrl:"https://gitee.com/apricotdream/devEnvMangerTool/releases", message:null };
    },
    async downloadAppUpdate(version, onProgress) {
      if (!IS_TAURI) {
        for (const stage of ["checksums", "downloading", "verifying", "ready"]) {
          onProgress?.({ stage, bytes:stage === "downloading" ? 10485760 : 0 });
          await _delay(180);
        }
        return "预览：安装包已下载并校验，可点击退出并安装";
      }
      const unlisten = await window.__TAURI__.event.listen("app-update-progress", (event) => onProgress?.(event.payload));
      try { return await _invoke("download_app_update", { version }); }
      finally { unlisten(); }
    },
    async installAppUpdate(version) {
      if (IS_TAURI) return _invoke("install_app_update", { version });
      await _delay(100);
    },

    async goPickProject() {
      if (IS_TAURI) return this.pickProject();
      return "C:\\projects\\go-demo";
    },

    async goCommand(request) {
      if (IS_TAURI) return _invoke("go_command", { request });
      await _delay(180);
      return goPreview(request);
    },

    async detectNode() {
      if (IS_TAURI) return _invoke("detect_node");
      await _delay(650);
      return JSON.parse(JSON.stringify(MOCK_ENV));
    },

    async readRegistry() {
      if (IS_TAURI) return _invoke("read_registry");
      await _delay(120);
      return MOCK_REGISTRY;
    },

    async setRegistry(url) {
      if (IS_TAURI) return _invoke("set_registry", { url });
      await _delay(300);
      return { ok: true, path: "~/.npmrc" };
    },

    async fixPath(dir) {
      if (IS_TAURI) return _invoke("fix_path", { dir });
      await _delay(400);
      return { ok: true, path: dir };
    },

    async installNode({ onProgress } = {}) {
      if (IS_TAURI) {
        let unlisten;
        try {
          unlisten = await window.__TAURI__.event.listen("install-progress", (e) => {
            const stage = e.payload?.stage;
            onProgress?.(stage === "done" ? 100 : 55, STAGE_TEXT[stage] || stage || "…");
          });
        } catch (_e) { /* 事件监听失败则仅用阶段消息 */ }
        try {
          const res = await _invoke("install_node");
          return res;
        } finally {
          unlisten?.();
        }
      }

      // ---- 浏览器预览：模拟流程 ----
      const stages = [
        ["连接镜像源…", 2],
        ["下载 node-v26.7.0-win-x64.zip", 40],
        ["解压安装包…", 70],
        ["写入配置…", 90],
        ["完成", 100],
      ];
      let prev = 0;
      for (const [text, target] of stages) {
        const steps = target - prev;
        for (let i = 0; i < steps; i++) {
          const pct = prev + i + 1;
          onProgress?.(pct, text);
          await _delay(50);
        }
        prev = target;
      }
      return { ok: true, version: "v26.7.0", dir: "C:\\Users\\dev\\AppData\\Local\\Programs\\nodejs\\v26.7.0" };
    },

    async testRegistry(url) {
      if (IS_TAURI) return _invoke("test_registry", { url });
      await _delay(600);
      return {
        reachable: true,
        status: 200,
        latencyMs: 68,
        throughputKbs: 412.5,
        needsAuth: false,
        error: null,
      };
    },

    /* ---------------- 账号 ---------------- */

    async listAccounts() {
      if (IS_TAURI) return _invoke("list_accounts");
      await _delay(150);
      return { active: null, accounts: [] };
    },

    async scanNpmrcCredentials() {
      if (IS_TAURI) return _invoke("scan_npmrc_credentials");
      await _delay(600);
      return [];
    },

    async importNpmrcCredential(registry) {
      if (IS_TAURI) return _invoke("import_npmrc_credential", { registry });
      await _delay(300);
      return { active: null, accounts: [] };
    },

    async saveAccount(account) {
      if (IS_TAURI) return _invoke("save_account", { account });
      await _delay(200);
      return { active: null, accounts: [account] };
    },

    async deleteAccount(id) {
      if (IS_TAURI) return _invoke("delete_account", { id });
      await _delay(150);
      return { active: null, accounts: [] };
    },

    async switchAccount(id) {
      if (IS_TAURI) return _invoke("switch_account", { id });
      await _delay(250);
      return { active: id, accounts: [] };
    },

    async testAccount(id) {
      if (IS_TAURI) return _invoke("test_account", { id });
      await _delay(500);
      return { reachable: true, status: 200, latencyMs: 42, throughputKbs: 1800, needsAuth: false, authUser: "admin", error: null };
    },

    /* ---------------- 证书信任 ---------------- */

    async probeCert(registry) {
      if (IS_TAURI) return _invoke("probe_cert", { registry });
      await _delay(300);
      return {
        host: "npm.example.invalid",
        port: 181,
        subject: "CN=npm.example.invalid",
        issuer: "CN=npm.example.invalid",
        selfSigned: true,
        sha256: "MOCKFINGERPRINT000000000000000000000000000000000000000000000000",
        pem: "-----BEGIN CERTIFICATE-----\nMOCK\n-----END CERTIFICATE-----",
        error: null,
      };
    },

    async acceptCert(registry) {
      if (IS_TAURI) return _invoke("accept_cert", { registry });
      await _delay(300);
      return { host: "mock", port: 181, selfSigned: true, sha256: "mock", pem: "", error: null };
    },

    async listTrustedCerts() {
      if (IS_TAURI) return _invoke("list_trusted_certs");
      await _delay(150);
      return { certs: [] };
    },

    async removeTrustedCert(host, port) {
      if (IS_TAURI) return _invoke("remove_trusted_cert", { host, port });
      await _delay(150);
      return { certs: [] };
    },

    /* ---------------- 包发布 ---------------- */

    async listPublishPackages(projectDir) {
      if (IS_TAURI) return _invoke("list_publish_packages", { projectDir });
      await _delay(250);
      if (new URLSearchParams(window.location.search).get("publishPreview")) {
        return ["one", "two", "three"].map((name) => ({ name:`@demo/${name}`, version:"1.0.0-beta.1", dir:`C:\\preview\\npm-workspace\\packages\\${name}`, hasDist:true, isPrivate:false }));
      }
      return [];
    },

    async checkRegistryPackage(registry, name) {
      if (IS_TAURI) return _invoke("check_registry_package", { registry, name });
      await _delay(200);
      return { exists: false, versions: [], error: null };
    },

    async publishPackages(packages, registry, build, otp = null, dryRun = false, access = null) {
      if (IS_TAURI) return _invoke("publish_packages", { packages, registry, build, otp, dryRun, access });
      await _delay(600);
      const scenario = new URLSearchParams(window.location.search).get("publishPreview");
      if (scenario === "otp" && !dryRun) {
        return packages.map((dir, index) => ({ name:dir, version:"1.0.0-beta.1", ok:otp === "123456" || index === 0,
          needsOtp:!otp && index === 1, message:otp ? "模拟：验证码重试成功" : index === 0 ? "模拟：发布成功" : index === 1 ? "模拟：需要验证码" : "模拟：Token 权限不足" }));
      }
      return packages.map((d) => ({ name:d, version:"0.0.0", ok:true, message:dryRun ? "模拟演练成功（未上传）" : "模拟发布成功" }));
    },

    async checkNodeExtraCa() {
      if (IS_TAURI) return _invoke("check_node_extra_ca");
      await _delay(100);
      return { allPemExists: false, envSet: false, envMatches: false };
    },

    /* ---------------- 版本 ---------------- */

    async listVersions() {
      if (IS_TAURI) return _invoke("list_versions");
      await _delay(200);
      return {
        nvmDetected: false,
        installDir: "C:\\Users\\dev\\AppData\\Local\\Programs\\nodejs",
        installed: [
          { version: "v22.20.0", current: true, sizeMb: 152 },
          { version: "v26.7.0", current: false, sizeMb: 168 },
        ],
        available: ["v26.7.0", "v24.9.1", "v22.20.0", "v20.19.4"],
        current: "v22.20.0",
      };
    },

    async installVersion(version, { onProgress } = {}) {
      if (IS_TAURI) {
        let unlisten;
        try {
          unlisten = await window.__TAURI__.event.listen("version-progress", (e) => {
            onProgress?.(e.payload?.stage);
          });
        } catch (_e) { /* 忽略监听失败 */ }
        try {
          return await _invoke("install_version", { version });
        } finally {
          unlisten?.();
        }
      }
      onProgress?.("downloading");
      await _delay(500);
      onProgress?.("extracting");
      await _delay(400);
      onProgress?.("done");
      return { ok: true, version, dir: "C:\\Users\\dev\\AppData\\Local\\Programs\\nodejs\\" + version };
    },

    async switchVersion(version) {
      if (IS_TAURI) return _invoke("switch_version", { version });
      await _delay(300);
      return "C:\\Users\\dev\\AppData\\Local\\Programs\\nodejs\\" + version;
    },

    async uninstallVersion(version) {
      if (IS_TAURI) return _invoke("uninstall_version", { version });
      await _delay(300);
      return "C:\\Users\\dev\\AppData\\Local\\Programs\\nodejs\\" + version;
    },

    async nvmInfo() {
      if (IS_TAURI) return _invoke("nvm_info");
      await _delay(150);
      return { detected: false, dirs: [], pathEntries: [] };
    },

    async uninstallNvm() {
      if (IS_TAURI) return _invoke("uninstall_nvm");
      await _delay(400);
      return "已卸载 nvm（模拟）";
    },

    /* ---------------- Git ---------------- */

    async gitListAccounts() {
      if (IS_TAURI) return _invoke("git_list_accounts");
      await _delay(200);
      return [];
    },

    async gitAddAccount(account, generateKey) {
      if (IS_TAURI) return _invoke("git_add_account", { account, generateKey });
      await _delay(300);
      return account;
    },

    async gitUpdateAccount(sourceAlias, account, generateKey) {
      if (IS_TAURI) return _invoke("git_update_account", { sourceAlias, account, generateKey });
      await _delay(300);
      return account;
    },

    async gitDeleteAccount(alias, deleteKeys) {
      if (IS_TAURI) return _invoke("git_delete_account", { alias, deleteKeys });
      await _delay(200);
      return null;
    },

    async gitTestConnection(alias) {
      if (IS_TAURI) return _invoke("git_test_connection", { alias });
      await _delay(800);
      return { success: true, output: "模拟连接成功" };
    },

    async gitSwitchIdentity(alias, local, cwd) {
      if (IS_TAURI) return _invoke("git_switch_identity", { alias, local, cwd });
      await _delay(300);
      return { alias, hostName: "", port: "22", sshUser: "git", keyPath: null, gitName: "", gitEmail: "" };
    },

    async gitGetIdentity(local, cwd) {
      if (IS_TAURI) return _invoke("git_get_identity", { local, cwd });
      await _delay(150);
      return { name: null, email: null };
    },

    async gitSyncRepo(repo) {
      if (IS_TAURI) return _invoke("git_sync_repo", { repo });
      await _delay(600);
      return { alias: "mock", output: "模拟同步完成" };
    },

    async gitReadPublicKey(keyPath) {
      if (IS_TAURI) return _invoke("git_read_public_key", { keyPath });
      await _delay(150);
      return "ssh-rsa AAAA... 模拟公钥";
    },

    async gitGenerateKey(alias, email, keyPath) {
      if (IS_TAURI) return _invoke("git_generate_key", { alias, email, keyPath });
      await _delay(400);
      return "C:\\Users\\dev\\.ssh\\id_rsa_" + alias;
    },

    async gitPickRepo() {
      if (IS_TAURI) {
        const sel = await _invoke("plugin:dialog|open", { options: { directory: true, title: "选择仓库目录" } });
        return typeof sel === "string" ? sel : "";
      }
      await _delay(100);
      return "";
    },

    async gitCheckEnv() {
      if (IS_TAURI) return _invoke("git_check_env");
      await _delay(100);
      return { git: true, ssh: true };
    },

    async gitGetKeyUrl(alias) {
      if (IS_TAURI) return _invoke("git_get_key_url", { alias });
      await _delay(150);
      return "https://github.com/settings/keys";
    },

    async gitOpenUrl(url) {
      if (IS_TAURI) return _invoke("git_open_url", { url });
      await _delay(100);
      return null;
    },

    /* ---------------- Git 环境检测 / 安装（MinGit 托管，仅 Windows） ---------------- */

    async detectGit() {
      if (IS_TAURI) return _invoke("detect_git");
      await _delay(100);
      return { installed: false, version: null, path: null, sshInstalled: false };
    },

    async gitVersions() {
      if (IS_TAURI) return _invoke("git_versions");
      await _delay(200);
      return [
        { version: "2.55.0", tag: "v2.55.0.windows.5" },
        { version: "2.53.0", tag: "v2.53.0.windows.4" },
        { version: "2.49.0", tag: "v2.49.0.windows.2" },
      ];
    },

    async installGitVersion(version, tag, { onProgress } = {}) {
      if (IS_TAURI) {
        let unlisten;
        try {
          unlisten = await window.__TAURI__.event.listen("git-install-progress", (e) => {
            onProgress?.(e.payload?.stage);
          });
        } catch (_e) { /* 忽略监听失败 */ }
        try {
          return await _invoke("install_git_version", { version, tag });
        } finally {
          unlisten?.();
        }
      }
      onProgress?.("downloading");
      await _delay(400);
      onProgress?.("extracting");
      await _delay(300);
      onProgress?.("done");
      return "C:\\Users\\dev\\AppData\\Local\\Programs\\Git";
    },

    async uninstallGit() {
      if (IS_TAURI) return _invoke("uninstall_git");
      await _delay(200);
      return "C:\\Users\\dev\\AppData\\Local\\Programs\\Git";
    },

    async gitOpenInManager(path, select) {
      if (IS_TAURI) return _invoke("git_open_in_manager", { path, select });
      await _delay(100);
      return null;
    },

    async listConfigDirs() {
      if (IS_TAURI) return _invoke("list_config_dirs");
      await _delay(150);
      return [
        { name: "npm 全局配置 (.npmrc)", path: "C:\\Users\\dev\\.npmrc", exists: true },
        { name: "npm 全局目录 (prefix)", path: "C:\\Users\\dev\\AppData\\Roaming\\npm", exists: true },
        { name: "Node 版本安装目录", path: "C:\\Users\\dev\\AppData\\Local\\Programs\\nodejs", exists: true },
        { name: "nvm 安装目录", path: "C:\\Users\\dev\\AppData\\Roaming\\nvm", exists: false },
        { name: "pip 配置 (pip.ini)", path: "C:\\Users\\dev\\AppData\\Roaming\\pip\\pip.ini", exists: true },
        { name: "Python 版本安装目录", path: "C:\\Users\\dev\\AppData\\Local\\Programs\\python", exists: true },
        { name: "SSH 密钥目录 (.ssh)", path: "C:\\Users\\dev\\.ssh", exists: true },
        { name: "Git 全局配置 (.gitconfig)", path: "C:\\Users\\dev\\.gitconfig", exists: true },
        { name: "DevEnvManager 配置目录", path: "C:\\Users\\dev\\AppData\\Roaming\\DevEnvManager", exists: true },
      ];
    },

    /* ---------------- 包管理器 ---------------- */

    async listPkgManagers() {
      if (IS_TAURI) return _invoke("list_pkg_managers");
      await _delay(200);
      return [
        { name: "pnpm", installed: true, version: "9.15.0", binPath: "C:\\...\\pnpm.cmd", dataDir: "C:\\Users\\dev\\AppData\\Local\\pnpm\\store" },
        { name: "yarn", installed: true, version: "1.22.22", binPath: "C:\\...\\yarn.cmd", dataDir: "C:\\Users\\dev\\AppData\\Local\\Yarn" },
        { name: "cnpm", installed: false, version: null, binPath: null, dataDir: "C:\\Users\\dev\\.cnpmrc" },
        { name: "pm2", installed: true, version: "5.4.2", binPath: "C:\\...\\pm2.cmd", dataDir: "C:\\Users\\dev\\.pm2" },
        { name: "bun", installed: true, version: "1.2.3", binPath: "C:\\Users\\dev\\.bun\\bin\\bun.exe", dataDir: "C:\\Users\\dev\\.bun" },
      ];
    },

    async installPkgManager(name, version) {
      if (IS_TAURI) return _invoke("install_pkg_manager", { name, version });
      await _delay(800);
      return `${name}@${version || "latest"} 已安装（模拟）`;
    },

    async uninstallPkgManager(name) {
      if (IS_TAURI) return _invoke("uninstall_pkg_manager", { name });
      await _delay(600);
      return `${name} 已卸载（模拟）`;
    },

    async switchPkgDataDir(name, target) {
      if (IS_TAURI) return _invoke("switch_pkg_data_dir", { name, target });
      await _delay(600);
      return `${name} 数据目录已切换到 ${target}（模拟）`;
    },

    async listWslDistros() {
      if (IS_TAURI) return _invoke("list_wsl_distros");
      await _delay(150);
      return ["Ubuntu", "Ubuntu-24.04"];
    },

    async installBunInWsl(distro, version) {
      if (IS_TAURI) return _invoke("install_bun_in_wsl", { distro, version });
      await _delay(900);
      return `bun 已安装到 WSL ${distro}（模拟）`;
    },

    async pickFolder(title = "选择目录") {
      if (IS_TAURI) {
        const sel = await _invoke("plugin:dialog|open", { options: { directory: true, title } });
        return typeof sel === "string" ? sel : "";
      }
      await _delay(100);
      return "";
    },

    /* ---------------- 项目管理 ---------------- */

    async pickProject() {
      if (IS_TAURI) {
        const sel = await _invoke("plugin:dialog|open", { options: { directory: true, title: "选择项目目录" } });
        return typeof sel === "string" ? sel : "";
      }
      await _delay(100);
      return new URLSearchParams(window.location.search).get("publishPreview") ? "C:\\preview\\npm-workspace" : "";
    },

    async inspectProject(path) {
      if (IS_TAURI) return _invoke("inspect_project", { path });
      await _delay(300);
      return {
        path,
        nodeRequirement: "v18.20.0",
        requirementSource: ".nvmrc",
        packageManager: "pnpm@9.15.0",
        registry: "https://registry.npmmirror.com",
        hasNvmrc: true,
        hasPackageJson: true,
        hasNpmrc: true,
        currentNode: "v26.7.0",
        nodeInstalled: true,
      };
    },

    async applyProject(path) {
      if (IS_TAURI) return _invoke("apply_project", { path });
      await _delay(500);
      return "已应用项目配置（模拟）";
    },

    async readProjectFile(path, name) {
      if (IS_TAURI) return _invoke("read_project_file", { path, name });
      await _delay(120);
      if (name === "package.json") return '{ "name": "demo", "packageManager": "pnpm@9.15.0" }';
      if (name === ".npmrc") return "registry=https://registry.npmmirror.com\n";
      return "v22.20.0\n";
    },

    async writeProjectFile(path, name, content) {
      if (IS_TAURI) return _invoke("write_project_file", { path, name, content });
      await _delay(150);
      return content;
    },

    /* ---------------- Python ---------------- */

    async detectPython() {
      if (IS_TAURI) return _invoke("detect_python");
      await _delay(650);
      return {
        installed: true,
        storeStub: false,
        realPythonExe: "C:\\Python312\\python.exe",
        pythonVersion: "3.12.4",
        pipVersion: "pip 24.0",
        inPath: true,
        pythonDirInPath: "C:\\Python312",
        foreign: [],
        network: [
          { name: "官方 PyPI", url: "https://pypi.org/simple/", ok: true, latencyMs: 263 },
          { name: "清华 TUNA", url: "https://pypi.tuna.tsinghua.edu.cn/simple/", ok: true, latencyMs: 529 },
          { name: "阿里云", url: "https://mirrors.aliyun.com/pypi/simple/", ok: true, latencyMs: 64 },
        ],
      };
    },

    async readPipIndex() {
      if (IS_TAURI) return _invoke("read_pip_index");
      await _delay(120);
      return "https://pypi.tuna.tsinghua.edu.cn/simple";
    },

    async setPipIndex(url) {
      if (IS_TAURI) return _invoke("set_pip_index", { url });
      await _delay(300);
      return { ok: true, path: "%APPDATA%\\pip\\pip.ini" };
    },

    async testPypi(url) {
      if (IS_TAURI) return _invoke("test_pypi", { url });
      await _delay(600);
      return { reachable: true, status: 200, latencyMs: 68, throughputKbs: 412.5, needsAuth: false, error: null };
    },

    async listPythonVersions() {
      if (IS_TAURI) return _invoke("list_python_versions");
      await _delay(200);
      return {
        pyLauncherDetected: false,
        installDir: "C:\\Users\\dev\\AppData\\Local\\Programs\\python",
        installed: [
          { version: "3.12.4", current: true, sizeMb: 112 },
          { version: "3.11.9", current: false, sizeMb: 108 },
        ],
        available: ["3.14.7", "3.13.15", "3.12.13", "3.11.15", "3.10.20"],
        current: "3.12.4",
      };
    },

    async installPythonVersion(version, { onProgress } = {}) {
      if (IS_TAURI) {
        let unlisten;
        try {
          unlisten = await window.__TAURI__.event.listen("python-version-progress", (e) => {
            onProgress?.(e.payload?.stage);
          });
        } catch (_e) { /* 忽略监听失败 */ }
        try {
          return await _invoke("install_python_version", { version });
        } finally {
          unlisten?.();
        }
      }
      onProgress?.("downloading");
      await _delay(500);
      onProgress?.("extracting");
      await _delay(400);
      onProgress?.("done");
      return { ok: true, version, dir: "C:\\Users\\dev\\AppData\\Local\\Programs\\python\\" + version };
    },

    async switchPythonVersion(version) {
      if (IS_TAURI) return _invoke("switch_python_version", { version });
      await _delay(300);
      return "C:\\Users\\dev\\AppData\\Local\\Programs\\python\\" + version;
    },

    async uninstallPythonVersion(version) {
      if (IS_TAURI) return _invoke("uninstall_python_version", { version });
      await _delay(300);
      return "C:\\Users\\dev\\AppData\\Local\\Programs\\python\\" + version;
    },

    async listPythonPkgManagers() {
      if (IS_TAURI) return _invoke("list_python_pkg_managers");
      await _delay(200);
      return [
        { name: "pip", installed: true, version: "24.0", binPath: "python -m pip", dataDir: "C:\\Users\\dev\\AppData\\Local\\pip\\cache" },
        { name: "uv", installed: false, version: null, binPath: null, dataDir: "C:\\Users\\dev\\AppData\\Local\\uv" },
        { name: "poetry", installed: true, version: "1.8.3", binPath: "C:\\...\\poetry.exe", dataDir: "C:\\Users\\dev\\AppData\\Roaming\\pypoetry" },
        { name: "pipenv", installed: false, version: null, binPath: null, dataDir: "C:\\Users\\dev\\AppData\\Local\\pipenv" },
        { name: "conda", installed: false, version: null, binPath: null, dataDir: "C:\\Users\\dev\\miniconda3" },
      ];
    },

    async installPythonPkgManager(name, version) {
      if (IS_TAURI) return _invoke("install_python_pkg_manager", { name, version });
      await _delay(800);
      return `${name} 已安装（模拟）`;
    },

    async inspectPythonProject(path) {
      if (IS_TAURI) return _invoke("inspect_python_project", { path });
      await _delay(300);
      return {
        path,
        pythonRequirement: "3.12.4",
        requirementSource: ".python-version",
        pkgTool: "uv",
        index: null,
        hasPythonVersionFile: true,
        hasPyprojectToml: true,
        hasRuntimeTxt: false,
        hasPipfile: false,
        currentPython: "3.14.5",
        pythonInstalled: false,
      };
    },

    async applyPythonProject(path) {
      if (IS_TAURI) return _invoke("apply_python_project", { path });
      await _delay(500);
      return "已应用 Python 项目配置（模拟）";
    },
  };
})();

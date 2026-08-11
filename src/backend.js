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
      return window.__TAURI__.core.invoke(cmd, args);
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

  const MOCK_REGISTRY = "http://192.168.3.8:8980/repository/npm-group/";

  /* ---------------- 后端 API ---------------- */

  const STAGE_TEXT = {
    connecting: "连接镜像源…",
    downloading: "下载中…",
    extracting: "解压中…",
    done: "完成",
  };

  window.Backend = {
    IS_TAURI,

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
      if (IS_TAURI && window.__TAURI__.dialog) {
        const sel = await window.__TAURI__.dialog.open({ directory: true, title: "选择仓库目录" });
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

    async gitOpenInManager(path, select) {
      if (IS_TAURI) return _invoke("git_open_in_manager", { path, select });
      await _delay(100);
      return null;
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

    /* ---------------- 项目管理 ---------------- */

    async pickProject() {
      if (IS_TAURI && window.__TAURI__.dialog) {
        const sel = await window.__TAURI__.dialog.open({ directory: true, title: "选择项目目录" });
        return typeof sel === "string" ? sel : "";
      }
      await _delay(100);
      return "";
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
  };
})();

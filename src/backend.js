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
  };
})();

/* Go 页面：沿用现有主题、确认弹窗和控制台，独立于其他语言页面。 */
(function () {
  "use strict";
  const $ = (selector) => document.querySelector(selector);
  const escape = (text) => String(text ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const call = (action, args = {}) => window.Backend.goCommand({ action, ...args });
  let ui;
  let activeTab = "status";
  let projectPath = "";
  let statusRequest = 0;
  const stageLabels = { downloading: "下载 Go 安装包…", verifying: "校验 SHA256…", extracting: "解压 Go…", dependencies: "下载项目依赖…", done: "安装完成" };

  function infoRows(items) {
    return `<dl class="go-details">${items.map(([label, value]) => `<dt>${escape(label)}</dt><dd class="mono">${escape(value || "—")}</dd>`).join("")}</dl>`;
  }

  function error(panel, error) {
    const box = $(`#go${panel}Error`);
    if (box) { box.textContent = error?.message || String(error); box.classList.remove("hidden"); }
    ui.log("err", `Go：${error?.message || error}`);
  }

  async function load(panel, task) {
    const box = $(`#go${panel}Error`);
    if (box) { box.textContent = ""; box.classList.add("hidden"); }
    const refresh = $(`#goPanel-${panel} [data-go-refresh]`);
    if (refresh) refresh.disabled = true;
    try { await task(); } catch (e) { error(panel, e); }
    finally { if (refresh) refresh.disabled = false; }
  }

  async function run(action, args, text) {
    let unlisten;
    try {
      if (window.Backend.IS_TAURI) {
        unlisten = await window.__TAURI__.event.listen("go-progress", (event) => {
          const label = stageLabels[event.payload?.stage];
          if (label) { $(".fsl-text").textContent = label; ui.log("info", label); }
        });
      }
      const result = await ui.loading(text, () => call(action, args));
      ui.log("info", result.message || "Go 操作完成");
      $("#goNotice").textContent = result.message || "Go 配置已更新";
      $("#goNotice").classList.remove("hidden");
      return result;
    } finally { unlisten?.(); }
  }

  async function status() {
    const id = ++statusRequest;
    await load("status", async () => {
      $("#goStatusInfo").textContent = "检测 Go 环境中…";
      const data = await call("status");
      if (id !== statusRequest) return;
      const env = data.env || {};
      $("#goStatusInfo").innerHTML = infoRows([
        ["安装状态", data.installed ? (data.managed ? "已安装 · 本工具托管" : "已安装 · 外部安装") : "未安装"],
        ["当前版本", data.version], ["可执行文件", data.path], ["GOROOT", env.GOROOT],
        ["GOPATH", env.GOPATH], ["GOBIN", env.GOBIN || "默认使用 GOPATH/bin"],
        ["GOPROXY", env.GOPROXY], ["GOSUMDB", env.GOSUMDB], ["GOTOOLCHAIN", env.GOTOOLCHAIN],
      ]);
      $("#goFixPath").disabled = !data.managed;
      $("#goStatusInstall").textContent = data.installed ? "安装其他版本" : "安装 Go";
      const warnings = [];
      if (data.gorootOverride) warnings.push(`检测到外部 GOROOT=${data.gorootOverride}，切换版本后请检查该环境变量，避免终端继续使用旧工具链。`);
      if (data.proxyOverride) warnings.push(`GOPROXY 环境变量正在覆盖配置文件：${data.proxyOverride}`);
      if (data.shadowedPath) warnings.push(`终端 PATH 优先命中 ${data.shadowedPath}，请调整外部安装的 PATH 顺序。`);
      $("#goStatusWarning").textContent = warnings.join("\n");
      $("#goStatusWarning").classList.toggle("hidden", !warnings.length);
      if ($('.nav-item.active')?.dataset.page === "go") {
        const pill = $("#healthPill");
        pill.className = `pill ${data.installed ? "pill-ok" : "pill-warn"}`;
        pill.textContent = data.installed ? "● 已安装" : "● 未安装";
      }
    });
  }

  async function proxy() {
    await load("proxy", async () => {
      const env = await call("proxy");
      $("#goProxyUrl").value = env.GOPROXY || "https://proxy.golang.org,direct";
      $("#goPrivate").value = env.GOPRIVATE || "";
      $("#goProxyConfig").textContent = `配置文件：${env.GOENV || "—"}；校验服务：${env.GOSUMDB || "默认"}`;
    });
  }

  async function versions() {
    await load("versions", async () => {
      $("#goInstalled").textContent = "读取版本列表中…";
      const data = await call("versions");
      $("#goInstallDir").textContent = `托管目录：${data.installDir}`;
      $("#goInstalled").innerHTML = data.installed.length ? data.installed.map((item) =>
        `<div class="go-list-row"><div><strong class="mono">${escape(item.version)}</strong>${item.current ? ' <span class="tag tag-info">当前</span>' : ""}</div><div class="go-row-actions"><button class="btn btn-mini btn-ghost" data-go-action="switch" data-version="${escape(item.version)}" ${item.current ? "disabled" : ""}>切换</button><button class="btn btn-mini btn-ghost btn-danger" data-go-action="uninstall" data-version="${escape(item.version)}">卸载</button></div></div>`
      ).join("") : '<div class="empty">尚无托管版本，选择可用版本或输入版本号安装。</div>';
      $("#goAvailable").innerHTML = data.available.map((version) => `<option value="${escape(version)}">`).join("");
      $("#goVersionInput").placeholder = data.available[0] || "如 go1.26.1；留空安装最新稳定版";
      if (data.error) error("versions", new Error(`在线版本查询失败，已安装版本仍可管理：${data.error}`));
    });
  }

  async function tools() {
    await load("tools", async () => {
      $("#goToolsList").textContent = "检测开发工具中…";
      const items = await call("tools");
      const labels = { gopls: "Go 语言服务", dlv: "Delve 调试器", goimports: "导入整理与格式化" };
      $("#goToolsList").innerHTML = items.map((item) => `<div class="go-list-row"><div><strong>${escape(item.name)}</strong><span class="hint"> · ${labels[item.name] || ""}</span><div class="go-meta mono">${escape(item.installed ? (item.version || "已安装") : "未安装")}<br>${escape(item.path || item.dataDir)}</div></div><button class="btn btn-mini btn-ghost" data-go-tool="${escape(item.name)}">${item.installed ? "更新/指定版本" : "安装"}</button></div>`).join("");
    });
  }

  async function project() {
    if (!projectPath) return;
    await load("project", async () => {
      $("#goProjectInfo").textContent = "读取项目配置中…";
      $("#goProjectApply").disabled = true;
      const data = await call("project", { path: projectPath });
      $("#goProjectInfo").innerHTML = infoRows([
        ["项目目录", data.path], ["模块", data.module], ["配置来源", data.source],
        ["最低 Go 版本", data.goVersion], ["建议工具链", data.toolchain], ["版本要求", data.requiredVersion],
      ]);
      $("#goProjectApply").disabled = false;
    });
  }

  function activate(tab) {
    activeTab = tab;
    document.querySelectorAll(".tab-panel").forEach((panel) => panel.classList.add("hidden"));
    document.querySelectorAll("#goTabBar .tab-item").forEach((button) => button.classList.toggle("active", button.dataset.goTab === tab));
    $(`#goPanel-${tab}`).classList.remove("hidden");
    ({ status, proxy, versions, tools, project })[tab]();
  }

  function init(options) {
    ui = options;
    $("#goTabBar").addEventListener("click", (event) => {
      const button = event.target.closest("[data-go-tab]");
      if (button) activate(button.dataset.goTab);
    });
    document.querySelectorAll("[data-go-refresh]").forEach((button) => button.addEventListener("click", () => activate(activeTab)));
    $("#goStatusInstall").addEventListener("click", () => activate("versions"));
    $("#goFixPath").addEventListener("click", async () => {
      try { await run("fix-path", {}, "修复 Go PATH…"); await status(); } catch (e) { error("status", e); }
    });
    $("#goProxyPreset").addEventListener("change", (event) => {
      if (event.target.value) $("#goProxyUrl").value = event.target.value;
    });
    $("#goProxySave").addEventListener("click", async () => {
      try {
        const result = await run("proxy-set", { url: $("#goProxyUrl").value.trim(), private: $("#goPrivate").value.trim() }, "保存 Go 代理…");
        if (result.proxyOverride) error("proxy", new Error(`配置已保存，但当前 GOPROXY 环境变量优先：${result.proxyOverride}`));
      } catch (e) { error("proxy", e); }
    });
    $("#goProxyTest").addEventListener("click", async () => {
      try {
        const results = await ui.loading("测试 Go 模块代理…", () => call("test", { url: $("#goProxyUrl").value.trim() }));
        $("#goProxyResult").textContent = results.map((r) => `${r.url}：${r.ok ? `可达 · ${r.latencyMs} ms` : `连接失败 · ${r.error}`}`).join("\n");
      } catch (e) { error("proxy", e); }
    });
    $("#goInstall").addEventListener("click", async () => {
      try {
        await run("install", { version: $("#goVersionInput").value.trim() || null }, "准备安装 Go…");
        await versions();
      } catch (e) { error("versions", e); }
    });
    $("#goInstalled").addEventListener("click", async (event) => {
      const button = event.target.closest("[data-go-action]");
      if (!button) return;
      const { goAction: action, version } = button.dataset;
      if (action === "uninstall" && !await ui.confirm("卸载 Go", `将删除本工具托管的 ${version}，当前版本会同时从 PATH 停用。项目代码和 GOPATH 中的数据保留。`, "卸载")) return;
      try { await run(action, { version }, action === "switch" ? "切换 Go 版本…" : "卸载 Go…"); await versions(); }
      catch (e) { error("versions", e); }
    });
    $("#goToolsList").addEventListener("click", (event) => {
      const button = event.target.closest("[data-go-tool]");
      if (!button) return;
      const name = button.dataset.goTool;
      ui.form({ title: `安装 ${name}`, hint: "通过 go install 安装到 GOBIN 或 GOPATH/bin。", fields: [
        { key: "version", label: "工具版本", type: "text", placeholder: "留空安装 latest，如 v0.20.0" },
      ], submitText: "安装", onSubmit: async (values) => {
        await run("tool-install", { name, version: values.version.trim() || null }, `安装 ${name}…`);
        await tools();
      } });
    });
    $("#goProjectPick").addEventListener("click", async () => {
      try {
        const path = await window.Backend.goPickProject();
        if (path) { projectPath = path; await project(); }
      } catch (e) { error("project", e); }
    });
    $("#goProjectApply").addEventListener("click", async () => {
      if (!await ui.confirm("应用 Go 项目环境", "将安装并切换到符合项目要求的 Go 版本，并在该项目目录下载模块依赖。", "应用")) return;
      try { await run("project-apply", { path: projectPath }, "准备项目 Go 环境…"); await project(); }
      catch (e) { error("project", e); }
    });
  }

  window.GoUI = { init, activate, status };
})();

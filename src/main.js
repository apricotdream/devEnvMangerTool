/**
 * 前端 UI 逻辑（模拟数据驱动，等待 Rust 后端接入）。
 */
(function () {
  "use strict";

  const $ = (sel) => document.querySelector(sel);
  const $$ = (sel) => Array.from(document.querySelectorAll(sel));

  const els = {
    console: $("#console"),
    btnDetect: $("#btnDetect"),
    btnReprobe: $("#btnReprobe"),
    btnApplyMirror: $("#btnApplyMirror"),
    btnInstall: $("#btnInstall"),
    btnFixPath: $("#btnFixPath"),
    btnClearLog: $("#btnClearLog"),
    customRegistry: $("#customRegistry"),
    healthPill: $("#healthPill"),
    netList: $("#netList"),
    currentRegistry: $("#currentRegistry"),
    progressArea: $("#progressArea"),
    progressFill: $("#progressFill"),
    progressText: $("#progressText"),
    progressPct: $("#progressPct"),
    installPath: $("#installPath"),
    fixStatus: $("#fixStatus"),
    fixHint: $("#fixHint"),
    modalBackdrop: $("#modalBackdrop"),
    modalTitle: $("#modalTitle"),
    modalBody: $("#modalBody"),
    modalCancel: $("#modalCancel"),
    modalConfirm: $("#modalConfirm"),
    envTag: $("#envTag"),
    tabBar: $("#tabBar"),
    gitTabBar: $("#gitTabBar"),
    btnTheme: $("#btnTheme"),
    themeIcon: $("#themeIcon"),
    btnTestRegistry: $("#btnTestRegistry"),
    testBadge: $("#testBadge"),
    // 版本 Tab
    nvmBanner: $("#nvmBanner"),
    btnUninstallNvm: $("#btnUninstallNvm"),
    btnRefreshVersions: $("#btnRefreshVersions"),
    currentVersion: $("#currentVersion"),
    installDir: $("#installDir"),
    installedList: $("#installedList"),
    availableList: $("#availableList"),
    versionProgressArea: $("#versionProgressArea"),
    versionProgressFill: $("#versionProgressFill"),
    versionProgressText: $("#versionProgressText"),
    versionProgressPct: $("#versionProgressPct"),
    // 账号 Tab
    btnAddAccount: $("#btnAddAccount"),
    accountList: $("#accountList"),
    accountForm: $("#accountForm"),
    accId: $("#accId"),
    accName: $("#accName"),
    accRegistry: $("#accRegistry"),
    accAuthType: $("#accAuthType"),
    tokenField: $("#tokenField"),
    basicFields: $("#basicFields"),
    accToken: $("#accToken"),
    accUser: $("#accUser"),
    accPass: $("#accPass"),
    btnToggleToken: $("#btnToggleToken"),
    btnSaveAccount: $("#btnSaveAccount"),
    btnCancelAccount: $("#btnCancelAccount"),
    // Git Tab
    gitEnvBanner: $("#gitEnvBanner"),
    gitOvAccounts: $("#gitOvAccounts"),
    gitOvIdentity: $("#gitOvIdentity"),
    gitOvKeys: $("#gitOvKeys"),
    gitOvEnv: $("#gitOvEnv"),
    gitGlobalIdentity: $("#gitGlobalIdentity"),
    btnGitRefresh: $("#btnGitRefresh"),
    btnGitAdd: $("#btnGitAdd"),
    gitAccountList: $("#gitAccountList"),
    gitAccountForm: $("#gitAccountForm"),
    gitSourceAlias: $("#gitSourceAlias"),
    gitAlias: $("#gitAlias"),
    gitHostName: $("#gitHostName"),
    gitPort: $("#gitPort"),
    gitSshUser: $("#gitSshUser"),
    gitName: $("#gitName"),
    gitEmail: $("#gitEmail"),
    gitKeyPath: $("#gitKeyPath"),
    gitGenerateKey: $("#gitGenerateKey"),
    btnGitSave: $("#btnGitSave"),
    btnGitCancel: $("#btnGitCancel"),
    gitKeyAccount: $("#gitKeyAccount"),
    gitKeyInfo: $("#gitKeyInfo"),
    gitPubKey: $("#gitPubKey"),
    gitCloneHint: $("#gitCloneHint"),
    btnGitCopyPubKey: $("#btnGitCopyPubKey"),
    btnGitKeyUrl: $("#btnGitKeyUrl"),
    btnGitGenKey: $("#btnGitGenKey"),
    btnGitSwitchGlobal: $("#btnGitSwitchGlobal"),
    btnGitSwitchLocal: $("#btnGitSwitchLocal"),
    gitLocalRepo: $("#gitLocalRepo"),
    gitRepoPath: $("#gitRepoPath"),
    btnGitPickRepo: $("#btnGitPickRepo"),
    btnGitSyncRepo: $("#btnGitSyncRepo"),
    gitSyncResult: $("#gitSyncResult"),
    _gitSelectedAlias: null,
  };

  let detecting = false;
  let busy = false;

  /* ---------------- 日志 ---------------- */

  function now() {
    return new Date().toLocaleTimeString("zh-CN", { hour12: false });
  }

  function log(level, msg) {
    const line = document.createElement("div");
    line.className = `line ${level}`;
    line.innerHTML = `<span class="t">${now()}</span> ${escapeHtml(msg)}`;
    els.console.appendChild(line);
    els.console.scrollTop = els.console.scrollHeight;
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) => ({
      "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
    })[c]);
  }

  /* ---------------- 状态渲染 ---------------- */

  function setStateDot(el, state) {
    el.className = `state-dot ${state}`;
  }

  function renderDetect(env) {
    // 安装状态
    const inst = $("#cardInstall [data-field=installed]");
    inst.textContent = env.installed ? "已安装" : "未安装";
    const instDot = inst.previousElementSibling;
    setStateDot(instDot, env.installed ? "ok" : "err");

    // 版本
    $("#cardVersion [data-field=version]").textContent = env.nodeVersion || "-";
    $("#cardNpm [data-field=npmVersion]").textContent = env.npmVersion || "-";

    // PATH
    const pathEl = $("#cardPath [data-field=path]");
    const pathDot = pathEl.previousElementSibling;
    if (env.installed && env.inPath) {
      pathEl.textContent = "已配置";
      pathEl.title = env.nodeDirInPath || "";
      setStateDot(pathDot, "ok");
    } else if (env.storeStub) {
      pathEl.textContent = "商店假命令干扰";
      setStateDot(pathDot, "warn");
    } else if (env.installed) {
      pathEl.textContent = "未在 PATH";
      setStateDot(pathDot, "warn");
    } else {
      pathEl.textContent = "未配置";
      setStateDot(pathDot, "err");
    }

    // 健康徽章
    const healthy = env.installed && env.inPath && !env.storeStub;
    const pill = els.healthPill;
    pill.className = "pill " + (healthy ? "pill-ok" : env.installed ? "pill-warn" : "pill-err");
    pill.textContent = healthy ? "● 健康" : env.installed ? "● 需注意" : "● 未安装";

    // 网络
    els.netList.innerHTML = "";
    for (const n of env.network) {
      const row = document.createElement("div");
      row.className = "net-row";
      row.innerHTML = `
        <div class="net-name"><span class="state-dot ${n.ok ? "ok" : "err"}"></span>${escapeHtml(n.name)}</div>
        <div class="net-latency mono">${n.ok ? `可达 · ${n.latencyMs}ms` : "不可达"}</div>`;
      els.netList.appendChild(row);
    }

    log("info", `检测完成：installed=${env.installed} version=${env.nodeVersion} stub=${env.storeStub}`);
  }

  /* ---------------- 检测 ---------------- */

  async function runDetect(reprobeOnly = false) {
    if (detecting || busy) return;
    detecting = true;
    els.btnDetect.disabled = true;
    els.btnReprobe.disabled = true;

    // 骨架屏
    if (!reprobeOnly) {
      const vals = $$(".overview-value");
      vals.forEach((v) => v.classList.add("skeleton"));
    }

    try {
      const env = await window.Backend.detectNode();
      renderDetect(env);
      await refreshRegistry();
      if (reprobeOnly) log("info", "网络测速完成");
    } catch (e) {
      log("err", `检测失败: ${e.message}`);
      els.healthPill.className = "pill pill-err";
      els.healthPill.textContent = "● 检测失败";
    } finally {
      detecting = false;
      els.btnDetect.disabled = false;
      els.btnReprobe.disabled = false;
      $$(".overview-value").forEach((v) => v.classList.remove("skeleton"));
    }
  }

  /* ---------------- 镜像 ---------------- */

  function syncRegistryUi(reg) {
    els.currentRegistry.textContent = reg || "未设置";
    const radios = $$('input[name="source"]');
    let matched = false;
    for (const r of radios) {
      if (r.value !== "__custom__" && r.value === reg) {
        r.checked = true;
        matched = true;
      }
    }
    if (!matched) {
      const custom = radios.find((r) => r.value === "__custom__");
      if (custom) {
        custom.checked = true;
        els.customRegistry.value = reg || "";
      }
    }
    els.customRegistry.disabled = !($('input[name="source"]:checked')?.value === "__custom__");
  }

  async function refreshRegistry() {
    try {
      const reg = await window.Backend.readRegistry();
      syncRegistryUi(reg);
    } catch (e) {
      log("err", `读取 registry 失败: ${e.message}`);
    }
  }

  function selectedSourceUrl() {
    const checked = $('input[name="source"]:checked');
    if (!checked) return null;
    return checked.value === "__custom__" ? els.customRegistry.value.trim() : checked.value;
  }

  async function applyMirror() {
    if (busy) return;
    const url = selectedSourceUrl();
    if (!url) {
      log("warn", "请先选择一个镜像源");
      return;
    }
    if (!/^https?:\/\/.+/.test(url)) {
      log("err", "无效 registry 地址（需以 http(s):// 开头）");
      return;
    }

    busy = true;
    els.btnApplyMirror.disabled = true;
    const result = $("#mirrorResult");
    result.textContent = "切换中…";
    try {
      const res = await window.Backend.setRegistry(url);
      if (res.ok) {
        result.textContent = "已切换 ✓";
        result.className = "result-msg ok";
        els.currentRegistry.textContent = url;
        log("info", `registry 已切换到 ${url}（写入 ${res.path}）`);
        log("warn", "已打开的终端不会生效，请重开终端");
      } else {
        result.textContent = "切换失败";
        result.className = "result-msg err";
      }
    } catch (e) {
      result.textContent = "失败";
      result.className = "result-msg err";
      log("err", `切换镜像失败: ${e.message}`);
    } finally {
      busy = false;
      els.btnApplyMirror.disabled = false;
      setTimeout(() => { result.textContent = ""; }, 3000);
    }
  }

  /* ---------------- 测试连接 ---------------- */

  async function testConnection() {
    if (busy) return;
    const url = selectedSourceUrl();
    if (!url) {
      log("warn", "请先选择一个镜像源");
      return;
    }
    busy = true;
    els.btnTestRegistry.disabled = true;
    const badge = els.testBadge;
    badge.classList.remove("hidden");
    badge.className = "test-badge pending";
    badge.textContent = "测试中…";

    try {
      const res = await window.Backend.testRegistry(url);
      let cls, text;
      if (res.reachable && res.needsAuth) {
        cls = "warn";
        text = "⚠ 可达 · 需认证";
      } else if (res.reachable) {
        cls = "ok";
        const kb = res.throughputKbs ? ` · ${Math.round(res.throughputKbs)} KB/s` : "";
        text = `✓ 可达 · ${res.latencyMs ?? "-"}ms${kb}`;
      } else {
        cls = "err";
        text = `✗ 不可达${res.error ? " · " + res.error : ""}`;
      }
      badge.className = `test-badge ${cls}`;
      badge.textContent = text;
      log("info", `测试连接 ${url} -> ${text}`);

      // 联动应用按钮：红牌（不可达）禁用
      els.btnApplyMirror.disabled = !res.reachable;
      if (!res.reachable) {
        log("warn", "测试失败，「应用镜像」已禁用；请修正地址后重试");
      }
    } catch (e) {
      badge.className = "test-badge err";
      badge.textContent = "✗ 测试失败";
      els.btnApplyMirror.disabled = true;
      log("err", `测试连接失败: ${e.message}`);
    } finally {
      busy = false;
      els.btnTestRegistry.disabled = false;
    }
  }

  /* ---------------- 主题切换 ---------------- */

  const THEME_KEY = "envmanager-theme";
  const THEME_ICONS = {
    "purple-dark": '<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>',
    light: '<circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>',
  };

  function applyTheme(theme) {
    document.documentElement.dataset.theme = theme;
    els.themeIcon.innerHTML = THEME_ICONS[theme] || THEME_ICONS["purple-dark"];
  }

  function initTheme() {
    let saved = "purple-dark";
    try {
      saved = localStorage.getItem(THEME_KEY) || "purple-dark";
    } catch (_e) { /* localStorage 不可用时用默认 */ }
    applyTheme(saved);
  }

  function toggleTheme() {
    const current = document.documentElement.dataset.theme || "purple-dark";
    const next = current === "light" ? "purple-dark" : "light";
    applyTheme(next);
    try {
      localStorage.setItem(THEME_KEY, next);
    } catch (_e) { /* 忽略持久化失败 */ }
    log("info", `主题已切换为 ${next === "light" ? "明亮" : "紫色深色"}`);
  }

  /* ---------------- Tab 切换 ---------------- */

  function activateTab(n) {
    $$(".tab-item").forEach((t) => t.classList.toggle("active", t.dataset.tab == n));
    $$(".tab-panel").forEach((p) => p.classList.add("hidden"));
    $("#tabPanel" + n).classList.remove("hidden");
    if (n == 3) loadVersions();
    if (n == 4) loadAccounts();
  }

  function bindTabs() {
    $$("#tabBar .tab-item").forEach((item) => {
      item.addEventListener("click", () => activateTab(item.dataset.tab));
    });
    // Git 子 Tab
    $$("#gitTabBar .tab-item").forEach((item) => {
      item.addEventListener("click", () => activateGitTab(item.dataset.gtab));
    });
  }

  // Git 页子 Tab 切换
  function activateGitTab(n) {
    $$("#gitTabBar .tab-item").forEach((t) => t.classList.toggle("active", t.dataset.gtab == n));
    $$(".git-panel").forEach((p) => p.classList.add("hidden"));
    $("#gitPanel" + n).classList.remove("hidden");
    if (n == 1) loadGitAccounts();
  }

  // 顶部栏随左侧导航页面切换
  function setPageTopbar(page) {
    const title = document.querySelector(".topbar-title h1");
    const subtitle = document.querySelector(".topbar-title p");
    if (page === "git") {
      title.textContent = "Git 环境";
      subtitle.textContent = "账号 · SSH · 身份 · 仓库同步";
      window.Backend.gitCheckEnv().then((env) => {
        const pill = els.healthPill;
        if (env.git && env.ssh) {
          pill.className = "pill pill-ok";
          pill.textContent = "● 已就绪";
        } else {
          pill.className = "pill pill-warn";
          pill.textContent = "● 缺失";
        }
      }).catch(() => {});
    } else {
      title.textContent = "Node.js 环境";
      subtitle.textContent = "检测 · 版本 · 镜像 · 登录";
      // node 健康徽章由 runDetect 更新
    }
  }

  /* ---------------- Node 版本 Tab ---------------- */

  async function loadVersions() {
    try {
      const info = await window.Backend.listVersions();
      const banner = els.nvmBanner;
      if (info.nvmDetected) {
        banner.textContent = "⚠ 检测到 nvm，版本切换需先卸载 nvm";
        banner.classList.remove("hidden");
        els.btnUninstallNvm.classList.remove("hidden");
      } else {
        banner.classList.add("hidden");
        els.btnUninstallNvm.classList.add("hidden");
      }
      els.currentVersion.textContent = info.current || "未设置";
      els.installDir.textContent = info.installDir || "-";
      renderInstalledVersions(info.installed || []);
      renderAvailableVersions(info.available || []);
    } catch (e) {
      log("err", `加载版本信息失败: ${e.message}`);
    }
  }

  function renderInstalledVersions(installed) {
    const list = els.installedList;
    list.innerHTML = "";
    if (!installed.length) {
      list.innerHTML = '<div class="empty">暂无已安装版本</div>';
      return;
    }
    installed.forEach((v) => {
      const div = document.createElement("div");
      div.className = "version-item";
      div.innerHTML =
        `<span class="ver">${escapeHtml(v.version)}</span>` +
        (v.current ? '<span class="version-badge">当前</span>' : "") +
        `<span class="ver-size">${v.sizeMb ?? "-"} MB</span>` +
        `<span class="ver-actions">` +
        (v.current
          ? ""
          : `<button class="btn btn-mini btn-ghost" data-act="switch" data-v="${v.version}">切换</button>`) +
        `<button class="btn btn-mini btn-ghost" data-act="uninstall" data-v="${v.version}">卸载</button>` +
        `</span>`;
      list.appendChild(div);
    });
  }

  function renderAvailableVersions(available) {
    const list = els.availableList;
    list.innerHTML = "";
    if (!available.length) {
      list.innerHTML = '<div class="empty">无法获取可用版本</div>';
      return;
    }
    available.forEach((v) => {
      const major = parseInt(v.replace("v", "").split(".")[0], 10);
      const isLts = major % 2 === 0;
      const div = document.createElement("div");
      div.className = "version-item";
      div.innerHTML =
        `<span class="ver">${escapeHtml(v)}</span>` +
        (isLts ? '<span class="version-badge lts">LTS</span>' : "") +
        `<span class="ver-actions">` +
        `<button class="btn btn-mini btn-primary" data-act="install" data-v="${v}">安装</button>` +
        `</span>`;
      list.appendChild(div);
    });
  }

  function onVersionAction(e) {
    const btn = e.target.closest("button[data-act]");
    if (!btn) return;
    const act = btn.dataset.act;
    const v = btn.dataset.v;
    if (act === "install") installVersion(v);
    else if (act === "switch") switchNodeVersion(v);
    else if (act === "uninstall") uninstallNodeVersion(v);
  }

  async function installVersion(v) {
    const info = await window.Backend.listVersions();
    if (info.nvmDetected) {
      log("warn", "检测到 nvm，请先卸载 nvm 后再安装/切换版本");
      return;
    }
    const ok = await showModal("确认安装", `将安装 Node ${v}\n\n来源: 淘宝 npmmirror`, "安装");
    if (!ok) return;

    els.versionProgressArea.classList.remove("hidden");
    els.btnRefreshVersions.disabled = true;
    try {
      const res = await window.Backend.installVersion(v, {
        onProgress: (stage) => {
          const text = {
            connecting: "连接镜像源…",
            downloading: "下载中…",
            extracting: "解压中…",
            done: "完成",
          }[stage] || stage || "…";
          els.versionProgressFill.style.width = stage === "done" ? "100%" : "55%";
          els.versionProgressText.textContent = text;
          els.versionProgressPct.textContent = stage === "done" ? "100%" : "…";
        },
      });
      log("info", `Node ${v} 安装完成: ${res.dir}`);
    } catch (err) {
      log("err", `安装失败: ${err}`);
    } finally {
      els.versionProgressArea.classList.add("hidden");
      els.btnRefreshVersions.disabled = false;
      loadVersions();
    }
  }

  async function switchNodeVersion(v) {
    const ok = await showModal("确认切换版本", `将把 PATH 切换到 Node ${v}\n\n切换后请重开终端生效。`, "切换");
    if (!ok) return;
    try {
      const dir = await window.Backend.switchVersion(v);
      log("info", `已切换到 ${v}（${dir}）`);
      log("warn", "请重开终端使 PATH 生效");
      loadVersions();
      runDetect(false);
    } catch (err) {
      log("err", `切换失败: ${err}`);
      loadVersions();
    }
  }

  async function uninstallNodeVersion(v) {
    const ok = await showModal("确认卸载", `将删除 Node ${v}\n\n此操作不可恢复。`, "删除");
    if (!ok) return;
    try {
      const dir = await window.Backend.uninstallVersion(v);
      log("info", `已卸载 ${v}`);
      loadVersions();
    } catch (err) {
      log("err", `卸载失败: ${err}`);
    }
  }

  async function handleUninstallNvm() {
    try {
      const info = await window.Backend.nvmInfo();
      if (!info.detected) {
        log("warn", "未检测到 nvm");
        loadVersions();
        return;
      }
      const dirsText = info.dirs.length ? info.dirs.join("\n") : "（无）";
      const pathText = info.pathEntries.length ? info.pathEntries.join("\n") : "（无）";
      const ok = await showModal(
        "确认卸载 nvm",
        `将执行以下操作（不可恢复）：\n\n` +
        `1. 删除目录：\n${dirsText}\n` +
        `2. 从 PATH 移除（含系统级，需管理员授权）：\n${pathText}\n\n` +
        `⚠ 警告：卸载后 nvm 管理的所有 Node 版本将被删除，` +
        `当前使用的 Node ${els.currentVersion.textContent} 将不可用！\n` +
        `之后可点击「安装 / 修复 Node」重装。`,
        "确认卸载",
      );
      if (!ok) {
        log("info", "已取消卸载 nvm");
        return;
      }
      const result = await window.Backend.uninstallNvm();
      log("info", result);
      log("warn", "nvm 已卸载，请重开终端使 PATH 生效");
      loadVersions();
    } catch (err) {
      log("err", `卸载 nvm 失败: ${err}`);
    }
  }

  /* ---------------- 上传登录 Tab ---------------- */

  async function loadAccounts() {
    try {
      const file = await window.Backend.listAccounts();
      els.accountsData = file;
      renderAccounts(file);
    } catch (e) {
      log("err", `加载账号失败: ${e.message}`);
    }
  }

  function renderAccounts(file) {
    const list = els.accountList;
    list.innerHTML = "";
    if (!file.accounts.length) {
      list.innerHTML = '<div class="empty">暂无账号，点击右上角「新增账号」</div>';
      return;
    }
    file.accounts.forEach((acc) => {
      const active = file.active === acc.id;
      const initial = (acc.name || "?").charAt(0).toUpperCase();
      const div = document.createElement("div");
      div.className = "account-item" + (active ? " active" : "");
      div.innerHTML =
        `<div class="acc-avatar">${escapeHtml(initial)}</div>` +
        `<div class="acc-info">` +
        `<div class="acc-name">${escapeHtml(acc.name)}${active ? '<span class="acc-active-badge">当前</span>' : ""}</div>` +
        `<div class="acc-registry">${escapeHtml(acc.registry)}</div>` +
        `</div>` +
        `<div class="acc-actions">` +
        (active
          ? ""
          : `<button class="btn btn-mini btn-ghost" data-act="switch" data-id="${acc.id}">切换</button>`) +
        `<button class="btn btn-mini btn-ghost" data-act="test" data-id="${acc.id}">测试</button>` +
        `<button class="btn btn-mini btn-ghost" data-act="edit" data-id="${acc.id}">编辑</button>` +
        `<button class="btn btn-mini btn-ghost" data-act="delete" data-id="${acc.id}">删除</button>` +
        `</div>`;
      list.appendChild(div);
    });
  }

  function onAccountAction(e) {
    const btn = e.target.closest("button[data-act]");
    if (!btn) return;
    const act = btn.dataset.act;
    const id = btn.dataset.id;
    const acc = (els.accountsData?.accounts || []).find((a) => a.id === id);
    if (act === "switch") switchAccount(acc, id);
    else if (act === "test") testAccount(acc, id);
    else if (act === "edit") openAccountForm(acc);
    else if (act === "delete") deleteAccount(acc, id);
  }

  async function switchAccount(acc, id) {
    const ok = await showModal(
      "切换账号",
      `将切换到账号「${acc?.name || id}」\n\n并把凭据写入 ~/.npmrc`,
      "切换",
    );
    if (!ok) return;
    try {
      await window.Backend.switchAccount(id);
      log("info", `已切换到账号「${acc?.name}」`);
      loadAccounts();
      refreshRegistry();
    } catch (err) {
      log("err", `切换账号失败: ${err}`);
    }
  }

  async function testAccount(acc, id) {
    try {
      const res = await window.Backend.testAccount(id);
      const kbs = res.throughputKbs ? ` · ${Math.round(res.throughputKbs)} KB/s` : "";
      const authPart = res.authUser
        ? ` · 认证: ${res.authUser}`
        : res.needsAuth
          ? " · 需认证"
          : "";
      const text = res.reachable
        ? `可达 · ${res.latencyMs}ms${kbs}${authPart}`
        : `不可达 · ${res.error || ""}`;
      log("info", `账号「${acc?.name}」测试: ${text}`);
    } catch (err) {
      log("err", `账号测试失败: ${err}`);
    }
  }

  async function deleteAccount(acc, id) {
    const ok = await showModal("删除账号", `确认删除账号「${acc?.name}」？`, "删除");
    if (!ok) return;
    try {
      await window.Backend.deleteAccount(id);
      log("info", `已删除账号「${acc?.name}」`);
      loadAccounts();
    } catch (err) {
      log("err", `删除账号失败: ${err}`);
    }
  }

  function openAccountForm(acc) {
    els.accountForm.classList.remove("hidden");
    els.accId.value = acc?.id || "";
    els.accName.value = acc?.name || "";
    els.accRegistry.value = acc?.registry || "";
    els.accAuthType.value = acc?.authType || "token";
    els.accToken.value = acc?.token || "";
    els.accUser.value = acc?.username || "";
    els.accPass.value = acc?.password || "";
    const t = els.accAuthType.value;
    els.tokenField.classList.toggle("hidden", t !== "token");
    els.basicFields.classList.toggle("hidden", t !== "basic");
    els.accToken.type = "password";
    els.btnToggleToken.textContent = "显示";
  }

  function closeAccountForm() {
    els.accountForm.classList.add("hidden");
  }

  async function saveAccountForm() {
    const name = els.accName.value.trim();
    const registry = els.accRegistry.value.trim();
    const authType = els.accAuthType.value;
    if (!name || !registry) {
      log("warn", "请填写账号名称和 Registry 地址");
      return;
    }
    if (authType === "token" && !els.accToken.value.trim()) {
      log("warn", "Token 认证需要填写 Token");
      return;
    }
    const id = els.accId.value || "acc_" + Date.now();
    const account = {
      id,
      name,
      registry,
      authType,
      token: authType === "token" ? els.accToken.value.trim() : null,
      username: authType === "basic" ? els.accUser.value.trim() : null,
      password: authType === "basic" ? els.accPass.value : null,
    };
    try {
      await window.Backend.saveAccount(account);
      log("info", `账号「${name}」已保存`);
      closeAccountForm();
      loadAccounts();
    } catch (err) {
      log("err", `保存账号失败: ${err}`);
    }
  }

  /* ---------------- Git Tab ---------------- */

  async function loadGitAccounts() {
    try {
      const accounts = await window.Backend.gitListAccounts();
      renderGitAccounts(accounts);
      // 全局身份
      const id = await window.Backend.gitGetIdentity(false, null);
      const idText = id.name ? `${id.name} <${id.email}>` : "未设置";
      els.gitGlobalIdentity.textContent = idText;
      // 概览卡片
      els.gitOvAccounts.textContent = accounts.length + " 个";
      els.gitOvIdentity.textContent = idText;
      const keys = accounts.filter((a) => a.keyPath).length;
      els.gitOvKeys.textContent = keys + " 个";
      // git/ssh 检测
      const env = await window.Backend.gitCheckEnv();
      els.gitOvEnv.textContent = env.git && env.ssh ? "已就绪" : "缺失";
      gitEnvCheck();
    } catch (e) {
      log("err", `加载 Git 账号失败: ${e.message}`);
    }
  }

  async function gitEnvCheck() {
    try {
      const env = await window.Backend.gitCheckEnv();
      const banner = els.gitEnvBanner;
      if (!env.git || !env.ssh) {
        const missing = [];
        if (!env.git) missing.push("Git");
        if (!env.ssh) missing.push("OpenSSH");
        banner.textContent =
          `⚠ 未检测到 ${missing.join("、")}。安装方式：` +
          `Windows → winget install Git.Git（SSH 在 Git 自带里）；` +
          `macOS → brew install git openssh；` +
          `Linux → sudo apt install git openssh-client`;
        banner.classList.remove("hidden");
      } else {
        banner.classList.add("hidden");
      }
    } catch (e) {
      log("warn", `Git 环境检测: ${e.message}`);
    }
  }

  function renderGitAccounts(accounts) {
    const list = els.gitAccountList;
    list.innerHTML = "";
    if (!accounts.length) {
      list.innerHTML = '<div class="empty">暂无 Git 账号，点击右上角「新增账号」</div>';
      return;
    }
    accounts.forEach((acc) => {
      const div = document.createElement("div");
      div.className = "account-item";
      const initial = (acc.alias || "?").charAt(0).toUpperCase();
      div.innerHTML =
        `<div class="acc-avatar">${escapeHtml(initial)}</div>` +
        `<div class="acc-info">` +
        `<div class="acc-name">${escapeHtml(acc.alias)} <span class="acc-registry" style="color:var(--text-faint)">${escapeHtml(acc.hostName)}:${escapeHtml(acc.port)} · ${escapeHtml(acc.sshUser)}</span></div>` +
        `<div class="acc-registry">${escapeHtml(acc.gitName)} &lt;${escapeHtml(acc.gitEmail)}&gt;</div>` +
        `</div>` +
        `<div class="acc-actions">` +
        `<button class="btn btn-mini btn-ghost" data-act="switch" data-alias="${acc.alias}">切换</button>` +
        `<button class="btn btn-mini btn-ghost" data-act="test" data-alias="${acc.alias}">测试</button>` +
        `<button class="btn btn-mini btn-ghost" data-act="key" data-alias="${acc.alias}">SSH密钥</button>` +
        `<button class="btn btn-mini btn-ghost" data-act="edit" data-alias="${acc.alias}">编辑</button>` +
        `<button class="btn btn-mini btn-ghost" data-act="delete" data-alias="${acc.alias}">删除</button>` +
        `</div>`;
      list.appendChild(div);
    });
  }

  function onGitAccountAction(e) {
    const btn = e.target.closest("button[data-act]");
    if (!btn) return;
    const act = btn.dataset.act;
    const alias = btn.dataset.alias;
    els._gitSelectedAlias = alias;
    if (act === "switch") gitSwitchIdentity(alias);
    else if (act === "test") gitTest(alias);
    else if (act === "key") {
      activateGitTab(2); // 自动跳到 SSH 密钥子 Tab
      gitShowPubKey(alias);
    }
    else if (act === "edit") openGitForm(alias);
    else if (act === "delete") gitDelete(alias);
  }

  async function gitSwitchIdentity(alias) {
    const ok = await showModal("切换 Git 身份", `将把全局 Git 身份切换为「${alias}」`, "切换");
    if (!ok) return;
    try {
      await window.Backend.gitSwitchIdentity(alias, false, null);
      log("info", `已切换 Git 身份为「${alias}」`);
      loadGitAccounts();
    } catch (err) {
      log("err", `切换身份失败: ${err}`);
    }
  }

  async function gitTest(alias) {
    try {
      const res = await window.Backend.gitTestConnection(alias);
      log("info", `账号「${alias}」连接测试: ${res.success ? "成功" : "失败"} · ${res.output}`);
    } catch (err) {
      log("err", `连接测试失败: ${err}`);
    }
  }

  async function gitShowPubKey(alias) {
    try {
      const accounts = await window.Backend.gitListAccounts();
      const acc = accounts.find((a) => a.alias === alias);
      const keyPath = acc?.keyPath || null;
      const pub = await window.Backend.gitReadPublicKey(keyPath);
      els.gitKeyAccount.textContent = `「${alias}」`;
      els.gitKeyInfo.textContent = keyPath ? `私钥: ${keyPath}` : "（未配置私钥路径）";
      els.gitPubKey.value = pub || "（无公钥，请先生成密钥）";
      // clone 指引
      const host = acc?.hostName || alias;
      const user = acc?.sshUser || "git";
      els.gitCloneHint.textContent = `公钥添加后 SSH clone: git clone git@${host}:owner/repo（或 git@${alias}:owner/repo）`;
      log("info", `已加载「${alias}」公钥`);
    } catch (err) {
      log("err", `读取公钥失败: ${err}`);
    }
  }

  async function gitCopyPubKey() {
    const pub = els.gitPubKey.value;
    if (!pub || pub.startsWith("（")) {
      log("warn", "当前没有可复制的公钥，请先加载账号的 SSH 密钥");
      return;
    }
    // 首选现代 clipboard API
    try {
      await navigator.clipboard.writeText(pub);
      log("info", "公钥已复制，可粘贴到平台的 SSH 密钥设置页");
      return;
    } catch (_e1) { /* 继续走回退 */ }
    // 回退：聚焦选中 + execCommand（兼容未聚焦窗口/旧环境）
    try {
      const ta = els.gitPubKey;
      ta.focus();
      ta.select();
      document.execCommand("copy");
      log("info", "公钥已复制（回退方式）");
    } catch (e2) {
      log("err", `复制失败: ${e2.message}`);
    }
  }

  async function gitOpenKeyUrl() {
    const alias = els._gitSelectedAlias;
    if (!alias) {
      log("warn", "请先选中一个账号（点「SSH密钥」）");
      return;
    }
    try {
      const url = await window.Backend.gitGetKeyUrl(alias);
      if (url) {
        log("info", `打开平台密钥页: ${url}`);
        window.Backend.gitOpenUrl(url);
      } else {
        log("warn", "该账号没有对应的平台密钥管理页");
      }
    } catch (err) {
      log("err", `获取密钥页失败: ${err}`);
    }
  }

  async function gitGenKey() {
    let alias = els._gitSelectedAlias;
    if (!alias) {
      const accounts = await window.Backend.gitListAccounts();
      if (accounts.length) {
        alias = accounts[0].alias;
        els._gitSelectedAlias = alias;
        log("info", `未选中账号，自动使用第一个「${alias}」`);
      } else {
        log("warn", "没有可用的 Git 账号，请先新增账号");
        return;
      }
    }
    const ok = await showModal("生成 SSH 密钥", `将为账号「${alias}」生成 RSA 密钥对`, "生成");
    if (!ok) return;
    try {
      const path = await window.Backend.gitGenerateKey(alias, "", null);
      log("info", `已生成密钥: ${path}`);
      await gitShowPubKey(alias);
      loadGitAccounts();
    } catch (err) {
      log("err", `生成密钥失败: ${err}`);
    }
  }

  async function gitSwitchGlobal() {
    const alias = els._gitSelectedAlias;
    if (!alias) {
      log("warn", "请先在账号列表点「切换」或先选中一个账号");
      return;
    }
    await gitSwitchIdentity(alias);
  }

  async function gitSwitchLocal() {
    const alias = els._gitSelectedAlias;
    const repo = els.gitLocalRepo.value.trim();
    if (!alias) {
      log("warn", "请先选中一个账号");
      return;
    }
    if (!repo) {
      log("warn", "请填写仓库目录");
      return;
    }
    try {
      await window.Backend.gitSwitchIdentity(alias, true, repo);
      log("info", `已在仓库 ${repo} 切换身份为「${alias}」`);
      loadGitAccounts();
    } catch (err) {
      log("err", `仓库级身份切换失败: ${err}`);
    }
  }

  async function gitDelete(alias) {
    const ok = await showModal("删除 Git 账号", `确认删除账号「${alias}」？\n（可同时删除 SSH 密钥）`, "删除");
    if (!ok) return;
    try {
      await window.Backend.gitDeleteAccount(alias, false);
      log("info", `已删除账号「${alias}」`);
      loadGitAccounts();
    } catch (err) {
      log("err", `删除失败: ${err}`);
    }
  }

  function openGitForm(alias) {
    els.gitAccountForm.classList.remove("hidden");
    els.gitSourceAlias.value = alias || "";
    if (alias) {
      // 编辑：填充现有账号（简化：从列表找）
      window.Backend.gitListAccounts().then((accounts) => {
        const acc = accounts.find((a) => a.alias === alias);
        if (acc) {
          els.gitAlias.value = acc.alias;
          els.gitHostName.value = acc.hostName;
          els.gitPort.value = acc.port;
          els.gitSshUser.value = acc.sshUser;
          els.gitName.value = acc.gitName;
          els.gitEmail.value = acc.gitEmail;
          els.gitKeyPath.value = acc.keyPath || "";
        }
      });
    } else {
      els.gitAlias.value = "";
      els.gitHostName.value = "";
      els.gitPort.value = "22";
      els.gitSshUser.value = "git";
      els.gitName.value = "";
      els.gitEmail.value = "";
      els.gitKeyPath.value = "";
    }
  }

  function closeGitForm() {
    els.gitAccountForm.classList.add("hidden");
  }

  async function saveGitForm() {
    const account = {
      alias: els.gitAlias.value.trim(),
      hostName: els.gitHostName.value.trim(),
      port: els.gitPort.value.trim() || "22",
      sshUser: els.gitSshUser.value.trim() || "git",
      keyPath: els.gitKeyPath.value.trim() || null,
      gitName: els.gitName.value.trim(),
      gitEmail: els.gitEmail.value.trim(),
    };
    if (!account.alias || !account.hostName) {
      log("warn", "请填写 Host 别名和主机地址");
      return;
    }
    const generateKey = els.gitGenerateKey.checked;
    const sourceAlias = els.gitSourceAlias.value;
    try {
      if (sourceAlias) {
        await window.Backend.gitUpdateAccount(sourceAlias, account, generateKey);
        log("info", `账号「${account.alias}」已更新`);
      } else {
        await window.Backend.gitAddAccount(account, generateKey);
        log("info", `账号「${account.alias}」已新增`);
      }
      closeGitForm();
      loadGitAccounts();
    } catch (err) {
      log("err", `保存 Git 账号失败: ${err}`);
    }
  }

  async function gitSyncRepo() {
    const repo = els.gitRepoPath.value.trim();
    if (!repo) {
      log("warn", "请先填写仓库目录路径");
      return;
    }
    els.gitSyncResult.textContent = "同步中…";
    try {
      const res = await window.Backend.gitSyncRepo(repo);
      els.gitSyncResult.textContent = `「${res.alias}」${res.output}`;
      els.gitSyncResult.className = "result-msg";
      log("info", `仓库同步: ${res.alias} · ${res.output}`);
    } catch (err) {
      els.gitSyncResult.textContent = `同步失败: ${err}`;
      els.gitSyncResult.className = "result-msg err";
      log("err", `仓库同步失败: ${err}`);
    }
  }

  async function gitPickRepo() {
    const path = await window.Backend.gitPickRepo();
    if (path) {
      els.gitRepoPath.value = path;
    }
  }

  /* ---------------- 确认弹窗 ---------------- */

  function showModal(title, body, confirmText = "确认执行") {
    els.modalTitle.textContent = title;
    els.modalBody.textContent = body;
    els.modalConfirm.textContent = confirmText;
    els.modalBackdrop.classList.remove("hidden");
    return new Promise((resolve) => {
      const done = (val) => {
        els.modalBackdrop.classList.add("hidden");
        els.modalCancel.removeEventListener("click", onCancel);
        els.modalConfirm.removeEventListener("click", onConfirm);
        resolve(val);
      };
      const onCancel = () => done(false);
      const onConfirm = () => done(true);
      els.modalCancel.addEventListener("click", onCancel);
      els.modalConfirm.addEventListener("click", onConfirm);
    });
  }

  /* ---------------- 一键修复 ---------------- */

  function setProgress(pct, text) {
    els.progressArea.classList.remove("hidden");
    els.progressFill.style.width = pct + "%";
    els.progressText.textContent = text;
    els.progressPct.textContent = pct + "%";
  }

  async function handleInstall() {
    if (busy) return;
    // 简化：预览阶段假设未安装或未在 PATH
    const targetDir = "C:\\Users\\dev\\AppData\\Local\\Programs\\nodejs";
    const ok = await showModal(
      "确认安装 Node",
      `将从淘宝 npmmirror 下载 Node LTS 并安装到：\n\n${targetDir}\n\n下载约 30MB。\n（预览版为模拟流程）`,
      "开始安装",
    );
    if (!ok) {
      log("info", "安装已取消");
      return;
    }

    busy = true;
    els.btnInstall.disabled = true;
    els.btnFixPath.disabled = true;
    log("cmd", "> 开始安装 Node");

    try {
      const res = await window.Backend.installNode({
        onProgress: (pct, text) => setProgress(pct, text),
      });
      if (res.ok) {
        log("info", `Node ${res.version} 安装完成 -> ${res.dir}`);
        els.installPath.textContent = `已安装: ${res.dir}`;
        // 安装后询问是否加入 PATH
        const addPath = await showModal(
          "加入 PATH",
          `是否将以下目录追加到用户 PATH？\n\n${res.dir}\n\n修改后请重开终端生效。`,
          "加入 PATH",
        );
        if (addPath) {
          const p = await window.Backend.fixPath(res.dir);
          log("info", `PATH 已更新: ${p.path}`);
          els.fixStatus.textContent = "PATH 已更新，请重开终端生效";
        }
      }
    } catch (e) {
      log("err", `安装失败: ${e.message}`);
      setProgress(0, "失败");
    } finally {
      busy = false;
      els.btnInstall.disabled = false;
      els.btnFixPath.disabled = false;
      setTimeout(() => els.progressArea.classList.add("hidden"), 2500);
    }
  }

  async function handleFixPath() {
    if (busy) return;
    const targetDir = "C:\\Users\\dev\\AppData\\Local\\Programs\\nodejs\\v26.7.0";
    const ok = await showModal(
      "确认修改 PATH",
      `将把以下目录追加到用户 PATH：\n\n${targetDir}\n\n修改后请重开终端生效。`,
      "追加到 PATH",
    );
    if (!ok) return;

    busy = true;
    try {
      const res = await window.Backend.fixPath(targetDir);
      log("info", `PATH 已更新: ${res.path}`);
      els.fixStatus.textContent = "已更新，请重开终端生效";
    } catch (e) {
      log("err", `修复 PATH 失败: ${e.message}`);
    } finally {
      busy = false;
    }
  }

  /* ---------------- 事件绑定 ---------------- */

  function bind() {
    bindTabs();

    els.btnDetect.addEventListener("click", () => {
      const activeNav = document.querySelector(".nav-item.active")?.dataset.page;
      if (activeNav === "git") {
        loadGitAccounts();
      } else {
        runDetect(false);
      }
    });
    els.btnReprobe.addEventListener("click", () => runDetect(true));
    els.btnApplyMirror.addEventListener("click", applyMirror);
    els.btnTestRegistry.addEventListener("click", testConnection);
    els.btnInstall.addEventListener("click", handleInstall);
    els.btnFixPath.addEventListener("click", handleFixPath);
    els.btnTheme.addEventListener("click", toggleTheme);

    // 版本 Tab
    els.btnRefreshVersions.addEventListener("click", loadVersions);
    els.btnUninstallNvm.addEventListener("click", handleUninstallNvm);
    els.installedList.addEventListener("click", onVersionAction);
    els.availableList.addEventListener("click", onVersionAction);

    // 账号 Tab
    els.accountList.addEventListener("click", onAccountAction);
    els.btnAddAccount.addEventListener("click", () => openAccountForm(null));
    els.btnCancelAccount.addEventListener("click", closeAccountForm);
    els.btnSaveAccount.addEventListener("click", saveAccountForm);
    els.btnToggleToken.addEventListener("click", () => {
      const input = els.accToken;
      const isPassword = input.type === "password";
      input.type = isPassword ? "text" : "password";
      els.btnToggleToken.textContent = isPassword ? "隐藏" : "显示";
    });
    els.accAuthType.addEventListener("change", () => {
      const t = els.accAuthType.value;
      els.tokenField.classList.toggle("hidden", t !== "token");
      els.basicFields.classList.toggle("hidden", t !== "basic");
    });

    els.btnClearLog.addEventListener("click", () => {
      els.console.innerHTML = "";
    });

    // 自定义源启用逻辑 + 换源时重置测试状态
    $$('input[name="source"]').forEach((r) => {
      r.addEventListener("change", () => {
        els.customRegistry.disabled = !(r.value === "__custom__" && r.checked);
        if (r.value === "__custom__" && r.checked) els.customRegistry.focus();
        els.testBadge.classList.add("hidden");
        els.btnApplyMirror.disabled = false;
      });
    });
    els.customRegistry.addEventListener("input", () => {
      els.testBadge.classList.add("hidden");
      els.btnApplyMirror.disabled = false;
    });

    // 导航（页面级切换：Node 显示 Node Tab 栏，Git 隐藏 Tab 栏只显示 git 内容）
    $$(".nav-item").forEach((item) => {
      item.addEventListener("click", () => {
        const page = item.dataset.page;
        $$(".nav-item").forEach((n) => n.classList.toggle("active", n === item));
        if (page === "node") {
          els.tabBar.classList.remove("hidden");
          els.gitTabBar.classList.add("hidden");
          setPageTopbar("node");
          activateTab(1);
        } else if (page === "git") {
          els.tabBar.classList.add("hidden");
          els.gitTabBar.classList.remove("hidden");
          $$(".tab-panel").forEach((p) => p.classList.add("hidden"));
          $("#tabPanel5").classList.remove("hidden");
          setPageTopbar("git");
          activateGitTab(1);
        } else {
          log("warn", `「${page}」将在后续里程碑支持`);
        }
      });
    });

    // Git Tab 按钮
    els.btnGitRefresh.addEventListener("click", loadGitAccounts);
    els.btnGitAdd.addEventListener("click", () => openGitForm(null));
    els.btnGitCancel.addEventListener("click", closeGitForm);
    els.btnGitSave.addEventListener("click", saveGitForm);
    els.gitAccountList.addEventListener("click", onGitAccountAction);
    els.btnGitPickRepo.addEventListener("click", gitPickRepo);
    els.btnGitSyncRepo.addEventListener("click", gitSyncRepo);
    els.btnGitGenKey.addEventListener("click", gitGenKey);
    els.btnGitSwitchGlobal.addEventListener("click", gitSwitchGlobal);
    els.btnGitSwitchLocal.addEventListener("click", gitSwitchLocal);
    els.btnGitCopyPubKey.addEventListener("click", gitCopyPubKey);
    els.btnGitKeyUrl.addEventListener("click", gitOpenKeyUrl);

    // Esc 关闭弹窗
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        els.modalBackdrop.classList.add("hidden");
      }
    });
  }

  /* ---------------- 启动 ---------------- */

  async function init() {
    bind();
    initTheme();
    if (window.Backend.IS_TAURI) {
      els.envTag.textContent = "Tauri 已连接";
      els.envTag.classList.add("tag-info");
      log("info", "EnvManager 已连接 Tauri 后端");
    } else {
      els.envTag.textContent = "模拟数据";
      log("info", "EnvManager 前端预览版启动（模拟数据）");
    }
    await runDetect(false);
  }

  document.addEventListener("DOMContentLoaded", init);
})();

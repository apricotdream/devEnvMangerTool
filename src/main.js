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
    modalBox: $("#modalBox"),
    modalTitle: $("#modalTitle"),
    modalBody: $("#modalBody"),
    modalActions: $("#modalActions"),
    modalClose: $("#modalClose"),
    fullscreenLoading: $("#fullscreenLoading"),
    envTag: $("#envTag"),
    tabBar: $("#tabBar"),
    gitTabBar: $("#gitTabBar"),
    btnTheme: $("#btnTheme"),
    themeIcon: $("#themeIcon"),
    btnOpenConfig: $("#btnOpenConfig"),
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
    gitKeyAccount: $("#gitKeyAccount"),
    gitKeyInfo: $("#gitKeyInfo"),
    gitPubKey: $("#gitPubKey"),
    gitCloneHint: $("#gitCloneHint"),
    btnGitCopyPubKey: $("#btnGitCopyPubKey"),
    btnGitKeyUrl: $("#btnGitKeyUrl"),
    btnGitGenKey: $("#btnGitGenKey"),
    btnGitSwitchGlobal: $("#btnGitSwitchGlobal"),
    btnGitSwitchLocal: $("#btnGitSwitchLocal"),
    btnGitSyncRepo: $("#btnGitSyncRepo"),
    gitSyncResult: $("#gitSyncResult"),
    _gitSelectedAlias: null,
    // 包管理器
    btnPkgRefresh: $("#btnPkgRefresh"),
    pkgList: $("#pkgList"),
    // 项目管理
    btnOpenProject: $("#btnOpenProject"),
    projectStatus: $("#projectStatus"),
    projectInfo: $("#projectInfo"),
    btnApplyProject: $("#btnApplyProject"),
    _projectPath: null,
    // 发布
    btnPubOpen: $("#btnPubOpen"),
    btnPubScan: $("#btnPubScan"),
    btnPubPublish: $("#btnPubPublish"),
    btnPubRefresh: $("#btnPubRefresh"),
    pubAccountSel: $("#pubAccountSel"),
    pubRegistry: $("#pubRegistry"),
    pubStatus: $("#pubStatus"),
    pubCaStatus: $("#pubCaStatus"),
    pubPackageList: $("#pubPackageList"),
    pubBuild: $("#pubBuild"),
    _pubProjectPath: null,
    _pubPackages: [],
    // Python
    pythonTabBar: $("#pythonTabBar"),
    pyBtnReprobe: $("#pyBtnReprobe"),
    pyNetList: $("#pyNetList"),
    pyForeignList: $("#pyForeignList"),
    pyBtnInstall: $("#pyBtnInstall"),
    pyBtnFixPath: $("#pyBtnFixPath"),
    pyFixStatus: $("#pyFixStatus"),
    pyProgressArea: $("#pyProgressArea"),
    pyProgressFill: $("#pyProgressFill"),
    pyProgressText: $("#pyProgressText"),
    pyProgressPct: $("#pyProgressPct"),
    pyInstallPath: $("#pyInstallPath"),
    pyBtnTestIndex: $("#pyBtnTestIndex"),
    pyBtnApplyIndex: $("#pyBtnApplyIndex"),
    pyTestBadge: $("#pyTestBadge"),
    pyCustomIndex: $("#pyCustomIndex"),
    pyCurrentIndex: $("#pyCurrentIndex"),
    pyMirrorResult: $("#pyMirrorResult"),
    pyLauncherBanner: $("#pyLauncherBanner"),
    pyBtnRefreshVersions: $("#pyBtnRefreshVersions"),
    pyCurrentVersion: $("#pyCurrentVersion"),
    pyInstallDir: $("#pyInstallDir"),
    pyInstalledList: $("#pyInstalledList"),
    pyAvailableList: $("#pyAvailableList"),
    pyVersionProgressArea: $("#pyVersionProgressArea"),
    pyVersionProgressFill: $("#pyVersionProgressFill"),
    pyVersionProgressText: $("#pyVersionProgressText"),
    pyVersionProgressPct: $("#pyVersionProgressPct"),
    pyBtnPkgRefresh: $("#pyBtnPkgRefresh"),
    pyPkgList: $("#pyPkgList"),
    pyBtnOpenProject: $("#pyBtnOpenProject"),
    pyProjectStatus: $("#pyProjectStatus"),
    pyProjectInfo: $("#pyProjectInfo"),
    pyBtnApplyProject: $("#pyBtnApplyProject"),
    _pyProjectPath: null,
  };

  let detecting = false;
  let pyDetecting = false;
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

  /* 按钮加载态：显示旋转圈 + 可选替换文字，结束时恢复原文字并解禁。 */
  function setBtnLoading(btn, loading, text) {
    if (!btn) return;
    if (loading) {
      if (!btn.dataset.origText) btn.dataset.origText = btn.textContent;
      btn.classList.add("is-loading");
      btn.disabled = true;
      if (text) btn.textContent = text;
    } else {
      btn.classList.remove("is-loading");
      btn.disabled = false;
      if (btn.dataset.origText) {
        btn.textContent = btn.dataset.origText;
        delete btn.dataset.origText;
      }
    }
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
    setBtnLoading(els.btnApplyMirror, true, "切换中…");
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
      setBtnLoading(els.btnApplyMirror, false);
      setTimeout(() => { result.textContent = ""; }, 3000);
    }
  }

  /* ---------------- 证书信任（自签/私有CA → 指纹锚定） ---------------- */

  function formatFingerprint(hex) {
    if (!hex) return "-";
    return (hex.match(/.{1,2}/g) || []).join(":").toUpperCase();
  }

  function showCertPrompt(info) {
    return new Promise((resolve) => {
      modalState = { kind: "cert", resolve };
      openModal({ title: "证书不受信任", width: "confirm" });
      els.modalBody.innerHTML = "";
      const wrap = document.createElement("div");
      wrap.className = "cert-prompt";
      const selfSignedLabel = info.selfSigned
        ? "是（自签名）"
        : "否（私有 CA / 中间证书不受信）";
      wrap.innerHTML =
        `<div class="cert-host">${escapeHtml(info.host)}:${escapeHtml(String(info.port))}</div>` +
        `<div class="cert-warn">该地址的 HTTPS 证书不被本机信任。请核对下方指纹，确认无误后再信任。</div>` +
        `<div class="cert-row"><span class="cert-k">主题 Subject</span><span class="cert-v mono">${escapeHtml(info.subject || "-")}</span></div>` +
        `<div class="cert-row"><span class="cert-k">签发者 Issuer</span><span class="cert-v mono">${escapeHtml(info.issuer || "-")}</span></div>` +
        `<div class="cert-row"><span class="cert-k">自签</span><span class="cert-v">${selfSignedLabel}</span></div>` +
        `<div class="cert-row"><span class="cert-k">SHA-256 指纹</span><span class="cert-v mono cert-fp">${escapeHtml(formatFingerprint(info.sha256))}</span></div>` +
        `<div class="cert-note">信任后仅对 <b>${escapeHtml(info.host)}:${escapeHtml(String(info.port))}</b> 生效，工具以该证书做锚定验证；同时写入 npm 的 <span class="mono">NODE_EXTRA_CA_CERTS</span>（新开的终端生效）。</div>`;
      els.modalBody.appendChild(wrap);
      renderActions([
        { text: "取消", cls: "btn-ghost", onClick: () => { closeModal(); resolve(false); } },
        { text: "信任此证书", cls: "btn-danger", onClick: () => { closeModal(); resolve(true); } },
      ]);
    });
  }

  // 处理测试结果中的证书不受信任：抓取证书 → 弹窗核对指纹 → 接受则落盘
  async function handleCertIssue(registry) {
    let info;
    try {
      info = await window.Backend.probeCert(registry);
    } catch (e) {
      log("err", `获取证书信息失败: ${e.message}`);
      return false;
    }
    if (info.error) {
      log("err", `获取证书信息失败: ${info.error}`);
      return false;
    }
    const ok = await showCertPrompt(info);
    if (!ok) {
      log("warn", `已取消信任 ${info.host}:${info.port}`);
      return false;
    }
    try {
      await window.Backend.acceptCert(registry);
      log("info", `已信任证书 ${info.host}:${info.port}（SHA-256 ${formatFingerprint(info.sha256)}）`);
      return true;
    } catch (e) {
      log("err", `信任证书失败: ${e.message}`);
      return false;
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
    setBtnLoading(els.btnTestRegistry, true, "测试中…");
    const badge = els.testBadge;
    badge.classList.remove("hidden");
    badge.className = "test-badge pending";
    badge.textContent = "测试中…";

    try {
      const res = await window.Backend.testRegistry(url);
      if (res.certIssue) {
        const accepted = await handleCertIssue(url);
        if (accepted) {
          busy = false;
          setBtnLoading(els.btnTestRegistry, false);
          badge.textContent = "已信任证书，重新测试…";
          return testConnection();
        }
      }
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
      setBtnLoading(els.btnTestRegistry, false);
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
    if (n == 6) loadPkgManagers();
    if (n == 7) loadProject();
    if (n == 8) {
      loadAccounts();
      loadPubTab();
    }
  }

  function bindTabs() {
    $$("#tabBar .tab-item").forEach((item) => {
      item.addEventListener("click", () => activateTab(item.dataset.tab));
    });
    // Git 子 Tab
    $$("#gitTabBar .tab-item").forEach((item) => {
      item.addEventListener("click", () => activateGitTab(item.dataset.gtab));
    });
    // Python 子 Tab
    $$("#pythonTabBar .tab-item").forEach((item) => {
      item.addEventListener("click", () => activatePythonTab(item.dataset.ptab));
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
    } else if (page === "python") {
      title.textContent = "Python 环境";
      subtitle.textContent = "检测 · 版本 · 镜像 · 包管理器";
    } else {
      title.textContent = "Node.js 环境";
      subtitle.textContent = "检测 · 版本 · 镜像 · 登录";
      // node 健康徽章由 runDetect 更新
    }
  }

  /* ---------------- Node 版本 Tab ---------------- */

  async function loadVersions() {
    setBtnLoading(els.btnRefreshVersions, true, "刷新中…");
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
    } finally {
      setBtnLoading(els.btnRefreshVersions, false);
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
        (acc.projectDir
          ? `<div class="acc-scope" title="${escapeHtml(acc.projectDir)}">项目级 · ${escapeHtml(acc.projectDir)}</div>`
          : `<div class="acc-scope">全局</div>`) +
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
    else if (act === "edit") openAccountModal(acc);
    else if (act === "delete") deleteAccount(acc, id);
  }

  async function switchAccount(acc, id) {
    const target = acc?.projectDir
      ? `项目 ${acc.projectDir}\\.npmrc`
      : "全局 ~/.npmrc";
    const ok = await showModal(
      "切换账号",
      `将切换到账号「${acc?.name || id}」\n\n并把凭据写入：\n${target}`,
      "切换",
    );
    if (!ok) return;
    try {
      await window.Backend.switchAccount(id);
      log("info", `已切换到账号「${acc?.name}」`);
      loadAccounts();
      populatePubAccounts();
      refreshRegistry();
    } catch (err) {
      log("err", `切换账号失败: ${err}`);
    }
  }

  async function testAccount(acc, id) {
    const btn = document.querySelector(`button[data-act="test"][data-id="${id}"]`);
    setBtnLoading(btn, true, "测试中…");
    try {
      const res = await window.Backend.testAccount(id);
      if (res.certIssue && acc?.registry) {
        const accepted = await handleCertIssue(acc.registry);
        if (accepted) return testAccount(acc, id);
      }
      const kbs = res.throughputKbs ? ` · ${Math.round(res.throughputKbs)} KB/s` : "";
      const authPart = res.authUser
        ? ` · 认证: ${res.authUser}`
        : res.needsAuth
          ? " · 需认证"
          : "";
      // npmjs 官方：whoami 只验证身份，不代表可发布；发布还需过 2FA（输 OTP）或 bypass-2FA token。
      const npmjsNote =
        acc?.registry && /registry\.npmjs\.org/i.test(acc.registry)
          ? "（认证≠可发布，npmjs 发布需 2FA/OTP 或 bypass-2FA token）"
          : "";
      const text = res.reachable
        ? `可达 · ${res.latencyMs}ms${kbs}${authPart}${npmjsNote}`
        : `不可达 · ${res.error || ""}`;
      log("info", `账号「${acc?.name}」测试: ${text}`);
    } catch (err) {
      log("err", `账号测试失败: ${err}`);
    } finally {
      setBtnLoading(btn, false);
    }
  }

  async function deleteAccount(acc, id) {
    const ok = await showModal("删除账号", `确认删除账号「${acc?.name}」？`, "删除");
    if (!ok) return;
    try {
      await window.Backend.deleteAccount(id);
      log("info", `已删除账号「${acc?.name}」`);
      loadAccounts();
      populatePubAccounts();
    } catch (err) {
      log("err", `删除账号失败: ${err}`);
    }
  }

  function openAccountModal(acc) {
    openFormModal({
      title: acc ? "编辑账号" : "新增账号",
      fields: [
        { key: "name", label: "账号名称", type: "text", required: true, placeholder: "如: 公司 Nexus" },
        {
          key: "registry", label: "Registry 地址", type: "text", required: true,
          placeholder: "https://192.168.3.8:8980/repository/npm-group/",
          pattern: /^https?:\/\/.+/, patternMsg: "需以 http(s):// 开头",
        },
        {
          key: "npmjsNote", type: "note",
          html: (v) => {
            const reg = String(v.registry || "").toLowerCase().replace(/\/+$/, "");
            const isNpmjs = reg === "https://registry.npmjs.org"
              || reg === "http://registry.npmjs.org"
              || reg.includes("//registry.npmjs.org");
            if (!isNpmjs) return "";
            if (v.authType === "basic") {
              return "⚠️ npmjs 官方<b>不接受 basic 认证</b>（用户名+密码），发布必失败。请改选 <b>Token</b>。";
            }
            return "npmjs 官方只认 <b>token</b>。还没有？<a href=\"https://www.npmjs.com/settings/apricotdream/tokens\" target=\"_blank\" rel=\"noopener\">去 npmjs 创建 token</a>（选 Publish 作用域，发布免 OTP）";
          },
        },
        {
          key: "authType", label: "认证类型", type: "select", defaultValue: "token",
          options: [
            { value: "token", label: "Token" },
            { value: "basic", label: "用户名 + 密码" },
          ],
        },
        {
          key: "token", label: "Token", type: "password", required: true,
          placeholder: "npm token", visibleWhen: (v) => v.authType === "token",
        },
        { key: "username", label: "用户名", type: "text", visibleWhen: (v) => v.authType === "basic" },
        { key: "password", label: "密码", type: "password", visibleWhen: (v) => v.authType === "basic" },
        {
          key: "scope", label: "作用范围", type: "select", defaultValue: "global",
          options: [
            { value: "global", label: "全局 ~/.npmrc" },
            { value: "project", label: "仅当前项目 .npmrc" },
          ],
        },
        {
          key: "projectDir", label: "目标项目", type: "dir", pick: "pickProject",
          required: true, visibleWhen: (v) => v.scope === "project",
        },
      ],
      initial: {
        name: acc?.name || "",
        registry: acc?.registry || "",
        authType: acc?.authType || "token",
        token: acc?.token || "",
        username: acc?.username || "",
        password: acc?.password || "",
        scope: acc?.projectDir ? "project" : "global",
        projectDir: acc?.projectDir || els._projectPath || "",
      },
      submitText: "保存",
      onSubmit: async (values) => {
        const authType = values.authType;
        const projectDir = values.scope === "project" ? values.projectDir.trim() : null;
        const account = {
          id: acc?.id || "acc_" + Date.now(),
          name: values.name.trim(),
          registry: values.registry.trim(),
          authType,
          token: authType === "token" ? values.token.trim() : null,
          username: authType === "basic" ? values.username.trim() : null,
          password: authType === "basic" ? values.password : null,
          projectDir,
        };
        await window.Backend.saveAccount(account);
        const synced = els.accountsData?.active === account.id;
        log("info", `账号「${account.name}」已保存${synced ? "，凭据已同步写入 .npmrc" : ""}`);
        loadAccounts();
        populatePubAccounts();
      },
    });
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
    else if (act === "edit") openGitAccountModal(alias);
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
    if (!alias) {
      log("warn", "请先选中一个账号");
      return;
    }
    openFormModal({
      title: "切换仓库级身份",
      hint: `将把仓库 .git/config 的 local 身份切换为「${alias}」，只影响该仓库，不改变全局配置。`,
      fields: [
        { key: "repo", label: "仓库目录", type: "dir", pick: "gitPickRepo", required: true },
      ],
      submitText: "切换",
      onSubmit: async (values) => {
        const repo = values.repo.trim();
        await window.Backend.gitSwitchIdentity(alias, true, repo);
        log("info", `已在仓库 ${repo} 切换身份为「${alias}」`);
        loadGitAccounts();
      },
    });
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

  async function openGitAccountModal(alias) {
    let sourceAlias = null;
    let acc = null;
    if (alias) {
      const accounts = await window.Backend.gitListAccounts();
      acc = accounts.find((a) => a.alias === alias);
      sourceAlias = acc?.alias || alias;
    }
    openFormModal({
      title: sourceAlias ? "编辑 Git 账号" : "新增 Git 账号",
      fields: [
        { key: "alias", label: "Host 别名", type: "text", required: true, placeholder: "github-work" },
        { key: "hostName", label: "主机地址", type: "text", required: true, placeholder: "github.com" },
        {
          key: "port", label: "SSH 端口", type: "number", defaultValue: "22", min: 1, max: 65535,
          required: true, validate: (v) => (Number(v) >= 1 && Number(v) <= 65535) ? null : "端口范围 1-65535",
        },
        { key: "sshUser", label: "SSH 用户", type: "text", defaultValue: "git" },
        { key: "gitName", label: "Git 用户名", type: "text", placeholder: "你的用户名" },
        {
          key: "gitEmail", label: "Git 邮箱", type: "text", placeholder: "you@example.com",
          pattern: /^[^@\s]+@[^@\s]+\.[^@\s]+$/, patternMsg: "邮箱格式不正确",
        },
        { key: "keyPath", label: "私钥路径（留空自动）", type: "text", placeholder: "C:\\Users\\you\\.ssh\\id_rsa_github" },
        { key: "generateKey", label: "同时生成 SSH 密钥", labelText: "同时生成 SSH 密钥", type: "checkbox", defaultValue: true },
      ],
      initial: {
        alias: acc?.alias || "",
        hostName: acc?.hostName || "",
        port: acc?.port || "22",
        sshUser: acc?.sshUser || "git",
        gitName: acc?.gitName || "",
        gitEmail: acc?.gitEmail || "",
        keyPath: acc?.keyPath || "",
        generateKey: true,
      },
      submitText: "保存",
      onSubmit: async (values) => {
        const account = {
          alias: values.alias.trim(),
          hostName: values.hostName.trim(),
          port: String(values.port).trim() || "22",
          sshUser: values.sshUser.trim() || "git",
          keyPath: values.keyPath.trim() || null,
          gitName: values.gitName.trim(),
          gitEmail: values.gitEmail.trim(),
        };
        const generateKey = values.generateKey;
        if (sourceAlias) {
          await window.Backend.gitUpdateAccount(sourceAlias, account, generateKey);
          log("info", `账号「${account.alias}」已更新`);
        } else {
          await window.Backend.gitAddAccount(account, generateKey);
          log("info", `账号「${account.alias}」已新增`);
        }
        loadGitAccounts();
      },
    });
  }

  async function gitSyncRepo() {
    openFormModal({
      title: "同步仓库",
      hint: "将对选择的仓库执行 git pull --ff-only，按当前身份匹配远程地址。",
      fields: [
        { key: "repo", label: "仓库目录", type: "dir", pick: "gitPickRepo", required: true },
      ],
      submitText: "同步",
      onSubmit: async (values) => {
        const repo = values.repo.trim();
        els.gitSyncResult.textContent = "同步中…";
        els.gitSyncResult.className = "result-msg";
        try {
          const res = await window.Backend.gitSyncRepo(repo);
          els.gitSyncResult.textContent = `「${res.alias}」${res.output}`;
          log("info", `仓库同步: ${res.alias} · ${res.output}`);
        } catch (err) {
          els.gitSyncResult.textContent = `同步失败: ${err}`;
          els.gitSyncResult.className = "result-msg err";
          log("err", `仓库同步失败: ${err}`);
          throw err; // 交给弹窗提示
        }
      },
    });
  }

  /* ---------------- 包管理器 Tab ---------------- */

  async function loadPkgManagers() {
    try {
      const managers = await window.Backend.listPkgManagers();
      renderPkgManagers(managers);
    } catch (e) {
      log("err", `加载包管理器失败: ${e.message}`);
    }
  }

  function renderPkgManagers(managers) {
    const list = els.pkgList;
    list.innerHTML = "";
    managers.forEach((m) => {
      const div = document.createElement("div");
      div.className = "pkg-item" + (m.installed ? " installed" : "");
      const dot = m.installed ? "ok" : "err";
      div.innerHTML =
        `<span class="pkg-name">${escapeHtml(m.name)}</span>` +
        `<div class="pkg-body">` +
        `<div class="pkg-version"><span class="state-dot ${dot}"></span> ${m.installed ? escapeHtml(m.version || "已安装") : "未安装"}</div>` +
        `<div class="pkg-meta">${m.binPath ? escapeHtml(m.binPath) : "位置: -"}${m.dataDir ? "  |  数据: " + escapeHtml(m.dataDir) : ""}</div>` +
        `</div>` +
        `<div class="pkg-actions">` +
        `<button class="btn btn-mini btn-ghost" data-act="install" data-name="${m.name}">安装/切换</button>` +
        (m.installed && m.dataDir ? `<button class="btn btn-mini btn-ghost" data-act="opendir" data-dir="${m.dataDir}">数据目录</button>` : "") +
        `</div>`;
      list.appendChild(div);
    });
  }

  function onPkgAction(e) {
    const btn = e.target.closest("button[data-act]");
    if (!btn) return;
    const act = btn.dataset.act;
    const name = btn.dataset.name;
    if (act === "install") installPkg(name);
    else if (act === "opendir") {
      window.Backend.gitOpenInManager(btn.dataset.dir, false).catch(() => {});
    }
  }

  async function installPkg(name) {
    const isBun = name === "bun";
    const hint = isBun
      ? `安装/升级 bun，留空安装最新版；可指定目标版本，如 v1.2.3。`
      : `安装/切换 ${name}，留空安装 latest；可指定版本，如 9.0.0。`;
    openFormModal({
      title: `安装/切换 ${name}`,
      hint,
      fields: [
        {
          key: "version", label: "版本号", type: "text", placeholder: "留空 = latest",
          pattern: /^v?\d+(\.\d+){1,2}(-[\w.]+)?$/, patternMsg: "版本号格式如 9.0.0 或 v1.2.3",
        },
      ],
      submitText: "继续",
      onSubmit: async (values) => {
        const version = values.version.trim() || null;
        const res = await window.Backend.installPkgManager(name, version);
        log("info", res);
        loadPkgManagers();
      },
    });
  }

  /* ---------------- 项目管理 Tab ---------------- */

  function loadProject() {
    // 有已打开项目则重新解析，否则显示空状态
    if (els._projectPath) {
      inspectProject(els._projectPath);
    }
  }

  async function openProject() {
    const path = await window.Backend.pickProject();
    if (path) {
      els._projectPath = path;
      inspectProject(path);
    }
  }

  async function inspectProject(path) {
    els.projectStatus.textContent = `正在解析 ${path} …`;
    try {
      const info = await window.Backend.inspectProject(path);
      renderProjectInfo(info);
    } catch (e) {
      els.projectStatus.textContent = `解析失败: ${e.message}`;
    }
  }

  function renderProjectInfo(info) {
    els.projectStatus.textContent = `项目: ${info.path}`;
    const box = els.projectInfo;
    let html = "";
    const nodeState = info.nodeRequirement
      ? info.nodeInstalled
        ? "已安装"
        : "未安装"
      : "-";
    const nodeMatch =
      info.nodeRequirement && info.currentNode
        ? info.nodeRequirement === info.currentNode
          ? "一致"
          : `需从 ${info.currentNode} 切换`
        : "-";
    html +=
      `<div class="pkg-item"><span class="pkg-name">Node</span><div class="pkg-body">` +
      `<div class="pkg-version"><span class="state-dot ${info.nodeInstalled ? "ok" : "warn"}"></span> 要求 ${escapeHtml(info.nodeRequirement || "-")}（${escapeHtml(info.requirementSource || "")}）</div>` +
      `<div class="pkg-meta">状态: ${nodeState} · 当前全局: ${escapeHtml(info.currentNode || "-")} · ${nodeMatch}</div>` +
      `</div></div>`;
    html +=
      `<div class="pkg-item"><span class="pkg-name">包管理器</span><div class="pkg-body">` +
      `<div class="pkg-version">${escapeHtml(info.packageManager || "未指定")}</div>` +
      `<div class="pkg-meta">来源: package.json packageManager（corepack）</div>` +
      `</div></div>`;
    html +=
      `<div class="pkg-item"><span class="pkg-name">registry</span><div class="pkg-body">` +
      `<div class="pkg-version mono">${escapeHtml(info.registry || "未指定（使用全局）")}</div>` +
      `<div class="pkg-meta">来源: 项目 ${info.hasNpmrc ? ".npmrc" : "（无项目 .npmrc）"}</div>` +
      `</div></div>`;

    // 项目配置文件：列出现有文件并提供编辑入口
    const files = [];
    if (info.hasNvmrc) files.push([".nvmrc", "Node 版本要求"]);
    if (info.hasPackageJson) files.push(["package.json", "依赖 / engines / packageManager"]);
    if (info.hasNpmrc) files.push([".npmrc", "registry 等 npm 配置"]);
    if (files.length) {
      html += `<div class="file-section-title">项目配置文件</div>`;
      for (const [name, desc] of files) {
        html +=
          `<div class="pkg-item file-row"><span class="pkg-name">${escapeHtml(name)}</span>` +
          `<div class="pkg-body"><div class="pkg-meta">${escapeHtml(desc)}</div></div>` +
          `<div class="pkg-actions"><button class="btn btn-mini" data-edit-file="${escapeHtml(name)}">编辑</button></div>` +
          `</div>`;
      }
    }

    box.innerHTML = html;
    box.querySelectorAll("[data-edit-file]").forEach((btn) => {
      btn.addEventListener("click", () => editProjectFile(btn.dataset.editFile));
    });
  }

  /* 打开项目配置文件编辑弹窗：读取 → 编辑 → 保存后重新解析项目 */
  function editProjectFile(name) {
    const path = els._projectPath;
    if (!path) return;
    modalState = { kind: "confirm", resolve: () => {} };
    openModal({ title: `编辑 ${name}`, width: "form" });
    els.modalBody.innerHTML = "";

    const wrap = document.createElement("div");
    wrap.className = "file-editor";
    const hint = document.createElement("div");
    hint.className = "modal-hint";
    hint.textContent = `${path}\\${name}`;
    const ta = document.createElement("textarea");
    ta.className = "file-editor-ta mono";
    ta.value = "加载中…";
    ta.spellcheck = false;
    ta.disabled = true;
    wrap.appendChild(hint);
    wrap.appendChild(ta);
    els.modalBody.appendChild(wrap);

    const save = async () => {
      try {
        await Backend.writeProjectFile(path, name, ta.value);
        closeModal();
        log("info", `已保存 ${name}`);
        inspectProject(path);
      } catch (e) {
        log("err", `保存 ${name} 失败: ${e}`);
      }
    };
    renderActions([
      { text: "取消", cls: "btn-ghost", onClick: closeModal },
      { text: "保存", cls: "btn-primary", onClick: save },
    ]);
    ta.addEventListener("keydown", (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        save();
      }
    });

    Backend.readProjectFile(path, name)
      .then((content) => {
        ta.value = content;
        ta.disabled = false;
        ta.focus();
      })
      .catch((e) => {
        log("err", `读取 ${name} 失败: ${e}`);
        ta.value = "";
        ta.disabled = false;
      });
  }

  async function applyProject() {
    const path = els._projectPath;
    if (!path) {
      log("warn", "请先打开一个项目目录");
      return;
    }
    const ok = await showModal("应用项目配置", `将按项目要求安装/切换 Node 版本，并激活包管理器。\n\n项目: ${path}`, "应用");
    if (!ok) return;
    try {
      const res = await window.Backend.applyProject(path);
      log("info", res);
      inspectProject(path);
      runDetect(false);
    } catch (e) {
      log("err", `应用失败: ${e}`);
    }
  }

  /* ---------------- 包发布 ---------------- */

  async function loadPubTab() {
    await populatePubAccounts();
    await refreshPubCa();
    if (els._pubProjectPath) scanPubPackages();
  }

  async function populatePubAccounts() {
    try {
      const file = await window.Backend.listAccounts();
      const sel = els.pubAccountSel;
      sel.innerHTML = '<option value="">-- 从上传登录账号选择 --</option>';
      for (const acc of file.accounts || []) {
        const opt = document.createElement("option");
        opt.value = acc.registry || "";
        opt.textContent = `${acc.name} · ${acc.registry || ""}`;
        sel.appendChild(opt);
      }
      const active = (file.accounts || []).find((a) => a.id === file.active);
      if (active?.registry) {
        sel.value = active.registry;
        els.pubRegistry.value = active.registry;
      } else if (!els.pubRegistry.value) {
        // 默认取全局下载源？发布源不同，保持空让用户填
        els.pubRegistry.value = "";
      }
    } catch (e) {
      log("warn", `加载账号用于发布源失败: ${e.message}`);
    }
  }

  async function refreshPubCa() {
    try {
      const info = await window.Backend.checkNodeExtraCa();
      const el = els.pubCaStatus;
      if (info.allPemExists) {
        if (info.envMatches) {
          el.textContent = "✓ npm 证书信任已生效（NODE_EXTRA_CA_CERTS 指向 all.pem）";
          el.className = "fix-status pub-ca ok";
        } else {
          el.textContent = "ℹ 已信任证书。发布时工具会自动注入 NODE_EXTRA_CA_CERTS，无需手动设置。";
          el.className = "fix-status pub-ca warn";
        }
      } else {
        el.textContent = "未检测到已信任证书。若发布源为自签证书，请先在上传登录/镜像源测试并信任。";
        el.className = "fix-status pub-ca warn";
      }
    } catch (e) {
      els.pubCaStatus.textContent = "证书信任状态读取失败";
    }
  }

  async function openPubProject() {
    const path = await window.Backend.pickProject();
    if (!path) return;
    els._pubProjectPath = path;
    scanPubPackages();
  }

  async function scanPubPackages() {
    const path = els._pubProjectPath;
    if (!path) {
      log("warn", "请先打开项目目录");
      return;
    }
    setBtnLoading(els.btnPubScan, true, "扫描中…");
    els.pubStatus.classList.add("status-loading");
    els.pubStatus.textContent = `正在扫描 ${path} …`;
    els.pubPackageList.innerHTML = '<div class="empty">扫描中…</div>';
    try {
      const pkgs = await window.Backend.listPublishPackages(path);
      els._pubPackages = pkgs || [];
      renderPubPackages(els._pubPackages);
      els.pubStatus.textContent = `项目: ${path} · 共 ${els._pubPackages.length} 个包`;
    } catch (e) {
      els.pubStatus.textContent = `扫描失败: ${e.message}`;
      els.pubPackageList.innerHTML = '<div class="empty">扫描失败</div>';
    } finally {
      setBtnLoading(els.btnPubScan, false);
      els.pubStatus.classList.remove("status-loading");
    }
  }

  function renderPubPackages(pkgs) {
    const box = els.pubPackageList;
    if (!pkgs.length) {
      box.innerHTML = '<div class="empty">未发现 packages/ 下的可发布包</div>';
      return;
    }
    const html = pkgs.map((p) => {
      const ckid = "pubck_" + p.dir.replace(/[^a-zA-Z0-9_]/g, "_");
      const dist = p.hasDist ? `<span class="pub-tag ok">dist ✓</span>` : `<span class="pub-tag warn">无 dist</span>`;
      const priv = p.isPrivate ? `<span class="pub-tag priv">私有</span>` : "";
      const disabled = p.isPrivate ? "disabled" : "";
      const checked = !p.isPrivate && p.hasDist ? "checked" : "";
      return `<label class="pub-pkg ${disabled ? "pub-pkg-disabled" : ""}">` +
        `<input type="checkbox" id="${ckid}" ${checked} ${disabled} />` +
        `<span class="pub-pkg-name mono">${escapeHtml(p.name)}@${escapeHtml(p.version)}</span>` +
        `${dist}${priv}` +
        `<span class="pub-pkg-dir">${escapeHtml(p.dir)}</span>` +
        `</label>`;
    }).join("");
    box.innerHTML = html;
  }

  async function publishSelected() {
    const registry = els.pubRegistry.value.trim();
    if (!registry) {
      log("warn", "请填写发布源 registry");
      return;
    }
    const selected = els._pubPackages.filter((p) => {
      const ckid = "pubck_" + p.dir.replace(/[^a-zA-Z0-9_]/g, "_");
      const cb = document.getElementById(ckid);
      return cb && cb.checked;
    });
    if (!selected.length) {
      log("warn", "未选择任何包");
      return;
    }
    const build = els.pubBuild.checked;
    const ok = await showModal(
      "发布确认",
      `将发布 ${selected.length} 个包到：\n${registry}\n\n${selected.map((p) => `· ${p.name}@${p.version}`).join("\n")}\n\n构建: ${build ? "是" : "否"}`,
      "发布",
    );
    if (!ok) return;
    setBtnLoading(els.btnPubPublish, true, "发布中…");
    els.pubStatus.classList.add("status-loading");
    els.pubStatus.textContent = `发布中（${selected.length} 个包）…`;
    try {
      const dirs = selected.map((p) => p.dir);
      let results = await window.Backend.publishPackages(dirs, registry, build, null);
      // 账号 2FA：若 npm 报 EOTP，提示输入一次性验证码并带 OTP 重发失败的包
      if (results.some((r) => r.needsOtp)) {
        const otp = await promptInputModal(
          "需要一次性验证码（2FA）",
          "该账号启用了两步验证。请在手机验证器查看 6 位验证码并输入。\n若输入后仍失败，可改用 bypass-2FA 的 Automation token（详见结果区提示）。",
          "6 位验证码",
          { text: "如何在验证器查看 OTP（npmjs 2FA 说明）", url: "https://go.npm.me/2fa-guide" },
        );
        if (otp) {
          els.pubStatus.textContent = `重新发布（带 OTP）…`;
          const failedDirs = dirs.filter((_, i) => !results[i].ok);
          results = await window.Backend.publishPackages(failedDirs, registry, build, otp);
        }
      }
      renderPubResults(results);
    } catch (e) {
      els.pubStatus.textContent = `发布失败: ${e.message}`;
      log("err", `发布失败: ${e.message}`);
    } finally {
      setBtnLoading(els.btnPubPublish, false);
      els.pubStatus.classList.remove("status-loading");
    }
  }

  function renderPubResults(results) {
    const box = els.pubPackageList;
    const okCount = results.filter((r) => r.ok).length;
    els.pubStatus.textContent = `发布完成：成功 ${okCount}/${results.length}`;
    const html = results.map((r) => {
      const badge = r.ok
        ? `<span class="pub-tag ok">✓ 成功</span>`
        : r.needsOtp
          ? `<span class="pub-tag priv">需验证码</span>`
          : `<span class="pub-tag priv">✗ 失败</span>`;
      const hintHtml = r.hint
        ? `<div class="pub-hint"><a href="${escapeHtml(r.hint.url)}" target="_blank" rel="noopener">${escapeHtml(r.hint.text)}</a></div>`
        : "";
      return `<div class="pub-result ${r.ok ? "pub-result-ok" : "pub-result-err"}">` +
        `<div class="pub-result-head">${badge} <span class="pub-pkg-name mono">${escapeHtml(r.name)}</span></div>` +
        `<div class="pub-result-msg">${escapeHtml(r.message)}</div>` +
        `${hintHtml}` +
        `</div>`;
    }).join("");
    box.innerHTML = html;
    results.forEach((r) => log(r.ok ? "info" : "err", `发布 ${r.name}: ${r.message}`));
  }

  /* ---------------- Python ---------------- */

  function activatePythonTab(n) {
    $$("#pythonTabBar .tab-item").forEach((t) => t.classList.toggle("active", t.dataset.ptab == n));
    $$(".tab-panel").forEach((p) => p.classList.add("hidden"));
    $("#pyPanel" + n).classList.remove("hidden");
    if (n == 1) loadPythonDetect(false);
    if (n == 3) loadPythonVersions();
    if (n == 4) loadPythonPkgManagers();
    if (n == 5) loadPythonProject();
  }

  function renderPythonDetect(env) {
    const inst = $("#pyCardInstall [data-field=pyInstalled]");
    inst.textContent = env.installed ? "已安装" : "未安装";
    setStateDot(inst.previousElementSibling, env.installed ? "ok" : "err");

    $("#pyCardVersion [data-field=pyVersion]").textContent = env.pythonVersion || "-";
    $("#pyCardPip [data-field=pyPip]").textContent = env.pipVersion || "-";

    const pathEl = $("#pyCardPath [data-field=pyPath]");
    const pathDot = pathEl.previousElementSibling;
    if (env.installed && env.inPath) {
      pathEl.textContent = "已配置";
      pathEl.title = env.pythonDirInPath || "";
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

    const healthy = env.installed && env.inPath && !env.storeStub;
    const pill = els.healthPill;
    pill.className = "pill " + (healthy ? "pill-ok" : env.installed ? "pill-warn" : "pill-err");
    pill.textContent = healthy ? "● 健康" : env.installed ? "● 需注意" : "● 未安装";

    els.pyNetList.innerHTML = "";
    for (const n of env.network || []) {
      const row = document.createElement("div");
      row.className = "net-row";
      row.innerHTML =
        `<div class="net-name"><span class="state-dot ${n.ok ? "ok" : "err"}"></span>${escapeHtml(n.name)}</div>` +
        `<div class="net-latency mono">${n.ok ? `可达 · ${n.latencyMs}ms` : "不可达"}</div>`;
      els.pyNetList.appendChild(row);
    }

    els.pyForeignList.innerHTML = "";
    for (const f of env.foreign || []) {
      const row = document.createElement("div");
      row.className = "net-row";
      row.innerHTML =
        `<div class="net-name"><span class="state-dot neutral"></span>外来: ${escapeHtml(f.kind)}</div>` +
        `<div class="net-latency mono">${escapeHtml(f.path)}${f.version ? " · " + escapeHtml(f.version) : ""}</div>`;
      els.pyForeignList.appendChild(row);
    }

    log("info", `Python 检测完成：installed=${env.installed} version=${env.pythonVersion} stub=${env.storeStub}`);
  }

  async function loadPythonDetect(reprobeOnly = false) {
    if (pyDetecting || busy) return;
    pyDetecting = true;
    try {
      const env = await window.Backend.detectPython();
      renderPythonDetect(env);
      await refreshPipIndex();
      if (reprobeOnly) log("info", "Python 网络测速完成");
    } catch (e) {
      log("err", `Python 检测失败: ${e.message}`);
      els.healthPill.className = "pill pill-err";
      els.healthPill.textContent = "● 检测失败";
    } finally {
      pyDetecting = false;
    }
  }

  /* ---- pip 镜像 ---- */

  function syncPipIndexUi(idx) {
    els.pyCurrentIndex.textContent = idx || "未设置";
    const radios = $$('input[name="pysource"]');
    let matched = false;
    for (const r of radios) {
      if (r.value !== "__custom__" && r.value === idx) {
        r.checked = true;
        matched = true;
      }
    }
    if (!matched) {
      const custom = radios.find((r) => r.value === "__custom__");
      if (custom) {
        custom.checked = true;
        els.pyCustomIndex.value = idx || "";
      }
    }
    els.pyCustomIndex.disabled = !($('input[name="pysource"]:checked')?.value === "__custom__");
  }

  async function refreshPipIndex() {
    try {
      const idx = await window.Backend.readPipIndex();
      syncPipIndexUi(idx);
    } catch (e) {
      log("err", `读取 pip index 失败: ${e.message}`);
    }
  }

  function selectedPypiUrl() {
    const checked = $('input[name="pysource"]:checked');
    if (!checked) return null;
    return checked.value === "__custom__" ? els.pyCustomIndex.value.trim() : checked.value;
  }

  async function applyPipMirror() {
    if (busy) return;
    const url = selectedPypiUrl();
    if (!url) {
      log("warn", "请先选择一个镜像源");
      return;
    }
    if (!/^https?:\/\/.+/.test(url)) {
      log("err", "无效 index 地址（需以 http(s):// 开头）");
      return;
    }
    busy = true;
    setBtnLoading(els.pyBtnApplyIndex, true, "切换中…");
    const result = els.pyMirrorResult;
    result.textContent = "切换中…";
    try {
      const res = await window.Backend.setPipIndex(url);
      if (res.ok) {
        result.textContent = "已切换 ✓";
        result.className = "result-msg ok";
        els.pyCurrentIndex.textContent = url;
        log("info", `pip index 已切换到 ${url}（写入 ${res.path}）`);
        log("warn", "已打开的终端不会生效，请重开终端");
      } else {
        result.textContent = "切换失败";
        result.className = "result-msg err";
      }
    } catch (e) {
      result.textContent = "失败";
      result.className = "result-msg err";
      log("err", `切换 pip 镜像失败: ${e.message}`);
    } finally {
      busy = false;
      setBtnLoading(els.pyBtnApplyIndex, false);
      setTimeout(() => { result.textContent = ""; }, 3000);
    }
  }

  async function testPipIndex() {
    if (busy) return;
    const url = selectedPypiUrl();
    if (!url) {
      log("warn", "请先选择一个镜像源");
      return;
    }
    busy = true;
    setBtnLoading(els.pyBtnTestIndex, true, "测试中…");
    const badge = els.pyTestBadge;
    badge.classList.remove("hidden");
    badge.className = "test-badge pending";
    badge.textContent = "测试中…";
    try {
      const res = await window.Backend.testPypi(url);
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
      log("info", `测试 pip 源 ${url} -> ${text}`);
      els.pyBtnApplyIndex.disabled = !res.reachable;
    } catch (e) {
      badge.className = "test-badge err";
      badge.textContent = "✗ 测试失败";
      els.pyBtnApplyIndex.disabled = true;
      log("err", `测试 pip 源失败: ${e.message}`);
    } finally {
      busy = false;
      setBtnLoading(els.pyBtnTestIndex, false);
    }
  }

  /* ---- Python 版本 ---- */

  async function loadPythonVersions() {
    setBtnLoading(els.pyBtnRefreshVersions, true, "刷新中…");
    try {
      const info = await window.Backend.listPythonVersions();
      const banner = els.pyLauncherBanner;
      if (info.pyLauncherDetected) {
        banner.textContent = "ℹ 检测到 py launcher（官方安装器），与自管版本可共存";
        banner.classList.remove("hidden");
      } else {
        banner.classList.add("hidden");
      }
      els.pyCurrentVersion.textContent = info.current || "未设置";
      els.pyInstallDir.textContent = info.installDir || "-";
      renderPyInstalled(info.installed || []);
      renderPyAvailable(info.available || []);
    } catch (e) {
      log("err", `加载 Python 版本信息失败: ${e.message}`);
    } finally {
      setBtnLoading(els.pyBtnRefreshVersions, false);
    }
  }

  function renderPyInstalled(installed) {
    const list = els.pyInstalledList;
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

  function renderPyAvailable(available) {
    const list = els.pyAvailableList;
    list.innerHTML = "";
    if (!available.length) {
      list.innerHTML = '<div class="empty">无法获取可用版本</div>';
      return;
    }
    available.forEach((v) => {
      const div = document.createElement("div");
      div.className = "version-item";
      div.innerHTML =
        `<span class="ver">${escapeHtml(v)}</span>` +
        `<span class="ver-actions">` +
        `<button class="btn btn-mini btn-primary" data-act="install" data-v="${v}">安装</button>` +
        `</span>`;
      list.appendChild(div);
    });
  }

  function onPyVersionAction(e) {
    const btn = e.target.closest("button[data-act]");
    if (!btn) return;
    const act = btn.dataset.act;
    const v = btn.dataset.v;
    if (act === "install") installPythonVersion(v);
    else if (act === "switch") switchPythonVersion(v);
    else if (act === "uninstall") uninstallPythonVersion(v);
  }

  async function installPythonVersion(v) {
    const ok = await showModal("确认安装", `将安装 Python ${v}\n\n来源: npmmirror（老版本走 python-build-standalone）`, "安装");
    if (!ok) return;
    els.pyVersionProgressArea.classList.remove("hidden");
    els.pyBtnRefreshVersions.disabled = true;
    try {
      const res = await window.Backend.installPythonVersion(v, {
        onProgress: (stage) => {
          const text = {
            connecting: "连接镜像源…",
            downloading: "下载中…",
            extracting: "解压中…",
            done: "完成",
          }[stage] || stage || "…";
          els.pyVersionProgressFill.style.width = stage === "done" ? "100%" : "55%";
          els.pyVersionProgressText.textContent = text;
          els.pyVersionProgressPct.textContent = stage === "done" ? "100%" : "…";
        },
      });
      log("info", `Python ${v} 安装完成: ${res.dir}`);
    } catch (err) {
      log("err", `安装失败: ${err}`);
    } finally {
      els.pyVersionProgressArea.classList.add("hidden");
      els.pyBtnRefreshVersions.disabled = false;
      loadPythonVersions();
    }
  }

  async function switchPythonVersion(v) {
    const ok = await showModal("确认切换版本", `将把 PATH 切换到 Python ${v}\n\n切换后请重开终端生效。`, "切换");
    if (!ok) return;
    try {
      const dir = await window.Backend.switchPythonVersion(v);
      log("info", `已切换到 ${v}（${dir}）`);
      log("warn", "请重开终端使 PATH 生效");
      loadPythonVersions();
      loadPythonDetect(false);
    } catch (err) {
      log("err", `切换失败: ${err}`);
      loadPythonVersions();
    }
  }

  async function uninstallPythonVersion(v) {
    const ok = await showModal("确认卸载", `将删除 Python ${v}\n\n此操作不可恢复。`, "删除");
    if (!ok) return;
    try {
      const dir = await window.Backend.uninstallPythonVersion(v);
      log("info", `已卸载 ${v}`);
      loadPythonVersions();
    } catch (err) {
      log("err", `卸载失败: ${err}`);
    }
  }

  function setPyProgress(pct, text) {
    els.pyProgressArea.classList.remove("hidden");
    els.pyProgressFill.style.width = pct + "%";
    els.pyProgressText.textContent = text;
    els.pyProgressPct.textContent = pct + "%";
  }

  async function handlePyInstall() {
    if (busy) return;
    const ok = await showModal("确认安装 Python", "将从 npmmirror 下载最新稳定版 Python 并安装。\n\n老版本将自动回退 python-build-standalone 源。", "开始安装");
    if (!ok) return;
    busy = true;
    els.pyBtnInstall.disabled = true;
    els.pyBtnFixPath.disabled = true;
    log("cmd", "> 开始安装 Python");
    try {
      const info = await window.Backend.listPythonVersions();
      const ver = info.available?.[0];
      if (!ver) {
        log("err", "无法获取可用版本");
        return;
      }
      const res = await window.Backend.installPythonVersion(ver, {
        onProgress: (stage) => {
          const text = { connecting: "连接镜像源…", downloading: "下载中…", extracting: "解压中…", done: "完成" }[stage] || stage || "…";
          setPyProgress(stage === "done" ? 100 : 55, text);
        },
      });
      if (res.ok) {
        log("info", `Python ${res.version} 安装完成 -> ${res.dir}`);
        els.pyInstallPath.textContent = `已安装: ${res.dir}`;
        const addPath = await showModal("加入 PATH", `是否将以下目录加入用户 PATH？\n\n${res.dir}\n\n修改后请重开终端生效。`, "加入 PATH");
        if (addPath) {
          const p = await window.Backend.switchPythonVersion(res.version);
          log("info", `PATH 已更新: ${p}`);
          els.pyFixStatus.textContent = "PATH 已更新，请重开终端生效";
        }
      }
    } catch (e) {
      log("err", `安装失败: ${e.message}`);
      setPyProgress(0, "失败");
    } finally {
      busy = false;
      els.pyBtnInstall.disabled = false;
      els.pyBtnFixPath.disabled = false;
      setTimeout(() => els.pyProgressArea.classList.add("hidden"), 2500);
    }
  }

  async function handlePyFixPath() {
    if (busy) return;
    const info = await window.Backend.listPythonVersions();
    const ver = info.current || info.installed?.[0]?.version;
    if (!ver) {
      log("warn", "未检测到已安装的 Python 版本");
      return;
    }
    const ok = await showModal("确认修改 PATH", `将把以下目录加入用户 PATH：\n\n${info.installDir}\\${ver}（及 Scripts）\n\n修改后请重开终端生效。`, "追加到 PATH");
    if (!ok) return;
    busy = true;
    try {
      const dir = await window.Backend.switchPythonVersion(ver);
      log("info", `PATH 已更新: ${dir}`);
      els.pyFixStatus.textContent = "已更新，请重开终端生效";
    } catch (e) {
      log("err", `修复 PATH 失败: ${e.message}`);
    } finally {
      busy = false;
    }
  }

  /* ---- Python 包管理器 ---- */

  async function loadPythonPkgManagers() {
    try {
      const managers = await window.Backend.listPythonPkgManagers();
      renderPythonPkgManagers(managers);
    } catch (e) {
      log("err", `加载 Python 包管理器失败: ${e.message}`);
    }
  }

  function renderPythonPkgManagers(managers) {
    const list = els.pyPkgList;
    list.innerHTML = "";
    managers.forEach((m) => {
      const div = document.createElement("div");
      div.className = "pkg-item" + (m.installed ? " installed" : "");
      const dot = m.installed ? "ok" : "err";
      div.innerHTML =
        `<span class="pkg-name">${escapeHtml(m.name)}</span>` +
        `<div class="pkg-body">` +
        `<div class="pkg-version"><span class="state-dot ${dot}"></span> ${m.installed ? escapeHtml(m.version || "已安装") : "未安装"}</div>` +
        `<div class="pkg-meta">${m.binPath ? escapeHtml(m.binPath) : "位置: -"}${m.dataDir ? "  |  数据: " + escapeHtml(m.dataDir) : ""}</div>` +
        `</div>` +
        `<div class="pkg-actions">` +
        `<button class="btn btn-mini btn-ghost" data-act="install" data-name="${m.name}">安装</button>` +
        `</div>`;
      list.appendChild(div);
    });
  }

  function onPyPkgAction(e) {
    const btn = e.target.closest("button[data-act]");
    if (!btn) return;
    const act = btn.dataset.act;
    const name = btn.dataset.name;
    if (act === "install") installPythonPkg(name);
  }

  async function installPythonPkg(name) {
    const ok = await showModal(`安装 ${name}`, `将执行: python -m pip install ${name}`, "继续");
    if (!ok) return;
    try {
      const res = await window.Backend.installPythonPkgManager(name, null);
      log("info", res);
      loadPythonPkgManagers();
    } catch (err) {
      log("err", `安装失败: ${err}`);
    }
  }

  /* ---- Python 项目 ---- */

  function loadPythonProject() {
    if (els._pyProjectPath) {
      inspectPythonProject(els._pyProjectPath);
    }
  }

  async function openPythonProject() {
    const path = await window.Backend.pickProject();
    if (path) {
      els._pyProjectPath = path;
      inspectPythonProject(path);
    }
  }

  async function inspectPythonProject(path) {
    els.pyProjectStatus.textContent = `正在解析 ${path} …`;
    try {
      const info = await window.Backend.inspectPythonProject(path);
      renderPythonProjectInfo(info);
    } catch (e) {
      els.pyProjectStatus.textContent = `解析失败: ${e.message}`;
    }
  }

  function renderPythonProjectInfo(info) {
    els.pyProjectStatus.textContent = `项目: ${info.path}`;
    const box = els.pyProjectInfo;
    let html = "";
    const reqState = info.pythonRequirement
      ? info.pythonInstalled
        ? "已安装"
        : "未安装"
      : "-";
    const match =
      info.pythonRequirement && info.currentPython
        ? info.pythonRequirement === info.currentPython
          ? "一致"
          : `需从 ${info.currentPython} 切换`
        : "-";
    html +=
      `<div class="pkg-item"><span class="pkg-name">Python</span><div class="pkg-body">` +
      `<div class="pkg-version"><span class="state-dot ${info.pythonInstalled ? "ok" : "warn"}"></span> 要求 ${escapeHtml(info.pythonRequirement || "-")}（${escapeHtml(info.requirementSource || "")}）</div>` +
      `<div class="pkg-meta">状态: ${reqState} · 当前全局: ${escapeHtml(info.currentPython || "-")} · ${match}</div>` +
      `</div></div>`;
    html +=
      `<div class="pkg-item"><span class="pkg-name">工具链</span><div class="pkg-body">` +
      `<div class="pkg-version">${escapeHtml(info.pkgTool || "未指定")}</div>` +
      `<div class="pkg-meta">来源: pyproject.toml [tool.*]</div>` +
      `</div></div>`;
    html +=
      `<div class="pkg-item"><span class="pkg-name">index</span><div class="pkg-body">` +
      `<div class="pkg-version mono">${escapeHtml(info.index || "未指定（使用全局）")}</div>` +
      `<div class="pkg-meta">来源: pyproject.toml</div>` +
      `</div></div>`;
    box.innerHTML = html;
  }

  async function applyPythonProject() {
    const path = els._pyProjectPath;
    if (!path) {
      log("warn", "请先打开一个项目目录");
      return;
    }
    const ok = await showModal("应用项目配置", `将按项目要求安装/切换 Python 版本。\n\n项目: ${path}`, "应用");
    if (!ok) return;
    try {
      const res = await window.Backend.applyPythonProject(path);
      log("info", res);
      inspectPythonProject(path);
      loadPythonDetect(false);
    } catch (e) {
      log("err", `应用失败: ${e}`);
    }
  }

  /* ---------------- 统一弹窗（确认 / 表单） ---------------- */

  let modalState = null;

  function openModal({ title, width }) {
    els.modalTitle.textContent = title;
    els.modalBox.classList.toggle("modal--form", width === "form");
    els.modalBackdrop.classList.remove("hidden");
  }

  function closeModal() {
    els.modalBackdrop.classList.add("hidden");
    modalState = null;
  }

  function renderActions(buttons) {
    els.modalActions.innerHTML = "";
    for (const b of buttons) {
      const el = document.createElement("button");
      el.className = `btn ${b.cls || "btn-ghost"}`;
      el.textContent = b.text;
      el.disabled = !!b.disabled;
      el.addEventListener("click", b.onClick);
      els.modalActions.appendChild(el);
    }
  }

  // 取消 / 遮罩 / Esc 的统一入口：confirm 直接关闭并 resolve(false)；form 走脏检查
  function requestModalClose() {
    const st = modalState;
    if (!st) return;
    if (st.kind === "confirm") {
      closeModal();
      st.resolve(false);
    } else if (st.kind === "form") {
      st.requestClose();
    }
  }

  function showModal(title, body, confirmText = "确认执行") {
    return new Promise((resolve) => {
      modalState = { kind: "confirm", resolve };
      openModal({ title, width: "confirm" });
      els.modalBody.innerHTML = "";
      const bodyEl = document.createElement("div");
      bodyEl.className = "modal-body--confirm";
      bodyEl.textContent = body;
      els.modalBody.appendChild(bodyEl);
      renderActions([
        { text: "取消", cls: "btn-ghost", onClick: () => { closeModal(); resolve(false); } },
        { text: confirmText, cls: "btn-danger", onClick: () => { closeModal(); resolve(true); } },
      ]);
    });
  }

  // 带单行输入框的弹窗：返回输入值（取消返回 null）。link 可选 {text,url}，显示为辅助链接。
  function promptInputModal(title, message, placeholder = "", link = null) {
    return new Promise((resolve) => {
      modalState = { kind: "confirm", resolve };
      openModal({ title, width: "confirm" });
      els.modalBody.innerHTML = "";
      const wrap = document.createElement("div");
      wrap.className = "modal-body--confirm";
      const msg = document.createElement("div");
      msg.style.marginBottom = "10px";
      msg.style.whiteSpace = "pre-wrap";
      msg.textContent = message;
      wrap.appendChild(msg);
      if (link && link.url) {
        const a = document.createElement("a");
        a.href = link.url;
        a.target = "_blank";
        a.rel = "noopener";
        a.className = "modal-link";
        a.textContent = link.text || link.url;
        wrap.appendChild(a);
      }
      const input = document.createElement("input");
      input.type = "text";
      input.className = "source-custom mono";
      input.placeholder = placeholder;
      input.style.width = "100%";
      wrap.appendChild(input);
      els.modalBody.appendChild(wrap);
      const finish = (val) => { closeModal(); resolve(val); };
      renderActions([
        { text: "取消", cls: "btn-ghost", onClick: () => finish(null) },
        { text: "确定", cls: "btn-primary", onClick: () => finish(input.value.trim()) },
      ]);
      input.focus();
      input.addEventListener("keydown", (e) => { if (e.key === "Enter") finish(input.value.trim()); });
    });
  }

  /* ---- 打开配置文件目录 ---- */

  function openOneConfigDir(d) {
    if (!d.exists) return;
    // select=true：文件（.npmrc/pip.ini/.gitconfig）在资源管理器中高亮；目录则正常打开
    Backend.gitOpenInManager(d.path, true).catch((e) => log("err", `打开失败 ${d.path}: ${e}`));
  }

  async function openConfigDirs() {
    let dirs;
    try {
      dirs = await Backend.listConfigDirs();
    } catch (e) {
      log("err", `读取配置目录失败: ${e}`);
      return;
    }
    modalState = { kind: "confirm", resolve: () => {} };
    openModal({ title: "配置文件目录", width: "confirm" });
    els.modalBody.innerHTML = "";
    const wrap = document.createElement("div");
    wrap.className = "config-dirs";
    const list = document.createElement("div");
    list.className = "config-dir-list";
    for (const d of dirs) {
      const row = document.createElement("div");
      row.className = "config-dir-row" + (d.exists ? "" : " missing");
      const info = document.createElement("div");
      info.className = "config-dir-info";
      const name = document.createElement("div");
      name.className = "config-dir-name";
      name.textContent = d.name + (d.exists ? "" : "（不存在）");
      const path = document.createElement("div");
      path.className = "config-dir-path mono";
      path.textContent = d.path;
      path.title = d.path;
      info.appendChild(name);
      info.appendChild(path);
      const btn = document.createElement("button");
      btn.className = "btn btn-mini";
      btn.textContent = "打开";
      btn.disabled = !d.exists;
      btn.addEventListener("click", () => openOneConfigDir(d));
      row.appendChild(info);
      row.appendChild(btn);
      list.appendChild(row);
    }
    wrap.appendChild(list);
    els.modalBody.appendChild(wrap);
    renderActions([
      {
        text: "全部打开",
        cls: "btn-primary",
        disabled: !dirs.some((d) => d.exists),
        onClick: () => {
          for (const d of dirs) openOneConfigDir(d);
        },
      },
      { text: "关闭", cls: "btn-ghost", onClick: closeModal },
    ]);
  }

  /* ---- 通用表单弹窗（schema 驱动） ---- */

  function openFormModal({ title, hint, fields, initial, submitText, onSubmit, width = "form" }) {
    const values = {};
    for (const f of fields) {
      if (f.type === "checkbox") {
        values[f.key] = initial && f.key in initial ? !!initial[f.key] : !!f.defaultValue;
      } else {
        values[f.key] = initial && f.key in initial ? initial[f.key] : (f.defaultValue ?? "");
      }
    }
    const initialSnapshot = { ...values };
    let submitting = false;

    renderFormModal();
    mountFormState();

    function mountFormState() {
      modalState = {
        kind: "form",
        requestClose: async () => {
          if (isDirty()) {
            const ok = await showModal("丢弃修改？", "表单有未保存的修改，确定要丢弃吗？", "丢弃修改");
            if (!ok) {
              renderFormModal();
              mountFormState();
              return;
            }
          }
          closeModal();
        },
      };
    }

    function isDirty() {
      return fields.some((f) => values[f.key] !== initialSnapshot[f.key]);
    }

    function renderFormModal() {
      openModal({ title, width });
      els.modalBody.innerHTML = "";
      const form = document.createElement("div");
      form.className = "form-modal";
      form.addEventListener("input", onFormInput);
      form.addEventListener("change", onFormInput);
      form.addEventListener("keydown", (e) => {
        if (e.key === "Enter" && !e.target.matches("textarea") && !e.target.matches("button")) {
          e.preventDefault();
          submit();
        }
      });

      const errBox = document.createElement("div");
      errBox.className = "form-error hidden";
      form.appendChild(errBox);

      if (hint) {
        const hintEl = document.createElement("div");
        hintEl.className = "modal-hint";
        hintEl.textContent = hint;
        form.appendChild(hintEl);
      }

      for (const f of fields) {
        form.appendChild(buildFieldWrapper(f));
      }
      els.modalBody.appendChild(form);

      renderActions([
        { text: "取消", cls: "btn-ghost", onClick: () => modalState && modalState.requestClose() },
        { text: submitText, cls: "btn-primary", onClick: submit },
      ]);
      updateFormVisibility();
      focusFirstField();
    }

    function buildFieldWrapper(f) {
      const wrapper = document.createElement("div");
      wrapper.className = "form-field";
      wrapper.dataset.fieldKey = f.key;

      if (f.type === "note") {
        const note = document.createElement("div");
        note.className = "form-note";
        note.dataset.noteFor = f.key;
        wrapper.appendChild(note);
        return wrapper;
      }
      if (f.type === "checkbox") {
        const chkLabel = document.createElement("label");
        chkLabel.className = "chk-label";
        const input = document.createElement("input");
        input.type = "checkbox";
        input.dataset.key = f.key;
        input.checked = !!values[f.key];
        chkLabel.appendChild(input);
        chkLabel.appendChild(document.createTextNode(f.labelText || f.label));
        wrapper.appendChild(chkLabel);
      } else {
        const label = document.createElement("label");
        label.textContent = f.label;
        wrapper.appendChild(label);
        wrapper.appendChild(buildControl(f));
      }

      const err = document.createElement("div");
      err.className = "field-error hidden";
      err.dataset.errorFor = f.key;
      wrapper.appendChild(err);
      return wrapper;
    }

    function buildControl(f) {
      if (f.type === "select") {
        const select = document.createElement("select");
        select.dataset.key = f.key;
        for (const o of f.options || []) {
          const opt = document.createElement("option");
          opt.value = o.value;
          opt.textContent = o.label;
          select.appendChild(opt);
        }
        select.value = values[f.key];
        return select;
      }
      if (f.type === "password") {
        const box = document.createElement("div");
        box.className = "secret-input";
        const input = document.createElement("input");
        input.type = "password";
        input.dataset.key = f.key;
        input.placeholder = f.placeholder || "";
        const toggle = document.createElement("button");
        toggle.type = "button";
        toggle.className = "btn btn-mini btn-ghost";
        toggle.textContent = "显示";
        toggle.addEventListener("click", () => {
          const show = input.type === "password";
          input.type = show ? "text" : "password";
          toggle.textContent = show ? "隐藏" : "显示";
        });
        box.appendChild(input);
        box.appendChild(toggle);
        return box;
      }
      if (f.type === "dir") {
        const box = document.createElement("div");
        box.className = "dir-picker";
        const input = document.createElement("input");
        input.type = "text";
        input.readOnly = true;
        input.dataset.key = f.key;
        input.value = values[f.key];
        input.placeholder = "选择目录…";
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "btn btn-mini btn-ghost";
        btn.textContent = "选择";
        btn.addEventListener("click", async () => {
          const path = await window.Backend[f.pick]();
          if (path) {
            input.value = path;
            values[f.key] = path;
            clearFieldError(f.key);
          }
        });
        box.appendChild(input);
        box.appendChild(btn);
        return box;
      }
      const input = document.createElement("input");
      input.type = f.type || "text";
      input.dataset.key = f.key;
      input.placeholder = f.placeholder || "";
      if (f.type === "number") {
        if (f.min) input.min = f.min;
        if (f.max) input.max = f.max;
      }
      input.value = values[f.key];
      return input;
    }

    function onFormInput(e) {
      const el = e.target.closest("[data-key]");
      if (!el || !els.modalBody.contains(el)) return;
      const key = el.dataset.key;
      if (el.type === "checkbox") values[key] = el.checked;
      else values[key] = el.value;
      clearFieldError(key);
      updateFormVisibility();
    }

    function updateFormVisibility() {
      for (const f of fields) {
        if (f.type === "note") {
          const note = els.modalBody.querySelector(`[data-note-for="${f.key}"]`);
          if (!note) continue;
          const html = f.html ? f.html(values) : "";
          note.innerHTML = html;
          note.classList.toggle("hidden", !html);
          continue;
        }
        if (!f.visibleWhen) continue;
        const wrapper = els.modalBody.querySelector(`[data-field-key="${f.key}"]`);
        if (!wrapper) continue;
        wrapper.classList.toggle("hidden", !f.visibleWhen(values));
      }
    }

    function focusFirstField() {
      const first = els.modalBody.querySelector(".form-field:not(.hidden) [data-key]");
      if (first && !first.readOnly) first.focus();
    }

    function clearFieldError(key) {
      const err = els.modalBody.querySelector(`[data-error-for="${key}"]`);
      if (err) { err.textContent = ""; err.classList.add("hidden"); }
      const input = els.modalBody.querySelector(`[data-key="${key}"]`);
      if (input) input.classList.remove("field-invalid");
    }

    function validate() {
      const errors = {};
      for (const f of fields) {
        if (f.type === "checkbox" || f.type === "note") continue;
        if (f.visibleWhen && !f.visibleWhen(values)) continue;
        const v = values[f.key];
        if (f.required && (v === "" || v == null)) { errors[f.key] = "此项必填"; continue; }
        if (v && f.pattern && !f.pattern.test(String(v))) { errors[f.key] = f.patternMsg || "格式不正确"; continue; }
        if (v && f.validate) { const msg = f.validate(v, values); if (msg) errors[f.key] = msg; }
      }
      return errors;
    }

    function showErrors(errors) {
      for (const f of fields) clearFieldError(f.key);
      const keys = Object.keys(errors);
      for (const key of keys) {
        const err = els.modalBody.querySelector(`[data-error-for="${key}"]`);
        if (err) { err.textContent = errors[key]; err.classList.remove("hidden"); }
        const input = els.modalBody.querySelector(`[data-key="${key}"]`);
        if (input) input.classList.add("field-invalid");
      }
      const firstKey = keys[0];
      const firstWrap = els.modalBody.querySelector(`[data-field-key="${firstKey}"]`);
      if (firstWrap) firstWrap.scrollIntoView({ block: "center", behavior: "smooth" });
    }

    function setFormError(msg) {
      const errBox = els.modalBody.querySelector(".form-error");
      if (!errBox) return;
      errBox.textContent = msg;
      errBox.classList.remove("hidden");
    }
    function clearFormError() {
      const errBox = els.modalBody.querySelector(".form-error");
      if (errBox) errBox.classList.add("hidden");
    }

    function setSubmitting(loading) {
      els.modalActions.querySelectorAll("button").forEach((b) => { b.disabled = loading; });
      const submitBtn = els.modalActions.querySelector(".btn-primary");
      if (submitBtn) submitBtn.textContent = loading ? "处理中…" : submitText;
    }

    async function submit() {
      if (submitting) return;
      const errors = validate();
      showErrors(errors);
      if (Object.keys(errors).length) { setFormError("请修正标记的字段后重试。"); return; }
      clearFormError();
      submitting = true;
      setSubmitting(true);
      try {
        await onSubmit({ ...values });
        closeModal();
      } catch (err) {
        submitting = false;
        setSubmitting(false);
        setFormError((err && err.message) ? err.message : String(err));
      }
    }
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
      } else if (activeNav === "python") {
        loadPythonDetect(false);
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
    els.btnOpenConfig.addEventListener("click", openConfigDirs);

    // 版本 Tab
    els.btnRefreshVersions.addEventListener("click", loadVersions);
    els.btnUninstallNvm.addEventListener("click", handleUninstallNvm);
    els.installedList.addEventListener("click", onVersionAction);
    els.availableList.addEventListener("click", onVersionAction);

    // 账号 Tab
    els.accountList.addEventListener("click", onAccountAction);
    els.btnAddAccount.addEventListener("click", () => openAccountModal(null));

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
          els.pythonTabBar.classList.add("hidden");
          setPageTopbar("node");
          activateTab(1);
        } else if (page === "git") {
          els.tabBar.classList.add("hidden");
          els.gitTabBar.classList.remove("hidden");
          els.pythonTabBar.classList.add("hidden");
          $$(".tab-panel").forEach((p) => p.classList.add("hidden"));
          $("#tabPanel5").classList.remove("hidden");
          setPageTopbar("git");
          activateGitTab(1);
        } else if (page === "python") {
          els.tabBar.classList.add("hidden");
          els.gitTabBar.classList.add("hidden");
          els.pythonTabBar.classList.remove("hidden");
          $$(".tab-panel").forEach((p) => p.classList.add("hidden"));
          setPageTopbar("python");
          activatePythonTab(1);
        } else {
          log("warn", `「${page}」将在后续里程碑支持`);
        }
      });
    });

    // Git Tab 按钮
    els.btnGitRefresh.addEventListener("click", loadGitAccounts);
    els.btnGitAdd.addEventListener("click", () => openGitAccountModal(null));
    els.gitAccountList.addEventListener("click", onGitAccountAction);
    els.btnGitSyncRepo.addEventListener("click", gitSyncRepo);
    els.btnGitGenKey.addEventListener("click", gitGenKey);
    els.btnGitSwitchGlobal.addEventListener("click", gitSwitchGlobal);
    els.btnGitSwitchLocal.addEventListener("click", gitSwitchLocal);
    els.btnGitCopyPubKey.addEventListener("click", gitCopyPubKey);
    els.btnGitKeyUrl.addEventListener("click", gitOpenKeyUrl);

    // 包管理器
    els.btnPkgRefresh.addEventListener("click", loadPkgManagers);
    els.pkgList.addEventListener("click", onPkgAction);

    // 项目管理
    els.btnOpenProject.addEventListener("click", openProject);
    els.btnApplyProject.addEventListener("click", applyProject);

    // 发布
    els.btnPubOpen.addEventListener("click", openPubProject);
    els.btnPubScan.addEventListener("click", scanPubPackages);
    els.btnPubPublish.addEventListener("click", publishSelected);
    els.btnPubRefresh.addEventListener("click", () => { refreshPubCa(); scanPubPackages(); });
    els.pubAccountSel.addEventListener("change", () => {
      els.pubRegistry.value = els.pubAccountSel.value;
    });

    // Python
    els.pyBtnReprobe.addEventListener("click", () => loadPythonDetect(true));
    els.pyBtnInstall.addEventListener("click", handlePyInstall);
    els.pyBtnFixPath.addEventListener("click", handlePyFixPath);
    els.pyBtnTestIndex.addEventListener("click", testPipIndex);
    els.pyBtnApplyIndex.addEventListener("click", applyPipMirror);
    els.pyBtnRefreshVersions.addEventListener("click", loadPythonVersions);
    els.pyInstalledList.addEventListener("click", onPyVersionAction);
    els.pyAvailableList.addEventListener("click", onPyVersionAction);
    els.pyBtnPkgRefresh.addEventListener("click", loadPythonPkgManagers);
    els.pyPkgList.addEventListener("click", onPyPkgAction);
    els.pyBtnOpenProject.addEventListener("click", openPythonProject);
    els.pyBtnApplyProject.addEventListener("click", applyPythonProject);
    $$('input[name="pysource"]').forEach((r) => {
      r.addEventListener("change", () => {
        els.pyCustomIndex.disabled = !(r.value === "__custom__" && r.checked);
        if (r.value === "__custom__" && r.checked) els.pyCustomIndex.focus();
        els.pyTestBadge.classList.add("hidden");
        els.pyBtnApplyIndex.disabled = false;
      });
    });
    els.pyCustomIndex.addEventListener("input", () => {
      els.pyTestBadge.classList.add("hidden");
      els.pyBtnApplyIndex.disabled = false;
    });

    // 弹窗关闭：关闭按钮 / 点击遮罩 / Esc
    els.modalClose.addEventListener("click", requestModalClose);
    els.modalBackdrop.addEventListener("click", (e) => {
      if (e.target === els.modalBackdrop) requestModalClose();
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") requestModalClose();
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

    // 启动加载：全屏遮罩，至少展示 400ms，避免模拟模式一闪而过
    const loadStart = performance.now();
    els.fullscreenLoading?.classList.remove("hidden");
    try {
      await runDetect(false);
    } finally {
      const remain = Math.max(0, 400 - (performance.now() - loadStart));
      setTimeout(() => els.fullscreenLoading?.classList.add("hidden"), remain);
    }
  }

  document.addEventListener("DOMContentLoaded", init);
})();

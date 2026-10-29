/* 启动检查只执行一次；手动检查独立触发，不随语言和标签切换。 */
(function () {
  "use strict";
  const $ = (id) => document.getElementById(id);
  let initialized = false;
  let checking = null;
  let downloading = false;
  let preparedVersion = null;
  let currentVersion = "—";
  let info = null;
  let lastChecked = "尚未检查";
  let statusText = "正在准备检查更新…";
  let hasError = false;

  function render() {
    $("appVersion").textContent = currentVersion === "—" ? "读取版本…" : `v${currentVersion}`;
    $("appUpdateEntry").title = `当前版本 ${currentVersion} · ${statusText}`;
    $("appUpdateEntry").classList.toggle("has-update", !!info?.available);
    $("updateCurrentVersion").textContent = currentVersion;
    $("updateLatestVersion").textContent = info?.latestVersion || "—";
    $("updateLastChecked").textContent = lastChecked;
    $("updateStatus").textContent = statusText;
    $("updateStatus").classList.toggle("is-error", hasError);
    $("updateNotes").textContent = info?.notes?.trim() || "该版本暂未提供更新说明。";
    $("updateNotesWrap").classList.toggle("hidden", !info?.available);
    $("updateDownload").classList.toggle("hidden", !info?.available || !!preparedVersion);
    $("updateDownload").disabled = !info?.canInstall || downloading || !!checking;
    $("updateDownload").textContent = downloading ? "下载中…" : "下载安装包";
    $("updateInstall").classList.toggle("hidden", !preparedVersion);
    $("updateInstallHint").classList.toggle("hidden", !preparedVersion);
    $("updateRelease").classList.toggle("hidden", !info?.releaseUrl);
    $("updateCheck").disabled = downloading || !!checking;
    $("updateCheck").textContent = checking ? "检查中…" : "检查更新";
    $("updateProgress").classList.toggle("hidden", !downloading);
    $("updateProgress").setAttribute("aria-busy", String(downloading));
  }

  function showNotice() {
    if (!info?.available) return;
    $("appUpdateNoticeText").textContent = `发现新版本 v${info.latestVersion}`;
    $("appUpdateNotice").classList.remove("hidden");
  }

  function check(automatic) {
    if (checking || downloading) return checking;
    hasError = false;
    statusText = "正在检查更新…";
    checking = (async () => {
      try {
        info = await window.Backend.checkAppUpdate(automatic);
        currentVersion = info.currentVersion;
        if (preparedVersion !== info.latestVersion) preparedVersion = null;
        statusText = info.message || (info.available ? `发现新版本 v${info.latestVersion}` : "当前已是最新版本");
        lastChecked = new Date().toLocaleString("zh-CN", { hour12:false });
        if (info.available) showNotice();
        else $("appUpdateNotice").classList.add("hidden");
      } catch (error) {
        hasError = true;
        statusText = error.message || String(error);
        lastChecked = "检查未完成，可手动重试";
      } finally {
        checking = null;
        render();
      }
    })();
    render();
    return checking;
  }

  function open() {
    render();
    if (!$("appUpdateDialog").open) $("appUpdateDialog").showModal();
  }

  async function init() {
    if (initialized) return;
    initialized = true;
    $("appUpdateEntry").addEventListener("click", open);
    $("appUpdateNoticeOpen").addEventListener("click", open);
    $("appUpdateNoticeClose").addEventListener("click", () => $("appUpdateNotice").classList.add("hidden"));
    $("appUpdateClose").addEventListener("click", () => $("appUpdateDialog").close());
    $("updateCheck").addEventListener("click", () => check(false));
    $("updateRelease").addEventListener("click", async () => {
      try { await window.Backend.gitOpenUrl(info.releaseUrl); }
      catch (error) { hasError = true; statusText = error.message; render(); }
    });
    $("updateDownload").addEventListener("click", async () => {
      if (!info?.canInstall || downloading || checking) return;
      downloading = true;
      hasError = false;
      preparedVersion = null;
      const version = info.latestVersion;
      statusText = "正在读取安装包校验信息…";
      render();
      try {
        const message = await window.Backend.downloadAppUpdate(version, (progress) => {
          const stages = { checksums:"读取 SHA256 校验信息…", downloading:"下载安装包…", verifying:"校验安装包…", ready:"安装包已准备就绪" };
          statusText = stages[progress.stage] || "正在准备更新…";
          if (progress.stage === "downloading" && progress.bytes) statusText += ` 已下载 ${(progress.bytes / 1048576).toFixed(1)} MB`;
          render();
        });
        statusText = message;
        preparedVersion = version;
      } catch (error) { hasError = true; statusText = error.message || String(error); }
      finally { downloading = false; render(); }
    });
    $("updateInstall").addEventListener("click", async () => {
      $("updateInstall").disabled = true;
      try {
        await window.Backend.installAppUpdate(preparedVersion);
        statusText = window.Backend.IS_TAURI ? "安装器已启动" : "预览：安装器启动成功，真实应用会退出并进入安装向导";
      } catch (error) { hasError = true; statusText = error.message || String(error); }
      finally { $("updateInstall").disabled = false; render(); }
    });
    try { currentVersion = await window.Backend.appVersion(); }
    catch (error) { statusText = `读取版本失败：${error.message}`; hasError = true; }
    render();
    // 不等待联网检查，应用的环境诊断与界面加载照常进行。
    void check(true);
  }

  window.AppUpdates = { init };
})();

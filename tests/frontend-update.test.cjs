const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const root = path.resolve(__dirname, '..');

function updatePage(backend) {
  const elements = new Map();
  const html = fs.readFileSync(path.join(root, 'src/index.html'), 'utf8');
  for (const [, id] of html.matchAll(/\bid="([^"]+)"/g)) {
    const classes = new Set();
    elements.set(id, {
      textContent: '', disabled: false, open: false, events: {},
      classList: {
        add: (c) => classes.add(c), remove: (c) => classes.delete(c), contains: (c) => classes.has(c),
        toggle: (c, enabled) => enabled ? classes.add(c) : classes.delete(c),
      },
      addEventListener(event, callback) { this.events[event] = callback; },
      setAttribute() {}, showModal() { this.open = true; }, close() { this.open = false; },
    });
  }
  const context = { window: { Backend: { IS_TAURI: true, appVersion: async () => '0.2.8-beta', ...backend } }, document: { getElementById: (id) => {
    assert.ok(elements.has(id), `页面缺少元素 ${id}`);
    return elements.get(id);
  } }, Date };
  vm.runInNewContext(fs.readFileSync(path.join(root, 'src/app-update.js'), 'utf8'), context);
  return { api: context.window.AppUpdates, get: (id) => elements.get(id) };
}

const settled = () => new Promise((resolve) => setImmediate(resolve));
const latest = { currentVersion:'0.2.8-beta', latestVersion:'0.2.9-beta', available:true, canInstall:true, notes:'更新说明', releaseUrl:'https://gitee.com/apricotdream/devEnvMangerTool/releases' };

test('启动只自动检查一次，打开弹窗不会检查，手动检查可以再次发起', async () => {
  const calls = [];
  const page = updatePage({ checkAppUpdate: async (automatic) => { calls.push(automatic); return latest; } });
  await page.api.init();
  await page.api.init();
  await settled();
  assert.deepEqual(calls, [true]);
  page.get('appUpdateEntry').events.click();
  page.get('appUpdateClose').events.click();
  assert.deepEqual(calls, [true]);
  await page.get('updateCheck').events.click();
  assert.deepEqual(calls, [true, false]);
  assert.equal(page.get('appVersion').textContent, 'v0.2.8-beta');
});

test('网络失败结束检查状态且不会自动重试，手动重试可恢复', async () => {
  let count = 0;
  const page = updatePage({ checkAppUpdate: async () => { if (++count === 1) throw Error('网络不可用'); return latest; } });
  await page.api.init(); await settled();
  assert.equal(page.get('updateCheck').disabled, false);
  assert.equal(page.get('updateStatus').textContent, '网络不可用');
  page.get('appUpdateEntry').events.click();
  await page.api.init();
  assert.equal(count, 1);
  await page.get('updateCheck').events.click();
  assert.equal(count, 2);
  assert.equal(page.get('updateDownload').disabled, false);
});

test('重复点击不会重复检查，下载成功后才能安装同一版本', async () => {
  let finishCheck;
  const installed = [];
  const downloaded = [];
  let count = 0;
  const page = updatePage({
    checkAppUpdate: () => { count++; return new Promise((resolve) => { finishCheck = resolve; }); },
    downloadAppUpdate: async (version, progress) => { downloaded.push(version); progress({stage:'verifying'}); return '已校验'; },
    installAppUpdate: async (version) => installed.push(version),
  });
  await page.api.init();
  page.get('updateCheck').events.click();
  assert.equal(count, 1);
  finishCheck(latest); await settled();
  assert.ok(page.get('updateInstall').classList.contains('hidden'));
  await page.get('updateDownload').events.click();
  assert.equal(page.get('updateInstall').classList.contains('hidden'), false);
  await page.get('updateInstall').events.click();
  assert.deepEqual(downloaded, ['0.2.9-beta']);
  assert.deepEqual(installed, ['0.2.9-beta']);
});

test('桌面发布桥接完整传递 OTP、dry-run 与可见性', async () => {
  const calls = [];
  const context = { window: { __TAURI__: { core: { invoke: async (command, args) => { calls.push({command, args}); return []; } } } }, setTimeout, URLSearchParams };
  vm.runInNewContext(fs.readFileSync(path.join(root, 'src/backend.js'), 'utf8'), context);
  await context.window.Backend.publishPackages(['/tmp/demo'], 'https://registry.npmjs.org', false, '123456', true, 'public');
  assert.deepEqual(JSON.parse(JSON.stringify(calls)), [{command:'publish_packages', args:{packages:['/tmp/demo'], registry:'https://registry.npmjs.org', build:false, otp:'123456', dryRun:true, access:'public'}}]);
});

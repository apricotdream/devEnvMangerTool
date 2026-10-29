const descriptions = {
  node: 'Node.js / 多版本安装与切换、npm 镜像、包管理器、项目配置与发布演练。',
  python: 'Python / 环境诊断、版本管理、pip 镜像、包管理器与项目环境配置。',
  go: 'Go / 多版本管理、模块代理、gopls 等开发工具，以及 go.mod / go.work 项目配置。',
  git: 'Git / SSH 账号与密钥、身份切换、连接测试和仓库同步。',
};
const tabs = [...document.querySelectorAll('[data-page]')];
tabs.forEach((button, index) => {
  button.addEventListener('click', () => {
    tabs.forEach((tab) => { const selected = tab === button; tab.setAttribute('aria-selected', selected); tab.tabIndex = selected ? 0 : -1; });
    document.getElementById('demoFrame').src = `demo/?page=${button.dataset.page}`;
    document.getElementById('featureCopy').textContent = descriptions[button.dataset.page];
  });
  button.tabIndex = index === 0 ? 0 : -1;
  button.addEventListener('keydown', (event) => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
    tabs[next].focus(); tabs[next].click();
  });
});
const commands = {
  go: ['devenv go status\ndevenv go versions\ndevenv go project . --apply', '检测 Go 环境，查看版本，为当前项目应用配置。'],
  publish: ['devenv node publish --dir . --dry-run --access public', '只执行打包演练，不上传到 npm；确认内容后可用 --yes 发布。'],
  update: ['devenv update check\ndevenv update download\ndevenv update install', '按需检查、下载校验并安装 CLI 自身更新。'],
};
document.querySelectorAll('[data-command]').forEach((button) => button.addEventListener('click', () => {
  document.querySelectorAll('[data-command]').forEach((option) => option.setAttribute('aria-pressed', option === button));
  const [command, description] = commands[button.dataset.command];
  document.getElementById('commandText').textContent = command;
  document.getElementById('copyStatus').textContent = description;
}));
document.getElementById('copyCommand').addEventListener('click', async () => {
  try { await navigator.clipboard.writeText(document.getElementById('commandText').textContent); document.getElementById('copyStatus').textContent = '已复制命令。'; }
  catch { document.getElementById('copyStatus').textContent = '浏览器未允许复制，请手动选择命令。'; }
});

// 零依赖构建：展示站与桌面共用前端，演示始终使用浏览器模拟数据。
import { cp, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.join(root, 'dist/site');
await mkdir(path.join(output, 'demo'), { recursive: true });
await cp(path.join(root, 'website'), output, { recursive: true });
await cp(path.join(root, 'src/logo.png'), path.join(output, 'assets/logo.png'));
for (const file of await readdir(path.join(root, 'src'))) {
  if (!/\.(js|css|html|png)$/.test(file)) continue;
  await cp(path.join(root, 'src', file), path.join(output, 'demo', file));
}
const index = path.join(output, 'demo/index.html');
const html = await readFile(index, 'utf8');
await writeFile(index, html.replace('<body>', '<body><div class="demo-banner">浏览器演示 · 模拟数据，不会更改本机环境 <a href="../">返回介绍 ↗</a></div>')
  .replace('</head>', '<style>.demo-banner{padding:8px 16px;background:#241b34;color:#e6edf3;font:11px/1.6 system-ui;text-align:center}.demo-banner a{color:#d4c3ff;margin-left:12px}.app{height:calc(100vh - 34px)!important}</style></head>'));
console.log(`展示站已构建：${output}`);

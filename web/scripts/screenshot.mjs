// SPDX-License-Identifier: Apache-2.0
// Full-page screenshot of the built explorer with a local Chrome over the
// DevTools protocol (adapted from design/preview/scripts/screenshot.mjs).
// usage: node scripts/screenshot.mjs out.png [width] [light|dark] [hash] [clickSelector]
// Starts `vite preview` on 4179 itself; needs `pnpm build` first. No network
// beyond localhost (the page makes no request unless a live button is clicked).
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [out = 'shot.png', w = '1440', theme = 'light', hash = '', click = ''] = process.argv.slice(2);
const width = Number(w);
const chrome = process.env.CHROME ?? 'google-chrome';
const PORT = 4183;
const CDP = 9345;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function waitFor(url) {
  for (let i = 0; i < 80; i++) {
    try {
      if ((await fetch(url)).ok) return;
    } catch {}
    await sleep(250);
  }
  throw new Error(`timed out: ${url}`);
}
const server = spawn('npx', ['vite', 'preview', '--port', String(PORT), '--strictPort'], { stdio: 'ignore' });
const profile = mkdtempSync(join(tmpdir(), 'sdx-shot-'));
const browser = spawn(chrome, ['--headless=new', '--disable-gpu', '--no-first-run', '--hide-scrollbars', `--user-data-dir=${profile}`, `--remote-debugging-port=${CDP}`, 'about:blank'], { stdio: 'ignore' });
try {
  await waitFor(`http://localhost:${PORT}/`);
  await waitFor(`http://127.0.0.1:${CDP}/json/version`);
  const page = (await (await fetch(`http://127.0.0.1:${CDP}/json/list`)).json()).find((t) => t.type === 'page');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener('open', r, { once: true }));
  let id = 0;
  const pending = new Map();
  ws.addEventListener('message', (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) (pending.get(m.id)(m), pending.delete(m.id));
  });
  const send = (method, params = {}) =>
    new Promise((res, rej) => {
      const n = ++id;
      pending.set(n, (m) => (m.error ? rej(new Error(`${method}: ${m.error.message}`)) : res(m.result)));
      ws.send(JSON.stringify({ id: n, method, params }));
    });
  const evaluate = async (expression) => (await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result.value;
  await send('Page.enable');
  await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: theme }] });
  await send('Emulation.setDeviceMetricsOverride', { width, height: 900, deviceScaleFactor: 1, mobile: width < 700 });
  await send('Page.navigate', { url: `http://localhost:${PORT}/${hash}` });
  await sleep(3000);
  await evaluate('document.fonts.ready.then(() => true)');
  if (click) {
    await evaluate(`(() => { const el = [...document.querySelectorAll('button,label')].find(b => b.textContent.includes(${JSON.stringify(click)})); el && el.click(); return !!el; })()`);
    await sleep(1500);
  }
  await evaluate(`(() => { const s = document.createElement('style'); s.textContent = '[class*="min-h-[100dvh]"]{min-height:0 !important}'; document.head.appendChild(s); return true; })()`);
  const docW = await evaluate('document.documentElement.scrollWidth');
  if (process.env.DEBUG_OVERFLOW) {
    console.log(await evaluate(`JSON.stringify([...document.querySelectorAll('body *')].filter(e => e.getBoundingClientRect().right > ${width} + 1).slice(0, 12).map(e => { let p = e, chain = []; while (p && chain.length < 6) { chain.push(p.tagName + '.' + String(p.className).slice(0, 40)); p = p.parentElement; } return Math.round(e.getBoundingClientRect().right) + ' ' + e.textContent.slice(0, 40) + ' <- ' + chain.join(' < '); }))`));
  }
  const height = Math.min(Number(await evaluate('document.documentElement.scrollHeight')), 16000);
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: width < 700 });
  await sleep(800);
  const shot = await send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(out, Buffer.from(shot.data, 'base64'));
  console.log(`wrote ${out} (${width}x${height}), document width ${docW}`);
  ws.close();
} finally {
  browser.kill();
  server.kill();
  await sleep(300);
  rmSync(profile, { recursive: true, force: true });
}

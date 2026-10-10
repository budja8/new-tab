// SPIKE harness helpers (adapted from measure-chrome/measure.mjs). CDP over --remote-debugging-pipe.
import { spawn, execFileSync } from 'node:child_process';
import fs from 'node:fs';

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
export const CHROME = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
export const BOUNDS = JSON.parse(process.env.BOUNDS || '{"left":-1920,"top":0,"width":1920,"height":1080}');

export async function launch(profileBase) {
  // Deleting old profiles under C:\ntspike fails with EPERM here, so every launch gets a fresh dir.
  const profile = `${profileBase}${Date.now().toString(36).slice(-5)}`;
  fs.mkdirSync(profile, { recursive: true });
  const proc = spawn(CHROME, [
    '--remote-debugging-pipe', '--enable-unsafe-extension-debugging', '--user-data-dir=' + profile,
    '--no-first-run', '--no-default-browser-check',
    '--disable-features=CalculateNativeWinOcclusion', '--disable-backgrounding-occluded-windows',
    `--window-position=${BOUNDS.left},${BOUNDS.top}`, `--window-size=${BOUNDS.width},${BOUNDS.height}`, 'about:blank',
  ], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  const wr = proc.stdio[3], rd = proc.stdio[4];
  let id = 0, buf = '';
  const pend = new Map();
  const events = [];
  rd.on('data', (d) => {
    buf += d.toString('utf8');
    let i;
    while ((i = buf.indexOf('\0')) >= 0) {
      const m = JSON.parse(buf.slice(0, i));
      buf = buf.slice(i + 1);
      if (m.id && pend.has(m.id)) {
        const p = pend.get(m.id);
        pend.delete(m.id);
        m.error ? p.rej(new Error(JSON.stringify(m.error))) : p.res(m.result);
      } else events.push(m);
    }
  });
  const send = (method, params = {}, sessionId) => new Promise((res, rej) => {
    const i = ++id;
    pend.set(i, { res, rej });
    wr.write(JSON.stringify({ id: i, method, params, sessionId }) + '\0');
  });
  await sleep(2500);
  await send('Target.setDiscoverTargets', { discover: true });
  const close = async () => {
    try { execFileSync('taskkill', ['/PID', String(proc.pid), '/T', '/F'], { stdio: 'ignore' }); } catch {}
    await sleep(2500);
    for (let k = 0; k < 4; k++) {
      try { fs.rmSync(profile, { recursive: true, force: true }); return true; } catch { await sleep(2000); }
    }
    return false;
  };
  return { proc, pid: proc.pid, profile, send, events, close };
}

export async function attach(send, targetId) {
  const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
  for (const m of ['Runtime.enable', 'Log.enable', 'Performance.enable', 'Page.enable']) await send(m, {}, sessionId);
  return sessionId;
}

export async function evaluate(send, s, expression, awaitPromise = false) {
  const r = await send('Runtime.evaluate', { expression, awaitPromise, returnByValue: true }, s);
  if (r.exceptionDetails) throw new Error('eval: ' + JSON.stringify(r.exceptionDetails).slice(0, 400));
  return r.result.value;
}

export async function setBounds(send, targetId) {
  const w = await send('Browser.getWindowForTarget', { targetId });
  await send('Browser.setWindowBounds', { windowId: w.windowId, bounds: { windowState: 'normal' } });
  await send('Browser.setWindowBounds', { windowId: w.windowId, bounds: { ...BOUNDS } });
  return (await send('Browser.getWindowBounds', { windowId: w.windowId })).bounds;
}

export function mem(pid) {
  try {
    const o = execFileSync('powershell', ['-NoProfile', '-Command', `$p=Get-Process -Id ${pid}; "$($p.WorkingSet64) $($p.PrivateMemorySize64)"`]).toString().trim().split(' ');
    return { wsMB: +(+o[0] / 1048576).toFixed(1), privMB: +(+o[1] / 1048576).toFixed(1) };
  } catch { return null; }
}

/** Opens the extension new tab page in `mode`; returns { targetId, s, info }. */
export async function openExt(send, url, mode) {
  const targetId = (await send('Target.createTarget', { url })).targetId;
  const s = await attach(send, targetId);
  await sleep(1000);
  for (let k = 0; k < 40; k++) {
    const ok = await evaluate(send, s, "document.readyState==='complete'&&typeof chrome!=='undefined'&&!!chrome.storage&&!!document.querySelector('#mode')");
    if (ok) break;
    await sleep(250);
  }
  await evaluate(send, s, `chrome.storage.local.set({settings:{mode:'${mode}',kpiVisible:true}}).then(()=>'ok')`, true);
  const bounds = await setBounds(send, targetId);
  await send('Page.reload', {}, s);
  await send('Page.bringToFront', {}, s);
  await send('Target.activateTarget', { targetId });
  await sleep(2500);
  const info = JSON.parse(await evaluate(send, s, `JSON.stringify({mode:document.querySelector('#mode').value, vis:document.visibilityState, iw:innerWidth, ih:innerHeight, dpr:devicePixelRatio, cw:document.getElementById('bg').width, ch:document.getElementById('bg').height, ctx: !!document.getElementById('bg').getContext})`));
  info.bounds = bounds;
  return { targetId, s, info };
}

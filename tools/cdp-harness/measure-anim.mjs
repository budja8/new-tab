// Medición de CPU/memoria de las animaciones nuevas contra `fire` (misma sesión), adaptado de measure-dna.mjs.
// Uso: node measure-anim.mjs  (env RUNS = "orden1;orden2;..." de modos separados por coma; OUT; SHOTS=1; HIDDEN=1)
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launch, attach, evaluate, setBounds, mem, sleep } from './lib2.mjs';

const dir = path.dirname(fileURLToPath(import.meta.url));
const EXT = 'C:/Users/Daniel Rodriguez/Documents/Tests/owns/new-tab/extension';
const WARM = +(process.env.WARM || 8000), WIN = +(process.env.WIN || 10000);
const NWIN = { fire: 4, synthwave: 3, dashboard: 3, dashNoIpc: 3, compile_sim: 4, pacman_install: 4, pacman: 4, mario: 4 };
const RUNS = (process.env.RUNS || 'fire,synthwave,compile_sim,pacman_install,dashboard,dashNoIpc,pacman,mario').split(';').map((r) => r.split(','));
const outFile = path.join(dir, process.env.OUT || 'results-anim.json');
const results = [];
const save = () => fs.writeFileSync(outFile, JSON.stringify(results, null, 1));

const instr = (stub) => `(() => {
  window.__steps = 0; window.__raf = 0; window.__sys = { cpu: 0, mem: 0, sto: 0, ms: 0, n: 0, stub: ${stub} };
  const ois = WebAssembly.instantiateStreaming;
  WebAssembly.instantiateStreaming = async (...a) => {
    const r = await ois.apply(WebAssembly, a);
    const ex = { ...r.instance.exports };
    const st = ex.engine_step, cp = ex.engine_cells_ptr, cl = ex.engine_cells_len, ew = ex.engine_width;
    let ptr = 0, len = 0, w = 0;
    ex.engine_cells_ptr = (p) => (ptr = cp(p));
    ex.engine_cells_len = (p) => (len = cl(p));
    ex.engine_width = (p) => (w = ew(p));
    ex.engine_step = (p) => { window.__steps++; return st(p); };
    window.__text = () => { if (!ptr || !len || !w) return null; const c = new Uint32Array(ex.memory.buffer, ptr, len); const rows = []; for (let i = 0; i < c.length / 3; i += w) { let s = ''; for (let k = i; k < i + w; k++) s += String.fromCodePoint(c[k * 3] || 32); rows.push(s.replace(/\\s+$/, '')); } return rows; };
    return { module: r.module, instance: { exports: ex } };
  };
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = (cb) => raf((t) => { window.__raf++; cb(t); });
  try {
    const s = chrome.system; const cache = {};
    for (const [k, api] of [['cpu', s.cpu], ['mem', s.memory], ['sto', s.storage]]) {
      const o = api.getInfo.bind(api);
      api.getInfo = (...a) => {
        window.__sys[k]++;
        if (window.__sys.stub && cache[k]) return Promise.resolve(structuredClone(cache[k]));
        const t = performance.now(); const p = o(...a);
        p.then((r) => { cache[k] = r; window.__sys.ms += performance.now() - t; window.__sys.n++; });
        return p;
      };
    }
  } catch (e) { window.__sysErr = String(e); }
})();`;

async function procs(send) { return (await send('SystemInfo.getProcessInfo')).processInfo; }
async function metrics(send, s) { const r = await send('Performance.getMetrics', {}, s); const o = {}; r.metrics.forEach((m) => (o[m.name] = m.value)); return o; }
const counters = (send, s) => evaluate(send, s, 'JSON.stringify([window.__steps||0, window.__raf||0, Math.round(performance.now()), window.__sys])').then(JSON.parse);

async function window1(send, s) {
  const p0 = await procs(send); const t0 = performance.now();
  const m0 = await metrics(send, s); const c0 = await counters(send, s);
  await sleep(WIN);
  const p1 = await procs(send); const t1 = performance.now();
  const m1 = await metrics(send, s); const c1 = await counters(send, s);
  const dt = (t1 - t0) / 1000;
  const by0 = new Map(p0.map((p) => [p.id, p]));
  const rows = [];
  for (const p of p1) { const a = by0.get(p.id); if (a) rows.push({ id: p.id, type: p.type, cpu: ((p.cpuTime - a.cpuTime) / dt) * 100 }); }
  const rend = rows.filter((r) => r.type === 'renderer').sort((a, b) => b.cpu - a.cpu);
  const r2 = (x) => +x.toFixed(2);
  const w = {
    dt: r2(dt), tab: r2(rend[0]?.cpu || 0), tabPid: rend[0]?.id, otherRenderers: r2(rend.slice(1).reduce((a, b) => a + b.cpu, 0)),
    gpu: r2(rows.filter((r) => r.type === 'GPU').reduce((a, b) => a + b.cpu, 0)),
    all: r2(rows.reduce((a, b) => a + b.cpu, 0)),
    byType: Object.fromEntries([...new Set(rows.map((r) => r.type))].map((t) => [t, r2(rows.filter((r) => r.type === t).reduce((a, b) => a + b.cpu, 0))])),
  };
  for (const k of ['TaskDuration', 'ScriptDuration', 'LayoutDuration', 'RecalcStyleDuration']) w[k] = r2(((m1[k] - m0[k]) / dt) * 100);
  w.heapMB = r2(m1.JSHeapUsedSize / 1048576);
  w.stepsPerS = r2((c1[0] - c0[0]) / dt);
  w.rafPerS = r2((c1[1] - c0[1]) / dt);
  w.pageT = [c0[2], c1[2]];
  w.sysCalls = { cpu: c1[3].cpu - c0[3].cpu, mem: c1[3].mem - c0[3].mem, sto: c1[3].sto - c0[3].sto, msPerCall: c1[3].n > c0[3].n ? r2((c1[3].ms - c0[3].ms) / (c1[3].n - c0[3].n)) : null };
  return w;
}

async function openExtInstr(send, events, url, mode, stub) {
  const targetId = (await send('Target.createTarget', { url: 'about:blank' })).targetId;
  const s = await attach(send, targetId);
  await send('Page.addScriptToEvaluateOnNewDocument', { source: instr(stub) }, s);
  await send('Page.navigate', { url }, s);
  for (let k = 0; k < 40; k++) {
    await sleep(250);
    const ok = await evaluate(send, s, "document.readyState==='complete'&&typeof chrome!=='undefined'&&!!chrome.storage&&!!document.querySelector('#mode')").catch(() => false);
    if (ok) break;
  }
  await evaluate(send, s, `chrome.storage.local.set({settings:{mode:'${mode}',kpiVisible:true}}).then(()=>'ok')`, true);
  const bounds = await setBounds(send, targetId);
  const before = events.length; // solo la carga medida (tras el reload)
  await send('Page.reload', {}, s);
  await send('Page.bringToFront', {}, s);
  await send('Target.activateTarget', { targetId });
  await sleep(2500);
  const info = JSON.parse(await evaluate(send, s, `JSON.stringify({mode:document.querySelector('#mode').value, vis:document.visibilityState, iw:innerWidth, ih:innerHeight, dpr:devicePixelRatio, cw:document.getElementById('bg').width, ch:document.getElementById('bg').height, steps:window.__steps, sysErr:window.__sysErr||null})`));
  info.bounds = bounds;
  if (!(info.steps > 0)) throw new Error('no steps: ' + JSON.stringify(info));
  return { targetId, s, info, before };
}

const consoleOf = (events, s, before) => events.slice(before).filter((e) => e.sessionId === s && /exceptionThrown|consoleAPICalled|entryAdded/.test(e.method)).map((e) => {
  const p = e.params;
  if (e.method === 'Runtime.consoleAPICalled') return `${p.type}: ${p.args.map((a) => a.value ?? a.description).join(' ')}`;
  if (e.method === 'Runtime.exceptionThrown') return 'EXCEPTION: ' + JSON.stringify(p.exceptionDetails).slice(0, 300);
  return `log.${p.entry.level}: ${p.entry.text}`.slice(0, 300);
});

const imgDir = 'C:/Users/Daniel Rodriguez/Documents/Tests/owns/new-tab/docs/reports/img';

for (const [i, order] of RUNS.entries()) {
  const label = `${process.env.TAG || 'R'}${i + 1}`;
  console.log(new Date().toISOString(), 'run', label, order.join(','));
  const B = await launch(`C:/ntmeas/${label}-`);
  const { send, events } = B;
  const run = { label, order, pid: B.pid, profile: B.profile, scen: {} };
  try {
    const ext = (await send('Extensions.loadUnpacked', { path: EXT })).id;
    for (const name of order) {
      const hidden = name.endsWith('Hidden');
      const mode = name === 'dashNoIpc' ? 'dashboard' : name.replace(/Hidden$/, '');
      const o = await openExtInstr(send, events, `chrome-extension://${ext}/newtab.html`, mode, name === 'dashNoIpc');
      const m = { info: o.info, wins: [] };
      let blank = null;
      if (process.env.SHOTS && !hidden) {
        for (const [k, at] of [[1, 6000], [2, 20000]]) {
          await sleep(at - (k === 2 ? 6000 : 0));
          const shot = await send('Page.captureScreenshot', { format: 'png' }, o.s);
          let buf = Buffer.from(shot.data, 'base64'); let file = `${mode}${k === 2 ? '-2' : ''}.png`;
          if (buf.length > 300 * 1024) { const j = await send('Page.captureScreenshot', { format: 'jpeg', quality: 80 }, o.s); buf = Buffer.from(j.data, 'base64'); file = file.replace('.png', '.jpg'); }
          fs.mkdirSync(imgDir, { recursive: true });
          fs.writeFileSync(path.join(imgDir, file), buf);
          (m.shots ??= []).push({ file, bytes: buf.length });
        }
        if (mode === 'dashboard') m.dashText = await evaluate(send, o.s, 'JSON.stringify(window.__text())').then(JSON.parse);
      }
      if (hidden) {
        await sleep(3000);
        m.visibleSys = await evaluate(send, o.s, 'JSON.stringify(window.__sys)').then(JSON.parse);
        blank = (await send('Target.createTarget', { url: 'about:blank' })).targetId;
        await send('Target.activateTarget', { targetId: blank });
        await sleep(1500);
        m.info.visAfter = await evaluate(send, o.s, 'document.visibilityState');
      }
      if (!process.env.SHOTS || hidden) {
        await sleep(hidden ? 2000 : WARM);
        for (let k = 0; k < (hidden ? 2 : NWIN[name]); k++) m.wins.push(await window1(send, o.s));
        const pl = await procs(send);
        m.rendMem = mem(m.wins[0].tabPid);
        m.gpuMem = pl.filter((p) => p.type === 'GPU').map((p) => mem(p.id));
      }
      if (mode === 'dashboard' && !m.dashText) m.dashText = await evaluate(send, o.s, 'JSON.stringify(window.__text())').then(JSON.parse);
      if (blank) { m.info.visEnd = await evaluate(send, o.s, 'document.visibilityState'); await send('Target.closeTarget', { targetId: blank }); }
      m.console = consoleOf(events, o.s, o.before);
      run.scen[name] = m;
      console.log(` ${name} ${o.info.cw}x${o.info.ch} vis=${o.info.vis}${hidden ? '->' + o.info.visAfter : ''}`, m.wins.map((w) => `${w.tab}/${w.gpu}/${(w.tab + w.gpu).toFixed(1)} st=${w.stepsPerS} raf=${w.rafPerS} cpuCalls=${w.sysCalls.cpu}`).join(' | '), 'mem', JSON.stringify(m.rendMem), 'console', JSON.stringify(m.console));
      await send('Target.closeTarget', { targetId: o.targetId });
      await sleep(1500);
    }
  } catch (e) {
    console.log('ERR', e.stack);
    run.fatal = String(e.stack);
  }
  run.profileRemoved = await B.close();
  results.push(run);
  save();
}
console.log('done');
process.exit(0);

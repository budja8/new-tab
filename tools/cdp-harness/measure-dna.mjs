// SPIKE: CPU measurement of renderer variants (adapted from measure-chrome/measure.mjs).
// One fresh Chrome + fresh profile per variant run; runs interleaved.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launch, attach, evaluate, setBounds, openExt, mem, sleep } from './lib2.mjs';

const dir = path.dirname(fileURLToPath(import.meta.url));
const WARM = +(process.env.WARM || 8000), WIN = +(process.env.WIN || 10000);
const PLAN = (process.env.PLAN || 'ctrl,dna,dnaB,dna,dnaL,ctrl').split(',');
const SCEN = { dna: ['dna', 'fire', 'dnaHidden'], dnaB: ['fire', 'dna', 'dnaHidden'], dnaL: ['dnaLong'], dnaLF: ['dnaLong', 'fire'], dnaFL: ['fire', 'dnaLong'] };
const FOLDER = { dna: 'dna', dnaB: 'dna', dnaL: 'dna', dnaLF: 'dna', dnaFL: 'dna' };
const outFile = path.join(dir, process.env.OUT || 'results-dna.json');
const results = [];
let __events = [];
const save = () => fs.writeFileSync(outFile, JSON.stringify(results, null, 1));

// Counts engine steps (= painted frames) and rAF ticks without touching the variant code.
const INSTR = `(() => {
  window.__steps = 0; window.__raf = 0;
  const ois = WebAssembly.instantiateStreaming;
  WebAssembly.instantiateStreaming = async (...a) => {
    const r = await ois.apply(WebAssembly, a);
    const ex = { ...r.instance.exports };
    const st = ex.engine_step, cp = ex.engine_cells_ptr, cl = ex.engine_cells_len;
    let ptr = 0, len = 0; window.__ink = [];
    ex.engine_cells_ptr = (p) => (ptr = cp(p));
    ex.engine_cells_len = (p) => (len = cl(p));
    ex.engine_step = (p) => { window.__steps++; const r2 = st(p); if (ptr && len && window.__steps % 30 === 0) { const c = new Uint32Array(ex.memory.buffer, ptr, len); let ink = 0; for (let i = 0; i < c.length; i += 3) if (c[i] !== 32) ink++; window.__ink.push([Math.round(performance.now()), ink]); } return r2; };
    return { module: r.module, instance: { exports: ex } };
  };
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = (cb) => raf((t) => { window.__raf++; cb(t); });
})();`;

async function procs(send) { return (await send('SystemInfo.getProcessInfo')).processInfo; }
async function metrics(send, s) { const r = await send('Performance.getMetrics', {}, s); const o = {}; r.metrics.forEach((m) => (o[m.name] = m.value)); return o; }
const counters = (send, s) => evaluate(send, s, 'JSON.stringify([window.__steps||0, window.__raf||0, (window.__ink||[]).at(-1)||null, Math.round(performance.now())])').then(JSON.parse);

async function window1(send, s) {
  const p0 = await procs(send); const t0 = performance.now();
  const m0 = s ? await metrics(send, s) : null; const c0 = s ? await counters(send, s) : null;
  await sleep(WIN);
  const p1 = await procs(send); const t1 = performance.now();
  const m1 = s ? await metrics(send, s) : null; const c1 = s ? await counters(send, s) : null;
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
  if (s) {
    for (const k of ['TaskDuration', 'ScriptDuration', 'LayoutDuration', 'RecalcStyleDuration']) w[k] = r2(((m1[k] - m0[k]) / dt) * 100);
    w.heapMB = r2(m1.JSHeapUsedSize / 1048576);
    w.stepsPerS = r2((c1[0] - c0[0]) / dt);
    w.rafPerS = r2((c1[1] - c0[1]) / dt);
    w.inkStart = c0[2]?.[1] ?? null; w.inkEnd = c1[2]?.[1] ?? null; w.pageT = [c0[3], c1[3]];
  }
  return w;
}

async function measure(send, s, fixed = 0) {
  await sleep(WARM);
  const wins = [];
  for (let k = 0; k < (fixed || 6); k++) {
    wins.push(await window1(send, s));
    if (!fixed && k >= 2) {
      const v = wins.map((w) => w.tab); const mean = v.reduce((a, b) => a + b, 0) / v.length;
      const last3 = v.slice(-3); if ((Math.max(...last3) - Math.min(...last3)) / Math.max(mean, 0.5) <= 0.3) break;
      if (mean < 2) break; // idle control: relative spread is meaningless near 0
    }
  }
  const pl = await procs(send);
  const gpuPids = pl.filter((p) => p.type === 'GPU').map((p) => p.id);
  return { wins, rendMem: mem(wins[0].tabPid), gpuMem: gpuPids.map(mem) };
}

for (const [i, v] of PLAN.entries()) {
  const label = `${i + 1}-${v}`;
  console.log(new Date().toISOString(), 'run', label);
  const B = await launch(`C:/ntdna/m${i}`);
  const { send, events } = B; __events = events;
  const run = { label, variant: v, profile: B.profile, scen: {} };
  try {
    if (v === 'ctrl') {
      const pages = (await send('Target.getTargets')).targetInfos.filter((t) => t.type === 'page');
      run.bounds = await setBounds(send, pages[0].targetId);
      await send('Target.activateTarget', { targetId: pages[0].targetId });
      run.scen.blank = await measure(send, null);
      console.log(' blank', run.scen.blank.wins.map((w) => `${w.tab}/${w.gpu}/${w.all}`).join(' '));
    } else {
      const ext = (await send('Extensions.loadUnpacked', { path: path.join(dir, FOLDER[v]) })).id;
      for (const mode of SCEN[v]) {
        const before = events.length;
        const o = await openExtInstr(send, `chrome-extension://${ext}/newtab.html`, mode === 'dnaHidden' || mode === 'dnaLong' ? 'dna' : mode);
        let blank = null;
        if (mode === 'dnaHidden') {
          // Segunda pestaña delante: la del fondo queda oculta (visibilityState = hidden).
          blank = (await send('Target.createTarget', { url: 'about:blank' })).targetId;
          await send('Target.activateTarget', { targetId: blank });
          await sleep(1500);
          o.info.visAfter = await evaluate(send, o.s, 'document.visibilityState');
        }
        const m = await measure(send, o.s, mode === 'dnaLong' ? 7 : 0);
        if (o.s) m.ink = await evaluate(send, o.s, 'JSON.stringify(window.__ink||[])').then(JSON.parse);
        m.info = o.info;
        if (blank) { m.info.visEnd = await evaluate(send, o.s, 'document.visibilityState'); await send('Target.closeTarget', { targetId: blank }); }
        m.errors = events.slice(before).filter((e) => /exceptionThrown|consoleAPICalled|entryAdded/.test(e.method)).map((e) => JSON.stringify(e.params).slice(0, 300));
        run.scen[mode] = m;
        console.log(` ${mode} ih=${o.info.ih} cells=${o.info.cw}x${o.info.ch}`, m.wins.map((w) => `${w.tab}/${w.gpu}/${(w.tab+w.gpu).toFixed(1)} st=${w.stepsPerS} ink=${w.inkStart}->${w.inkEnd}`).join(' | '), 'err', m.errors.length);
        await send('Target.closeTarget', { targetId: o.targetId });
        await sleep(1500);
      }
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

async function openExtInstr(send, url, mode) {
  const targetId = (await send('Target.createTarget', { url: 'about:blank' })).targetId;
  const s = await attach(send, targetId);
  await send('Page.addScriptToEvaluateOnNewDocument', { source: INSTR }, s);
  await send('Page.navigate', { url }, s);
  for (let k = 0; k < 40; k++) {
    await sleep(250);
    const ok = await evaluate(send, s, "document.readyState==='complete'&&typeof chrome!=='undefined'&&!!chrome.storage&&!!document.querySelector('#mode')").catch(() => false);
    if (ok) break;
  }
  await evaluate(send, s, `chrome.storage.local.set({settings:{mode:'${mode}',kpiVisible:true}}).then(()=>'ok')`, true);
  const bounds = await setBounds(send, targetId);
  await send('Page.reload', {}, s);
  await send('Page.bringToFront', {}, s);
  await send('Target.activateTarget', { targetId });
  await sleep(2500);
  const info = JSON.parse(await evaluate(send, s, `JSON.stringify({mode:document.querySelector('#mode').value, vis:document.visibilityState, iw:innerWidth, ih:innerHeight, dpr:devicePixelRatio, cw:document.getElementById('bg').width, ch:document.getElementById('bg').height, steps:window.__steps})`));
  info.bounds = bounds;
  if (!(info.steps > 0) && mode !== 'image') {
    await sleep(5000);
    const late = await evaluate(send, s, 'JSON.stringify({steps:window.__steps, cw:document.getElementById("bg").width, msg:document.getElementById("message").textContent, rs:document.readyState})');
    const ev = __events.filter((e) => e.sessionId === s).map((e) => e.method + ' ' + JSON.stringify(e.params).slice(0, 300)).filter((x) => /console|exception|Log.entry/.test(x));
    throw new Error('instrumentation not active / no steps: ' + JSON.stringify(info) + ' late=' + late + ' events=' + JSON.stringify(ev));
  }
  return { targetId, s, info };
}

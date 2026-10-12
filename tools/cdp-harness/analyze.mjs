import fs from 'node:fs';
const runs = JSON.parse(fs.readFileSync('results-anim.json'));
const mean = (a) => a.reduce((x, y) => x + y, 0) / a.length;
const f1 = (x) => (x == null || Number.isNaN(x) ? '-' : x.toFixed(1));
const f2 = (x) => x.toFixed(2);
const modes = ['fire', 'synthwave', 'compile_sim', 'pacman_install', 'dashboard', 'dashNoIpc', 'pacman', 'mario'];
const valid = (w) => w.stepsPerS > 0 && w.rafPerS > 0;
const discarded = [];
const S = {};
for (const m of modes) {
  const wins = [], perRun = [], memT = [], memG = [];
  for (const r of runs) {
    const sc = r.scen[m]; if (!sc) continue;
    sc.wins.forEach((w, k) => { if (!valid(w)) discarded.push(`${r.label} ${m} w${k + 1}: st=${w.stepsPerS} raf=${w.rafPerS}`); });
    const v = sc.wins.filter(valid); wins.push(...v.map((w) => ({ ...w, run: r.label })));
    const fire = r.scen.fire.wins.filter(valid);
    const k = (ws, f) => mean(ws.map(f));
    perRun.push({ run: r.label, tab: k(v, (w) => w.tab) / k(fire, (w) => w.tab), gpu: k(v, (w) => w.gpu) / k(fire, (w) => w.gpu), sum: k(v, (w) => w.tab + w.gpu) / k(fire, (w) => w.tab + w.gpu), all: k(v, (w) => w.all) / k(fire, (w) => w.all) });
    if (sc.rendMem) memT.push(sc.rendMem.privMB);
    memG.push(...(sc.gpuMem || []).filter(Boolean).map((g) => g.privMB));
  }
  const col = (f) => { const a = wins.map(f); return { mean: mean(a), min: Math.min(...a), max: Math.max(...a) }; };
  S[m] = {
    n: wins.length, tab: col((w) => w.tab), gpu: col((w) => w.gpu), sum: col((w) => w.tab + w.gpu), all: col((w) => w.all), browser: col((w) => w.byType.browser || 0),
    task: col((w) => w.TaskDuration), script: col((w) => w.ScriptDuration), steps: col((w) => w.stepsPerS), raf: col((w) => w.rafPerS),
    cpuCalls: col((w) => w.sysCalls.cpu), msPerCall: wins.map((w) => w.sysCalls.msPerCall).filter((x) => x != null),
    ratio: { tab: mean(perRun.map((p) => p.tab)), gpu: mean(perRun.map((p) => p.gpu)), sum: mean(perRun.map((p) => p.sum)), all: mean(perRun.map((p) => p.all)), perRun },
    memTab: memT, memGpu: memG, heap: col((w) => w.heapMB),
  };
}
const L = [];
L.push('Modo | n | pestaña media (min-max) | GPU media (min-max) | suma media (min-max) | todos los procesos | browser | razón vs fire pestaña/GPU/suma (rango suma por corrida) | pasos/s | Task%/Script% | mem priv pestaña MB | mem priv GPU MB');
for (const m of modes) {
  const s = S[m];
  const rr = s.ratio.perRun.map((p) => p.sum);
  L.push(`${m} | ${s.n} | ${f1(s.tab.mean)} (${f1(s.tab.min)}-${f1(s.tab.max)}) | ${f1(s.gpu.mean)} (${f1(s.gpu.min)}-${f1(s.gpu.max)}) | ${f1(s.sum.mean)} (${f1(s.sum.min)}-${f1(s.sum.max)}) | ${f1(s.all.mean)} | ${f1(s.browser.mean)} | ${f2(s.ratio.tab)}x / ${f2(s.ratio.gpu)}x / ${f2(s.ratio.sum)}x (${f2(Math.min(...rr))}-${f2(Math.max(...rr))}) | ${f2(s.steps.mean)} (${f2(s.steps.min)}-${f2(s.steps.max)}) | ${f1(s.task.mean)}/${f1(s.script.mean)} | ${Math.min(...s.memTab).toFixed(1)}-${Math.max(...s.memTab).toFixed(1)} | ${Math.min(...s.memGpu).toFixed(1)}-${Math.max(...s.memGpu).toFixed(1)}`);
}
L.push('', 'Por corrida (pestaña/GPU/suma, media de la corrida):');
for (const r of runs) {
  L.push(`${r.label} orden ${r.order.join(',')}${r.fatal ? ' FATAL ' + r.fatal.slice(0, 200) : ''}`);
  for (const [m, sc] of Object.entries(r.scen)) L.push(`  ${m}: ` + sc.wins.map((w) => `${w.tab}/${w.gpu}/${(w.tab + w.gpu).toFixed(1)} st${w.stepsPerS} t${w.pageT.map((x) => Math.round(x / 1000)).join('-')}`).join(' | ') + `  mem ${sc.rendMem?.privMB}/${(sc.gpuMem || []).map((g) => g?.privMB).join(',')}  console=${JSON.stringify(sc.console)}`);
}
L.push('', 'Descartadas: ' + (discarded.length ? discarded.join('; ') : 'ninguna'));
L.push('', 'dashboard llamadas cpu.getInfo por ventana: ' + JSON.stringify(S.dashboard.cpuCalls) + ' ms/llamada (latencia async) media ' + f1(mean(S.dashboard.msPerCall)));
L.push('dashNoIpc llamadas por ventana: ' + JSON.stringify(S.dashNoIpc.cpuCalls));
fs.writeFileSync('summary.txt', L.join('\n'));
fs.writeFileSync('summary.json', JSON.stringify({ summary: S, discarded }, null, 1));
console.log(L.join('\n'));

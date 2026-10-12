// Comparación píxel a píxel del Renderer 2D real vs GlRenderer real con el Engine real (semilla fija).
import init, { Engine, cell_stride } from './pkg/newtab_core.js';
import { Renderer } from './src/renderer.js';
import { GlRenderer } from './src/gl-renderer.js';

const SEED = 12345;
const SIZES = [[211, 58], [40, 20]];
const FRAMES = { synthwave: 600, compile_sim: 1800, pacman_install: 1500, dashboard: 400, pacman: 1500, mario: 1500 };
const REQUIRED = {
  pacman: ['ᗤ', 'ᗢ', 'ᗧ', 'ᗣ', 'ᗛ', '⍛'],
  mario: ['▄', '▀', '█', '░', '◢', '◣'],
};
const hex = (c) => '#' + (c >>> 0).toString(16).padStart(6, '0').slice(-6);

function setInner(w, h) {
  Object.defineProperty(window, 'innerWidth', { value: w, configurable: true });
  Object.defineProperty(window, 'innerHeight', { value: h, configurable: true });
  Object.defineProperty(window, 'devicePixelRatio', { value: 1, configurable: true });
}

const comp = document.createElement('canvas');
const cctx = comp.getContext('2d', { willReadFrequently: true });
function pixels(src) {
  comp.width = src.width; comp.height = src.height;
  cctx.fillStyle = '#000'; cctx.fillRect(0, 0, comp.width, comp.height);
  cctx.drawImage(src, 0, 0);
  return cctx.getImageData(0, 0, comp.width, comp.height).data;
}

// Diferencia GL vs 2D; las celdas culpables se resumen por (glifo, fg, bg).
function diff(glC, c2d, cells, cols, pw, ph) {
  const a = pixels(glC), b = pixels(c2d);
  let max = 0, over2 = 0, nonzero = 0;
  const bad = new Map();
  for (let i = 0; i < a.length; i += 4) {
    const d = Math.max(Math.abs(a[i] - b[i]), Math.abs(a[i + 1] - b[i + 1]), Math.abs(a[i + 2] - b[i + 2]));
    if (d > 0) nonzero++;
    if (d > max) max = d;
    if (d > 2) {
      over2++;
      const p = i / 4, x = p % glC.width, y = Math.floor(p / glC.width);
      const ci = Math.floor(y / ph) * cols + Math.floor(x / pw), j = ci * 3;
      const key = `${String.fromCodePoint(cells[j] || 32)} U+${cells[j].toString(16)} fg ${hex(cells[j + 1])} bg ${cells[j + 2] ? hex(cells[j + 2]) : '0'}`;
      const e = bad.get(key) || { px: 0, max: 0 }; e.px++; e.max = Math.max(e.max, d); bad.set(key, e);
    }
  }
  return { max, over2, nonzero, offenders: [...bad.entries()].sort((x, y) => y[1].px - x[1].px).slice(0, 12) };
}

function dashFeed(engine, f) {
  const s = 10 * 3600 + 20 * 60 + 30 + Math.floor(f / 5);
  const loads = Array.from({ length: 12 }, (_, i) => (Math.sin(f * 0.07 + i) + 1) / 2);
  engine.feed(Float32Array.of(Math.floor(s / 3600) % 24, Math.floor(s / 60) % 60, s % 60, 9.5 + 0.01 * (f % 50), 15.7, 0, 456.6, 12, ...loads));
  if (f === 0) engine.feed_text('Chrome: 155.0.0.0\nOS: Windows\nCores: 12\nJS heap: 3 / 4192 MB\nNetwork: 4g 1.7 Mbps 50 ms\nBattery: 73 % (discharging)\nLanguage: es-ES');
}

async function run() {
  const wasm = await init();
  const out = { seed: SEED, results: [] };
  for (const [cols, rows] of SIZES) {
    setInner(cols * 9 + 5, rows * 16 + 7);
    for (const [mode, frames] of Object.entries(FRAMES)) {
      const glC = document.createElement('canvas'), c2d = document.createElement('canvas');
      const gl = new GlRenderer(glC, { stride: cell_stride() }), r2 = new Renderer(c2d, { stride: cell_stride() });
      const s1 = gl.fit(), s2 = r2.fit();
      const engine = new Engine(SEED, s1.cols, s1.rows);
      engine.set_fps(30);
      if (!engine.set_animation(mode)) { out.results.push({ mode, error: 'set_animation false' }); continue; }
      const res = { mode, grid: `${s1.cols}x${s1.rows}`, grid2d: `${s2.cols}x${s2.rows}`, canvas: `${glC.width}x${glC.height}`, frames, checkpoints: 0, max: 0, over2: 0, worst: null, offenders: {}, cps: new Set(), fgs: new Set(), bgs: new Set(), triples: new Map(), skipped: 0 };
      let prev = null, pending = false;
      for (let f = 0; f < frames; f++) {
        if (mode === 'dashboard') dashFeed(engine, f);
        engine.step();
        const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
        const changed = !prev || cells.some((v, i) => v !== prev[i]);
        gl.draw(cells, engine.width(), engine.height());
        r2.draw(cells, engine.width(), engine.height());
        for (let j = 0; j < cells.length; j += 3) {
          res.cps.add(cells[j]); res.fgs.add(cells[j + 1]); res.bgs.add(cells[j + 2]);
          const k = `${cells[j]},${cells[j + 1]},${cells[j + 2]}`;
          if (cells[j] !== 32 || cells[j + 2] !== 0) res.triples.set(k, [cells[j], cells[j + 1], cells[j + 2]]);
        }
        if (f < 3 || f % 50 === 0 || f === frames - 1) pending = true;
        if (pending) {
          if (!changed && f !== frames - 1) { res.skipped++; } else {
            if (!changed) gl.invalidate(), r2.invalidate(), gl.draw(cells, engine.width(), engine.height()), r2.draw(cells, engine.width(), engine.height());
            const d = diff(glC, c2d, cells, s1.cols, gl.pw, gl.ph);
            res.checkpoints++;
            if (d.max > res.max) { res.max = d.max; res.worst = { frame: f, ...d }; }
            res.over2 += d.over2;
            for (const [k, v] of d.offenders) { const e = res.offenders[k] || { px: 0, max: 0, frames: 0 }; e.px += v.px; e.max = Math.max(e.max, v.max); e.frames++; res.offenders[k] = e; }
            pending = false;
          }
        }
        prev = Uint32Array.from(cells);
      }
      // GL tras invalidate (repintado completo) vs 2D incremental, último frame.
      {
        const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
        gl.invalidate(); gl.draw(cells, engine.width(), engine.height());
        const d = diff(glC, c2d, cells, s1.cols, gl.pw, gl.ph);
        res.afterInvalidate = { max: d.max, over2: d.over2 };
      }
      // Sintético: cada (glifo, fg, bg) visto + los glifos exigidos con todos los fg vistos, en una sola cuadrícula.
      {
        const list = [...res.triples.values()];
        const fgList = [...res.fgs].filter((x) => x !== 0);
        for (const ch of REQUIRED[mode] || []) for (const fg of fgList) { list.push([ch.codePointAt(0), fg, 0]); for (const bg of [...res.bgs].filter((x) => x !== 0)) list.push([ch.codePointAt(0), fg, bg]); }
        const n = s1.cols * s1.rows;
        const pages = Math.ceil(list.length / n);
        let smax = 0, sover = 0; const soff = {};
        for (let pg = 0; pg < pages; pg++) {
          const cells = new Uint32Array(n * 3);
          for (let i = 0; i < n; i++) { cells[i * 3] = 32; }
          list.slice(pg * n, pg * n + n).forEach((t, i) => cells.set(t, i * 3));
          gl.invalidate(); r2.invalidate();
          gl.draw(cells, s1.cols, s1.rows); r2.draw(cells, s1.cols, s1.rows);
          const d = diff(glC, c2d, cells, s1.cols, gl.pw, gl.ph);
          smax = Math.max(smax, d.max); sover += d.over2;
          for (const [k, v] of d.offenders) soff[k] = v;
        }
        res.synthetic = { combos: list.length, pages, max: smax, over2: sover, offenders: soff, required: (REQUIRED[mode] || []).map((c) => [c, res.cps.has(c.codePointAt(0))]) };
      }
      res.glyphs = [...res.cps].sort((a, b) => a - b).map((c) => String.fromCodePoint(c)).join('');
      res.nonAscii = [...res.cps].filter((c) => c > 126).sort((a, b) => a - b).map((c) => `${String.fromCodePoint(c)}U+${c.toString(16)}`);
      res.fgCount = res.fgs.size; res.bgList = [...res.bgs].map((x) => (x ? hex(x) : '0'));
      res.atlasSlots = gl.slots?.size ?? null;
      delete res.cps; delete res.fgs; delete res.bgs; res.triples = res.triples.size;
      out.results.push(res);
      engine.free();
      gl.gl.getExtension('WEBGL_lose_context')?.loseContext();
    }
  }
  return out;
}
window.__verify = run().catch((e) => ({ fatal: String(e.stack || e) }));

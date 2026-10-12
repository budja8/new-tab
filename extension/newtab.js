import init, { Engine, cell_stride, image_target_dims, image_to_ascii } from './pkg/newtab_core.js';
import { createRenderer } from './src/create-renderer.js';
import { watchDpr } from './src/dpr-watch.js';
import { MAX_COLS, MAX_ROWS, loadImage, saveImage, validateImage } from './src/imagestore.js';
import { renderKpi } from './src/kpi.js';
import { createLimiter } from './src/limiter.js';
import { loadSettings, saveSettings } from './src/settings.js';
import { createKpiStore } from './src/store.js';
import { createSysinfo } from './src/sysinfo.js';

const FPS = { fire: 30, rain: 30, starfield: 30, pipes: 30, dna: 30, synthwave: 30, compile_sim: 30, pacman_install: 30, dashboard: 5, pacman: 30, mario: 30 };
const MAX_FILE_BYTES = 20 * 1024 * 1024;
const REDUCED_MOTION_WARMUP = 120;

const $ = (id) => document.getElementById(id);

function say(text) {
  $('message').textContent = text;
}

async function main() {
  const wasm = await init();
  const area = chrome.storage.local;
  const settings = await loadSettings(area);
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');

  const { renderer, backend, reason } = createRenderer($('bg'), { stride: cell_stride() });
  console.info(`[newtab] renderer: ${backend} (${reason})`);
  const size = renderer.fit();
  const engine = new Engine((Math.random() * 0xffffffff) >>> 0, size.cols, size.rows);

  let rafId = 0;
  let animated = false;
  let limiter = createLimiter(30);
  let active = ''; // modo efectivo en pantalla
  let sysinfo = null; // solo existe una vez usado el modo dashboard
  let lastText = '';

  // El panel de sistema recibe sus datos desde JS justo antes de avanzar/pintar.
  function feedDashboard() {
    engine.feed(sysinfo.data());
    const text = sysinfo.text();
    if (text !== lastText) {
      lastText = text;
      engine.feed_text(text);
    }
  }

  // La vista se recrea en cada frame: la memoria de WASM puede crecer y soltar el buffer anterior.
  const paint = () => {
    const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
    renderer.draw(cells, engine.width(), engine.height());
  };

  function frame(now) {
    rafId = requestAnimationFrame(frame);
    if (!limiter.ready(now)) return;
    if (active === 'dashboard') feedDashboard();
    engine.step();
    paint();
  }

  function start() {
    if (!rafId && !document.hidden) rafId = requestAnimationFrame(frame);
  }

  function stop() {
    if (rafId) cancelAnimationFrame(rafId);
    rafId = 0;
  }

  let modeGen = 0;

  async function applyMode(mode) {
    const gen = ++modeGen;
    stop();
    sysinfo?.stop();
    animated = false;
    active = '';
    let effective = mode;

    if (mode === 'image') {
      let img = null;
      try {
        img = await loadImage();
      } catch (err) {
        console.error('No se pudo leer la imagen guardada', err);
      }
      if (gen !== modeGen) return; // Llegó una llamada más nueva mientras se leía IndexedDB.
      if (!img || !engine.load_image(img.data, img.w, img.h)) {
        effective = 'fire';
        say('No hay una imagen válida guardada: se muestra el fuego.');
      }
    }

    if (effective !== 'image') {
      engine.set_fps(FPS[effective]);
      engine.set_animation(effective);
    }
    renderer.invalidate();
    active = effective;
    if (effective === 'dashboard') {
      sysinfo ??= createSysinfo();
      lastText = '';
    }

    if (effective === 'image') {
      paint(); // Imagen estática: se pinta una vez y no hay bucle.
      return;
    }
    if (reducedMotion.matches) {
      if (effective === 'dashboard') {
        try {
          await sysinfo.sample(); // un solo frame, pero con datos reales
        } catch (err) {
          console.error('No se pudieron leer las métricas', err);
        }
        if (gen !== modeGen) return;
        feedDashboard();
      }
      for (let i = 0; i < REDUCED_MOTION_WARMUP; i++) engine.step();
      paint(); // Con prefers-reduced-motion se muestra un solo frame.
      return;
    }
    animated = true;
    limiter = createLimiter(FPS[effective]);
    if (effective === 'dashboard' && !document.hidden) sysinfo.start();
    start();
  }

  document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
      stop();
      sysinfo?.stop();
    } else if (animated) {
      limiter.reset();
      if (active === 'dashboard') sysinfo.start();
      start();
    }
  });

  let resizeTimer = 0;
  const scheduleRefit = () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      const next = renderer.fit();
      engine.resize(next.cols, next.rows);
      if (animated) renderer.invalidate();
      else applyMode(settings.mode);
    }, 150);
  };
  window.addEventListener('resize', scheduleRefit);
  watchDpr(scheduleRefit); // un cambio de escala (otro monitor) no siempre dispara `resize`

  // --- Panel de KPI (se actualiza por eventos, sin polling) ---
  const kpiStore = createKpiStore(area);
  const kpiEl = $('kpi');
  const refreshKpi = async () => renderKpi(kpiEl, await kpiStore.readToday());
  chrome.storage.onChanged.addListener((changes, areaName) => {
    if (areaName === 'local' && Object.keys(changes).some((k) => k.startsWith('day:'))) refreshKpi();
  });

  function setKpiVisible(visible) {
    settings.kpiVisible = visible;
    kpiEl.hidden = !visible;
    $('kpi-visible').checked = visible;
    saveSettings(area, settings);
  }

  // --- Ajustes ---
  const gear = $('gear');
  const panel = $('settings');
  gear.addEventListener('click', () => {
    panel.hidden = !panel.hidden;
    gear.setAttribute('aria-expanded', String(!panel.hidden));
  });

  $('mode').value = settings.mode;
  $('mode').addEventListener('change', async (e) => {
    settings.mode = e.target.value;
    say('');
    await saveSettings(area, settings);
    await applyMode(settings.mode);
  });

  $('kpi-visible').addEventListener('change', (e) => setKpiVisible(e.target.checked));
  document.addEventListener('keydown', (e) => {
    const typing = /^(INPUT|SELECT|TEXTAREA)$/.test(e.target.tagName);
    if (!typing && !e.ctrlKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === 'k') {
      setKpiVisible(kpiEl.hidden);
    }
  });

  $('image-file').addEventListener('change', async (e) => {
    const file = e.target.files[0];
    e.target.value = '';
    if (!file) return;
    try {
      say('Convirtiendo…');
      await importImage(file);
      settings.mode = 'image';
      $('mode').value = 'image';
      await saveSettings(area, settings);
      await applyMode('image');
      say('Imagen guardada.');
    } catch (err) {
      say(err.message || 'No se pudo convertir la imagen.');
    }
  });

  kpiEl.hidden = !settings.kpiVisible;
  $('kpi-visible').checked = settings.kpiVisible;
  await applyMode(settings.mode);
  await refreshKpi();
}

/** Reduce la imagen en el navegador, la convierte a ASCII con WASM y la guarda. */
async function importImage(file) {
  if (!file.type.startsWith('image/')) throw new Error('El archivo no es una imagen.');
  if (file.size > MAX_FILE_BYTES) throw new Error('La imagen pesa más de 20 MB.');

  let bitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    throw new Error('No se pudo leer la imagen (formato no compatible).');
  }
  try {
    const [w, h] = image_target_dims(bitmap.width, bitmap.height, MAX_COLS, MAX_ROWS);
    const canvas = new OffscreenCanvas(w, h);
    const g = canvas.getContext('2d', { willReadFrequently: true });
    g.drawImage(bitmap, 0, 0, w, h);
    const rgba = g.getImageData(0, 0, w, h).data;
    const packed = image_to_ascii(new Uint8Array(rgba.buffer, rgba.byteOffset, rgba.byteLength), w, h, false);
    const img = { w, h, data: packed };
    if (!validateImage(img)) throw new Error('No se pudo convertir la imagen.');
    await saveImage(img);
  } finally {
    bitmap.close();
  }
}

main().catch((err) => {
  console.error(err);
  say('Error al iniciar: ' + (err.message || err));
});

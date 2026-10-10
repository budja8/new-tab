import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';

const wasmPath = new URL('../extension/pkg/newtab_core_bg.wasm', import.meta.url);
const glue = '../extension/pkg/newtab_core.js';
const built = existsSync(wasmPath);
const opts = { skip: built ? false : 'falta extension/pkg: ejecuta npm run build:wasm' };

async function load() {
  const mod = await import(glue);
  const wasm = await mod.default({ module_or_path: readFileSync(wasmPath) });
  return { mod, wasm };
}

test('el buffer de celdas se comparte por memoria y respeta el stride', opts, async () => {
  const { mod, wasm } = await load();
  const stride = mod.cell_stride();
  assert.equal(stride, 3);

  const engine = new mod.Engine(1, 20, 10);
  assert.equal(engine.set_animation('fire'), true);
  engine.step();

  assert.equal(engine.cells_len(), 20 * 10 * stride);
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  const bottomLeft = (9 * 20 + 0) * stride;
  assert.equal(String.fromCodePoint(cells[bottomLeft]), 'M');
  assert.equal(cells[bottomLeft + 1], 0xffffffff);
  engine.free();
});

test('una animación desconocida se rechaza sin romper el motor', opts, async () => {
  const { mod } = await load();
  const engine = new mod.Engine(1, 5, 5);
  assert.equal(engine.set_animation('nope'), false);
  engine.free();
});

test('imagen → ASCII empaquetado → motor', opts, async () => {
  const { mod, wasm } = await load();
  const [w, h] = mod.image_target_dims(1000, 500, 200, 100);
  assert.deepEqual([w, h], [200, 55]);

  const packed = mod.image_to_ascii(new Uint8Array([255, 255, 255, 255]), 1, 1, false);
  assert.deepEqual([...packed], [9, 255, 255, 255]);

  const engine = new mod.Engine(1, 5, 3);
  assert.equal(engine.load_image(packed, 1, 1), true);
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  assert.equal(String.fromCodePoint(cells[(1 * 5 + 2) * 3]), '@');
  assert.equal(engine.load_image(new Uint8Array([1, 2, 3]), 1, 1), false);
  engine.free();
});

test('pipes: se activa, acepta set_fps y dibuja glifos de tubería', opts, async () => {
  const { mod, wasm } = await load();
  const engine = new mod.Engine(1, 20, 10);
  assert.equal(engine.set_animation('pipes'), true);
  engine.set_fps(60);
  for (let i = 0; i < 30; i++) engine.step();
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  const glyphs = new Set([0x2551, 0x2550, 0x25cf]);
  let found = false;
  for (let i = 0; i < cells.length; i += 3) if (glyphs.has(cells[i])) found = true;
  assert.ok(found, 'ninguna celda con ║ ═ ●');
  engine.free();
});

test('dna: se activa y dibuja nodos de hebra', opts, async () => {
  const { mod, wasm } = await load();
  const engine = new mod.Engine(1, 20, 10);
  assert.equal(engine.set_animation('dna'), true);
  for (let i = 0; i < 30; i++) engine.step();
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  let found = false;
  for (let i = 0; i < cells.length; i += 3) if (cells[i] === 0x25cf || cells[i] === 0x6f) found = true;
  assert.ok(found, 'ninguna celda con ● u o');
  engine.free();
});

test('synthwave: se activa y dibuja sol, cuadrícula y título', opts, async () => {
  const { mod, wasm } = await load();
  const engine = new mod.Engine(1, 80, 24);
  assert.equal(engine.set_animation('synthwave'), true);
  for (let i = 0; i < 10; i++) engine.step();
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  const seen = new Set();
  for (let i = 0; i < cells.length; i += 3) seen.add(cells[i]);
  assert.ok(seen.has(0x2588), 'ninguna celda con █');
  assert.ok(seen.has(0x2550), 'ninguna celda con ═');
  const title = '** SYNTHWAVE HORIZON **';
  const x0 = Math.floor((80 - title.length) / 2);
  for (let i = 0; i < title.length; i++) assert.equal(cells[(1 * 80 + x0 + i) * 3], title.codePointAt(i));
  engine.free();
});

test('compile_sim: se activa y dibuja la primera línea de descarga', opts, async () => {
  const { mod, wasm } = await load();
  const engine = new mod.Engine(1, 80, 24);
  assert.equal(engine.set_animation('compile_sim'), true);
  for (let i = 0; i < 5; i++) engine.step();
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  const line = '  Downloading libc v0.2.155 from crates.io...';
  for (let i = 0; i < line.length; i++) assert.equal(cells[(0 * 80 + 1 + i) * 3], line.codePointAt(i));
  engine.free();
});

test('pacman_install: se activa y dibuja la primera línea de sincronización', opts, async () => {
  const { mod, wasm } = await load();
  const engine = new mod.Engine(1, 80, 24);
  assert.equal(engine.set_animation('pacman_install'), true);
  for (let i = 0; i < 5; i++) engine.step();
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  const line = ':: Synchronizing package databases...';
  for (let i = 0; i < line.length; i++) assert.equal(cells[(0 * 80 + 1 + i) * 3], line.codePointAt(i));
  engine.free();
});

test('pacman: se activa y dibuja el HUD, el laberinto y a Pac-Man', opts, async () => {
  const { mod, wasm } = await load();
  const engine = new mod.Engine(1, 80, 24);
  assert.equal(engine.set_animation('pacman'), true);
  for (let i = 0; i < 5; i++) engine.step();
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  const title = 'RETRO PAC-MAN ARCADE';
  for (let i = 0; i < title.length; i++) assert.equal(cells[(0 * 80 + 1 + i) * 3], title.codePointAt(i));
  const seen = new Set();
  for (let i = 0; i < cells.length; i += 3) seen.add(cells[i]);
  assert.ok(seen.has(0x00b7), 'ningún punto');
  assert.ok(seen.has(0x2550), 'ningún divisor ═');
  engine.free();
});

test('dashboard: feed/feed_text alimentan el reloj, las barras y el panel', opts, async () => {
  const { mod, wasm } = await load();
  const engine = new mod.Engine(1, 100, 30);
  assert.equal(engine.set_animation('dashboard'), true);
  engine.feed(Float32Array.of(12, 34, 56, 4, 16, 100, 200, 2, 0.5, 0.25));
  engine.feed_text('Chrome: 126\nCores: 8');
  engine.step();
  engine.render();
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  const at = (x, y) => String.fromCodePoint(cells[(y * 100 + x) * 3]);
  const line = (x, y, n) => Array.from({ length: n }, (_, i) => at(x + i, y)).join('');
  // '1' del reloj: x0 = (100-38)/2 = 31; el sprite "  ██" se dibuja en las columnas 33 y 34 de la fila 1
  assert.equal(at(33, 1), '█');
  assert.equal(at(31, 1), ' ');
  assert.equal(line(20, 7, 3), 'RAM');
  assert.equal(line(20, 8, 4), 'DISK');
  assert.equal(line(35, 10, 4), 'CPU0');
  assert.equal(line(35, 11, 4), 'CPU1');
  assert.equal(line(1, 12, 13), '═ BROWSER (RE');
  assert.equal(line(1, 13, 11), 'Chrome: 126');
  // feed corrupto o corto y feed_text vacío no provocan pánico
  engine.feed(new Float32Array(0));
  engine.feed(Float32Array.of(NaN, Infinity, -1, 1e30, NaN, 5, 5, 1e9, NaN));
  engine.feed_text('');
  engine.step();
  engine.free();
});

test('feed y feed_text son no-op en otras animaciones', opts, async () => {
  const { mod } = await load();
  const engine = new mod.Engine(1, 20, 10);
  engine.feed(Float32Array.of(1, 2, 3));
  engine.feed_text('A: b');
  assert.equal(engine.set_animation('fire'), true);
  engine.feed(Float32Array.of(1, 2, 3, 4, 5, 6, 7, 0));
  engine.feed_text('A: b');
  engine.step();
  engine.free();
});

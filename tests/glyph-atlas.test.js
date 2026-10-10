import test from 'node:test';
import assert from 'node:assert/strict';
import {
  CAPACITY,
  SLOTS_PER_ROW,
  atlasSize,
  createSlotAllocator,
  fitsTexture,
  glyphBlock,
  luminanceBucket,
} from '../extension/src/glyph-atlas.js';

test('el espacio y el codepoint 0 son celdas en blanco: ranura 0, sin rasterizar', () => {
  const slots = createSlotAllocator();
  let created = 0;
  assert.equal(slots.slotFor(32, () => created++), 0);
  assert.equal(slots.slotFor(0, () => created++), 0);
  assert.equal(created, 0);
  assert.equal(slots.size, 0);
});

test('un glifo nuevo recibe la ranura 1 y avisa una sola vez', () => {
  const slots = createSlotAllocator();
  const seen = [];
  assert.equal(slots.slotFor(65, (cp, slot) => seen.push([cp, slot])), 1);
  assert.equal(slots.slotFor(65, (cp, slot) => seen.push([cp, slot])), 1);
  assert.deepEqual(seen, [[65, 1]]);
  assert.equal(slots.size, 1);
});

test('glifos distintos reciben ranuras consecutivas', () => {
  const slots = createSlotAllocator();
  assert.deepEqual([65, 66, 67].map((cp) => slots.slotFor(cp)), [1, 2, 3]);
});

test('emoji, CJK y glifos de caja se asignan como cualquier otro', () => {
  const slots = createSlotAllocator();
  for (const cp of [0x1f600, 0x4e2d, 0x2588, 0x10ffff]) {
    assert.ok(slots.slotFor(cp) > 0, cp.toString(16));
  }
  assert.equal(slots.size, 4);
});

test('con el atlas lleno los glifos nuevos caen en blanco y los conocidos se conservan', () => {
  const slots = createSlotAllocator(4); // ranuras 1, 2 y 3
  const seen = [];
  assert.equal(slots.slotFor(65, (c) => seen.push(c)), 1);
  assert.equal(slots.slotFor(66, (c) => seen.push(c)), 2);
  assert.equal(slots.slotFor(67, (c) => seen.push(c)), 3);
  assert.equal(slots.slotFor(68, (c) => seen.push(c)), 0);
  assert.equal(slots.slotFor(66), 2);
  assert.deepEqual(seen, [65, 66, 67]);
  assert.equal(slots.size, 3);
});

test('glyphBlock ubica cada ranura en una rejilla de 64 columnas con 8 filas de luminancia', () => {
  assert.equal(SLOTS_PER_ROW, 64);
  assert.deepEqual(glyphBlock(1, 9, 16), { x: 9, y: 0, w: 9, h: 128 });
  assert.deepEqual(glyphBlock(64, 9, 16), { x: 0, y: 128, w: 9, h: 128 });
  assert.deepEqual(glyphBlock(65, 9, 16), { x: 9, y: 128, w: 9, h: 128 });
});

test('atlasSize crece con el tamaño de celda (dpr)', () => {
  assert.equal(CAPACITY, 512);
  assert.deepEqual(atlasSize(9, 16), { width: 576, height: 1024 });
  assert.deepEqual(atlasSize(27, 48), { width: 1728, height: 3072 });
});

test('fitsTexture compara el atlas con el tamaño máximo de textura', () => {
  assert.equal(fitsTexture(4096, 9, 16), true);
  assert.equal(fitsTexture(1024, 9, 16), true);
  assert.equal(fitsTexture(1000, 9, 16), false);
  assert.equal(fitsTexture(4096, 27, 48), true);
  assert.equal(fitsTexture(2048, 27, 48), false);
});

test('luminanceBucket reproduce la fórmula del shader del spike (valores derivados a mano)', () => {
  // Si una falla, revisa primero la transcripción de la fórmula, no el valor esperado.
  assert.equal(luminanceBucket(0), 6); // sin color → gris por defecto 0xcccccc
  assert.equal(luminanceBucket(0xffffffff), 7); // blanco
  assert.equal(luminanceBucket(0xff000000), 0); // negro opaco (distinto de "sin color")
  assert.equal(luminanceBucket(0xffff6a07), 4); // naranja del fuego
  assert.equal(luminanceBucket(0xffe72f2f), 2); // rojo del fuego
  assert.equal(luminanceBucket(0xff6f1717), 0); // rojo oscuro del fuego
});

test('luminanceBucket es monótono en los grises y alcanza las 8 cubetas', () => {
  const seen = new Set();
  let last = -1;
  for (let g = 0; g < 256; g++) {
    const b = luminanceBucket((0xff000000 | (g << 16) | (g << 8) | g) >>> 0);
    assert.ok(b >= last, `g=${g}`);
    assert.ok(b >= 0 && b <= 7);
    seen.add(b);
    last = b;
  }
  assert.equal(seen.size, 8);
});

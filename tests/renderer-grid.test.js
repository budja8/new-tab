import test from 'node:test';
import assert from 'node:assert/strict';
import { gridFor, MAX_DIM } from '../extension/src/renderer.js';

for (const [w, h, dpr] of [[1280, 720, 1.5], [1536, 864, 1.25]]) {
  test(`la cuadrícula llena la ventana sin excederla (${w}x${h} @${dpr})`, () => {
    const g = gridFor(w, h, dpr, 9, 16);
    assert.ok(g.cols * g.pw <= w * dpr);
    assert.ok((g.cols + 1) * g.pw > w * dpr);
    assert.ok(g.rows * g.ph <= h * dpr);
    assert.ok((g.rows + 1) * g.ph > h * dpr);
  });
}

test('dpr 1 no cambia el resultado anterior', () => {
  assert.deepEqual(gridFor(1280, 720, 1, 9, 16), { pw: 9, ph: 16, cols: 142, rows: 45 });
});

test('las columnas se acotan a MAX_DIM', () => {
  const g = gridFor(10240, 1440, 0.25, 9, 16);
  assert.equal(g.cols, 1024);
  assert.equal(g.cols, MAX_DIM);
});

test('las filas se acotan a MAX_DIM', () => {
  assert.equal(gridFor(800, 20000, 1, 9, 16).rows, MAX_DIM);
});

test('una ventana de 1x1 da una cuadrícula de 1x1', () => {
  const g = gridFor(1, 1, 1, 9, 16);
  assert.equal(g.cols, 1);
  assert.equal(g.rows, 1);
});

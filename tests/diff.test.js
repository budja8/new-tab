import test from 'node:test';
import assert from 'node:assert/strict';
import { diffCells } from '../extension/src/diff.js';

const cells = (...rows) => new Uint32Array(rows.flat());
const run = (prev, cur, stride) => {
  const out = new Int32Array(cur.length);
  const n = diffCells(prev, cur, out, stride);
  return [...out.subarray(0, n)];
};

test('sin frame previo todas las celdas cambian', () => {
  assert.deepEqual(run(null, cells([65, 1, 0], [66, 2, 0], [67, 3, 0])), [0, 1, 2]);
});

test('frames idénticos no cambian nada', () => {
  const a = cells([65, 1, 0], [66, 2, 0]);
  assert.deepEqual(run(a, a.slice()), []);
});

test('detecta cambios en carácter, fg o bg por separado', () => {
  const prev = cells([65, 1, 0], [66, 2, 0], [67, 3, 0], [68, 4, 0]);
  const cur = cells([99, 1, 0], [66, 9, 0], [67, 3, 7], [68, 4, 0]);
  assert.deepEqual(run(prev, cur), [0, 1, 2]);
});

test('si cambia el tamaño del buffer se repinta todo', () => {
  assert.deepEqual(run(cells([65, 1, 0]), cells([65, 1, 0], [66, 2, 0])), [0, 1]);
});

test('respeta un stride distinto', () => {
  const prev = new Uint32Array([1, 2, 3, 4]);
  const cur = new Uint32Array([1, 2, 9, 4]);
  assert.deepEqual(run(prev, cur, 2), [1]);
});

test('un buffer vacío no produce celdas', () => {
  assert.deepEqual(run(null, new Uint32Array(0)), []);
});

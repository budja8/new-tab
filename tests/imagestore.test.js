import test from 'node:test';
import assert from 'node:assert/strict';
import { MAX_COLS, MAX_IMAGE_BYTES, MAX_ROWS, validateImage } from '../extension/src/imagestore.js';

const img = (w, h, data = new Uint8Array(w * h * 4)) => ({ w, h, data });

test('los topes coinciden con el spec', () => {
  assert.equal(MAX_COLS, 200);
  assert.equal(MAX_ROWS, 100);
  assert.equal(MAX_IMAGE_BYTES, 200 * 1024);
});

test('la cuadrícula máxima cabe en el tope de bytes', () => {
  assert.equal(validateImage(img(MAX_COLS, MAX_ROWS)), true);
  assert.ok(MAX_COLS * MAX_ROWS * 4 <= MAX_IMAGE_BYTES);
});

test('acepta una imagen mínima de 1×1', () => {
  assert.equal(validateImage(img(1, 1)), true);
});

test('rechaza datos que no cuadran con las dimensiones', () => {
  assert.equal(validateImage(img(2, 2, new Uint8Array(15))), false);
  assert.equal(validateImage(img(2, 2, new Uint8Array(17))), false);
  assert.equal(validateImage(img(2, 2, new Uint8Array(0))), false);
});

test('rechaza dimensiones inválidas o fuera del tope', () => {
  assert.equal(validateImage(img(0, 5, new Uint8Array(0))), false);
  assert.equal(validateImage(img(-1, 5, new Uint8Array(0))), false);
  assert.equal(validateImage(img(1.5, 2, new Uint8Array(12))), false);
  assert.equal(validateImage(img(MAX_COLS + 1, 1)), false);
  assert.equal(validateImage(img(1, MAX_ROWS + 1)), false);
  assert.equal(validateImage(img(NaN, 2, new Uint8Array(8))), false);
});

test('rechaza valores que no son una imagen', () => {
  assert.equal(validateImage(null), false);
  assert.equal(validateImage(undefined), false);
  assert.equal(validateImage({}), false);
  assert.equal(validateImage({ w: 1, h: 1, data: [0, 0, 0, 0] }), false);
  assert.equal(validateImage({ w: 1, h: 1, data: 'abcd' }), false);
});

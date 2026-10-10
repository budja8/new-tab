import test from 'node:test';
import assert from 'node:assert/strict';
import '../extension/content/keyname.js';

const { keyName } = globalThis.NT;
const ev = (o) => ({
  key: '',
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  shiftKey: false,
  getModifierState: () => false,
  ...o,
});

test('una letra simple es una tecla sin atajo', () => {
  assert.deepEqual(keyName(ev({ key: 'e' })), { key: 'e', shortcut: null });
});

test('la mayúscula con shift cuenta como la misma tecla', () => {
  assert.deepEqual(keyName(ev({ key: 'E', shiftKey: true })), { key: 'e', shortcut: null });
});

test('espacio, escape y flechas tienen nombre legible', () => {
  assert.equal(keyName(ev({ key: ' ' })).key, 'space');
  assert.equal(keyName(ev({ key: 'Escape' })).key, 'esc');
  assert.equal(keyName(ev({ key: 'ArrowLeft' })).key, 'left');
  assert.equal(keyName(ev({ key: 'Enter' })).key, 'enter');
  assert.equal(keyName(ev({ key: 'Backspace' })).key, 'backspace');
});

test('los modificadores solos no cuentan', () => {
  for (const key of ['Control', 'Shift', 'Alt', 'Meta', 'AltGraph']) {
    assert.equal(keyName(ev({ key })), null, key);
  }
});

test('ctrl+c es un atajo', () => {
  assert.deepEqual(keyName(ev({ key: 'c', ctrlKey: true })), { key: 'c', shortcut: 'ctrl+c' });
});

test('el orden de modificadores es estable: ctrl, alt, meta, shift', () => {
  assert.equal(keyName(ev({ key: 't', ctrlKey: true, shiftKey: true })).shortcut, 'ctrl+shift+t');
  assert.equal(keyName(ev({ key: 'l', metaKey: true })).shortcut, 'meta+l');
  assert.equal(keyName(ev({ key: 'x', ctrlKey: true, altKey: true, metaKey: true, shiftKey: true })).shortcut, 'ctrl+alt+meta+shift+x');
});

test('AltGr (ctrl+alt en Windows) escribe un carácter y no es un atajo', () => {
  const altGr = ev({ key: '@', ctrlKey: true, altKey: true, getModifierState: (m) => m === 'AltGraph' });
  assert.deepEqual(keyName(altGr), { key: '@', shortcut: null });
});

test('teclas muertas, IME y desconocidas no cuentan', () => {
  for (const key of ['Dead', 'Unidentified', 'Process', '']) {
    assert.equal(keyName(ev({ key })), null, JSON.stringify(key));
  }
  assert.equal(keyName(ev({ key: undefined })), null);
  assert.equal(keyName(ev({ key: 42 })), null);
});

test('nombres absurdamente largos se descartan', () => {
  assert.equal(keyName(ev({ key: 'x'.repeat(50) })), null);
});

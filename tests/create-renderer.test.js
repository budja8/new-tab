import test from 'node:test';
import assert from 'node:assert/strict';
import { chooseBackend, createRenderer, isSoftwareRenderer } from '../extension/src/create-renderer.js';

const okInfo = { hasWebGL2: true, rendererName: 'ANGLE (Intel, Intel(R) UHD Graphics Direct3D11 vs_5_0 ps_5_0)', maxTextureSize: 16384, pw: 9, ph: 16 };

test('detecta los renderizadores por software', () => {
  assert.equal(isSoftwareRenderer('ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)), SwiftShader driver)'), true);
  assert.equal(isSoftwareRenderer('llvmpipe (LLVM 15.0.7, 256 bits)'), true);
  assert.equal(isSoftwareRenderer('Microsoft Basic Render Driver'), true);
  assert.equal(isSoftwareRenderer(okInfo.rendererName), false);
  assert.equal(isSoftwareRenderer(undefined), false);
  assert.equal(isSoftwareRenderer(''), false);
});

test('sin WebGL2 se usa el renderer 2D', () => {
  const d = chooseBackend({ hasWebGL2: false, pw: 9, ph: 16 });
  assert.equal(d.backend, '2d');
  assert.match(d.reason, /WebGL2/);
});

test('una GPU por software se descarta: no hay ganancia con WebGL', () => {
  const d = chooseBackend({ ...okInfo, rendererName: 'Google SwiftShader' });
  assert.equal(d.backend, '2d');
  assert.match(d.reason, /software/i);
});

test('si el atlas no cabe en una textura se usa el renderer 2D', () => {
  const d = chooseBackend({ ...okInfo, maxTextureSize: 512 });
  assert.equal(d.backend, '2d');
  assert.match(d.reason, /atlas/);
});

test('con WebGL2 por hardware se elige gl y se informa el renderizador', () => {
  const d = chooseBackend(okInfo);
  assert.equal(d.backend, 'gl');
  assert.equal(d.reason, okInfo.rendererName);
});

test('createRenderer construye el renderer WebGL cuando el sondeo lo permite', () => {
  const canvas = { id: 'bg' };
  const made = [];
  const out = createRenderer(canvas, { stride: 3 }, {
    probe: () => okInfo,
    makeGl: (c, o) => { made.push(['gl', c, o]); return { kind: 'gl' }; },
    make2d: () => assert.fail('no debe construir el 2D'),
    replaceCanvas: () => assert.fail('no debe reemplazar el canvas'),
    warn: () => {},
  });
  assert.equal(out.backend, 'gl');
  assert.deepEqual(out.renderer, { kind: 'gl' });
  assert.equal(out.canvas, canvas);
  assert.deepEqual(made, [['gl', canvas, { stride: 3 }]]);
});

test('createRenderer no toca WebGL cuando el sondeo lo descarta', () => {
  const canvas = { id: 'bg' };
  const out = createRenderer(canvas, {}, {
    probe: () => ({ hasWebGL2: false, pw: 9, ph: 16 }),
    makeGl: () => assert.fail('no debe construir WebGL'),
    make2d: (c) => ({ kind: '2d', c }),
    replaceCanvas: () => assert.fail('no debe reemplazar el canvas'),
    warn: () => {},
  });
  assert.equal(out.backend, '2d');
  assert.equal(out.canvas, canvas);
  assert.match(out.reason, /WebGL2/);
});

test('si el constructor de WebGL falla se reemplaza el canvas y se usa el 2D', () => {
  const original = { id: 'viejo' };
  const fresh = { id: 'nuevo' };
  const warnings = [];
  const out = createRenderer(original, {}, {
    probe: () => okInfo,
    makeGl: () => { throw new Error('shader roto'); },
    make2d: (c) => ({ kind: '2d', c }),
    replaceCanvas: (c) => { assert.equal(c, original); return fresh; },
    warn: (...args) => warnings.push(args.join(' ')),
  });
  assert.equal(out.backend, '2d');
  assert.equal(out.canvas, fresh); // el contexto WebGL fallido bloquea el canvas original
  assert.deepEqual(out.renderer, { kind: '2d', c: fresh });
  assert.match(out.reason, /shader roto/);
  assert.equal(warnings.length, 1);
});

test('si el sondeo lanza se trata como sin WebGL2 y no se rompe', () => {
  const canvas = { id: 'bg' };
  const warnings = [];
  const out = createRenderer(canvas, {}, {
    probe: () => { throw new Error('sondeo roto'); },
    makeGl: () => assert.fail('no debe construir WebGL'),
    make2d: (c) => ({ kind: '2d', c }),
    replaceCanvas: () => assert.fail('no debe reemplazar el canvas'),
    warn: (...args) => warnings.push(args.join(' ')),
  });
  assert.equal(out.backend, '2d');
  assert.match(out.reason, /WebGL2/);
  assert.equal(out.canvas, canvas);
  assert.equal(warnings.length, 1);
});

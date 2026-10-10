import test from 'node:test';
import assert from 'node:assert/strict';
import { watchDpr } from '../extension/src/dpr-watch.js';

/** `window` mínimo: matchMedia registra consultas y `fire()` imita el evento `change` con `once`. */
function fakeWin(dpr) {
  const win = {
    devicePixelRatio: dpr,
    queries: [],
    matchMedia(q) {
      const m = {
        q,
        handlers: [],
        addEventListener(_type, h) { this.handlers.push(h); },
        removeEventListener(_type, h) { this.handlers = this.handlers.filter((x) => x !== h); },
        fire() { const hs = this.handlers; this.handlers = []; hs.forEach((h) => h()); },
      };
      win.queries.push(m);
      return m;
    },
  };
  return win;
}

test('arma la consulta con el DPR actual y avisa al cambiar, rearmando con el nuevo', () => {
  const win = fakeWin(1.5);
  let calls = 0;
  watchDpr(() => calls++, win);
  assert.equal(win.queries.length, 1);
  assert.equal(win.queries[0].q, '(resolution: 1.5dppx)');

  win.devicePixelRatio = 2;
  win.queries[0].fire();
  assert.equal(calls, 1);
  assert.equal(win.queries.length, 2);
  assert.equal(win.queries[1].q, '(resolution: 2dppx)');
});

test('varios cambios seguidos avisan cada vez', () => {
  const win = fakeWin(1);
  let calls = 0;
  watchDpr(() => calls++, win);
  win.devicePixelRatio = 1.25;
  win.queries[0].fire();
  win.devicePixelRatio = 1.5;
  win.queries[1].fire();
  win.devicePixelRatio = 1;
  win.queries[2].fire();
  assert.equal(calls, 3);
  assert.equal(win.queries.length, 4);
});

test('stop() detiene los avisos y no vuelve a armar la consulta', () => {
  const win = fakeWin(1);
  let calls = 0;
  const stop = watchDpr(() => calls++, win);
  stop();
  win.queries[0].fire();
  assert.equal(calls, 0);
  assert.equal(win.queries.length, 1);
});

test('si onChange lanza, la vigilancia sigue armada', () => {
  const win = fakeWin(1);
  watchDpr(() => { throw new Error('boom'); }, win);
  win.devicePixelRatio = 2;
  assert.throws(() => win.queries[0].fire());
  assert.equal(win.queries.length, 2);
  assert.equal(win.queries[1].handlers.length, 1);
});

test('stop() llamado dentro de onChange no deja escuchas huérfanas', () => {
  const win = fakeWin(1);
  let calls = 0;
  let stop;
  stop = watchDpr(() => { calls++; stop(); }, win);
  win.queries[0].fire();
  assert.equal(calls, 1);
  const last = win.queries[win.queries.length - 1];
  assert.equal(last.handlers.length, 0);
  last.fire();
  assert.equal(calls, 1);
});

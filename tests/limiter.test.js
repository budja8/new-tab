import test from 'node:test';
import assert from 'node:assert/strict';
import { createLimiter } from '../extension/src/limiter.js';

/** Cuenta cuántos de los ticks de un monitor de `hz` Hz durante 1 s dejan pasar un frame. */
function framesPerSecond(fps, hz, jitterMs = 0) {
  const limiter = createLimiter(fps);
  const step = 1000 / hz;
  let ready = 0;
  for (let k = 0; k * step < 1000; k++) {
    const jitter = jitterMs ? ((k * 7919) % 3 - 1) * jitterMs : 0;
    if (limiter.ready(k * step + jitter)) ready++;
  }
  return ready;
}

test('el primer tick siempre pasa', () => {
  assert.equal(createLimiter(30).ready(1234), true);
});

test('30 FPS en un monitor de 60 Hz deja pasar ~30 frames por segundo', () => {
  const n = framesPerSecond(30, 60);
  assert.ok(n >= 29 && n <= 31, `pasaron ${n}`);
});

test('60 FPS en un monitor de 60 Hz no se reduce a 30 por el jitter de rAF', () => {
  const n = framesPerSecond(60, 60, 1);
  assert.ok(n >= 57, `pasaron ${n}`);
});

test('en un monitor de 144 Hz con 60 FPS no pasa de ~60', () => {
  const n = framesPerSecond(60, 144);
  assert.ok(n >= 58 && n <= 62, `pasaron ${n}`);
});

test('tras una pausa larga no ráfaga frames atrasados', () => {
  const limiter = createLimiter(30);
  assert.equal(limiter.ready(0), true);
  assert.equal(limiter.ready(10_000), true);
  assert.equal(limiter.ready(10_001), false);
  assert.equal(limiter.ready(10_034), true);
});

test('reset permite pintar de inmediato', () => {
  const limiter = createLimiter(30);
  limiter.ready(0);
  assert.equal(limiter.ready(5), false);
  limiter.reset();
  assert.equal(limiter.ready(6), true);
});

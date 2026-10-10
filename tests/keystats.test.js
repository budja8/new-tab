import test from 'node:test';
import assert from 'node:assert/strict';
import {
  OTHERS,
  applyBatch,
  cutoffKey,
  dateKey,
  emptyDay,
  pruneDay,
  pruneTop,
  summarize,
  topEntries,
  wpm,
} from '../extension/src/keystats.js';

const MIN = 60_000;
const T0 = new Date(2026, 9, 8, 15, 0, 0).getTime(); // 8 oct 2026, 15:00 local
const batch = (o = {}) => ({ host: 'github.com', total: 3, keys: { e: 2, space: 1 }, shortcuts: {}, ...o });

test('dateKey usa la fecha local', () => {
  assert.equal(dateKey(T0), '2026-10-08');
});

test('cutoffKey resta 90 días', () => {
  assert.equal(cutoffKey(T0), '2026-07-10');
});

test('applyBatch acumula total, teclas, atajos y sitios', () => {
  const day = emptyDay();
  applyBatch(day, batch({ shortcuts: { 'ctrl+c': 1 } }), T0);
  applyBatch(day, batch({ host: 'example.com', total: 1, keys: { e: 1 } }), T0);
  assert.equal(day.total, 4);
  assert.deepEqual(day.keys, { e: 3, space: 1 });
  assert.deepEqual(day.shortcuts, { 'ctrl+c': 1 });
  assert.deepEqual(day.sites, { 'github.com': 3, 'example.com': 1 });
});

test('claves peligrosas se ignoran y no contaminan el objeto', () => {
  const day = emptyDay();
  const evil = JSON.parse('{"__proto__": 5, "constructor": 2, "e": 1}');
  applyBatch(day, batch({ host: '__proto__', total: 8, keys: evil }), T0);
  assert.equal(Object.hasOwn(day.keys, '__proto__'), false);
  assert.equal(Object.hasOwn(day.keys, 'constructor'), false);
  assert.equal(Object.hasOwn(day.sites, '__proto__'), false);
  assert.equal(Object.getPrototypeOf(day.keys), Object.prototype);
  assert.equal({}.polluted, undefined);
  assert.deepEqual(day.keys, { e: 1 });
});

test('el hostname reservado "otros" no se mezcla con el agregado', () => {
  const day = emptyDay();
  applyBatch(day, batch({ host: OTHERS }), T0);
  assert.equal(Object.hasOwn(day.sites, OTHERS), false);
});

test('conteos inválidos (NaN, negativos, texto) se ignoran', () => {
  const day = emptyDay();
  applyBatch(day, batch({ total: 'x', keys: { e: 'y' } }), T0);
  applyBatch(day, batch({ total: -4 }), T0);
  applyBatch(day, batch({ total: NaN }), T0);
  applyBatch(day, null, T0);
  applyBatch(day, undefined, T0);
  assert.deepEqual(day, emptyDay());
});

test('active_ms suma un minuto por cada minuto nuevo con actividad', () => {
  const day = emptyDay();
  applyBatch(day, batch(), T0);
  applyBatch(day, batch(), T0 + 10_000); // mismo minuto
  assert.equal(day.active_ms, MIN);
  applyBatch(day, batch(), T0 + 2 * MIN);
  assert.equal(day.active_ms, 2 * MIN);
});

test('wpm: cada 5 teclas son una palabra, promediadas por minuto activo', () => {
  const m = Math.floor(T0 / MIN);
  assert.equal(wpm({ [m]: 50 }, T0), 10);
  assert.equal(wpm({ [m]: 50, [m - 1]: 100 }, T0), 15); // (150/5)/2
  assert.equal(wpm({}, T0), 0);
});

test('wpm ignora minutos fuera de la ventana de 5 minutos', () => {
  const m = Math.floor(T0 / MIN);
  assert.equal(wpm({ [m - 10]: 500, [m]: 25 }, T0), 5);
  assert.equal(wpm({ [m - 10]: 500 }, T0), 0);
});

test('los minutos viejos de recent se podan al llegar un lote nuevo', () => {
  const day = emptyDay();
  applyBatch(day, batch(), T0);
  applyBatch(day, batch(), T0 + 10 * MIN);
  assert.equal(Object.keys(day.recent).length, 1);
});

test('pruneTop conserva el top 20 y agrupa el resto en otros', () => {
  const map = {};
  for (let i = 0; i < 30; i++) map[`k${i}`] = 30 - i; // k0=30 … k29=1
  const out = pruneTop(map);
  assert.equal(Object.keys(out).length, 21);
  assert.equal(out.k0, 30);
  assert.equal(Object.hasOwn(out, 'k20'), false);
  assert.equal(out[OTHERS], 10 + 9 + 8 + 7 + 6 + 5 + 4 + 3 + 2 + 1);
});

test('pruneTop suma el otros previo y no lo cuenta dentro del top', () => {
  const map = { [OTHERS]: 100 };
  for (let i = 0; i < 25; i++) map[`k${i}`] = 100 - i;
  const out = pruneTop(map);
  assert.equal(Object.keys(out).filter((k) => k !== OTHERS).length, 20);
  assert.equal(out[OTHERS], 100 + (80 + 79 + 78 + 77 + 76));
});

test('pruneTop no toca mapas pequeños', () => {
  const map = { a: 2, b: 1, [OTHERS]: 7 };
  assert.deepEqual(pruneTop(map), map);
});

test('pruneDay poda teclas, atajos y sitios', () => {
  const day = emptyDay();
  for (let i = 0; i < 30; i++) {
    day.keys[`k${i}`] = i + 1;
    day.shortcuts[`s${i}`] = i + 1;
    day.sites[`h${i}.com`] = i + 1;
  }
  pruneDay(day);
  for (const map of [day.keys, day.shortcuts, day.sites]) {
    assert.equal(Object.keys(map).length, 21);
  }
});

test('topEntries ordena de mayor a menor y excluye otros', () => {
  const top = topEntries({ a: 1, b: 5, c: 3, [OTHERS]: 99 }, 2);
  assert.deepEqual(top, [['b', 5], ['c', 3]]);
});

test('summarize de un día inexistente devuelve ceros', () => {
  assert.deepEqual(summarize(undefined, T0), {
    total: 0,
    wpm: 0,
    topKeys: [],
    topShortcuts: [],
    topSites: [],
  });
});

test('summarize arma el resumen del panel', () => {
  const day = emptyDay();
  applyBatch(day, batch({ total: 50, keys: { e: 30, a: 20 }, shortcuts: { 'ctrl+c': 2 } }), T0);
  const s = summarize(day, T0);
  assert.equal(s.total, 50);
  assert.equal(s.wpm, 10);
  assert.deepEqual(s.topKeys, [['e', 30], ['a', 20]]);
  assert.deepEqual(s.topShortcuts, [['ctrl+c', 2]]);
  assert.deepEqual(s.topSites, [['github.com', 50]]);
});

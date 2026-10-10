import test from 'node:test';
import assert from 'node:assert/strict';
import { createKpiStore } from '../extension/src/store.js';

const T0 = new Date(2026, 9, 8, 15, 0, 0).getTime();
const DAY = 86_400_000;
const batch = (o = {}) => ({ host: 'github.com', total: 2, keys: { e: 2 }, shortcuts: {}, ...o });

/** Imita `chrome.storage.local`; `delay` fuerza que las operaciones se intercalen. */
function fakeArea(initial = {}, { delay = 0 } = {}) {
  const data = { ...initial };
  const wait = () => (delay ? new Promise((r) => setTimeout(r, delay)) : Promise.resolve());
  return {
    data,
    failNextSet: false,
    async get(keys) {
      await wait();
      if (keys === null) return structuredClone(data);
      const out = {};
      for (const k of Array.isArray(keys) ? keys : [keys]) {
        if (Object.hasOwn(data, k)) out[k] = structuredClone(data[k]);
      }
      return out;
    },
    async set(obj) {
      await wait();
      if (this.failNextSet) {
        this.failNextSet = false;
        throw new Error('QUOTA_BYTES exceeded');
      }
      Object.assign(data, structuredClone(obj));
    },
    async remove(keys) {
      await wait();
      for (const k of Array.isArray(keys) ? keys : [keys]) delete data[k];
    },
  };
}

test('record guarda el lote bajo la clave del día', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  await store.record(batch());
  assert.equal(area.data['day:2026-10-08'].total, 2);
  assert.equal(store.todayKey(), 'day:2026-10-08');
});

test('varios record concurrentes no pierden conteos', async () => {
  const area = fakeArea({}, { delay: 1 });
  const store = createKpiStore(area, () => T0);
  await Promise.all(Array.from({ length: 10 }, () => store.record(batch({ total: 3, keys: { a: 3 } }))));
  assert.equal(area.data['day:2026-10-08'].total, 30);
  assert.equal(area.data['day:2026-10-08'].keys.a, 30);
});

test('un fallo de storage rechaza ese record pero no rompe la cola', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  area.failNextSet = true;
  await assert.rejects(store.record(batch()), /QUOTA_BYTES/);
  await store.record(batch({ total: 5, keys: { b: 5 } }));
  assert.equal(area.data['day:2026-10-08'].total, 5);
});

test('al empezar un día nuevo se borran los registros de más de 90 días', async () => {
  const area = fakeArea({
    'day:2026-07-01': { total: 1 }, // más viejo que el corte (2026-07-10)
    'day:2026-07-10': { total: 1 }, // justo en el corte: se conserva
    'day:2026-10-07': { total: 1 },
    settings: { mode: 'fire' }, // claves ajenas no se tocan
  });
  const store = createKpiStore(area, () => T0);
  await store.record(batch());
  assert.equal(Object.hasOwn(area.data, 'day:2026-07-01'), false);
  assert.equal(Object.hasOwn(area.data, 'day:2026-07-10'), true);
  assert.equal(Object.hasOwn(area.data, 'day:2026-10-07'), true);
  assert.equal(Object.hasOwn(area.data, 'settings'), true);
});

test('la poda de días viejos solo corre cuando el día es nuevo', async () => {
  const area = fakeArea();
  let fullScans = 0;
  const get = area.get.bind(area);
  area.get = (keys) => {
    if (keys === null) fullScans += 1;
    return get(keys);
  };
  const store = createKpiStore(area, () => T0);
  await store.record(batch());
  await store.record(batch());
  await store.record(batch());
  assert.equal(fullScans, 1);
});

test('el día nuevo arranca de cero', async () => {
  const area = fakeArea();
  let now = T0;
  const store = createKpiStore(area, () => now);
  await store.record(batch());
  now = T0 + DAY;
  await store.record(batch({ total: 7, keys: { z: 7 } }));
  assert.equal(area.data['day:2026-10-08'].total, 2);
  assert.equal(area.data['day:2026-10-09'].total, 7);
});

test('record poda a las 20 entradas más usadas', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  const keys = {};
  for (let i = 0; i < 40; i++) keys[`k${i}`] = 1;
  await store.record(batch({ total: 40, keys }));
  assert.equal(Object.keys(area.data['day:2026-10-08'].keys).length, 21);
});

test('readToday resume el día actual y es cero si no hay datos', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  assert.equal((await store.readToday()).total, 0);
  await store.record(batch({ total: 50, keys: { e: 50 } }));
  const s = await store.readToday();
  assert.equal(s.total, 50);
  assert.equal(s.wpm, 10);
  assert.deepEqual(s.topKeys, [['e', 50]]);
});

test('un lote con dominio hostil no contamina nada', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  await store.record(batch({ host: '__proto__' }));
  const day = area.data['day:2026-10-08'];
  assert.equal(Object.hasOwn(day.sites, '__proto__'), false);
  assert.equal({}.polluted, undefined);
});

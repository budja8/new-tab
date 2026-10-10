import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createSysinfo } from '../extension/src/sysinfo.js';

const GB = 1024 ** 3;

/** chrome.system.* de mentira: cada getInfo de CPU devuelve el siguiente muestreo. */
function mockChrome({ cpu = [], memory, units, avail = {}, failStorage = false } = {}) {
  let i = 0;
  const calls = { cpu: 0, memory: 0, storage: 0 };
  const api = {
    system: {
      cpu: {
        getInfo: async () => {
          calls.cpu++;
          const sample = cpu[Math.min(i++, cpu.length - 1)];
          return { processors: sample.map(([total, idle]) => ({ usage: { total, idle, kernel: 0, user: 0 } })) };
        },
      },
      memory: {
        getInfo: async () => {
          calls.memory++;
          return memory;
        },
      },
      storage: {
        getInfo: async () => {
          calls.storage++;
          if (failStorage) throw new Error('boom');
          return units;
        },
        getAvailableCapacity: async (id) => ({ id, availableCapacity: avail[id] }),
      },
    },
  };
  return { api, calls };
}

const fixedNow = () => new Date(2026, 9, 9, 13, 45, 7);
const make = (opts = {}) =>
  createSysinfo({ chromeApi: opts.chromeApi, nav: opts.nav ?? {}, perf: opts.perf ?? {}, now: opts.now ?? fixedNow });

test('CPU: carga por núcleo = delta entre dos muestras, 0 en la primera', async () => {
  const { api } = mockChrome({
    cpu: [
      [[100, 50], [200, 100]],
      [[200, 100], [300, 100]], // núcleo 0: dTotal 100, dIdle 50 -> 0.5; núcleo 1: dTotal 100, dIdle 0 -> 1
    ],
  });
  const s = make({ chromeApi: api });
  await s.sample();
  assert.deepEqual([...s.data()].slice(7), [2, 0, 0]);
  await s.sample();
  assert.deepEqual([...s.data()].slice(7), [2, 0.5, 1]);
});

test('CPU: dTotal <= 0 da carga 0 y el resultado se acota a [0, 1]', async () => {
  const { api } = mockChrome({
    cpu: [
      [[100, 50], [100, 10]],
      [[100, 60], [200, 500]], // dTotal 0 -> 0; idle crece más que total -> 0 (no negativo)
    ],
  });
  const s = make({ chromeApi: api });
  await s.sample();
  await s.sample();
  assert.deepEqual([...s.data()].slice(7), [2, 0, 0]);
});

test('RAM: usada = capacity - available, en GB', async () => {
  const { api } = mockChrome({ cpu: [[]], memory: { capacity: 16 * GB, availableCapacity: 10 * GB } });
  const s = make({ chromeApi: api });
  await s.sample();
  const d = s.data();
  assert.equal(d[3], 6);
  assert.equal(d[4], 16);
});

test('disco: suma la capacidad de las unidades fixed sin duplicados y sin espacio usado', async () => {
  const { api } = mockChrome({
    cpu: [[]],
    units: [
      { id: 'a', type: 'fixed', capacity: 500 * GB },
      { id: 'b', type: 'removable', capacity: 64 * GB },
      { id: 'c', type: 'fixed', capacity: 500 * GB }, // duplicada (p. ej. Google Drive)
      { id: 'd', type: 'fixed', capacity: 100 * GB },
    ],
  });
  const s = make({ chromeApi: api });
  await s.sample();
  assert.deepEqual([...s.data()].slice(5, 7), [0, 600]); // solo capacidad (500 + 100): no hay espacio libre en escritorio
  assert.equal(s.text(), '');
});

test('disco: sin unidades fixed o con fallos la capacidad es 0', async () => {
  let { api } = mockChrome({ cpu: [[]], units: [{ id: 'b', type: 'removable', capacity: GB }] });
  let s = make({ chromeApi: api });
  await s.sample();
  assert.equal(s.data()[6], 0);

  ({ api } = mockChrome({ cpu: [[]], failStorage: true }));
  s = make({ chromeApi: api });
  await s.sample();
  assert.equal(s.data()[6], 0);
});

test('sin chrome.system el panel avisa que hay que recargar la extensión', async () => {
  const s = make({ chromeApi: {}, nav: {} });
  await s.sample();
  assert.equal(s.text(), 'System: reload the extension');
});

test('APIs ausentes o que lanzan nunca lanzan', async () => {
  for (const chromeApi of [undefined, {}, { system: {} }, { system: { cpu: { getInfo: () => { throw new Error('x'); } }, memory: { getInfo: async () => { throw new Error('y'); } } } }]) {
    const s = createSysinfo({ chromeApi, nav: undefined, perf: undefined, now: fixedNow });
    await s.sample();
    assert.equal(s.data().length, 8);
    assert.equal(typeof s.text(), 'string');
    s.start();
    s.stop();
  }
});

test('data(): layout exacto y hora local de now() en el momento de la llamada', async () => {
  const { api } = mockChrome({
    cpu: [[[10, 5]]],
    memory: { capacity: 8 * GB, availableCapacity: 6 * GB },
  });
  let t = new Date(2026, 0, 1, 1, 2, 3);
  const s = make({ chromeApi: api, now: () => t });
  await s.sample();
  let d = s.data();
  assert.ok(d instanceof Float32Array);
  assert.deepEqual([...d], [1, 2, 3, 2, 8, 0, 0, 1, 0]);
  t = new Date(2026, 0, 1, 23, 59, 58);
  assert.deepEqual([...s.data()].slice(0, 3), [23, 59, 58]);
});

test('data() antes de cualquier muestreo trae ceros y hora', () => {
  const s = make({});
  assert.deepEqual([...s.data()], [13, 45, 7, 0, 0, 0, 0, 0]);
});

test('text() con todos los datos opcionales', async () => {
  const nav = {
    userAgentData: { brands: [{ brand: 'Not A;Brand', version: '99' }, { brand: 'Google Chrome', version: '126' }], platform: 'Windows' },
    platform: 'Win32',
    hardwareConcurrency: 8,
    language: 'es-MX',
    connection: { effectiveType: '4g', downlink: 10, rtt: 50 },
    getBattery: async () => ({ level: 0.87, charging: true }),
  };
  const perf = { memory: { usedJSHeapSize: 12 * 1048576, jsHeapSizeLimit: 4096 * 1048576 } };
  const s = make({ nav, perf });
  await s.sample();
  assert.equal(
    s.text(),
    ['Chrome: 126', 'OS: Windows', 'Cores: 8', 'JS heap: 12 / 4096 MB', 'Network: 4g 10 Mbps 50 ms', 'Battery: 87 % (charging)', 'Language: es-MX'].join('\n'),
  );
});

test('text() sin datos opcionales omite esas líneas; usa la UA y nav.platform de respaldo', async () => {
  const nav = {
    userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.6422.60 Safari/537.36',
    platform: 'Linux x86_64',
    getBattery: async () => {
      throw new Error('no');
    },
  };
  const s = make({ nav });
  await s.sample();
  assert.equal(s.text(), 'Chrome: 125.0.6422.60\nOS: Linux x86_64');
  assert.equal(make({ nav: {} }).text(), '');
});

test('battery descargando', async () => {
  const s = make({ nav: { getBattery: async () => ({ level: 0.5, charging: false }) } });
  await s.sample();
  assert.equal(s.text(), 'Battery: 50 % (discharging)');
});

test('start hace un muestreo inmediato y uno por segundo; stop detiene el intervalo', async (t) => {
  t.mock.timers.enable({ apis: ['setInterval'] });
  const { api, calls } = mockChrome({ cpu: [[[1, 1]]], memory: { capacity: GB, availableCapacity: GB } });
  const s = make({ chromeApi: api });
  const flush = () => new Promise((r) => setImmediate(r));
  s.start();
  s.start(); // idempotente
  assert.equal(calls.cpu, 1);
  await flush();
  t.mock.timers.tick(1000);
  assert.equal(calls.cpu, 2);
  await flush();
  t.mock.timers.tick(1000);
  await flush();
  assert.equal(calls.cpu, 3);
  s.stop();
  t.mock.timers.tick(5000);
  await flush();
  assert.equal(calls.cpu, 3);
  s.stop(); // idempotente
});

test('sin start no hay polling', (t) => {
  t.mock.timers.enable({ apis: ['setInterval'] });
  const { api, calls } = mockChrome({ cpu: [[[1, 1]]] });
  make({ chromeApi: api });
  t.mock.timers.tick(10000);
  assert.equal(calls.cpu, 0);
});

test('el manifest declara los permisos system.* y conserva storage', () => {
  const m = JSON.parse(readFileSync(new URL('../extension/manifest.json', import.meta.url), 'utf8'));
  for (const p of ['storage', 'system.cpu', 'system.memory', 'system.storage']) assert.ok(m.permissions.includes(p), p);
});

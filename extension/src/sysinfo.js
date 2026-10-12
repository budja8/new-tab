// Métricas que reporta el navegador (chrome.system.*, navigator, performance) para el modo `dashboard`.
// Todas las dependencias se inyectan; una API ausente o que falla nunca lanza.
const GB = 1024 ** 3;
const MB = 1024 ** 2;

export function createSysinfo({
  chromeApi = globalThis.chrome,
  nav = globalThis.navigator,
  perf = globalThis.performance,
  now = () => new Date(),
} = {}) {
  let prev = null;
  let loads = [];
  let ram = [0, 0];
  let diskTotal = 0;
  let battery = null;
  let timer = 0;
  let inflight = null;

  const sys = chromeApi?.system;
  const safe = async (fn) => {
    try {
      return await fn();
    } catch {
      return undefined;
    }
  };

  async function sampleCpu() {
    const info = await safe(() => sys.cpu.getInfo());
    const cur = (info?.processors ?? []).map((p) => p.usage ?? {});
    loads = cur.map((u, i) => {
      const p = prev?.[i];
      const dT = p ? u.total - p.total : 0;
      const l = dT > 0 ? 1 - (u.idle - p.idle) / dT : 0;
      return Number.isFinite(l) ? Math.min(1, Math.max(0, l)) : 0;
    });
    prev = cur;
  }

  async function sampleRam() {
    const m = await safe(() => sys.memory.getInfo());
    ram = m ? [(m.capacity - m.availableCapacity) / GB, m.capacity / GB] : [0, 0];
  }

  // Chrome de escritorio no expone el espacio libre (getAvailableCapacity es solo de ChromeOS): se informa la capacidad.
  async function sampleDisk() {
    const units = (await safe(() => sys.storage.getInfo())) ?? [];
    const caps = new Set(units.filter((x) => x.type === 'fixed').map((x) => x.capacity)); // sin duplicados (p. ej. Google Drive)
    diskTotal = [...caps].reduce((a, c) => a + c, 0) / GB;
  }

  async function sampleBattery() {
    const b = await safe(() => nav.getBattery());
    battery = b ? { level: b.level, charging: b.charging } : null;
  }

  /** Un muestreo completo; si ya hay uno en curso devuelve el mismo. */
  function sample() {
    inflight ??= Promise.all([sampleCpu(), sampleRam(), sampleDisk(), sampleBattery()]).finally(() => {
      inflight = null;
    });
    return inflight;
  }

  function start() {
    if (timer) return;
    sample();
    timer = setInterval(sample, 1000);
  }

  function stop() {
    clearInterval(timer);
    timer = 0;
  }

  /** Float32Array con el layout de `Engine.feed`; la hora es la local en este instante. */
  function data() {
    const t = now();
    return Float32Array.of(t.getHours(), t.getMinutes(), t.getSeconds(), ...ram, 0, diskTotal, loads.length, ...loads);
  }

  function text() {
    const ua = nav?.userAgentData;
    const brand = ua?.brands?.find((b) => /Chrome|Chromium/.test(b.brand));
    const version = brand?.version ?? nav?.userAgent?.match(/Chrome\/([\d.]+)/)?.[1];
    const platform = ua?.platform || nav?.platform;
    const heap = perf?.memory;
    const c = nav?.connection;
    return [
      version && `Chrome: ${version}`,
      platform && `OS: ${platform}`,
      chromeApi && !sys && 'System: reload the extension',
      nav?.hardwareConcurrency && `Cores: ${nav.hardwareConcurrency}`,
      heap && `JS heap: ${Math.round(heap.usedJSHeapSize / MB)} / ${Math.round(heap.jsHeapSizeLimit / MB)} MB`,
      c && `Network: ${c.effectiveType} ${c.downlink} Mbps ${c.rtt} ms`,
      battery && `Battery: ${Math.round(battery.level * 100)} % (${battery.charging ? 'charging' : 'discharging'})`,
      nav?.language && `Language: ${nav.language}`,
    ]
      .filter(Boolean)
      .join('\n');
  }

  return { start, stop, sample, data, text };
}

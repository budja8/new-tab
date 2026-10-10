// Muestreo de datos del sistema para la animación `dashboard`. Sin DOM: se prueba con Node.
const GB = 1024 ** 3;
const MB = 1048576;
const POLL_MS = 1000;

export function createSysinfo({ chromeApi, nav = {}, perf = {}, now = () => new Date() } = {}) {
  nav = nav ?? {};
  perf = perf ?? {};
  let ram = [0, 0];
  let diskTotal = 0;
  let loads = [];
  let prevCpu = null;
  let battery = null;
  let timer = 0;

  async function sampleCpu() {
    try {
      const info = await chromeApi.system.cpu.getInfo();
      const procs = info.processors.map((p) => p.usage);
      loads = procs.map((u, i) => {
        const p = prevCpu?.[i];
        if (!p) return 0;
        const dTotal = u.total - p.total;
        if (!(dTotal > 0)) return 0;
        return Math.min(1, Math.max(0, (dTotal - (u.idle - p.idle)) / dTotal));
      });
      prevCpu = procs;
    } catch {
      loads = [];
    }
  }

  async function sampleMemory() {
    try {
      const m = await chromeApi.system.memory.getInfo();
      ram = [(m.capacity - m.availableCapacity) / GB, m.capacity / GB];
    } catch {
      ram = [0, 0];
    }
  }

  // En escritorio no hay espacio libre (getAvailableCapacity es solo de ChromeOS): solo capacidad.
  // Las unidades fixed de igual tamaño se cuentan una vez (p. ej. la unidad virtual de Google Drive).
  async function sampleStorage() {
    try {
      const units = await chromeApi.system.storage.getInfo();
      const sizes = new Set(units.filter((u) => u.type === 'fixed').map((u) => u.capacity));
      diskTotal = [...sizes].reduce((a, b) => a + b, 0) / GB;
    } catch {
      diskTotal = 0;
    }
  }

  async function sampleBattery() {
    try {
      const b = await nav.getBattery();
      battery = { level: b.level, charging: b.charging };
    } catch {
      battery = null;
    }
  }

  async function sample() {
    await Promise.all([sampleCpu(), sampleMemory(), sampleStorage(), sampleBattery()]);
  }

  function data() {
    const t = now();
    return Float32Array.of(t.getHours(), t.getMinutes(), t.getSeconds(), ram[0], ram[1], 0, diskTotal, loads.length, ...loads);
  }

  function chromeVersion() {
    const brand = nav.userAgentData?.brands?.find((b) => b.brand === 'Google Chrome');
    if (brand) return brand.version;
    return /Chrome\/([\d.]+)/.exec(nav.userAgent ?? '')?.[1];
  }

  function text() {
    const lines = [];
    if (chromeApi && !chromeApi.system) lines.push('System: reload the extension');
    const v = chromeVersion();
    if (v) lines.push(`Chrome: ${v}`);
    const os = nav.userAgentData?.platform || nav.platform;
    if (os) lines.push(`OS: ${os}`);
    if (nav.hardwareConcurrency) lines.push(`Cores: ${nav.hardwareConcurrency}`);
    if (perf.memory) {
      lines.push(`JS heap: ${Math.round(perf.memory.usedJSHeapSize / MB)} / ${Math.round(perf.memory.jsHeapSizeLimit / MB)} MB`);
    }
    const c = nav.connection;
    if (c) lines.push(`Network: ${c.effectiveType} ${c.downlink} Mbps ${c.rtt} ms`);
    if (battery) lines.push(`Battery: ${Math.round(battery.level * 100)} % (${battery.charging ? 'charging' : 'discharging'})`);
    if (nav.language) lines.push(`Language: ${nav.language}`);
    return lines.join('\n');
  }

  return {
    sample,
    data,
    text,
    start() {
      if (timer) return;
      sample();
      timer = setInterval(sample, POLL_MS);
    },
    stop() {
      clearInterval(timer);
      timer = 0;
    },
  };
}

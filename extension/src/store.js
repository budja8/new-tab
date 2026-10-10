import { applyBatch, cutoffKey, dateKey, emptyDay, pruneDay, summarize } from './keystats.js';

const PREFIX = 'day:';

/**
 * Persistencia de estadísticas sobre un `area` con la forma de `chrome.storage.local`.
 * Un registro por día (`day:YYYY-MM-DD`). Las escrituras se serializan en una cola
 * porque el service worker puede recibir varios lotes a la vez.
 */
export function createKpiStore(area, now = Date.now) {
  let chain = Promise.resolve();

  const keyFor = (ms) => PREFIX + dateKey(ms);

  async function removeOld(ms) {
    const all = await area.get(null);
    const cutoff = PREFIX + cutoffKey(ms);
    const stale = Object.keys(all).filter((k) => k.startsWith(PREFIX) && k < cutoff);
    if (stale.length) await area.remove(stale);
  }

  async function write(batch) {
    const ms = now();
    const key = keyFor(ms);
    const got = await area.get(key);
    const isNewDay = !Object.hasOwn(got, key);
    const day = pruneDay(applyBatch(isNewDay ? emptyDay() : got[key], batch, ms));
    await area.set({ [key]: day });
    if (isNewDay) await removeOld(ms);
  }

  return {
    todayKey: () => keyFor(now()),

    /** Encola un lote. Rechaza si falla, pero la cola sigue viva para los siguientes. */
    record(batch) {
      const run = chain.then(() => write(batch));
      chain = run.catch(() => {});
      return run;
    },

    async readToday() {
      const ms = now();
      const key = keyFor(ms);
      const got = await area.get(key);
      return summarize(got[key], ms);
    },
  };
}

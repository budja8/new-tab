// Lógica pura de estadísticas de teclas: sin DOM ni chrome.*, para probarla con Node.

export const TOP_LIMIT = 20;
export const KEEP_DAYS = 90;
export const WPM_WINDOW_MIN = 5;
export const OTHERS = 'otros';

const MIN_MS = 60_000;
const DAY_MS = 86_400_000;
// Claves que nunca se aceptan como nombre de tecla/atajo/sitio.
const RESERVED = new Set(['__proto__', 'constructor', 'prototype', OTHERS]);

export function dateKey(ms) {
  const d = new Date(ms);
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function cutoffKey(ms, keepDays = KEEP_DAYS) {
  return dateKey(ms - keepDays * DAY_MS);
}

export function emptyDay() {
  return { total: 0, keys: {}, shortcuts: {}, sites: {}, active_ms: 0, recent: {} };
}

function count(v) {
  return Number.isFinite(v) && v > 0 ? Math.floor(v) : 0;
}

function bump(map, key, n) {
  if (n === 0 || typeof key !== 'string' || key === '' || RESERVED.has(key)) return;
  map[key] = (Object.hasOwn(map, key) ? map[key] : 0) + n;
}

/** Suma un lote del content script al registro del día. Muta y devuelve `day`. */
export function applyBatch(day, batch, nowMs) {
  const total = count(batch?.total);
  if (total === 0) return day;

  day.total += total;
  for (const [k, v] of Object.entries(batch.keys ?? {})) bump(day.keys, k, count(v));
  for (const [k, v] of Object.entries(batch.shortcuts ?? {})) bump(day.shortcuts, k, count(v));
  bump(day.sites, batch.host, total);

  const minute = Math.floor(nowMs / MIN_MS);
  if (!Object.hasOwn(day.recent, minute)) day.active_ms += MIN_MS;
  day.recent[minute] = (Object.hasOwn(day.recent, minute) ? day.recent[minute] : 0) + total;
  for (const m of Object.keys(day.recent)) {
    if (Number(m) <= minute - WPM_WINDOW_MIN) delete day.recent[m];
  }
  return day;
}

/** Conserva las `limit` entradas más altas; el resto se suma en `otros`. */
export function pruneTop(map, limit = TOP_LIMIT) {
  const entries = Object.entries(map)
    .filter(([k]) => k !== OTHERS)
    .sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1));
  if (entries.length <= limit) return map;

  const kept = Object.fromEntries(entries.slice(0, limit));
  const rest = entries.slice(limit).reduce((sum, [, v]) => sum + v, 0);
  kept[OTHERS] = rest + (Object.hasOwn(map, OTHERS) ? map[OTHERS] : 0);
  return kept;
}

export function pruneDay(day) {
  day.keys = pruneTop(day.keys);
  day.shortcuts = pruneTop(day.shortcuts);
  day.sites = pruneTop(day.sites);
  return day;
}

/** Palabras por minuto: cada 5 teclas = 1 palabra, promedio por minuto activo de los últimos 5. */
export function wpm(recent, nowMs) {
  const minute = Math.floor(nowMs / MIN_MS);
  let keys = 0;
  let active = 0;
  for (let m = minute - WPM_WINDOW_MIN + 1; m <= minute; m++) {
    if (Object.hasOwn(recent, m)) {
      keys += recent[m];
      active += 1;
    }
  }
  return active === 0 ? 0 : Math.round(keys / 5 / active);
}

export function topEntries(map, n = 5) {
  return Object.entries(map)
    .filter(([k]) => k !== OTHERS)
    .sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))
    .slice(0, n);
}

export function summarize(day, nowMs) {
  const d = day ?? emptyDay();
  return {
    total: d.total,
    wpm: wpm(d.recent ?? {}, nowMs),
    topKeys: topEntries(d.keys),
    topShortcuts: topEntries(d.shortcuts),
    topSites: topEntries(d.sites),
  };
}

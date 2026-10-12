// Cuenta teclas en la página. Solo guarda NOMBRES de tecla y conteos, nunca el
// texto escrito ni su orden. Se carga después de keyname.js (ver manifest.json).
(function () {
  const FLUSH_MS = 3000;
  const host = (location.hostname || 'local').replace(/^www\./, '');

  let pending = null;
  let timer = null;

  function flush() {
    timer = null;
    if (!pending) return;
    const batch = pending;
    pending = null;
    try {
      chrome.runtime.sendMessage({ type: 'keys', batch }).catch(() => {});
    } catch (_) {
      // El contexto de la extensión se invalida al actualizarla: puede lanzar de forma
      // síncrona (este catch) o rechazar la promesa (el .catch de arriba). Se descarta el lote.
    }
  }

  function onKeyDown(e) {
    if (!e.isTrusted || e.repeat) return;

    // composedPath()[0] llega hasta el campo real aunque esté dentro de un shadow DOM.
    const target = (e.composedPath && e.composedPath()[0]) || e.target;
    if (target && target.tagName === 'INPUT' && target.type === 'password') return;

    const named = globalThis.NT.keyName(e);
    if (!named) return;

    pending = pending || { host, total: 0, keys: {}, shortcuts: {} };
    pending.total += 1;
    pending.keys[named.key] = (pending.keys[named.key] || 0) + 1;
    if (named.shortcut) {
      pending.shortcuts[named.shortcut] = (pending.shortcuts[named.shortcut] || 0) + 1;
    }
    if (!timer) timer = setTimeout(flush, FLUSH_MS);
  }

  addEventListener('keydown', onKeyDown, true);
  addEventListener('pagehide', flush);
})();

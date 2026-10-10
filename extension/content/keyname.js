// Script clásico: los content scripts de MV3 no pueden usar `import` estático.
// Expone `NT.keyName` en el ámbito global del script.
(function (root) {
  const MODIFIERS = new Set(['Control', 'Shift', 'Alt', 'Meta', 'AltGraph']);
  const IGNORED = new Set(['Dead', 'Unidentified', 'Process']);
  const NAMES = {
    ' ': 'space',
    Escape: 'esc',
    ArrowUp: 'up',
    ArrowDown: 'down',
    ArrowLeft: 'left',
    ArrowRight: 'right',
  };
  const MAX_NAME = 20;

  /**
   * Convierte un KeyboardEvent en `{ key, shortcut }`, o `null` si no debe contarse.
   * Solo devuelve el *nombre* de la tecla, nunca el texto escrito.
   */
  function keyName(e) {
    const k = e.key;
    if (typeof k !== 'string' || k === '' || MODIFIERS.has(k) || IGNORED.has(k)) return null;

    const key = NAMES[k] ?? k.toLowerCase();
    if (key.length > MAX_NAME) return null;

    // AltGr llega como ctrl+alt en Windows: escribe un carácter, no es un atajo.
    const altGraph = typeof e.getModifierState === 'function' && e.getModifierState('AltGraph');
    let shortcut = null;
    if ((e.ctrlKey || e.altKey || e.metaKey) && !altGraph) {
      const parts = [];
      if (e.ctrlKey) parts.push('ctrl');
      if (e.altKey) parts.push('alt');
      if (e.metaKey) parts.push('meta');
      if (e.shiftKey) parts.push('shift');
      parts.push(key);
      shortcut = parts.join('+');
    }
    return { key, shortcut };
  }

  root.NT = root.NT || {};
  root.NT.keyName = keyName;
})(globalThis);

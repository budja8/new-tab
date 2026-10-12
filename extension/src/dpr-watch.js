/**
 * Avisa cuando cambia `devicePixelRatio` (por ejemplo, al mover la ventana a otro monitor), algo que
 * no siempre dispara `resize`. `matchMedia` solo notifica el cambio respecto al valor con el que se
 * creó la consulta, así que se rearma con el valor nuevo. Devuelve una función para detenerlo.
 */
export function watchDpr(onChange, win = window) {
  let stopped = false;
  let mql = null;
  const handler = () => {
    if (stopped) return;
    arm(); // rearmar primero: si `onChange` lanza, la vigilancia sigue viva
    onChange();
  };
  function arm() {
    mql = win.matchMedia(`(resolution: ${win.devicePixelRatio}dppx)`);
    mql.addEventListener('change', handler, { once: true });
  }
  arm();
  return () => {
    stopped = true;
    mql.removeEventListener('change', handler);
  };
}

/**
 * Índices de las celdas que cambiaron entre `prev` y `cur` (equivale al double
 * buffering de cwork). Escribe en `out` y devuelve cuántas son; no asigna memoria.
 * Si `prev` es null o de otro tamaño, todas las celdas cuentan como cambiadas.
 */
export function diffCells(prev, cur, out, stride = 3) {
  const cells = Math.floor(cur.length / stride);
  let n = 0;
  if (!prev || prev.length !== cur.length) {
    for (let i = 0; i < cells; i++) out[n++] = i;
    return n;
  }
  for (let i = 0, j = 0; i < cells; i++, j += stride) {
    for (let k = 0; k < stride; k++) {
      if (prev[j + k] !== cur[j + k]) {
        out[n++] = i;
        break;
      }
    }
  }
  return n;
}

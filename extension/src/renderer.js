import { diffCells } from './diff.js';

const DEFAULT_FG = 0xffcccccc;
const MAX_SPRITES = 2048;

/** Tope por lado de la cuadrícula (los dos renderers lo comparten). */
export const MAX_DIM = 1024;

/**
 * Geometría de la cuadrícula en píxeles físicos: tamaño de celda redondeado y la cantidad de
 * columnas y filas que caben en la ventana (entre 1 y MAX_DIM por lado).
 */
export function gridFor(innerW, innerH, dpr, cellW, cellH) {
  const pw = Math.max(1, Math.round(cellW * dpr));
  const ph = Math.max(1, Math.round(cellH * dpr));
  const cols = Math.min(MAX_DIM, Math.max(1, Math.floor((innerW * dpr) / pw)));
  const rows = Math.min(MAX_DIM, Math.max(1, Math.floor((innerH * dpr) / ph)));
  return { pw, ph, cols, rows };
}

const css = (c) => `rgb(${(c >> 16) & 255},${(c >> 8) & 255},${c & 255})`;

/**
 * Pinta el buffer de celdas de WASM en un canvas. Cada (carácter, color) se dibuja
 * una sola vez en un sprite y después cada celda es un `drawImage`, que es mucho
 * más barato que `fillText`. Solo se repintan las celdas que cambiaron.
 */
export class Renderer {
  constructor(canvas, { stride = 3, cellW = 9, cellH = 16, fontSize = 14, font = 'Consolas, "Cascadia Mono", "Courier New", monospace' } = {}) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.stride = stride;
    this.cellW = cellW;
    this.cellH = cellH;
    this.fontSize = fontSize;
    this.font = font;
    this.sprites = new Map();
    this.prev = null;
    this.scratch = new Int32Array(0);
    this.cols = 0;
    this.rows = 0;
    this.dpr = 1;
    this.pw = cellW;
    this.ph = cellH;
  }

  /** Ajusta el canvas a la ventana y devuelve el tamaño de la cuadrícula. */
  fit() {
    this.dpr = window.devicePixelRatio || 1;
    const { pw, ph, cols, rows } = gridFor(window.innerWidth, window.innerHeight, this.dpr, this.cellW, this.cellH);
    Object.assign(this, { pw, ph, cols, rows });

    this.canvas.width = this.cols * this.pw;
    this.canvas.height = this.rows * this.ph;
    this.canvas.style.width = `${(this.cols * this.pw) / this.dpr}px`;
    this.canvas.style.height = `${(this.rows * this.ph) / this.dpr}px`;

    this.sprites.clear();
    this.prev = null;
    return { cols: this.cols, rows: this.rows };
  }

  /** Obliga a repintar todo en el siguiente `draw`. */
  invalidate() {
    this.prev = null;
  }

  sprite(ch, fg) {
    // 4 bits por canal: acota la cantidad de sprites distintos (fotos con miles de colores).
    const q = (fg || DEFAULT_FG) & 0xf0f0f0;
    const key = ch * 0x1000000 + q;
    let sprite = this.sprites.get(key);
    if (sprite) return sprite;

    if (this.sprites.size >= MAX_SPRITES) this.sprites.clear();
    sprite = new OffscreenCanvas(this.pw, this.ph);
    const g = sprite.getContext('2d');
    g.font = `${Math.round(this.fontSize * this.dpr)}px ${this.font}`;
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    g.fillStyle = css(q | (q >> 4));
    g.fillText(String.fromCodePoint(ch), this.pw / 2, this.ph / 2);
    this.sprites.set(key, sprite);
    return sprite;
  }

  /** `cells` es la vista sobre la memoria de WASM: `[ch, fg, bg]` por celda. */
  draw(cells, cols, rows) {
    if (cols !== this.cols || rows !== this.rows || cells.length !== cols * rows * this.stride) return;

    if (this.scratch.length < cols * rows) this.scratch = new Int32Array(cols * rows);
    const changed = diffCells(this.prev, cells, this.scratch, this.stride);
    const ctx = this.ctx;

    for (let k = 0; k < changed; k++) {
      const i = this.scratch[k];
      const j = i * this.stride;
      const x = (i % cols) * this.pw;
      const y = Math.floor(i / cols) * this.ph;
      const ch = cells[j];
      const fg = cells[j + 1];
      const bg = cells[j + 2];

      ctx.clearRect(x, y, this.pw, this.ph);
      if (bg !== 0) {
        ctx.fillStyle = css(bg);
        ctx.fillRect(x, y, this.pw, this.ph);
      }
      if (ch !== 32 && ch !== 0) ctx.drawImage(this.sprite(ch, fg), x, y);
    }

    if (!this.prev || this.prev.length !== cells.length) this.prev = new Uint32Array(cells.length);
    this.prev.set(cells);
  }
}

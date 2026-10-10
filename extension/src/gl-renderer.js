import {
  DEFAULT_FG,
  LUMINANCE_ROWS,
  SLOTS_PER_ROW,
  atlasSize,
  createSlotAllocator,
  glyphBlock,
  luminanceBucket,
} from './glyph-atlas.js';
import { gridFor } from './renderer.js';

const VS = `#version 300 es
void main(){ vec2 p=vec2(float((gl_VertexID<<1)&2),float(gl_VertexID&2)); gl_Position=vec4(p*2.0-1.0,0.0,1.0); }`;

// Cada texel de uCells es una celda [ranura | cubeta<<16, fg, bg]. El atlas guarda máscaras de cobertura
// (canal alfa) y el color se aplica aquí; un bg de 0 deja la celda transparente.
const FS = `#version 300 es
precision highp float; precision highp int; precision highp usampler2D;
uniform usampler2D uCells; uniform sampler2D uAtlas; uniform ivec2 uCell; uniform int uH;
out vec4 o;
vec3 rgb(uint c){ return vec3(float((c>>16)&255u),float((c>>8)&255u),float(c&255u))/255.0; }
void main(){
  ivec2 p=ivec2(gl_FragCoord.xy); p.y=uH-1-p.y;
  ivec2 c=p/uCell; ivec2 l=p-c*uCell;
  uvec3 d=texelFetch(uCells,c,0).rgb;
  int g=int(d.r&65535u); int lb=int(d.r>>16);
  uint fg=d.g==0u?${DEFAULT_FG}u:(d.g&0xffffffu); fg&=0xf0f0f0u; fg|=fg>>4;
  float a=g>0?texelFetch(uAtlas,ivec2((g%${SLOTS_PER_ROW})*uCell.x+l.x,((g/${SLOTS_PER_ROW})*${LUMINANCE_ROWS}+lb)*uCell.y+l.y),0).a:0.0;
  vec3 f=rgb(fg);
  o=d.b!=0u?vec4(mix(rgb(d.b),f,a),1.0):vec4(f*a,a);
}`;

/**
 * Pinta el buffer de celdas de WASM con WebGL2: una sola llamada de dibujo por frame. Misma interfaz
 * que `Renderer` (2D), que queda como respaldo cuando WebGL2 no está disponible.
 */
export class GlRenderer {
  constructor(canvas, { stride = 3, cellW = 9, cellH = 16, fontSize = 14, font = 'Consolas, "Cascadia Mono", "Courier New", monospace' } = {}) {
    if (stride !== 3) throw new Error('GlRenderer espera stride 3');
    const gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: false, depth: false, stencil: false, preserveDrawingBuffer: false });
    if (!gl) throw new Error('WebGL2 no disponible');
    Object.assign(this, {
      canvas, gl, stride, cellW, cellH, fontSize, font,
      cols: 0, rows: 0, dpr: 1, pw: cellW, ph: cellH,
      prev: null, tx: new Uint32Array(0), lost: false,
      slots: createSlotAllocator(),
      cellTex: null, atlasTex: null,
    });
    this.onGlyph = (cp, slot) => this.addGlyph(cp, slot);
    canvas.addEventListener('webglcontextlost', (e) => {
      e.preventDefault(); // permite que el navegador restaure el contexto
      this.lost = true;
    });
    canvas.addEventListener('webglcontextrestored', () => {
      this.lost = false;
      this.init();
      this.layout();
    });
    this.init();
  }

  /** Compila el programa y deja la geometría lista; se repite al restaurar el contexto. */
  init() {
    const gl = this.gl;
    const shader = (type, src) => {
      const s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
      return s;
    };
    const prog = gl.createProgram();
    gl.attachShader(prog, shader(gl.VERTEX_SHADER, VS));
    gl.attachShader(prog, shader(gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
    gl.useProgram(prog);
    this.uCell = gl.getUniformLocation(prog, 'uCell');
    this.uH = gl.getUniformLocation(prog, 'uH');
    gl.uniform1i(gl.getUniformLocation(prog, 'uCells'), 0);
    gl.uniform1i(gl.getUniformLocation(prog, 'uAtlas'), 1);
    gl.bindVertexArray(gl.createVertexArray());
    this.cellTex = null;
    this.atlasTex = null;
  }

  /** Ajusta el canvas a la ventana y devuelve el tamaño de la cuadrícula. */
  fit() {
    this.dpr = window.devicePixelRatio || 1;
    const { pw, ph, cols, rows } = gridFor(window.innerWidth, window.innerHeight, this.dpr, this.cellW, this.cellH);
    Object.assign(this, { pw, ph, cols, rows });

    this.canvas.width = cols * pw;
    this.canvas.height = rows * ph;
    this.canvas.style.width = `${(cols * pw) / this.dpr}px`;
    this.canvas.style.height = `${(rows * ph) / this.dpr}px`;

    this.layout();
    return { cols, rows };
  }

  /** (Re)crea las texturas con el atlas vacío; también se usa al restaurar el contexto. */
  layout() {
    const gl = this.gl;
    if (!this.cols || gl.isContextLost()) return;
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.uniform2i(this.uCell, this.pw, this.ph);
    gl.uniform1i(this.uH, this.canvas.height);

    if (this.cellTex) gl.deleteTexture(this.cellTex);
    this.cellTex = this.texture(0);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGB32UI, this.cols, this.rows);

    if (this.atlasTex) gl.deleteTexture(this.atlasTex);
    this.atlasTex = this.texture(1);
    const { width, height } = atlasSize(this.pw, this.ph);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, width, height);
    gl.activeTexture(gl.TEXTURE0);

    this.slots = createSlotAllocator(); // el atlas nuevo está vacío
    this.prev = null;
  }

  texture(unit) {
    const gl = this.gl;
    const t = gl.createTexture();
    gl.activeTexture(gl.TEXTURE0 + unit);
    gl.bindTexture(gl.TEXTURE_2D, t);
    for (const p of [gl.TEXTURE_MIN_FILTER, gl.TEXTURE_MAG_FILTER]) gl.texParameteri(gl.TEXTURE_2D, p, gl.NEAREST);
    for (const p of [gl.TEXTURE_WRAP_S, gl.TEXTURE_WRAP_T]) gl.texParameteri(gl.TEXTURE_2D, p, gl.CLAMP_TO_EDGE);
    return t;
  }

  /** Rasteriza un glifo nuevo en sus 8 filas de luminancia, igual que los sprites del renderer 2D. */
  addGlyph(cp, slot) {
    const gl = this.gl;
    const { x, y, w, h } = glyphBlock(slot, this.pw, this.ph);
    const canvas = new OffscreenCanvas(w, h);
    const g = canvas.getContext('2d');
    g.font = `${Math.round(this.fontSize * this.dpr)}px ${this.font}`;
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    const ch = String.fromCodePoint(cp);
    for (let lb = 0; lb < LUMINANCE_ROWS; lb++) {
      const v = lb * 32 + 16;
      g.fillStyle = `rgb(${v},${v},${v})`;
      g.save();
      g.beginPath();
      g.rect(0, lb * this.ph, w, this.ph);
      g.clip();
      g.fillText(ch, w / 2, lb * this.ph + this.ph / 2);
      g.restore();
    }
    gl.activeTexture(gl.TEXTURE1);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, x, y, gl.RGBA, gl.UNSIGNED_BYTE, canvas);
    gl.activeTexture(gl.TEXTURE0);
  }

  /** Obliga a repintar todo en el siguiente `draw`. */
  invalidate() {
    this.prev = null;
  }

  /** `cells` es la vista sobre la memoria de WASM: `[ch, fg, bg]` por celda. */
  draw(cells, cols, rows) {
    if (this.lost || cols !== this.cols || rows !== this.rows || cells.length !== cols * rows * 3) return;
    if (this.tx.length !== cells.length) {
      this.tx = new Uint32Array(cells.length);
      this.prev = null;
    }
    const { prev, tx } = this;

    // Traduce solo las celdas que cambiaron: ranura del glifo + cubeta de luminancia en un u32.
    let dirty = !prev;
    for (let j = 0; j < cells.length; j += 3) {
      const cp = cells[j];
      const fg = cells[j + 1];
      const bg = cells[j + 2];
      if (prev && prev[j] === cp && prev[j + 1] === fg && prev[j + 2] === bg) continue;
      tx[j] = this.slots.slotFor(cp, this.onGlyph) | (luminanceBucket(fg) << 16);
      tx[j + 1] = fg;
      tx[j + 2] = bg;
      dirty = true;
    }
    if (!dirty) return; // el canvas conserva el último frame

    const gl = this.gl;
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, cols, rows, gl.RGB_INTEGER, gl.UNSIGNED_INT, tx);
    gl.drawArrays(gl.TRIANGLES, 0, 3);

    if (!this.prev) this.prev = new Uint32Array(cells.length);
    this.prev.set(cells);
  }
}

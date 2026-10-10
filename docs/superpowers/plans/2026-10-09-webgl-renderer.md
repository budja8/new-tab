# Renderer WebGL2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reemplazar el dibujo celda por celda del canvas 2D por un renderer WebGL2 de una sola llamada de dibujo por frame, para bajar el consumo de CPU y GPU de la pestaña, conservando el renderer 2D actual como respaldo.

**Architecture:** Un `GlRenderer` con la misma interfaz que `Renderer` (`fit()`, `invalidate()`, `draw(cells, cols, rows)`) sube el buffer de celdas de WASM como textura de datos y un fragment shader resuelve cada píxel contra un atlas de glifos dinámico (8 filas por luminancia, que reproducen el pre-blend de texto de Skia). Una fábrica `createRenderer` elige WebGL2 o el renderer 2D según lo que el navegador pueda hacer. La lógica pura (ranuras del atlas, cubeta de luminancia, decisión de backend, vigilancia del DPR) vive en módulos sin DOM, probados con Node.

**Tech Stack:** JavaScript ES modules, WebGL2, Rust/`wasm-pack` (un cambio pequeño en el HUD de starfield), Node 24 (`node --test`).

**Spec:** `docs/superpowers/specs/2026-10-08-new-tab-ascii-design.md` (§5 se actualiza en la tarea 6). **Evidencia del diseño:** el spike `...\scratchpad\sdd\spike-renderer-report.md`; el prototipo ganador está en `...\scratchpad\spike\b\src\renderer.js` y la ruta completa de la carpeta temporal de la sesión es `C:\Users\DANIEL~1\AppData\Local\Temp\claude\C--Users-Daniel-Rodriguez-Documents-Tests-owns-new-tab\551d3df9-9fd5-496e-b351-05fc26dbcc22\scratchpad\`.

**Convención de rutas:** todos los comandos se ejecutan desde la raíz del proyecto, `C:\Users\Daniel Rodriguez\Documents\Tests\owns\new-tab`.

**Sin git:** el proyecto no es un repositorio, por decisión del usuario. Este plan no tiene pasos de commit; cada tarea termina con una verificación.

## Decisión abierta: presupuesto de tamaño

Hoy JS+CSS+HTML suma **39,824 bytes** (38.9 KB). El tope del spec es 50 KB = 51,200 bytes, así que el margen es de **11,376 bytes**. El prototipo del spike pesa 6.8 KB solo; la versión de producción añade atlas dinámico, el respaldo, el manejo de contexto perdido y la vigilancia del DPR. La estimación de este plan es **+11 a +13 KB**, es decir, justo en el límite o ligeramente por encima. La tarea 6 tiene una compuerta de tamaño: si `npm run check:size` falla, el ejecutor **no** mutila el código ni sube el tope por su cuenta; lo reporta y la decisión (por ejemplo, subir el tope a 60 KB) la toma el usuario.

## Global Constraints

- Manifest V3; permisos exactamente `["storage"]`; sin red; sin `host_permissions`. No se toca el manifest.
- `.wasm` ≤ 150 KB. JS + CSS + HTML ≤ 50 KB (`npm run check:size`), sujeto a la decisión abierta de arriba.
- Una sola llamada de dibujo por frame; un solo bucle de animación a la vez; pausa total con `visibilitychange`; sin bucle con imagen estática o con `prefers-reduced-motion`; sin timers sueltos.
- FPS objetivo: `fire` 30, `rain` 30, **`starfield` 30** (cambia de 60 a 30 en este plan).
- La interfaz del renderer no cambia: `fit() -> { cols, rows }`, `invalidate()`, `draw(cells: Uint32Array, cols, rows)`; el buffer de celdas es `[codepoint, fg, bg]` por celda (`u32`, stride 3; colores `0xFFRRGGBB`, `0` = sin color). `MAX_DIM = 1024` y `gridFor()` se conservan y los dos renderers los comparten.
- El renderer 2D actual (`extension/src/renderer.js`) se conserva como respaldo sin cambios de comportamiento.
- Sin dependencias npm nuevas. Comentarios y mensajes en español.

## Review Focus

Entradas que el diseño implica pero que ninguna prueba obvia cubriría; cada una tiene su prueba o verificación en la tarea indicada.

1. **WebGL2 no disponible, GPU por software (SwiftShader) o textura de atlas que no cabe**: se usa el renderer 2D y la pestaña nunca queda en blanco. Tareas 4 y 6.
2. **Glifos inesperados** (emoji, CJK, codepoints fuera del BMP) **y atlas lleno**: no revienta; el glifo se dibuja en blanco o se rasteriza como cualquier otro. Tarea 2.
3. **Pérdida y restauración del contexto WebGL, y cambio de DPR** (otro monitor) sin evento `resize`: la pestaña no se queda en blanco ni con la escala vieja. Tareas 3, 4 y 6.
4. **Frame sin cambios** no vuelve a subir datos; el primer frame tras `fit()` o `invalidate()` repinta todo; las celdas sin cambios conservan su ranura entre frames. Tareas 3 y 6.
5. **Tamaño del paquete** ≤ 50 KB tras el cambio. Tarea 6.

## Estructura de archivos

```
core/src/animation/starfield.rs      # modifica: textos del HUD en ASCII
extension/pkg/                       # regenerado por wasm-pack
extension/src/glyph-atlas.js         # nuevo: ranuras, geometría del atlas, cubeta de luminancia (puro)
extension/src/gl-renderer.js         # nuevo: GlRenderer (WebGL2)
extension/src/create-renderer.js     # nuevo: elige backend, sondeo de WebGL2, respaldo
extension/src/dpr-watch.js           # nuevo: detecta cambios de DPR
extension/newtab.js                  # modifica: usa la fábrica, starfield a 30 FPS, vigila el DPR
tests/glyph-atlas.test.js            # nuevo
tests/create-renderer.test.js        # nuevo
tests/dpr-watch.test.js              # nuevo
docs/measurements.md                 # actualiza con las nuevas mediciones
docs/superpowers/specs/...-design.md # §5 actualizado
```

---

### Task 1: Textos del HUD de starfield en ASCII puro

**Files:**
- Modify: `core/src/animation/starfield.rs` (constantes nuevas, dos líneas del HUD, una prueba)
- Regenerate: `extension/pkg/` (con `npm run build:wasm`)

**Interfaces:**
- Produces: constantes privadas `WARNING_PROMPT` y `HYPERDRIVE_PROMPT` (`&str`, ASCII). El resto de la API de WASM no cambia.

Por qué: los emojis `⚠️` y `⚡` se dibujan a color y no encajan en una celda monoespaciada. Con ASCII el atlas del renderer WebGL no necesita una ruta de emoji, y de paso desaparece el glifo recortado del aviso.

- [ ] **Step 1: Escribir la prueba (falla)**

En el módulo `tests` de `core/src/animation/starfield.rs`, antes de la llave de cierre del módulo, agregar:
```rust
    #[test]
    fn hud_prompts_are_plain_ascii() {
        // Los emojis se dibujan a color y no caben en una celda monoespaciada.
        assert!(WARNING_PROMPT.is_ascii());
        assert!(HYPERDRIVE_PROMPT.is_ascii());
    }
```

- [ ] **Step 2: Ejecutar para verificar que falla**

Run: `cargo test --manifest-path core/Cargo.toml starfield`
Expected: error de compilación `cannot find value WARNING_PROMPT in this scope`.

- [ ] **Step 3: Implementar**

Debajo del arreglo `STAR_COLORS` (después de su `];`), agregar:
```rust
// Texto del HUD en ASCII puro: los emojis se dibujan a color y no encajan en una celda monoespaciada.
const WARNING_PROMPT: &str = "!! WARNING: HYPERDRIVE INITIATING !!";
const HYPERDRIVE_PROMPT: &str = ">> HYPERDRIVE ACTIVE <<";
```
En `draw`, reemplazar `let prompt = "⚠️  WARNING: HYPERDRIVE INITIATING  ⚠️";` por `let prompt = WARNING_PROMPT;` y `let prompt = "⚡ HYPERDRIVE ACTIVE ⚡";` por `let prompt = HYPERDRIVE_PROMPT;`. No cambiar nada más (el cálculo de `len`, `px` y la caja quedan igual).

- [ ] **Step 4: Ejecutar para verificar que pasa**

Run: `cargo test --manifest-path core/Cargo.toml`
Expected: 56 pruebas pasan (las 55 anteriores más la nueva), sin warnings. La prueba `warning_state_shows_the_hud_box` sigue valiendo: el texto nuevo mide 36 caracteres, `px = (80 - 36) / 2 = 22` y la caja sigue en la fila 11.

- [ ] **Step 5: Recompilar WASM y verificar**

Run:
```bash
npm run build:wasm
npm run test:js
npm run check:size
```
Expected: el build termina; `test:js` pasa (67 pruebas, incluida la de humo de WASM); `check:size` sigue en verde (el `.wasm` casi no cambia de tamaño).

---

### Task 2: Lógica pura del atlas de glifos (`glyph-atlas.js`)

**Files:**
- Create: `extension/src/glyph-atlas.js`
- Test: `tests/glyph-atlas.test.js`

**Interfaces:**
- Produces: `DEFAULT_FG = 0xcccccc`, `SLOTS_PER_ROW = 64`, `CAPACITY = 512`, `LUMINANCE_ROWS = 8`; `createSlotAllocator(capacity = CAPACITY) -> { slotFor(cp: number, onNew?: (cp, slot) => void): number, size: number }` (la ranura `0` es la celda en blanco); `glyphBlock(slot, pw, ph) -> { x, y, w, h }`; `atlasSize(pw, ph, capacity = CAPACITY) -> { width, height }`; `fitsTexture(maxSize, pw, ph, capacity = CAPACITY) -> boolean`; `luminanceBucket(fg: number) -> 0..7`.

- [ ] **Step 1: Escribir las pruebas (fallan)**

`tests/glyph-atlas.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  CAPACITY,
  SLOTS_PER_ROW,
  atlasSize,
  createSlotAllocator,
  fitsTexture,
  glyphBlock,
  luminanceBucket,
} from '../extension/src/glyph-atlas.js';

test('el espacio y el codepoint 0 son celdas en blanco: ranura 0, sin rasterizar', () => {
  const slots = createSlotAllocator();
  let created = 0;
  assert.equal(slots.slotFor(32, () => created++), 0);
  assert.equal(slots.slotFor(0, () => created++), 0);
  assert.equal(created, 0);
  assert.equal(slots.size, 0);
});

test('un glifo nuevo recibe la ranura 1 y avisa una sola vez', () => {
  const slots = createSlotAllocator();
  const seen = [];
  assert.equal(slots.slotFor(65, (cp, slot) => seen.push([cp, slot])), 1);
  assert.equal(slots.slotFor(65, (cp, slot) => seen.push([cp, slot])), 1);
  assert.deepEqual(seen, [[65, 1]]);
  assert.equal(slots.size, 1);
});

test('glifos distintos reciben ranuras consecutivas', () => {
  const slots = createSlotAllocator();
  assert.deepEqual([65, 66, 67].map((cp) => slots.slotFor(cp)), [1, 2, 3]);
});

test('emoji, CJK y glifos de caja se asignan como cualquier otro', () => {
  const slots = createSlotAllocator();
  for (const cp of [0x1f600, 0x4e2d, 0x2588, 0x10ffff]) {
    assert.ok(slots.slotFor(cp) > 0, cp.toString(16));
  }
  assert.equal(slots.size, 4);
});

test('con el atlas lleno los glifos nuevos caen en blanco y los conocidos se conservan', () => {
  const slots = createSlotAllocator(4); // ranuras 1, 2 y 3
  const seen = [];
  assert.equal(slots.slotFor(65, (c) => seen.push(c)), 1);
  assert.equal(slots.slotFor(66, (c) => seen.push(c)), 2);
  assert.equal(slots.slotFor(67, (c) => seen.push(c)), 3);
  assert.equal(slots.slotFor(68, (c) => seen.push(c)), 0);
  assert.equal(slots.slotFor(66), 2);
  assert.deepEqual(seen, [65, 66, 67]);
  assert.equal(slots.size, 3);
});

test('glyphBlock ubica cada ranura en una rejilla de 64 columnas con 8 filas de luminancia', () => {
  assert.equal(SLOTS_PER_ROW, 64);
  assert.deepEqual(glyphBlock(1, 9, 16), { x: 9, y: 0, w: 9, h: 128 });
  assert.deepEqual(glyphBlock(64, 9, 16), { x: 0, y: 128, w: 9, h: 128 });
  assert.deepEqual(glyphBlock(65, 9, 16), { x: 9, y: 128, w: 9, h: 128 });
});

test('atlasSize crece con el tamaño de celda (dpr)', () => {
  assert.equal(CAPACITY, 512);
  assert.deepEqual(atlasSize(9, 16), { width: 576, height: 1024 });
  assert.deepEqual(atlasSize(27, 48), { width: 1728, height: 3072 });
});

test('fitsTexture compara el atlas con el tamaño máximo de textura', () => {
  assert.equal(fitsTexture(4096, 9, 16), true);
  assert.equal(fitsTexture(1024, 9, 16), true);
  assert.equal(fitsTexture(1000, 9, 16), false);
  assert.equal(fitsTexture(4096, 27, 48), true);
  assert.equal(fitsTexture(2048, 27, 48), false);
});

test('luminanceBucket reproduce la fórmula del shader del spike (valores derivados a mano)', () => {
  // Si una falla, revisa primero la transcripción de la fórmula, no el valor esperado.
  assert.equal(luminanceBucket(0), 6); // sin color → gris por defecto 0xcccccc
  assert.equal(luminanceBucket(0xffffffff), 7); // blanco
  assert.equal(luminanceBucket(0xff000000), 0); // negro opaco (distinto de "sin color")
  assert.equal(luminanceBucket(0xffff6a07), 4); // naranja del fuego
  assert.equal(luminanceBucket(0xffe72f2f), 2); // rojo del fuego
  assert.equal(luminanceBucket(0xff6f1717), 0); // rojo oscuro del fuego
});

test('luminanceBucket es monótono en los grises y alcanza las 8 cubetas', () => {
  const seen = new Set();
  let last = -1;
  for (let g = 0; g < 256; g++) {
    const b = luminanceBucket((0xff000000 | (g << 16) | (g << 8) | g) >>> 0);
    assert.ok(b >= last, `g=${g}`);
    assert.ok(b >= 0 && b <= 7);
    seen.add(b);
    last = b;
  }
  assert.equal(seen.size, 8);
});
```

- [ ] **Step 2: Ejecutar para verificar que falla**

Run: `node --test tests/glyph-atlas.test.js`
Expected: FAIL con `Cannot find module '../extension/src/glyph-atlas.js'`.

- [ ] **Step 3: Implementar**

`extension/src/glyph-atlas.js`:
```js
// Lógica pura del atlas de glifos del renderer WebGL: sin DOM ni WebGL, para probarla con Node.
//
// El atlas es una textura con una rejilla de `SLOTS_PER_ROW` columnas de ranuras. Cada ranura
// guarda un glifo en 8 filas contiguas, una por cubeta de luminancia: Skia elige la máscara de
// cobertura del texto según la luminancia (de 3 bits) del color de relleno, y el atlas la reproduce
// para que el resultado coincida con el renderer 2D.

export const DEFAULT_FG = 0xcccccc; // color por defecto de una celda con carácter pero sin color
export const SLOTS_PER_ROW = 64;
export const CAPACITY = 512;
export const LUMINANCE_ROWS = 8;

/**
 * Asigna una ranura a cada codepoint la primera vez que aparece. La ranura 0 es la celda en
 * blanco (espacio, codepoint 0 o atlas lleno). No asigna memoria por llamada: `slotFor` se invoca
 * por cada celda de cada frame.
 */
export function createSlotAllocator(capacity = CAPACITY) {
  const slots = new Map();
  let next = 1;
  return {
    /** Ranura del codepoint; `onNew(cp, slot)` se llama una vez cuando hay que rasterizarlo. */
    slotFor(cp, onNew) {
      if (cp === 0 || cp === 32) return 0;
      const known = slots.get(cp);
      if (known !== undefined) return known;
      if (next >= capacity) return 0;
      const slot = next++;
      slots.set(cp, slot);
      if (onNew) onNew(cp, slot);
      return slot;
    },
    get size() {
      return slots.size;
    },
  };
}

/** Rectángulo (en píxeles de la textura) que ocupa la ranura con sus 8 filas de luminancia. */
export function glyphBlock(slot, pw, ph) {
  return {
    x: (slot % SLOTS_PER_ROW) * pw,
    y: Math.floor(slot / SLOTS_PER_ROW) * LUMINANCE_ROWS * ph,
    w: pw,
    h: LUMINANCE_ROWS * ph,
  };
}

export function atlasSize(pw, ph, capacity = CAPACITY) {
  return {
    width: SLOTS_PER_ROW * pw,
    height: Math.ceil(capacity / SLOTS_PER_ROW) * LUMINANCE_ROWS * ph,
  };
}

export function fitsTexture(maxSize, pw, ph, capacity = CAPACITY) {
  const { width, height } = atlasSize(pw, ph, capacity);
  return width <= maxSize && height <= maxSize;
}

function expand3(v) {
  const q = v >> 5; // 3 bits
  return (q << 5) | (q << 2) | (q >> 1);
}

/**
 * Cubeta de luminancia (0..7) del color de relleno, igual que el shader del spike: el color se
 * cuantiza a 4 bits por canal (como el renderer 2D), después a 3 bits, y se mide su luminancia.
 */
export function luminanceBucket(fg) {
  const c = (fg === 0 ? DEFAULT_FG : fg & 0xffffff) & 0xf0f0f0;
  const w = c | (c >> 4);
  const lum = 0.2126 * expand3((w >> 16) & 255) + 0.7152 * expand3((w >> 8) & 255) + 0.0722 * expand3(w & 255);
  return Math.floor(lum + 0.5) >> 5;
}
```

- [ ] **Step 4: Ejecutar para verificar que pasa**

Run: `node --test tests/glyph-atlas.test.js`
Expected: 10 pruebas pasan. Verificación aritmética de los valores fijados: `(255,106,7)` cuantiza a `(255,102,0)`, luego a 3 bits `(7,3,0)` → `(255,109,0)`; luminancia `54.21 + 77.96 = 132.17`, `+0.5` y `floor` = 132, `>> 5` = 4.

---

### Task 3: `GlRenderer` (WebGL2)

**Files:**
- Create: `extension/src/gl-renderer.js`

**Interfaces:**
- Consumes: `CAPACITY`, `DEFAULT_FG`, `LUMINANCE_ROWS`, `SLOTS_PER_ROW`, `atlasSize`, `createSlotAllocator`, `glyphBlock`, `luminanceBucket` (tarea 2); `gridFor` (de `renderer.js`).
- Produces: `class GlRenderer { constructor(canvas, { stride = 3, cellW = 9, cellH = 16, fontSize = 14, font }); fit() -> { cols, rows }; invalidate(); draw(cells, cols, rows) }`. El constructor lanza si WebGL2 no está disponible, si `stride !== 3` o si el shader no compila.

Este archivo depende de WebGL y del DOM, así que no tiene prueba automática (lo declara el plan; se verifica contra el renderer 2D píxel a píxel en la tarea 6). Se verifica estáticamente aquí.

- [ ] **Step 1: Escribir `extension/src/gl-renderer.js`**

```js
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
```

- [ ] **Step 2: Verificación estática**

Run:
```bash
node --check extension/src/gl-renderer.js
node -e "import('./extension/src/gl-renderer.js').then(m => console.log(Object.keys(m)))"
```
Expected: sin errores de sintaxis; el segundo imprime `[ 'GlRenderer' ]` (el módulo se puede importar bajo Node porque no toca el DOM al cargarse).
Revisar además por lectura que: `draw` no asigna memoria por frame salvo al cambiar de tamaño; `slotFor` recibe `this.onGlyph` ya enlazado (no una función nueva por celda); `layout()` reinicia `slots` y `prev` junto con el atlas; y `fit()` conserva el contrato de retorno `{ cols, rows }`.

---

### Task 4: Fábrica `createRenderer` y vigilancia del DPR

**Files:**
- Create: `extension/src/create-renderer.js`, `extension/src/dpr-watch.js`
- Test: `tests/create-renderer.test.js`, `tests/dpr-watch.test.js`

**Interfaces:**
- Consumes: `fitsTexture` (tarea 2), `GlRenderer` (tarea 3), `Renderer` y `gridFor` (de `renderer.js`).
- Produces: `isSoftwareRenderer(name) -> boolean`; `chooseBackend({ hasWebGL2, rendererName, maxTextureSize, pw, ph }) -> { backend: 'gl' | '2d', reason: string }`; `probeWebGL2({ cellW = 9, cellH = 16 }) -> info`; `createRenderer(canvas, opts, deps) -> { renderer, backend, reason, canvas }`; `watchDpr(onChange, win = window) -> stop()`.

- [ ] **Step 1: Escribir las pruebas (fallan)**

`tests/create-renderer.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { chooseBackend, createRenderer, isSoftwareRenderer } from '../extension/src/create-renderer.js';

const okInfo = { hasWebGL2: true, rendererName: 'ANGLE (Intel, Intel(R) UHD Graphics Direct3D11 vs_5_0 ps_5_0)', maxTextureSize: 16384, pw: 9, ph: 16 };

test('detecta los renderizadores por software', () => {
  assert.equal(isSoftwareRenderer('ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)), SwiftShader driver)'), true);
  assert.equal(isSoftwareRenderer('llvmpipe (LLVM 15.0.7, 256 bits)'), true);
  assert.equal(isSoftwareRenderer('Microsoft Basic Render Driver'), true);
  assert.equal(isSoftwareRenderer(okInfo.rendererName), false);
  assert.equal(isSoftwareRenderer(undefined), false);
  assert.equal(isSoftwareRenderer(''), false);
});

test('sin WebGL2 se usa el renderer 2D', () => {
  const d = chooseBackend({ hasWebGL2: false, pw: 9, ph: 16 });
  assert.equal(d.backend, '2d');
  assert.match(d.reason, /WebGL2/);
});

test('una GPU por software se descarta: no hay ganancia con WebGL', () => {
  const d = chooseBackend({ ...okInfo, rendererName: 'Google SwiftShader' });
  assert.equal(d.backend, '2d');
  assert.match(d.reason, /software/i);
});

test('si el atlas no cabe en una textura se usa el renderer 2D', () => {
  const d = chooseBackend({ ...okInfo, maxTextureSize: 512 });
  assert.equal(d.backend, '2d');
  assert.match(d.reason, /atlas/);
});

test('con WebGL2 por hardware se elige gl y se informa el renderizador', () => {
  const d = chooseBackend(okInfo);
  assert.equal(d.backend, 'gl');
  assert.equal(d.reason, okInfo.rendererName);
});

test('createRenderer construye el renderer WebGL cuando el sondeo lo permite', () => {
  const canvas = { id: 'bg' };
  const made = [];
  const out = createRenderer(canvas, { stride: 3 }, {
    probe: () => okInfo,
    makeGl: (c, o) => { made.push(['gl', c, o]); return { kind: 'gl' }; },
    make2d: () => assert.fail('no debe construir el 2D'),
    replaceCanvas: () => assert.fail('no debe reemplazar el canvas'),
    warn: () => {},
  });
  assert.equal(out.backend, 'gl');
  assert.deepEqual(out.renderer, { kind: 'gl' });
  assert.equal(out.canvas, canvas);
  assert.deepEqual(made, [['gl', canvas, { stride: 3 }]]);
});

test('createRenderer no toca WebGL cuando el sondeo lo descarta', () => {
  const canvas = { id: 'bg' };
  const out = createRenderer(canvas, {}, {
    probe: () => ({ hasWebGL2: false, pw: 9, ph: 16 }),
    makeGl: () => assert.fail('no debe construir WebGL'),
    make2d: (c) => ({ kind: '2d', c }),
    replaceCanvas: () => assert.fail('no debe reemplazar el canvas'),
    warn: () => {},
  });
  assert.equal(out.backend, '2d');
  assert.equal(out.canvas, canvas);
  assert.match(out.reason, /WebGL2/);
});

test('si el constructor de WebGL falla se reemplaza el canvas y se usa el 2D', () => {
  const original = { id: 'viejo' };
  const fresh = { id: 'nuevo' };
  const warnings = [];
  const out = createRenderer(original, {}, {
    probe: () => okInfo,
    makeGl: () => { throw new Error('shader roto'); },
    make2d: (c) => ({ kind: '2d', c }),
    replaceCanvas: (c) => { assert.equal(c, original); return fresh; },
    warn: (...args) => warnings.push(args.join(' ')),
  });
  assert.equal(out.backend, '2d');
  assert.equal(out.canvas, fresh); // el contexto WebGL fallido bloquea el canvas original
  assert.deepEqual(out.renderer, { kind: '2d', c: fresh });
  assert.match(out.reason, /shader roto/);
  assert.equal(warnings.length, 1);
});
```

`tests/dpr-watch.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { watchDpr } from '../extension/src/dpr-watch.js';

/** `window` mínimo: matchMedia registra consultas y `fire()` imita el evento `change` con `once`. */
function fakeWin(dpr) {
  const win = {
    devicePixelRatio: dpr,
    queries: [],
    matchMedia(q) {
      const m = {
        q,
        handlers: [],
        addEventListener(_type, h) { this.handlers.push(h); },
        removeEventListener(_type, h) { this.handlers = this.handlers.filter((x) => x !== h); },
        fire() { const hs = this.handlers; this.handlers = []; hs.forEach((h) => h()); },
      };
      win.queries.push(m);
      return m;
    },
  };
  return win;
}

test('arma la consulta con el DPR actual y avisa al cambiar, rearmando con el nuevo', () => {
  const win = fakeWin(1.5);
  let calls = 0;
  watchDpr(() => calls++, win);
  assert.equal(win.queries.length, 1);
  assert.equal(win.queries[0].q, '(resolution: 1.5dppx)');

  win.devicePixelRatio = 2;
  win.queries[0].fire();
  assert.equal(calls, 1);
  assert.equal(win.queries.length, 2);
  assert.equal(win.queries[1].q, '(resolution: 2dppx)');
});

test('varios cambios seguidos avisan cada vez', () => {
  const win = fakeWin(1);
  let calls = 0;
  watchDpr(() => calls++, win);
  win.devicePixelRatio = 1.25;
  win.queries[0].fire();
  win.devicePixelRatio = 1.5;
  win.queries[1].fire();
  win.devicePixelRatio = 1;
  win.queries[2].fire();
  assert.equal(calls, 3);
  assert.equal(win.queries.length, 4);
});

test('stop() detiene los avisos y no vuelve a armar la consulta', () => {
  const win = fakeWin(1);
  let calls = 0;
  const stop = watchDpr(() => calls++, win);
  stop();
  win.queries[0].fire();
  assert.equal(calls, 0);
  assert.equal(win.queries.length, 1);
});
```

- [ ] **Step 2: Ejecutar para verificar que fallan**

Run: `node --test tests/create-renderer.test.js tests/dpr-watch.test.js`
Expected: FAIL con `Cannot find module` para ambos.

- [ ] **Step 3: Implementar**

`extension/src/create-renderer.js`:
```js
import { GlRenderer } from './gl-renderer.js';
import { fitsTexture } from './glyph-atlas.js';
import { Renderer, gridFor } from './renderer.js';

const SOFTWARE = /swiftshader|llvmpipe|softpipe|software|basic render/i;

export function isSoftwareRenderer(name) {
  return SOFTWARE.test(name ?? '');
}

/** Decide el backend según lo que el navegador puede hacer. Sin ganancia con GPU por software. */
export function chooseBackend({ hasWebGL2, rendererName, maxTextureSize, pw, ph }) {
  if (!hasWebGL2) return { backend: '2d', reason: 'WebGL2 no disponible' };
  if (isSoftwareRenderer(rendererName)) return { backend: '2d', reason: `GPU por software (${rendererName})` };
  if (!fitsTexture(maxTextureSize, pw, ph)) return { backend: '2d', reason: 'el atlas no cabe en una textura' };
  return { backend: 'gl', reason: rendererName || 'WebGL2' };
}

/** Sondea WebGL2 en un canvas desechable: un contexto fallido bloquearía el canvas real. */
export function probeWebGL2({ cellW = 9, cellH = 16 } = {}) {
  const { pw, ph } = gridFor(window.innerWidth, window.innerHeight, window.devicePixelRatio || 1, cellW, cellH);
  const gl = document.createElement('canvas').getContext('webgl2');
  if (!gl) return { hasWebGL2: false, pw, ph };
  const ext = gl.getExtension('WEBGL_debug_renderer_info');
  const info = {
    hasWebGL2: true,
    rendererName: ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : '',
    maxTextureSize: gl.getParameter(gl.MAX_TEXTURE_SIZE),
    pw,
    ph,
  };
  gl.getExtension('WEBGL_lose_context')?.loseContext();
  return info;
}

const replaceCanvas = (canvas) => {
  const fresh = canvas.cloneNode(false);
  canvas.replaceWith(fresh);
  return fresh;
};

/**
 * Crea el renderer WebGL2 si se puede y, si no, el 2D. Devuelve también el canvas final: si el
 * constructor de WebGL falla después de crear el contexto, el canvas original queda bloqueado y
 * se reemplaza por uno nuevo.
 */
export function createRenderer(canvas, opts = {}, deps = {}) {
  const {
    probe = probeWebGL2,
    makeGl = (c, o) => new GlRenderer(c, o),
    make2d = (c, o) => new Renderer(c, o),
    replaceCanvas: replace = replaceCanvas,
    warn = console.warn,
  } = deps;

  let { backend, reason } = chooseBackend(probe(opts));
  if (backend === 'gl') {
    try {
      return { renderer: makeGl(canvas, opts), backend, reason, canvas };
    } catch (err) {
      warn('[newtab] WebGL falló; se usa el renderer 2D:', err.message);
      backend = '2d';
      reason = `WebGL falló: ${err.message}`;
      canvas = replace(canvas);
    }
  }
  return { renderer: make2d(canvas, opts), backend, reason, canvas };
}
```
Nota: la dependencia inyectable se llama `replaceCanvas` (así la pasan las pruebas), y la variable local se llama `replace` para no chocar con la función por defecto del mismo nombre definida arriba.

`extension/src/dpr-watch.js`:
```js
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
    onChange();
    arm();
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
```

- [ ] **Step 4: Ejecutar para verificar que pasan**

Run: `node --test tests/create-renderer.test.js tests/dpr-watch.test.js`
Expected: 8 pruebas de `create-renderer` y 3 de `dpr-watch` pasan, sin ruido en la salida (las advertencias se inyectan con `warn`).

- [ ] **Step 5: Verificar todo el JS**

Run: `npm run test:js`
Expected: 88 pruebas pasan (67 anteriores + 10 de `glyph-atlas` + 8 de `create-renderer` + 3 de `dpr-watch`).

---

### Task 5: Conectar la página (`newtab.js`)

**Files:**
- Modify: `extension/newtab.js`

**Interfaces:**
- Consumes: `createRenderer` (tarea 4), `watchDpr` (tarea 4).

`newtab.js` depende de `chrome.*` y del DOM y no tiene pruebas automáticas; se verifica estáticamente aquí y en el navegador en la tarea 6.

- [ ] **Step 1: Cambiar los imports**

Reemplazar la línea `import { Renderer } from './src/renderer.js';` por nada (se elimina), y agregar, antes de la línea `import { MAX_COLS, MAX_ROWS, loadImage, saveImage, validateImage } from './src/imagestore.js';`, estas dos:
```js
import { createRenderer } from './src/create-renderer.js';
import { watchDpr } from './src/dpr-watch.js';
```
(Orden alfabético por ruta: `create-renderer`, `dpr-watch`, `imagestore`, ...).

- [ ] **Step 2: Starfield a 30 FPS**

Reemplazar `const FPS = { fire: 30, rain: 30, starfield: 60 };` por:
```js
const FPS = { fire: 30, rain: 30, starfield: 30 };
```

- [ ] **Step 3: Crear el renderer con la fábrica**

Reemplazar `const renderer = new Renderer($('bg'), { stride: cell_stride() });` por:
```js
  const { renderer, backend, reason } = createRenderer($('bg'), { stride: cell_stride() });
  console.info(`[newtab] renderer: ${backend} (${reason})`);
```

- [ ] **Step 4: Refactorizar el reajuste y vigilar el DPR**

Reemplazar el bloque:
```js
  let resizeTimer = 0;
  window.addEventListener('resize', () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      const next = renderer.fit();
      engine.resize(next.cols, next.rows);
      if (animated) renderer.invalidate();
      else applyMode(settings.mode);
    }, 150);
  });
```
por:
```js
  let resizeTimer = 0;
  const scheduleRefit = () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      const next = renderer.fit();
      engine.resize(next.cols, next.rows);
      if (animated) renderer.invalidate();
      else applyMode(settings.mode);
    }, 150);
  };
  window.addEventListener('resize', scheduleRefit);
  watchDpr(scheduleRefit); // un cambio de escala (otro monitor) no siempre dispara `resize`
```

- [ ] **Step 5: Verificación estática**

Run:
```bash
node --check extension/newtab.js
npm run test:js
```
Expected: sin errores de sintaxis y 88 pruebas pasan. Comprobar además que ningún otro archivo importaba `Renderer` desde `newtab.js` y que `grep -n "new Renderer" extension/newtab.js` no devuelve nada (la clase `Renderer` sigue existiendo en `renderer.js` como respaldo, usada solo por la fábrica).

---

### Task 6: Verificación píxel a píxel, medición, compuerta de tamaño y documentación

**Files:**
- Modify: `docs/measurements.md`, `docs/superpowers/specs/2026-10-08-new-tab-ascii-design.md`
- Trabajo temporal (fuera del proyecto): `...\scratchpad\spike\final\`

Esta tarea reutiliza el arnés del spike (en `...\scratchpad\spike\`: `lib.mjs`, `measure.mjs`, `verify.mjs`, la carpeta `verify\` y las copias `base\` y `b\`). Ese arnés ya carga extensiones con `Extensions.loadUnpacked`, mide CPU con `SystemInfo.getProcessInfo` y compara píxeles. Léelo antes de adaptarlo. No modifiques archivos del proyecto salvo los dos de documentación.

- [ ] **Step 1: Compuerta de tamaño**

Run: `npm run check:size`
Expected: JS+CSS+HTML ≤ 50 KB (51,200 bytes). **Si falla:** no recortes comentarios ni subas el tope; anota el número exacto, reporta BLOCKED y espera la decisión del usuario (ver "Decisión abierta"). Si pasa, anota el número exacto y el margen restante.

- [ ] **Step 2: Copiar la extensión real al área temporal**

Copiar `extension\` completo a `...\scratchpad\spike\final\` (esa es la copia que se verifica y mide; el original no se toca). Adaptar el arnés de verificación (`verify.mjs` y la página `verify\`) para que compare, con el mismo buffer de celdas y semilla fija, el `Renderer` 2D contra el `GlRenderer` reales del proyecto (importados desde la copia), en: `fire` (400 frames), `rain` (1500 frames, con un rayo), `starfield` (2400 frames; ahora con el HUD ASCII) y modo imagen con un degradado sintético (caracteres de rampa y colores de 24 bits).

- [ ] **Step 3: Verificación píxel a píxel**

Expected: `fire`, `rain` y `starfield` idénticos (diferencia máxima ≤ 2/255, 0 píxeles > 8); imagen con diferencia ≤ 22/255 en ≤ 0.05 % de los píxeles; ninguna celda obsoleta (el GL con `invalidate()` y sin él da lo mismo); sin errores ni excepciones de consola. Capturar además una captura de pantalla de cada modo y revisarla. Si el modo `starfield` muestra el HUD en celdas distintas del 2D, reportarlo con coordenadas.

- [ ] **Step 4: Verificar el respaldo y los mensajes de consola**

Con el mismo arnés, lanzar una instancia de Chrome aparte con `--disable-gpu` (o `--use-gl=disabled`) y la extensión cargada. Expected: la consola registra `[newtab] renderer: 2d (...)` con el motivo, la pestaña muestra el fuego (captura no vacía) y no hay excepciones. En la instancia normal debe registrar `[newtab] renderer: gl (...)` con el nombre de la GPU. Probar también la pérdida de contexto: desde `Runtime.evaluate` en la página, `const e = document.getElementById('bg').getContext('webgl2').getExtension('WEBGL_lose_context'); e.loseContext(); setTimeout(() => e.restoreContext(), 500)` y comprobar con una captura 2 s después que el canvas se repinta y no queda en blanco (la animación sigue; en modo imagen, tras `restoreContext` la imagen debe volver a verse al siguiente `draw`/redimensionado: si no vuelve, reportarlo como hallazgo con el detalle).

- [ ] **Step 5: Medir**

Con el mismo método del spike (ventana ≈ 1920×1080, 8 s de calentamiento y ≥ 3 ventanas de 10 s, corridas intercaladas y un `about:blank` de control al inicio y al final), medir la copia nueva (`final`) y la copia previa del spike (`base`, el renderer 2D antiguo con starfield a 60 FPS) para `fire`, `rain` y `starfield`, y la pestaña con el fondo `fire` oculta (debe seguir ≈ 0). Reportar CPU de pestaña, GPU y suma, memoria y la tasa de pasos entregada (30/30/30). Comparar con las cifras del spike (`b`: fire 16.6, starfield 32.3 a 60 FPS, rain 14.7). Expected: `final` mejora claramente a `base` en los tres; `starfield` a 30 FPS debe quedar bastante por debajo de su cifra a 60 FPS.

- [ ] **Step 6: Actualizar la documentación**

(a) `docs/measurements.md`: agregar una sección "Renderer WebGL2" con la tabla antes/después (valores del paso 5, con la nota de que el arnés infla las cifras absolutas), el tamaño del paquete del paso 1 y las conclusiones; ajustar la sección "Conclusiones" (la hipótesis de 1 a 5 % se cumple o no según lo medido: dilo con el número).
(b) Spec `docs/superpowers/specs/2026-10-08-new-tab-ascii-design.md`, §5: leer la sección y editar (no reescribirla entera): sustituir los puntos de "Renderer" sobre el atlas de glifos con `drawImage` y el diff por celda por una descripción del `GlRenderer` (una llamada de dibujo por frame, textura de datos con las celdas, atlas dinámico con 8 filas de luminancia que reproducen el pre-blend de texto de Skia, respaldo al renderer 2D cuando no hay WebGL2, la GPU es por software o falla el contexto, y reconstrucción al perder el contexto o cambiar el DPR); cambiar `starfield 60` por `starfield 30` en la lista de FPS objetivo; y en "Presupuesto de tamaño" actualizar la nota del tope de JS+CSS+HTML con el número real del paso 1.

- [ ] **Step 7: Limpieza y verificación final**

Cerrar cualquier Chrome que hayas lanzado (solo tu propio árbol de procesos; el Chrome del usuario no se toca nunca). Ejecutar: `npm run test && npm run check:size`. Expected: Rust 56 y JS 88 pruebas pasan, `check:size` termina con código 0. Entregar la lista de pasos manuales para el usuario: abrir una pestaña nueva con cada fondo; Windows al 125 % y 150 % (sin franja ni recorte); zoom 25 % en un monitor ancho; mover la ventana entre monitores con escala distinta (no debe verse borrosa ni desfasada); cambiar de fondo varias veces seguidas; modo imagen; y comparar CPU en `Shift+Esc` contra la lectura anterior del usuario (fire: 35.1 % de un núcleo).

---

## Self-review

**Cobertura del diseño aprobado:** (1) renderer WebGL2 con la misma interfaz y fábrica con respaldo 2D → tareas 3 y 4; (2) atlas dinámico con 8 filas de luminancia → tareas 2 y 3; (3) HUD sin emoji → tarea 1 (y por eso no hay ruta de color); (4) pérdida de contexto y cambio de DPR → tareas 3 (manejadores `webglcontextlost/restored`), 4 (`watchDpr`) y 5 (conexión), verificados en la tarea 6; (5) pruebas puras con Node y el arnés píxel a píxel → tareas 2, 4 y 6; (6) tamaño → decisión abierta y compuerta en la tarea 6; (7) starfield a 30 FPS → tarea 5.

**Placeholders:** ninguno. La tarea 6 delega la adaptación del arnés porque depende de archivos temporales que el ejecutor debe leer; los criterios de aceptación (umbrales, escenarios, comandos) están dados.

**Consistencia de tipos:** `createSlotAllocator().slotFor(cp, onNew)`, `glyphBlock`, `atlasSize`, `fitsTexture`, `luminanceBucket` se definen en la tarea 2 y se usan con esas firmas en las tareas 3 y 4. `GlRenderer` expone `fit/invalidate/draw` como `Renderer`. `createRenderer(canvas, opts, deps)` devuelve `{ renderer, backend, reason, canvas }`, y `newtab.js` solo usa `renderer`, `backend` y `reason`. El nombre de la dependencia inyectada es `replaceCanvas` en la fábrica y en las pruebas (ver el ajuste obligatorio de la tarea 4).

**Riesgo de la derivación a mano:** los valores fijados de `luminanceBucket` (6, 7, 0, 4, 2, 0) están calculados a mano a partir de la fórmula del shader del spike. Si una prueba falla, la causa más probable es un error de transcripción de la fórmula; la prueba de monotonía y de las 8 cubetas es independiente de esos valores.

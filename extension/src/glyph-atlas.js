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

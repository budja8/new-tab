// Una sola imagen ASCII ya convertida (4 bytes por celda), guardada en IndexedDB.

export const MAX_COLS = 200;
export const MAX_ROWS = 100;
export const MAX_IMAGE_BYTES = 200 * 1024;

const DB_NAME = 'newtab-ascii';
const STORE = 'image';
const KEY = 'current';

/** `img = { w, h, data }` donde `data` son `w*h*4` bytes empaquetados por `image_to_ascii`. */
export function validateImage(img) {
  if (!img || !(img.data instanceof Uint8Array)) return false;
  const { w, h, data } = img;
  if (!Number.isInteger(w) || !Number.isInteger(h)) return false;
  if (w < 1 || h < 1 || w > MAX_COLS || h > MAX_ROWS) return false;
  return data.length === w * h * 4 && data.length <= MAX_IMAGE_BYTES;
}

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function withStore(mode, fn) {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(STORE, mode);
      const req = fn(tx.objectStore(STORE));
      // Resolver al completar la transacción (commit), no al éxito de la petición.
      tx.oncomplete = () => resolve(req.result);
      tx.onabort = () => reject(tx.error ?? req.error);
      tx.onerror = () => reject(tx.error ?? req.error);
      req.onerror = () => reject(req.error);
    });
  } finally {
    db.close();
  }
}

export async function saveImage(img) {
  if (!validateImage(img)) throw new Error('Imagen inválida o demasiado grande');
  await withStore('readwrite', (s) => s.put({ w: img.w, h: img.h, data: img.data }, KEY));
}

/** Devuelve la imagen guardada, o `null` si no hay o está corrupta. */
export async function loadImage() {
  const img = await withStore('readonly', (s) => s.get(KEY));
  return validateImage(img) ? img : null;
}

export async function clearImage() {
  await withStore('readwrite', (s) => s.delete(KEY));
}

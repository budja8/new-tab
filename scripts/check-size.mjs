// Falla si se superan los topes de tamaño del spec (sección 5).
import { readdirSync, statSync } from 'node:fs';
import { extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../extension/', import.meta.url));
// El tope de JS+CSS+HTML subió de 50 a 60 KB (renderer WebGL2) y de 60 a 70 KB (dashboard); ambas decisiones del usuario, 2026-10-09.
const LIMITS = { wasm: 150 * 1024, text: 70 * 1024 };

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)]
  );
}

let wasm = 0;
let text = 0;
for (const file of walk(root)) {
  const ext = extname(file);
  const size = statSync(file).size;
  if (ext === '.wasm') wasm += size;
  else if (['.js', '.css', '.html'].includes(ext)) text += size;
}

const kb = (n) => `${(n / 1024).toFixed(1)} KB`;
console.log(`wasm:         ${kb(wasm)} / ${kb(LIMITS.wasm)}`);
console.log(`js+css+html:  ${kb(text)} / ${kb(LIMITS.text)}`);

const failures = [];
if (wasm === 0) failures.push('No hay .wasm: ejecuta `npm run build:wasm` primero.');
if (wasm > LIMITS.wasm) failures.push('El .wasm supera el tope.');
if (text > LIMITS.text) failures.push('JS+CSS+HTML supera el tope.');
if (failures.length) {
  console.error(failures.join('\n'));
  process.exit(1);
}

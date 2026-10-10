# Estado del proyecto, decisiones y plan (corte del 2026-10-09)

Este documento reúne todo lo que se decidió y lo que queda por hacer, para retomar el proyecto sin depender de la conversación original. Está pensado para leerse junto con:

- [`README.md`](../README.md): qué es y cómo construirlo.
- [`docs/superpowers/specs/2026-10-08-new-tab-ascii-design.md`](superpowers/specs/2026-10-08-new-tab-ascii-design.md): especificación (vigente, actualizada hasta `pacman`).
- [`docs/measurements.md`](measurements.md): mediciones de rendimiento.
- [`docs/superpowers/plans/`](superpowers/plans/): los dos planes ejecutados (v1 y renderer WebGL2).
- [`docs/reports/`](reports/): reportes de medición y del spike del renderer; `docs/reports/ledgers/` con los registros de ejecución de cada fase (incluyen los "minors" diferidos).
- [`tools/cdp-harness/`](../tools/cdp-harness/): el arnés de medición con Chrome (ver su README).

## 1. Qué es

Extensión de Chrome (MV3) que reemplaza la pestaña nueva por un fondo ASCII animado, o por una imagen convertida a ASCII, más un panel de estadísticas de teclas pulsadas dentro de Chrome. El núcleo (animaciones, conversión de imagen) está en Rust compilado a WebAssembly (`core/`); el resto es JavaScript sin bundler (`extension/`). Las animaciones se portaron desde `refs/cwork`, la conversión de imagen desde `refs/rust-ascii`.

Restricción principal del usuario: **tiene que ser eficiente** (poca CPU, RAM y tamaño), mucho menos que correr lo mismo en una terminal.

## 2. Estado actual (verificado el 2026-10-09)

| Cosa | Valor |
|---|---|
| Pruebas Rust (`cargo test`) | 205 pasan |
| Pruebas JS (`node --test`) | 121 pasan |
| `.wasm` | 80.1 KB (tope 150 KB) |
| JS + CSS + HTML (incluye el glue de `pkg/`) | 59.8 KB (tope **70 KB**) |
| Modos disponibles | `fire`, `rain`, `starfield`, `pipes`, `dna`, `synthwave`, `compile_sim`, `pacman_install`, `dashboard`, `pacman`, `image` |
| Sin portar de cwork | `mario` (diseño ya presentado, ver sección 5) |

Cómo obtener esas cifras: `npm test` y `npm run check:size` (ver README).

### Modos y FPS

Todos a **30 FPS**, excepto `dashboard` a **5 FPS** (decisión del usuario: nada se mueve de forma fluida, el reloj y las barras cambian una vez por segundo). Con la pestaña oculta el bucle se detiene (consumo ≈ 0). Con `prefers-reduced-motion` o en modo imagen se pinta un solo frame.

| Modo | Etiqueta en el selector | Notas |
|---|---|---|
| `fire` | Fuego | v1 |
| `rain` | Lluvia | v1 |
| `starfield` | Campo de estrellas | v1; HUD en ASCII (sin emoji) |
| `pipes` | Tuberías | se llena al 80 % y se queda quieto 4 s (en segundos reales) antes de limpiarse |
| `dna` | ADN | hélice 3D + registro "GENETIC SEQUENCING DATABASE"; composición centrada (layout B) |
| `synthwave` | Synthwave | sol, cuadrícula en perspectiva, tres ondas; título `** SYNTHWAVE HORIZON **` en ASCII |
| `compile_sim` | Compilación | simulación de `cargo build`; ritmo original de 60 ticks/s; bloque centrado (106 col.) |
| `pacman_install` | Instalación (pacman) | simulación de `pacman -Syu`; bloque centrado (77 col.); línea de paquetes partida en 3 |
| `dashboard` | Panel de sistema | datos reales del navegador (ver sección 4); 5 FPS |
| `pacman` | Pac-Man | piloto automático; energizante de 6 s; salvaguarda de 90 s |
| `image` | Imagen ASCII | imagen del usuario, guardada en IndexedDB (≤ 200×100 celdas) |

## 3. Decisiones del usuario (no reabrir sin que él lo pida)

Proceso y alcance:
- **Sin commits ni `git init`** durante el desarrollo (no había repo). Al subirlo, el usuario crea el repo.
- Trabajo en **modo con agentes** (un subagente implementa, otro revisa), con TDD, y **una animación a la vez**.
- **Las mediciones de rendimiento de las animaciones nuevas quedan para el final** (las de `synthwave`, `compile_sim`, `pacman_install`, `dashboard` y `pacman` están pendientes; ver sección 5). Antes de `synthwave` sí se midió cada animación (`pipes`, `dna`) contra `fire` en la misma sesión.
- Alcance de las estadísticas de teclas: **solo dentro de Chrome** por ahora; el sistema completo es una fase posterior.

Renderer y rendimiento:
- Se eligió el renderer **WebGL2** (una llamada de dibujo por frame, atlas de glifos con 8 filas de luminancia que replica el pre-blend de Skia) con el renderer 2D como respaldo. Consumo real del usuario con `fire`: 35.1 % → 8.8 % de un núcleo; memoria 60,968 K → 32,908 K.
- **Tope de tamaño**: `.wasm` ≤ 150 KB; JS+CSS+HTML subió de 50 → 60 KB (renderer WebGL2) y de 60 → 70 KB (`dashboard`, cuando el margen quedó en 25 bytes). `scripts/check-size.mjs` lo hace cumplir.
- **No** agregar un conmutador "invertir" para imágenes de fondo claro, y **no** subir la opacidad de los paneles de KPI/ajustes. No volver a proponerlo.

Por animación:
- `pipes`: llenado hasta 80 % (no 55 %), imagen final quieta 4 s, y **30 FPS** (arrancó en 60).
- `dna`: **layout B** (composición centrada horizontalmente).
- `compile_sim`, `pacman_install`: centradas como `dna`; `pacman_install` parte la línea de paquetes en varias líneas como hace pacman real.
- `dashboard`: panel **BROWSER** en lugar de la tabla de procesos (`chrome.processes` no existe en Chrome estable); **5 FPS**; el disco se muestra como fila de solo capacidad (`DISK  456.6 GB total`) bajo la barra de RAM, sin barra (ver sección 4).
- `pacman`: símbolos originales en una tabla `GLYPHS`; energizante de **6 s reales** (la original lo hacía durar 1 s aunque el HUD decía 6); salvaguarda: tras 90 s sin comer pasa de nivel.
- Todos los emoji de las animaciones originales se reemplazaron por ASCII (starfield, synthwave, pacman, y lo que venga).

## 4. Notas técnicas que conviene recordar

- **Panics en WASM matan la pestaña.** Todo port debe usar `i32`, sin restas `u16`/`usize` que desborden, sin rangos aleatorios vacíos, sin `unwrap` y con tamaños 0×0 y mínimos probados. Cada animación tiene una prueba de barrido de tamaños.
- **Ritmo de las animaciones basadas en ticks.** Las de cwork cuentan a 60 ticks por segundo. `set_fps(fps)` + `steps = max(1, round(60/fps))` ejecuta varios pasos lógicos por frame para conservar la velocidad real (patrón de `compile_sim`, `pacman_install`, `pacman`).
- **Sin formato de float** (`{:.1}`, `{:>5.1}`): engorda el `.wasm`; se usan enteros en décimas.
- **Cuadrícula máxima** 1024 por lado (`MAX_GRID_DIM`). El buffer de celdas es `[codepoint, fg, bg]` en `u32` (stride 3), colores `0xFFRRGGBB`, `0` = sin color.
- **`dashboard` y las APIs de Chrome**: `chrome.system.cpu/memory/storage` requieren permisos nuevos en el manifest; **si la extensión no se recarga tras agregarlos, no hay datos** (síntoma: solo reloj y panel BROWSER). `chrome.system.storage.getAvailableCapacity` solo existe en ChromeOS, así que en escritorio solo se puede mostrar capacidad. Las unidades `fixed` de igual capacidad se cuentan una vez (en Windows aparece una unidad virtual "Google Drive" con el mismo tamaño que el disco). El canal JS→WASM es `Engine::feed(Float32Array)` y `Engine::feed_text(string)` (layout documentado en el spec).
- **Medición**: el arnés CDP infla los absolutos y cambia entre sesiones; comparar siempre razones contra `fire` de la misma sesión. La referencia absoluta en Chrome normal es el Administrador de tareas (`Shift+Esc`).

## 5. Pendiente y plan

### Para retomar en orden

1. **Verificar a ojo en Chrome las animaciones nuevas** (el usuario ya vio funcionando `dna`, `synthwave`, `compile_sim` y `dashboard` con RAM/CPU/disco; `pacman_install` no tuvo una confirmación explícita y `pacman` aún no lo ha visto):
   - `pacman`: confirmar que los símbolos `ᗤ ᗢ ᗧ ᗣ ᗛ ⍛` se ven bien con las fuentes de Windows. Si salen como cuadrados o desalineados, pasar a ASCII (`<`, `>`, `^`, `v`, `M`) cambiando solo la tabla `GLYPHS` en `core/src/animation/pacman.rs`.
2. **`mario`** (última de cwork). El diseño ya se presentó y **está pendiente de visto bueno del usuario**:
   - Portar `refs/cwork/src/animation/mario.rs` (runner lateral con paralaje, piloto automático, goombas, tuberías, bloques `?`, monedas).
   - HUD sin emoji: `🪙 x00` → `● x00`; pie `AUTOPILOT ACTIVE | JUMPING & STOMPING` (sin `q`).
   - 2 pasos lógicos por frame a 30 FPS (constantes pensadas para 60 ticks/s).
   - Golpe al bloque `?` por cruce (la original exige igualdad exacta de fila y puede saltársela, porque Mario sube más de una fila por tick).
   - Seguridad: `i32`, tamaño mínimo 20×12 (por debajo solo cielo y suelo), sin `u16`.
   - Integración: archivo `core/src/animation/mario.rs`, modo `mario` ("Mario"), `MODES`, HTML, `FPS` a 30.
3. **Mediciones finales** (decisión del usuario): medir `synthwave`, `compile_sim`, `pacman_install`, `dashboard`, `pacman` y `mario` contra `fire` de la misma sesión con el arnés de `tools/cdp-harness/`, y registrarlas en `docs/measurements.md`. Esperado: `dashboard` el más barato; `pacman` y `mario` redibujan toda la pantalla en cada frame; `synthwave` cambia pocas celdas por frame. Confirmar también con `Shift+Esc` en el Chrome del usuario.
4. **Subir el repo.** Ver "Preparar el repo" más abajo.

### Fases posteriores acordadas

- **Estadísticas de teclas de todo el sistema** (hoy solo dentro de Chrome): reutilizar `refs/kbd-stats` (Python) o reescribirlo en Rust, con mensajería nativa o un servidor local hacia la extensión. Requiere su propio spec (brainstorming, plan, implementación).

### Mejoras menores diferidas

De las listas de "minors" de los registros (`docs/reports/ledgers/`), las que más valen la pena:

- Mensaje claro cuando falla el fallback de imagen invisible.
- Top 20 de teclas que se bloquea durante el día.
- Atlas de glifos mayor que `MAX_TEXTURE_SIZE` tras un cambio de DPR (solo si DPR > 16).
- Imagen estática + redimensionado durante una pérdida de contexto WebGL.
- Diferencias de cubeta de luminancia frente a Skia en bordes (22/255 en ~0.1 % de píxeles con colores aleatorios).
- El panel de KPI tapa parte del registro de `dna` en ventanas ≤ 1280 px (el usuario decidió no tocar la opacidad de los paneles).
- Pruebas débiles señaladas por los revisores: `the_autopilot_actually_plays` en `pacman` (solo comprueba que cambie puntaje, nivel o vidas), y varias aserciones laxas en `pipes`/`dna`.
- Margen de JS: si se acerca a 70 KB, decidir con el usuario subir el tope otra vez o recortar.

## 6. Cómo se trabajó (para repetirlo)

Flujo por animación, acordado con el usuario:

1. Leer el original en `refs/cwork/src/animation/`.
2. Presentar un **diseño corto en el chat**: qué hace, qué cambia por WASM (seguridad, ritmo, números sin float, emoji), decisiones del usuario con una recomendación.
3. Esperar el "sí" del usuario.
4. Implementar con un subagente (TDD: pruebas, ver que fallan, implementar, ver que pasan), con la lista exacta de archivos a tocar (`core/src/animation/<modo>.rs`, `mod.rs`, `engine.rs`, `extension/src/settings.js` → `MODES`, `newtab.html` → opción, `newtab.js` → `FPS`, pruebas JS en `tests/settings.test.js` y `tests/wasm-smoke.test.js`).
5. Verificar por cuenta propia: `cargo test`, `node --test`, `npm run check:size`.
6. Actualizar el spec y estas notas.

Trucos aprendidos:
- Los subagentes a veces no pueden escribir `.md`: pedir el reporte en la respuesta o en `.txt`.
- En Windows, `cargo test` a veces falla con "os error 32" (archivo bloqueado): reintentar.
- Los perfiles temporales de Chrome del arnés no se pueden borrar (acceso denegado): carpetas `C:\nt*` (`ntspike`, `ntfinal`, `ntpipes`, `ntdna`, `ntdash`); son basura segura de borrar a mano.

## 7. Preparar el repo

- Hay un `.gitignore` con `node_modules/`, `core/target/` y `extension/pkg/` (generados). **Para cargar la extensión hay que construir primero** (`npm run build:wasm`); ver README.
- `refs/` pesa ~3 GB y contiene otros proyectos (`cwork`, `kbd-stats`, `rust-ascii`) que son referencia de solo lectura. **No está ignorada**: decide si va al repo, es un submódulo, o se ignora (por tamaño, lo normal es ignorarla o excluir sus `target/`).
- No hay `LICENSE`; agrégala si el repo será público.

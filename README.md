# New Tab ASCII

Extensión de Chrome (Manifest V3) que reemplaza la pestaña nueva por un fondo ASCII animado, o por una imagen convertida a ASCII, y muestra un panel con estadísticas de las teclas que pulsas dentro de Chrome. El núcleo está escrito en **Rust y compilado a WebAssembly**; el renderer usa **WebGL2** (con respaldo en canvas 2D) para gastar poca CPU.

> Estado y plan: [`docs/PROJECT-STATUS.md`](docs/PROJECT-STATUS.md). Especificación: [`docs/superpowers/specs/2026-10-08-new-tab-ascii-design.md`](docs/superpowers/specs/2026-10-08-new-tab-ascii-design.md). Mediciones: [`docs/measurements.md`](docs/measurements.md).

## Qué hace

- **Fondos animados** (portados de otros proyectos en Rust):
  `fire` (Fuego), `rain` (Lluvia), `starfield` (Campo de estrellas), `pipes` (Tuberías), `dna` (ADN), `synthwave`, `compile_sim` (Compilación), `pacman_install` (Instalación de pacman), `dashboard` (Panel de sistema), `pacman` (Pac-Man) y `mario`.
- **Imagen ASCII**: eliges una imagen y se guarda convertida en el navegador (IndexedDB), a color y estática.
- **Panel de KPI de teclas**: teclas más usadas, atajos, ritmo de escritura. Solo cuenta nombres de teclas (nunca el texto escrito) y omite los campos de contraseña. Se activa y desactiva con la tecla `K`.
- **Panel de sistema** (`dashboard`): reloj, RAM, CPU por núcleo y datos del navegador, a partir de `chrome.system.*` y de lo que reporta el navegador.
- Ajustes con el engrane de la esquina (fondo y panel de KPI).

## Instalar y probar

Requisitos: Node 20+, Rust (con el target `wasm32-unknown-unknown`) y [`wasm-pack`](https://rustwasm.github.io/wasm-pack/). Para minimizar el `.wasm`, `wasm-pack` descarga `wasm-opt`.

```bash
npm run build:wasm      # compila core/ a extension/pkg/ (obligatorio antes de cargar la extensión)
```

Luego, en Chrome:

1. Abre `chrome://extensions` y activa el **modo de desarrollador**.
2. **Cargar descomprimida** y elige la carpeta `extension/`.
3. Abre una pestaña nueva. Elige el fondo en el engrane.

> Si cambias el `manifest.json` (por ejemplo, los permisos), **recarga la extensión** en `chrome://extensions`; si no, el panel de sistema aparece sin datos.

## Comandos

| Comando | Qué hace |
|---|---|
| `npm run build:wasm` | Compila el núcleo Rust a WebAssembly en `extension/pkg/` (release, `opt-level = "z"`, LTO, `wasm-opt -Oz`). |
| `npm test` | Pruebas de Rust y de JavaScript. |
| `npm run test:rust` | Solo `cargo test` del núcleo. |
| `npm run test:js` | Solo las pruebas de Node (`node --test`). |
| `npm run check:size` | Falla si el `.wasm` pasa de 150 KB o JS+CSS+HTML pasa de 70 KB. |

Tamaños a la fecha: `.wasm` 87.8 KB y JS+CSS+HTML 59.9 KB.

## Estructura

```
core/                       Núcleo Rust → WASM (crate newtab-core)
  src/lib.rs, engine.rs     Engine (API para JS): set_animation, set_fps, step, feed, load_image…
  src/grid.rs, rng.rs       Cuadrícula de celdas [carácter, color de texto, color de fondo] y generador pseudoaleatorio propio
  src/ascii.rs              Conversión imagen → ASCII
  src/animation/*.rs        Una animación por archivo (fire, rain, starfield, pipes, dna, synthwave,
                            compile_sim, pacman_install, dashboard, pacman, mario)
extension/                  La extensión (carpeta que se carga en Chrome)
  manifest.json, newtab.*   Pestaña nueva (HTML, CSS, JS principal)
  background.js, content/   Service worker y content scripts del conteo de teclas
  src/                      Renderers (WebGL2 y 2D), atlas de glifos, limitador de FPS, ajustes,
                            almacén de imagen, KPI, muestreo de sistema para el dashboard
  pkg/                      Generado por wasm-pack (no se versiona)
tests/                      Pruebas de JavaScript (Node)
scripts/check-size.mjs      Verificación de tamaños
tools/cdp-harness/          Arnés de medición con Chrome (referencia)
docs/                       Spec, planes, mediciones, estado del proyecto y reportes
refs/                       Proyectos de referencia (solo lectura): cwork, rust-ascii, kbd-stats
```

## Cómo agregar una animación

1. Crear `core/src/animation/<modo>.rs` con una estructura que implemente `Animation` (`update`, `draw`; opcionalmente `set_fps`, `feed`).
2. Registrarla en `core/src/animation/mod.rs` y en el `match` de `Engine::set_animation` (`core/src/engine.rs`).
3. En `extension/`: agregar el modo a `MODES` (`src/settings.js`), una `<option>` en `newtab.html` y su FPS en `newtab.js`.
4. Agregar pruebas (Rust y las de `tests/settings.test.js` y `tests/wasm-smoke.test.js`), compilar y correr `npm test` y `npm run check:size`.

Reglas del núcleo: nada puede entrar en pánico (un pánico en WASM mata la pestaña), así que se usa aritmética `i32`, sin restas que desborden ni rangos vacíos, y se prueban tamaños 0×0 y mínimos. Evita el formato de decimales (`{:.1}`): engorda el `.wasm`. Más detalles en `docs/PROJECT-STATUS.md`.

## Privacidad

El conteo de teclas se guarda solo en `chrome.storage.local` de tu navegador (90 días, con top 20 y "otros"). Se registran nombres de teclas, no el texto que escribes, y se ignoran los campos de contraseña. Nada sale de tu equipo. La extensión pide los permisos `storage`, `system.cpu`, `system.memory` y `system.storage` (estos tres, solo para el panel de sistema).

## Licencia

Sin definir todavía.

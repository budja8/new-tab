# Extensión New Tab ASCII: diseño

Fecha: 2026-10-08

## 1. Objetivo

Extensión de Chrome (Manifest V3) que reemplaza la página de nueva pestaña con:

- Un **fondo ASCII a pantalla completa**: una animación portada de `refs/cwork` o una imagen del usuario convertida a ASCII (lógica de `refs/rust-ascii`).
- Un **panel de KPI superpuesto** con estadísticas de teclas presionadas **solo dentro de Chrome**.

La lógica reutilizada de Rust se compila a **WebAssembly**. La página debe ser eficiente: consumo de CPU y RAM bajo, y tamaño de la extensión acotado.

### Criterios de éxito

- Al abrir una pestaña nueva se ve el fondo elegido y el panel de KPI.
- Con la pestaña oculta, el consumo del bucle de dibujo es cero.
- Con imagen estática, no hay bucle de dibujo activo.
- Los topes de tamaño de la sección 5 se cumplen y se verifican.
- No se almacena texto escrito por el usuario, solo conteos.

## 2. Alcance

### Incluido (v1)

- Animaciones `fire`, `rain`, `starfield`, `pipes` ("Tuberías", agregada el 2026-10-09), `dna` ("ADN", agregada el 2026-10-09), `synthwave` ("Synthwave"), `compile_sim` ("Compilación"), `pacman_install` ("Instalación (pacman)"), `dashboard` ("Panel de sistema") y `pacman` ("Pac-Man"), todas agregadas el 2026-10-09, portadas desde `refs/cwork/src/animation/`.
- Una imagen cargada por el usuario, convertida a ASCII.
- Panel de KPI: total de teclas del día, WPM, top 5 teclas, atajos más frecuentes y teclas por sitio.
- Selector de modo de fondo (ajuste simple) y atajo para ocultar el panel.

### Fuera de alcance (v1)

- Estadísticas de todo el sistema (fase posterior; ver sección 9).
- Heatmap de teclado e histórico visual.
- Las otras animaciones de cwork (`mario`). Se agregan de a poco midiendo cada una (`pipes` y `dna` ya se portaron el 2026-10-09).
- Partículas reactivas al audio (`particles` de rust-ascii).

## 3. Decisiones tomadas

| Decisión | Elección | Motivo |
|---|---|---|
| Integración de Rust | WASM con `wasm-pack` | Reutiliza el código real; el costo de WASM es marginal frente al de dibujar |
| Disposición de la UI | Fondo ASCII + panel KPI superpuesto | Un solo bucle de dibujo activo; se parece a una pantalla de inicio normal |
| Alcance de teclas | Solo Chrome en v1 | No requiere programa externo; el sistema completo queda para después |
| Métricas | Nivel intermedio | Cabe en un panel pequeño; fecha por registro deja listo el histórico |
| Imagen | Se convierte al subirla y se guarda la cuadrícula resultante | Cada pestaña solo lee datos ya procesados |

### Referencia de rendimiento (medido en esta máquina, 12 núcleos)

`cwork.exe` en release a 60 FPS, solo el proceso (sin el costo del host de consola):

| Animación | CPU | RAM |
|---|---|---|
| fire | 0.93 % | 8.4 MB |
| rain | 0.19 % | 7.6 MB |
| starfield | 0.18 % | 7.6 MB |
| dna | 0.20 % | 7.6 MB |

Estas cifras son el piso del cálculo en Rust. El costo en Chrome incluye el compositor y el canvas, y se medirá por separado.

## 4. Arquitectura

```
new-tab/
├── core/                  # crate Rust → WASM
│   └── src/ { lib.rs, rng.rs, grid.rs, ascii.rs, engine.rs, animation/{fire,rain,starfield,pipes,dna,synthwave,compile_sim,pacman_install,dashboard,pacman}.rs }
├── extension/             # lo que se carga en Chrome (sin core/target)
│   ├── manifest.json
│   ├── newtab.html / newtab.js / newtab.css
│   ├── background.js      # service worker: agrega y guarda conteos
│   ├── content/           # content script: solo cuenta teclas
│   ├── src/               # renderer (2D), gl-renderer, glyph-atlas, create-renderer, dpr-watch, diff, limiter, kpi, keystats, store, settings, imagestore
│   └── pkg/               # generado por wasm-pack
├── scripts/check-size.mjs
└── tests/
```

La extensión vive en `extension/` para no incluir `core/target` (cientos de MB) al cargarla.

### Frontera Rust / JS

Rust es dueño del **estado y la lógica** (animaciones y conversión de imagen). JS solo pide un frame y lo pinta.

Funciones expuestas por el crate:

- `Engine::new(seed, w, h)` y `set_animation(nombre)`: crea el motor y elige `fire`, `rain`, `starfield`, `pipes`, `dna`, `synthwave`, `compile_sim`, `pacman_install`, `dashboard` o `pacman`; `set_fps(fps)` le pasa el FPS objetivo (lo usa `pipes` para medir su pausa en segundos).
- `step()`: avanza un frame.
- `cells_ptr()` / `cells_len()`: buffer plano en memoria compartida, `[carácter, fg, bg]` por celda (`u32`), sin serializar por frame.
- `image_target_dims(src_w, src_h, max_w, max_h)` e `image_to_ascii(rgba, w, h, invert)`: JS reduce la imagen y Rust la convierte a celdas empaquetadas (`[índice, r, g, b]`, 4 bytes por celda). `load_image()` la muestra, centrada y reducida si no cabe.

### Adaptaciones al portar desde cwork

- `crossterm::style::Color` se reemplaza por colores `u32` (`0xFFRRGGBB`; `0` equivale a `Reset`). Las 3 animaciones solo usan `crossterm` para `Color`.
- `rand::thread_rng()` pasa a un RNG propio (xorshift64\*) sembrado desde JS, sin `rand` ni `getrandom`: añade menos al `.wasm` y la semilla fija permite pruebas reproducibles.
- Los rangos aleatorios vacíos (`gen_range` hacía panic) y las restas en `u16` (desbordaban con cuadrículas pequeñas) se sustituyen por versiones que no hacen panic: en WASM un panic mata la pestaña.
- El trait `Animation` (`update` + `draw` sobre un `Buffer` de celdas) se mantiene casi igual. El `Buffer` pierde el método de diff hacia un writer de terminal; el diff pasa a JS (sección 5).

Las 3 animaciones suman 590 líneas (`fire` 110, `rain` 200, `starfield` 280).

## 5. Renderizado y rendimiento

### Bucle de dibujo (uno solo activo a la vez)

```
requestAnimationFrame → ¿toca frame? (limitador por FPS objetivo)
  → wasm.step()
  → lee cells() de memoria compartida
  → renderer.draw(cells)       # traduce solo las celdas cambiadas
  → gl.drawArrays(…)           # una sola llamada de dibujo por frame (WebGL2)
```

### Renderer

- **`GlRenderer` (WebGL2)**, el renderer por defecto: una sola llamada de dibujo por frame. Las celdas se suben a una textura de datos (`RGB32UI`, un texel por celda: ranura del glifo + cubeta de luminancia, `fg`, `bg`) y un shader de fragmentos compone la pantalla completa. Solo se traducen las celdas que cambiaron y un frame sin cambios no sube nada.
- **Atlas de glifos dinámico**: cada codepoint recibe una ranura la primera vez que aparece y se rasteriza una sola vez, en 8 filas de luminancia que reproducen el pre-blend de texto de Skia (la máscara de cobertura depende de la luminancia del color); así el resultado es prácticamente idéntico al del renderer 2D (diferencia máxima de 1/255 en las animaciones; en modo imagen hasta 22/255 en una fracción muy pequeña de los píxeles). El color se aplica en el shader.
- **Respaldo**: el renderer 2D (`renderer.js`: sprites por carácter y color cuantizado, `drawImage` por celda cambiada) se usa cuando no hay WebGL2, la GPU es por software (SwiftShader, Microsoft Basic Render Driver…), el atlas no cabe en una textura o el contexto falla al crearse (el canvas bloqueado se reemplaza por uno nuevo). La consola registra `[newtab] renderer: gl|2d (motivo)`.
- **Robustez**: al perder el contexto WebGL se espera su restauración y se reconstruyen programa, texturas y atlas, repintando el último frame (la imagen estática vuelve a verse); un cambio de `devicePixelRatio` (otro monitor) se vigila con `matchMedia` y reconstruye la cuadrícula y el atlas aunque no haya `resize`.
- **Diff por celda**, equivalente al double buffer de cwork.
- La cuadrícula se calcula con `window.innerWidth/innerHeight`, contando las columnas y filas en píxeles de dispositivo (la celda se redondea a píxeles enteros, para que el canvas nunca exceda la ventana) y con un tope de 1024 por lado, igual que el motor. Un evento `resize` con *debounce* de 150 ms la recalcula (equivale a `ResizeObserver` para una página que ocupa toda la ventana).

### Controles de consumo

- Pausa total con `visibilitychange`: se cancela el `requestAnimationFrame` si la pestaña no está visible.
- FPS objetivo: `fire` 30, `rain` 30, `starfield` 30 (bajó de 60 a 30 con el renderer WebGL2, 2026-10-09), `pipes` 30 (arrancó en 60 el 2026-10-09 y el usuario lo bajó a 30 el mismo día por el consumo; ver `docs/measurements.md`), `dna` 30 (2026-10-09), `synthwave` 30, `compile_sim` 30, `pacman_install` 30, `pacman` 30, `dashboard` 5 (2026-10-09; nada se mueve de forma fluida, el reloj y las barras cambian 1 vez por segundo). Se ajustan con mediciones.
- `pipes`: 4 tuberías de colores dejan un rastro de glifos de caja (`║ ═ ╔ ╗ ╚ ╝`, `●` al nacer) hasta llenar el 80 % de las celdas; entonces el dibujo se queda quieto 4 segundos y se limpia para empezar de nuevo. La pausa se mide en segundos a partir del FPS objetivo (`Engine::set_fps(fps)` → `fps × 4` frames), así dura lo mismo a 60 que a 30 FPS.
- `dna`: una doble hélice 3D que gira (dos hebras de `●` verde y magenta en el lado cercano y `o` atenuada en el lejano, con puentes `─` cada 3 filas que alternan cian y amarillo) y, a su derecha, un registro "GENETIC SEQUENCING DATABASE" que suma una línea cada 25 frames (un 5 % son líneas rojas de mutación) y conserva las últimas `alto − 6`. En cuadrículas de más de 55 columnas la composición hélice + registro (101 columnas) se **centra horizontalmente** (desplazamiento `max(0, (ancho − 101)/2 − 11)`; 44 columnas en 211); en cuadrículas de menos de 112 columnas el registro (columnas 42 a 111) se recorta por la derecha, como en la versión original en una terminal angosta, y con 55 columnas o menos solo se dibuja la hélice, centrada.
- `synthwave`: escena retro en capas (sol con franjas y degradado rojo→dorado sobre el horizonte al 46 % de la altura, cuadrícula morada en perspectiva con 7 líneas `═` y 15 radiales `│╲╱`, tres ondas violeta/magenta/cian y el título `** SYNTHWAVE HORIZON **` en ASCII). Solo depende del contador de frames (sin azar). Aritmética `i32`, fase de las ondas en `f64`. Sin medir aún: el usuario dejó las mediciones de rendimiento para el final.
- `compile_sim`: simulación de una compilación de `cargo` (descarga 14 dependencias, las compila con barra de progreso, compila la app con un `warning` y un `error[E0599]` en el último archivo, parche, `cargo clean`, recompila, Linking y Finished; pausa 3 s y reinicia). Texto con color, historial de `alto − 5` líneas y línea activa debajo. Mantiene el ritmo original de 60 ticks por segundo con `set_fps` (2 pasos lógicos por frame a 30 FPS). Bloque centrado horizontalmente (ancho 106 = la línea más larga; margen `max(1, (ancho − 106)/2)`). Porcentajes en enteros. Sin medir aún.
- `pacman_install`: simulación de `pacman -Syu` (sincroniza bases, lista 9 paquetes, prompt, hasta 2 descargas en paralelo con velocidad aleatoria, 4 comprobaciones, 9 actualizaciones con barra, hooks, Finished; pausa 2 s y reinicia). Igual que `compile_sim`: ritmo de 60 ticks por segundo con `set_fps`, bloque centrado (BLOCK_W 77), números con decimales formateados con enteros. Desviación: la línea `Packages (9) …` (~185 columnas) se parte en 3 líneas de log (≤ 77 columnas) como hace pacman real. Sin medir aún.
- `dashboard`: reloj grande HH:MM:SS, barras de RAM y disco, una barra por núcleo de CPU (hasta 16, dos columnas con ancho > 70 y ≥ 4 núcleos) y un panel BROWSER (versión de Chrome, SO, núcleos, memoria JS, red, batería, idioma) en lugar de la tabla de procesos de la original (`chrome.processes` no existe en Chrome estable). Los datos son del navegador, no simulados: `extension/src/sysinfo.js` muestrea cada 1 s (solo con el modo activo y la pestaña visible) `chrome.system.cpu/memory/storage` (permisos nuevos en el manifest) y los pasa al WASM con `Engine::feed(Float32Array)` (layout: hora, min, seg, RAM usada/total GB, disco usado/total GB, N núcleos, cargas 0..1) y `Engine::feed_text(string)` (líneas "Etiqueta: valor", máx. 12 de 80 caracteres); `Animation` ganó `feed`/`feed_text` con implementación vacía por defecto. Datos saneados (NaN, negativos, slice corto). Desde 2026-10-09 la barra DISK no se muestra en escritorio: `chrome.system.storage.getAvailableCapacity` solo existe en ChromeOS, así que no hay espacio libre; bajo la barra de RAM se dibuja la fila `DISK  456.6 GB total` (solo capacidad, sin barra ni porcentaje; JS suma las unidades `fixed` sin duplicar las de igual tamaño y pasa el total en `feed[6]` con usado = 0) y, si falta `chrome.system` (extensión no recargada tras agregar permisos), la línea `System: reload the extension`. Sin formato de float (décimas con enteros). Sin medir aún.
- `pacman`: Pac-Man con piloto automático (laberinto de bloques aleatorios sin celdas selladas, puntos y energizantes, 4 fantasmas con objetivos distintos, vidas, niveles, puntaje). Mantiene el ritmo original de 60 ticks por segundo con `set_fps`. Desviaciones: el energizante baja por movimiento (dura 6 s reales, como dice el original y su HUD; la original lo bajaba por tick y duraba 1 s), salvaguarda que pasa de nivel tras 90 s sin comer, tamaño mínimo 12×10 (por debajo solo HUD), título/pie sin emoji (`RETRO PAC-MAN ARCADE` / `AUTOPILOT RUNNING`), los fantasmas nunca entran en muros. Los símbolos `ᗤ ᗢ ᗧ ᗣ ᗛ ⍛` están en una sola tabla (`GLYPHS`) por si hay que pasar a ASCII. Sin medir aún.
- Respeto a `prefers-reduced-motion`: se muestra un frame estático.
- Sin timers sueltos: el panel de KPI se actualiza por eventos, no por polling.

### Flujo de la imagen

`<input type=file>` → `createImageBitmap` → reducción en `OffscreenCanvas` al tamaño de `image_target_dims()` → `image_to_ascii()` → celdas empaquetadas (4 bytes por celda) → IndexedDB.

Sin la crate `image` el `.wasm` cabe en su tope. El empaquetado a 4 bytes por celda hace que la cuadrícula máxima (200×100) pese 80 KB; con 12 bytes por celda serían 240 KB, por encima del tope de 200 KB.

En cada pestaña nueva el renderer pinta la cuadrícula **una sola vez** y no arranca ningún bucle.

### Presupuesto de tamaño

| Pieza | Tope |
|---|---|
| `.wasm` (`opt-level="z"`, `lto`, `wasm-opt`) | ≤ 150 KB |
| JS + CSS + HTML | ≤ 70 KB (71,680 bytes) |
| Imagen ASCII guardada (cuadrícula, no la original) | ≤ 200 KB, una sola imagen |
| Estadísticas de teclas | solo conteos agregados, pocos KB |

El tope subió de 60 KB a 70 KB el 2026-10-09 (decisión del usuario) al agregar `dashboard`, cuando el margen quedó en 25 bytes. El tope de JS + CSS + HTML subió de 50 KB a 60 KB el 2026-10-09 (decisión del usuario) para que quepa el renderer WebGL2; medido ese día: 54,215 bytes (52.9 KB), margen de 7,225 bytes. Con `pipes` (2026-10-09): 54,630 bytes (margen 6,810) y `.wasm` de 31,441 bytes. Con `dna` (2026-10-09): 54,694 bytes (margen 6,746) y `.wasm` de 43,506 bytes.  Con `synthwave` (2026-10-09): JS+CSS+HTML 53.8 KB y `.wasm` 44.1 KB (cifras de `check:size`).  Con `pacman` (2026-10-09): `.wasm` 80.1 KB y JS+CSS+HTML 59.8 KB de 70. Con `dashboard` (2026-10-09): JS+CSS+HTML 59.8 KB de 60 (margen de ~0.1 KB tras el ajuste del disco: cualquier cambio de JS exigirá subir el tope o recortar) y `.wasm` 73.5 KB.

La cuadrícula de la imagen tiene un máximo (inicialmente 200×100 celdas). Cada animación nueva suma pocos KB de WASM.

## 6. Estadísticas de teclas (KPI)

### Captura (solo Chrome)

- Un content script escucha `keydown` y registra solo el **nombre de la tecla** (`e`, `space`, `ctrl+c`), nunca el contenido de campos ni texto acumulado.
- Se ignoran los campos `type=password`. La extensión no corre en incógnito por defecto.
- Los eventos se agrupan en memoria y se envían al service worker en lotes cada pocos segundos.

### Almacenamiento (`chrome.storage.local`)

```
{ "2026-10-08": { total, keys:{e:1045,…}, shortcuts:{"ctrl+c":12}, sites:{"github.com":340}, active_ms } }
```

- Un registro por día.
- Se guardan solo las **20 teclas, 20 atajos y 20 sitios** más usados por día (el resto se agrupa en `otros`), y se descartan los días de más de 90 días.
- Solo el dominio del sitio, no la URL completa.

### Métricas del panel

- Total de teclas de hoy.
- WPM con ventana deslizante de los últimos minutos activos (cada 5 teclas cuentan como una palabra).
- Top 5 teclas y atajos más frecuentes.
- Teclas por sitio.
- El panel se oculta con un atajo y su estado se recuerda.

### Permisos mínimos

`storage` y content script sobre `<all_urls>`. Sin `history`, sin `tabs` completo, sin red. La extensión no hace peticiones externas.

El manifest declara además `content_security_policy.extension_pages = "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'"`, que Chrome exige para instanciar WebAssembly en páginas de extensión. No añade ningún permiso.

## 7. Pruebas

- **Rust**: pruebas unitarias en `core/` con `cargo test`, con RNG de semilla fija.
- **JS**: pruebas de la lógica pura (agregación de conteos, WPM, poda a 90 días) con un runner ligero, sin DOM.
- **Manual guiado**: cargar la extensión descomprimida; comprobar los tres modos, el límite de tamaño de la imagen y la pausa al cambiar de pestaña.
- **Medición**: CPU y RAM por animación (Administrador de tareas de Chrome con `Shift+Esc` y panel Performance) contra las referencias de cwork.
- **Verificación de tamaño**: un chequeo que falla si se superan los topes de la sección 5.

## 8. Riesgos y supuestos

- El costo real en Chrome no está medido todavía; la estimación de 1 a 5 % de CPU y 30 a 60 MB de RAM por la pestaña es una hipótesis a verificar con el primer prototipo.
- Chrome puede suspender el service worker de MV3; el agrupado en lotes debe tolerar que el worker se reinicie.
- Los glifos del atlas dependen de la fuente monoespaciada disponible; se fijará una pila de fuentes y se comprobará el ancho de celda.
- Se asume que `fire`, `rain` y `starfield` no usan nada más de `crossterm` que `Color` (verificado por `grep`); si el portado revela otra dependencia, se ajusta aquí.

## 9. Fase posterior: estadísticas de todo el sistema

Fuera de v1. Opciones a evaluar entonces:

- Reutilizar `refs/kbd-stats` (Python con `keyboard`, `psutil` y `win32gui`), que ya registra teclas globales y app activa.
- Reescribirlo en Rust como programa local.
- Conectarlo con la extensión por Native Messaging o por un servidor local.

El formato de datos por día (sección 6) se diseña para poder fusionar esa fuente después.

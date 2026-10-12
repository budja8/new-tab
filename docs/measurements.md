# Mediciones

Máquina: 12 núcleos, Windows 11. Chrome 154.0.8037.98. Fecha: 2026-10-08.

Todas las cifras de CPU de la extensión son **% de un solo núcleo**, la misma unidad del Administrador de tareas de Chrome (por eso pueden pasar de 100 en total). Las de cwork, en cambio, están repartidas entre los 12 núcleos.

## Referencia: `cwork.exe` en release, 60 FPS

Solo el proceso, sin el costo del host de consola. CPU repartida entre 12 núcleos.

| Animación | CPU (12 núcleos) | RAM |
|---|---|---|
| fire | 0.93 % | 8.4 MB |
| rain | 0.19 % | 7.6 MB |
| starfield | 0.18 % | 7.6 MB |

## Extensión, medida con un arnés (CDP) en un Chrome aparte

Chrome con perfil temporal, ventana de 1540×731 (no los 1920×1080 pedidos: Chrome la recortó), pestaña de la extensión visible salvo donde se indica. Media de 3 ventanas de 10 s tras 8 s de calentamiento. Fuente: `scratchpad\sdd\measure-chrome-report.md` de la sesión (datos crudos en `scratchpad\measure-chrome\results.json`).

| Fondo | FPS objetivo | CPU pestaña | CPU GPU | CPU total Chrome | RAM pestaña (WS / privada) | RAM GPU (WS / privada) |
|---|---|---|---|---|---|---|
| fire | 30 | 63.4 | 70.3 | 141.7 | 105.9 / 43.8 MB | 150.4 / 122.1 MB |
| rain | 30 | 9.1 | 18.2 | 28.5 | 95.3 / 41.6 MB | 134.4 / 113.4 MB |
| starfield | 60 | 25.0 | 33.9 | 60.6 | 100.4 / 45.6 MB | 154.0 / 133.7 MB |
| fire, pestaña oculta | — | 0.07 | 0.01 | 0.56 | 83.4 / 29.9 MB | 128.1 / 108.4 MB |
| `about:blank` en reposo (control) | — | 0.2 | 0.27 | 8.4 (ruido de arranque; luego 3.3 y 0.9) | 76.3 / 33.5 MB | 106.7 / 88.7 MB |

No medidos: rain durante un destello de rayo, rain y starfield con la pestaña oculta, e imagen estática. El modo imagen falló en el arnés (la ruta larga del perfil temporal rompió IndexedDB), no en la extensión.

Observaciones del arnés:
- Con la pestaña oculta el consumo es ≈ 0: la pausa por `visibilitychange` funciona.
- En `fire` el costo de layout y estilo es 0. El script ocupa ≈ 28 % de un hilo, pero el total de tareas ronda el 59 %: el resto es pintura y envío del canvas. El proceso de GPU cuesta más que la pestaña en todos los modos animados.
- La frecuencia de refresco de la pantalla (rAF) fue de 60.67 Hz. No es la tasa de frames de la página.

## Extensión, lectura del Administrador de tareas de Chrome (del usuario, Chrome normal)

Fondo `fire`, pestaña visible, pantalla completa.

| Columna | Valor |
|---|---|
| CPU | 35.1 (% de un núcleo) |
| Huella de memoria | 60,968 K (≈ 59.5 MB) |
| Memoria de GPU | 25,677 K (≈ 25 MB) |

El arnés dio más CPU que esta lectura con una ventana más chica, por lo que probablemente infla las cifras (la causa no se pudo explicar). Para decidir, la lectura del usuario es la referencia en Chrome normal y las del arnés sirven para comparar modos entre sí.

### Misma lectura con el renderer WebGL2 (2026-10-09, usuario, Chrome normal)

Fondo `fire`, pestaña visible, misma pantalla. Una sola captura de la fila "Tab: Nueva pestaña".

| Columna | Antes (renderer 2D) | Ahora (WebGL2) | Cambio |
|---|---|---|---|
| CPU de la pestaña | 35.1 (% de un núcleo) | 8.8 | −75 % |
| Huella de memoria | 60,968 K (≈ 59.5 MB) | 32,908 K (≈ 32 MB) | −46 % |
| Memoria de GPU | 25,677 K (≈ 25 MB) | 19,412 K (≈ 19 MB) | −24 % |

La fila del Administrador de tareas es solo el proceso de la pestaña; el proceso de GPU es otra fila que no se capturó. 8.8 % de un núcleo equivale a menos del 1 % de una máquina de 12 núcleos.

## Renderer WebGL2 (2026-10-09)

El renderer por defecto pasó a ser `GlRenderer` (WebGL2, una llamada de dibujo por frame); el renderer 2D queda como respaldo. `starfield` bajó de 60 a 30 FPS.

### Método

Mismo arnés del spike del renderer: Chrome 154 aparte, perfil temporal nuevo por corrida, extensión cargada con `Extensions.loadUnpacked`, ventana de 1920×1080 en el monitor secundario (dpr 1): página de 1904×928, cuadrícula 211×58 = 12,238 celdas en todas las corridas. CPU = % de un núcleo (deltas de `cpuTime` de `SystemInfo.getProcessInfo`); 8 s de calentamiento y de 3 a 6 ventanas de 10 s. Corridas intercaladas en dos tandas (`ctrl, base, final, final, base, base, final, ctrl` y `final, base, final, base, final, ctrl`). `base` = copia de la extensión anterior (renderer 2D, `starfield` a 60 FPS); `final` = copia del código actual. GPU: Intel UHD (i7-1355U) por hardware, ANGLE D3D11.

Se descartaron dos mediciones por causas externas al código: `starfield` de una corrida de `base` (la ventana dejó de recibir `requestAnimationFrame`: 0 pasos/s) y una corrida de `final` cuya página no había inicializado a los 2.5 s de recargar (no se reprodujo en 25 aperturas seguidas). Quedan 5 corridas de `base` (4 en `starfield`) y 5 de `final`.

### Resultados (CPU % de un núcleo, media de todas las ventanas)

| Fondo | `base` pestaña / GPU / suma | `final` pestaña / GPU / suma | Cambio en la suma | Pasos/s `base` → `final` |
|---|---|---|---|---|
| fire | 87.5 / 98.4 / 185.9 | 10.9 / 13.8 / 24.8 | −87 % | 27.0 → 30.0 |
| rain | 14.7 / 24.9 / 39.6 | 8.7 / 12.4 / 21.2 | −46 % | 30.0 → 30.0 |
| starfield | 26.0 / 41.9 / 67.9 (60 FPS) | 9.5 / 13.4 / 22.9 (30 FPS) | −66 % | 59.8 → 30.0 |
| fire, pestaña oculta | 0.2 / 0.0 / 0.2 | 0.1 / 0.0 / 0.1 | ≈ 0 | 0 → 0 |
| `about:blank` (control) | 0.6 / 0.5 / 1.1 | — | — | — |

- Rango de la suma por ventana: `final` fire 20.1-33.6, rain 19.5-24.5, starfield 19.1-36.5; `base` fire 177.6-192.6, rain 32.7-51.3, starfield 44.3-92.3.
- `base` no llega a 30 pasos/s en `fire` (27.0 de media, 21.7 en la peor ventana): satura. `final` entrega 30.0 en los tres.
- Tiempo de tareas de la página en `fire`: 83.8 % → 5.8 %; script: 67.0 % → 3.2 %.
- Memoria privada de la pestaña: `base` 40-53 MB, `final` 29-37 MB. Memoria privada del proceso de GPU: `base` 110-132 MB, `final` 111-120 MB.
- Consola de `final`: solo `[newtab] renderer: gl (ANGLE (Intel, Intel(R) UHD Graphics …))`; sin errores ni excepciones en ninguna de las dos variantes.

**El arnés infla las cifras absolutas.** En esta sesión `base` fire dio 185.9 de suma, casi el doble que en el spike de la mañana (96.0) con la misma copia y la misma ventana; el Administrador de tareas del usuario mostró 35.1 % de pestaña en Chrome normal con el renderer anterior. Solo valen las razones: la pestaña de `fire` cuesta 0.125 veces lo de antes (87.5 → 10.9). Aplicada a la lectura del usuario, `fire` quedaría cerca de 4-5 % de un núcleo en la pestaña; era una extrapolación y la medición real en Chrome normal dio 8.8 % (razón 0.25 en vez de 0.125), así que la razón del arnés subestima el costo que queda. En el spike, el prototipo WebGL2 (`b`) costó 16.6 en `fire`, 14.7 en `rain` y 32.3 en `starfield` a 60 FPS (suma pestaña + GPU) con una razón frente a `base` de 0.17 en `fire`; la versión de producción da 0.13.

### Fidelidad (comparación píxel a píxel)

Con el mismo buffer de celdas y semilla fija, el `GlRenderer` y el `Renderer` 2D reales dan el mismo resultado: diferencia máxima 0/255 en `fire` (400 frames), 1/255 en `rain` (1500 frames, con el frame del rayo) y 0/255 en `starfield` (2400 frames, con los dos estados del HUD); 0 píxeles con diferencia > 8. En modo imagen (degradado de 160×60 con los 10 caracteres de la rampa y 9,600 colores de 24 bits) la diferencia máxima es 22/255 en el 0.026 % de los píxeles. Con una imagen de prueba más dura (200×100, colores de 24 bits pseudoaleatorios) la diferencia máxima sigue siendo 22/255, pero en el 0.0996 % de los píxeles (0.0844 % a dpr 1.25), por encima del 0.05 % que se usó como criterio con el degradado; la causa probable es un desacuerdo de la cubeta de luminancia de 3 bits con el cálculo de Skia (que usa el color linealizado) cerca de los bordes de cubeta; es cosmético y queda diferido. El `GlRenderer` da lo mismo de forma incremental que tras `invalidate()` (sin celdas obsoletas). A dpr 1.25 (emulado) los resultados son iguales.

## Tamaños (`npm run check:size`)

| Pieza | Medido | Tope |
|---|---|---|
| .wasm | 27.2 KB (27,889 bytes) | 150 KB |
| js+css+html (incluye el glue de `pkg/`) | 52.9 KB (54,215 bytes) con el renderer WebGL2; antes 38.9 KB | 60 KB (61,440 bytes; subió de 50 KB el 2026-10-09 para el renderer WebGL2). Margen: 7,225 bytes |
| imagen guardada (máx. 200×100 celdas × 4 bytes) | 80 KB | 200 KB |

## Conclusiones

- Con el renderer 2D, la hipótesis del spec (entre 1 y 5 % de CPU) **no se cumplía** en `fire` ni en `starfield`.
- El costo estaba en la pintura del canvas y en la GPU, no en el cálculo en Rust (que en cwork costaba menos del 1 %). El renderer WebGL2 lo confirma: en el arnés, `fire` baja de 185.9 a 24.8 (−87 %), `rain` de 39.6 a 21.2 (−46 %) y `starfield` de 67.9 a 22.9 (−66 %, en parte por pasar de 60 a 30 FPS), con el mismo resultado en pantalla en las animaciones (en modo imagen hay diferencias de color de hasta 22/255 en una fracción muy pequeña de los píxeles).
- En el arnés la hipótesis sigue sin cumplirse en números absolutos (`fire` 10.9 % de pestaña), pero el arnés infla las cifras. En Chrome normal, el Administrador de tareas (`Shift+Esc`) del usuario mostró 8.8 % de un núcleo para la pestaña de `fire` (antes 35.1 %): una mejora del 75 %, aunque todavía por encima del 1-5 % de la hipótesis original (que no precisaba si hablaba de un núcleo o de la máquina entera; sobre 12 núcleos son menos del 1 %). La extrapolación previa (4-5 %) fue optimista.
- Con la pestaña oculta el consumo sigue en ≈ 0.
- Confirmado por el usuario en su Chrome (2026-10-09): el consumo real con `Shift+Esc` (8.8 % frente a 35.1 %) y que la extensión funciona correctamente.
- Confirmado por el usuario (2026-10-09): el cambio entre escalas de Windows de 125 % y 150 % funciona y todo se ve como antes.
- Confirmado por el usuario (2026-10-09): el modo imagen (imagen ASCII estática) sigue mostrándose tras ocultar la pestaña y volver a mostrarla, con una imagen real (un logo a color). No queda ninguna comprobación pendiente de esta serie.

## Animación `pipes` (2026-10-09)

`pipes` ("Tuberías"): 4 tuberías de colores dejan un rastro de glifos de caja (`║ ═ ╔ ╗ ╚ ╝`, `●` al nacer); al llegar al 80 % de las celdas el dibujo se queda quieto 4 segundos (medidos en segundos: `set_fps(fps)` → `fps × 4` frames) y se limpia para empezar de nuevo. La página la corrió a 60 FPS al principio; el usuario la dejó en **30 FPS** (2026-10-09) tras ver estas mediciones. Las filas de 60 FPS de abajo se conservan como referencia.

### Método

Mismo arnés que en "Renderer WebGL2": Chrome 154 aparte con perfil temporal nuevo por corrida, extensión cargada con `Extensions.loadUnpacked` desde una **copia** de `extension/` (`pipes`) y otra copia idéntica salvo `FPS.pipes = 30` (`pipes30`); ventana de 1920×1080 en el monitor secundario (dpr 1), página de 1904×928, cuadrícula 211×58 = 12,238 celdas. CPU = % de un núcleo (deltas de `cpuTime` de `SystemInfo.getProcessInfo`), 8 s de calentamiento y ventanas de 10 s. Corridas intercaladas `ctrl, pipes, pipes30, pipes30, pipes, pipes, pipes30, ctrl`; en cada corrida de `pipes` se midió también `fire` como referencia de la misma sesión y `pipes` con otra pestaña delante (oculta). Corridas largas aparte (`pipesL, pipes30, pipesL`): 7 ventanas seguidas de 10 s que cubren un ciclo completo. Los pasos del motor y los ticks de `requestAnimationFrame` se cuentan envolviendo las exportaciones del WASM y `requestAnimationFrame` desde fuera, sin tocar el código.

A tamaño completo el llenado tarda ≈ 42 s a 60 FPS, así que las ventanas normales (de ≈ 11 s a ≈ 41 s de vida de la página) cubren el crecimiento del 19 % al 76 % de las celdas; la pausa y el reinicio solo entran en las corridas largas (pausa a los ≈ 43 s, reinicio a los ≈ 47 s) y en una corrida normal en la que la pausa llegó antes (ver abajo).

### Fidelidad (comparación píxel a píxel)

Con el `Engine` real (semilla fija 12345, cuadrícula 211×58, celda 9×16) se compararon el `Renderer` 2D y el `GlRenderer` reales sobre el mismo buffer: con `set_fps(1)` (pausa de 4 frames) durante 5,361 frames, dos ciclos completos (pausa en los frames 2526-2529 y 5057-5060, al 80.02 % de celdas; reinicio con 4 `●`), y con `set_fps(60)` durante 3,066 frames (pausa de exactamente **240 frames**, 2526-2765, y 300 frames del ciclo siguiente). Puntos de control cada 300 frames, al entrar en la pausa, a mitad de ella, en el reinicio y 1 y 60 frames después. Resultado: diferencia máxima **0/255** en los 48 puntos, 0 píxeles con diferencia > 8; el `GlRenderer` da lo mismo de forma incremental que tras `invalidate()` y que uno nuevo (0/255); el 2D también (0/255). Durante la pausa el canvas no cambia: la imagen del último frame de la pausa es idéntica (0/255) a la del primero, en los dos renderers. A dpr 1.25 emulado (216×58, celda 11×20) los resultados son iguales: 0/255 en todo, pausa de 4 y de 240 frames, canvas congelado. Aparecen los 7 glifos y los 7 colores de la paleta; consola limpia.

Capturas de la página real: a los ≈ 3 s se ven tuberías finas de doble línea con esquinas bien unidas; a los ≈ 15 s, unas 3,600 celdas; casi lleno y en la pausa, una trama densa de los 7 colores con algunos `●`; 3 s tras el reinicio, tuberías nuevas sobre fondo limpio. Entre celdas consecutivas de `═` y `║` se ve una línea de 1 px de fondo (la fuente no ocupa toda la celda); es igual en el renderer 2D y no afecta las esquinas.

### Pausa en la página real

Ventana pequeña (la mínima que permite Chrome: página de 500×168, cuadrícula de 55×10) para ver varios ciclos en 40 s. Dos fuentes: el hash de las celdas en cada paso del motor (instrumentado desde fuera) y capturas de pantalla del canvas cada ≈ 330 ms.

| FPS | Pausas en 41.5 s | Duración de la pausa (pasos idénticos) | Frames | Pasos/s y rAF/s durante la pausa | Ciclo completo | Capturas iguales seguidas |
|---|---|---|---|---|---|---|
| 60 | 7 | 4.000-4.049 s | 240 | 59.3-60.0 / 59.5-60.0 | 5.89-5.95 s | 12-13 capturas, congelado entre 3.56 y 4.23 s |
| 30 | 5 | 3.991-4.008 s | 120 | 29.9-30.1 / 59.9-60.1 | 7.73-7.81 s | 12-13 capturas, congelado entre 3.54 y 4.23 s |

A tamaño completo (60 FPS) la pausa empezó a los 42.5 s (80 % = 9,793 celdas) y duró 4.001 s. En todos los casos, tras la pausa queda el fondo limpio con 4 `●` y vuelve a crecer; 2,488 pasos sin errores, consola con solo la línea `[newtab] renderer: gl (…)`. En una corrida del arnés la pausa llegó antes, al 67 % (8,177 celdas, a los ≈ 36 s): el código pausaba también si las 4 tuberías morían en el mismo paso (defecto corregido: ahora solo pausa cuando las cuatro tuberías intentan reaparecer y ninguna encuentra una celda libre); la pausa duró ≈ 4 s.

### Resultados (CPU % de un núcleo)

| Caso | n ventanas | Pestaña | GPU | Suma | Suma min-max | Pasos/s | Memoria privada pestaña / GPU |
|---|---|---|---|---|---|---|---|
| `pipes` 60 FPS, crecimiento | 21 | 15.8 | 29.3 | 45.1 | 33.2-57.8 | 59.7 | 28-38 MB / 118-135 MB |
| `pipes` 60 FPS, ventanas con pausa y reinicio | 3 | 11.5 | 16.5 | 28.0 | 24.0-30.9 | 59.7 | — |
| `pipes` 30 FPS, crecimiento | 12 | 10.0 | 14.3 | 24.3 | 20.2-30.3 | 30.0 | 32-37 MB / 112-114 MB |
| `fire` 30 FPS (misma sesión) | 9 | 10.6 | 12.0 | 22.6 | 21.8-24.1 | 30.0 | 32-35 MB / 115 MB |
| `pipes`, pestaña oculta | 9 | 0.1 | 0.0 | 0.1 | 0.0-0.5 | 0 | 28 MB / 113-115 MB |
| `about:blank` (control) | 6 | 0.3 | 0.5 | 0.8 | 0.1-1.6 | — | — |

- Razones frente a `fire` de la misma sesión: `pipes` a 60 FPS cuesta 1.5× en la pestaña, 2.4× en la GPU y 2.0× en la suma; a 30 FPS, 0.94×, 1.19× y 1.08×.
- La variación es alta en `pipes` a 60 FPS (suma por ventana de 33 a 58; las corridas largas dieron menos que las intercaladas: 33-49 frente a 45-58). Las conclusiones usan razones, no absolutos.
- Se descartó una ventana: la última de una corrida larga (0 pasos/s y 0 rAF/s: Chrome dejó de dar frames a la ventana, causa externa, igual que en la medición del renderer).
- Tiempo de tareas de la página: `pipes` 60 FPS 7.4 %, 30 FPS 4.3 %, `fire` 5.7 %.
- Consola: solo `[newtab] renderer: gl (ANGLE (Intel, Intel(R) UHD Graphics …))` (una por carga); sin errores ni excepciones.

**El arnés infla las cifras absolutas** (en la medición del renderer `fire` dio 10.9 % de pestaña en el arnés y 8.8 % en el Administrador de tareas del usuario). En esta sesión `fire` dio 10.6 / 12.0 / 22.6, cerca de los 10.9 / 13.8 / 24.8 de la sesión anterior. Aplicando la razón a la lectura del usuario, `pipes` a 60 FPS rondaría 13 % de un núcleo en la pestaña, y el proceso de GPU, más del doble que con `fire`; a 30 FPS, lo mismo que `fire` (≈ 8-9 %). Es una extrapolación.

### Tamaño (`npm run check:size`)

`.wasm` 30.7 KB (31,441 bytes) de 150 KB. JS + CSS + HTML 53.3 KB (54,630 bytes) de 60 KB (61,440 bytes): margen de 6,810 bytes.

### Conclusión

La animación se ve y se comporta como se pidió: el GL y el 2D dan píxeles idénticos y la pausa dura 4.0 s tanto a 60 como a 30 FPS. A 60 FPS cuesta el doble que `fire` (suma pestaña + GPU) y la GPU carga 2.4 veces más; a 30 FPS cuesta lo mismo que `fire`. **Se recomienda 30 FPS** si se quiere el mismo consumo que los otros fondos; 60 FPS no rompe nada y solo se justifica si se prefiere el llenado más rápido (a 30 FPS la pantalla completa tarda ≈ 84 s en llenarse en vez de ≈ 42 s). Pendiente: la lectura del usuario en `Shift+Esc` con `pipes` a 60 FPS.

## Animación `dna` (2026-10-09)

`dna` ("ADN"): una doble hélice 3D que gira (hebras de `●` verde y magenta en el lado cercano, `o` atenuada en el lejano; puentes `─` cada 3 filas, alternando cian y amarillo) y, a la derecha, el registro "GENETIC SEQUENCING DATABASE" (una línea nueva cada 25 frames; un 5 % son líneas rojas `*** MUTATION DETECTED ...`; guarda las últimas `alto − 6`). La página la corre a **30 FPS**. Disposición B (elegida por el usuario): en cuadrículas de más de 55 columnas la composición hélice + registro (101 columnas) se centra con un desplazamiento `max(0, (ancho − 101)/2 − 11)`.

### Método

Mismo arnés que en `pipes`: Chrome 154 aparte con perfil temporal nuevo por corrida, extensión cargada con `Extensions.loadUnpacked` desde una **copia** idéntica de `extension/` (`dna`, comprobada con `diff -rq`); ventana de 1920×1080 en el monitor secundario (dpr 1), página de 1904×928, cuadrícula 211×58. CPU = % de un núcleo (deltas de `cpuTime` de `SystemInfo.getProcessInfo`), 8 s de calentamiento y ventanas de 10 s. Corridas intercaladas `ctrl, dna, dnaB, dna, dnaL, ctrl` y luego `ctrl, dnaLF, dnaFL, ctrl`: cada corrida `dna` mide `dna`, `fire` (referencia de la misma sesión) y `dna` con otra pestaña delante (oculta); `dnaB` lo mismo con `fire` primero; `dnaL`, `dnaLF` y `dnaFL` son corridas largas de 7 ventanas seguidas (de ≈ 11 a ≈ 81 s de vida de la página), las dos últimas con `fire` en el mismo Chrome. Pasos del motor y ticks de `requestAnimationFrame` contados envolviendo las exportaciones del WASM desde fuera, sin tocar el código.

En la pantalla completa el registro empieza vacío y se llena (52 líneas) a los ≈ 43 s; las ventanas normales (≈ 11-41 s) cubren el llenado (de ≈ 13 a ≈ 49 líneas) y las corridas largas cubren además 30-40 s con el registro lleno y desplazándose.

### Fidelidad (comparación píxel a píxel)

Con el `Engine` real (semilla 12345, `set_fps(30)`, `set_animation('dna')`) se compararon el `Renderer` 2D y el `GlRenderer` reales sobre el mismo buffer:

| Cuadrícula | Frames | Puntos de control | GL vs 2D | GL tras `invalidate()` / GL nuevo / 2D nuevo vs incremental |
|---|---|---|---|---|
| 211×58 (dpr 1, celda 9×16) | 2,500 (≈ 20 vueltas de la hélice, 100 líneas de registro) | 36 | 0/255, 0 px > 2 | 0/255 |
| 216×58 (dpr 1.25 emulado, celda 11×20) | 2,500 | 36 | 0/255 | 0/255 |
| 40×20 (estrecha: sin registro) | 600 | 14 (×2 dpr) | 0/255 | 0/255 |
| 90×30 (registro recortado a la derecha) | 1,200 | 23 (×2 dpr) | 0/255 | 0/255 |

Puntos de control: frames 1, 2, 10, 24-26 (primera línea), 63, 126, cada 100, la primera mutación (frame 425) y 25 frames después, el registro lleno (frame 1300 en 211×58, 600 en 90×30) y el primer desplazamiento. **Diferencia máxima 0/255 en todos**, 0 píxeles con diferencia > 2. Se ven 52 codepoints (incluidos `● o ─ ═`) y los 8 colores de la animación; consola limpia. Composición en el buffer (211×58): tinta de la columna 55 a la 155 (márgenes de 55 y 55 columnas), hélice 55-73, cabecera una sola vez en la columna 86, registro 86-155 (las líneas normales llegan a la 132; las de mutación y la cabecera, a la 155); los puentes alternan bien cian/amarillo en todas las filas. En 216×58 (dpr 1.25): márgenes 57 y 58. En 40×20 la hélice queda centrada (columnas 10-30). En 90×30 la composición empieza en la columna 11 y el registro se corta en la 89.

Capturas de la página real (≈ 3 s, ≈ 15 s, ≈ 45 s): la hélice se lee como una hélice que gira (el lado cercano en `●` brillante y el lejano en `o` oscura, cruces visibles), los puentes alternan cian y amarillo, el registro se lee bien (verde y cian, mutaciones en rojo) y la cabecera aparece una vez. A los 45 s el registro está lleno (52 líneas, 3 de mutación). El panel de KPI (arriba a la derecha, columnas 183-209, filas 0-14) no toca el registro en 1904 px.

### Disposición a otros anchos (página de 928 px de alto, dpr 1; ventana redimensionada con `Browser.setWindowBounds`)

| Ancho de página | Cuadrícula | Desplazamiento | Tinta (columnas) | Márgenes izq. / der. | ¿Panel KPI encima del registro? |
|---|---|---|---|---|---|
| 1904 px | 211×58 | 44 | 55-155 | 55 / 55 (495 / 500 px) | No |
| 1536 px | 170×58 | 23 | 34-134 | 34 / 35 | No |
| 1280 px | 142×58 | 9 | 20-120 | 20 / 21 | Solo en las columnas 113-120: la cola de la cabecera y de las líneas de mutación cuando pasan por las filas 3-14 |
| 1084 px | 120×58 | 0 | 11-111 | 11 / 8 | Sí (columnas 92-118, filas 0-14): cabecera y líneas de mutación; las normales (hasta la columna 88) no |
| 984 px | 109×58 | 0 | 11-108 | 11 / 0 (recortado) | Sí (columnas 80-107, filas 0-14): cabecera, mutaciones y el final de las líneas normales |
| 500 px | 55×58 | (estrecha) | 17-37 (solo hélice) | 17 / 17 | El panel tapa la hebra derecha en las filas 2-14 |

Desde 123 columnas los márgenes son iguales o difieren en 1. Entre 112 y 122 columnas el desplazamiento es 0: margen izquierdo de 11 columnas y derecho de 0 a 10 (en 120 columnas, 11 y 8); es lo que da la fórmula, no un defecto. Con menos de 112 columnas el registro se recorta por la derecha. El panel es translúcido, así que lo que queda debajo se ve atenuado. El registro sobrevive a los cambios de tamaño.

### Resultados (CPU % de un núcleo)

| Caso | n ventanas | Pestaña | GPU | Suma | Suma min-max | Pasos/s | Tareas / script de la página | Memoria privada pestaña / GPU |
|---|---|---|---|---|---|---|---|---|
| `dna` 30 FPS, todas | 30 | 6.3 | 9.3 | 15.6 | 13.7-22.3 | 30.0 | 2.8 % / 0.9 % | 30-36 MB / 120-121 MB |
| `dna`, llenado del registro (≈ 11-41 s) | 18 | 6.4 | 9.4 | 15.8 | 13.7-21.1 | 30.0 | | |
| `dna`, registro lleno (≈ 51-81 s) | 9 | 6.2 | 9.1 | 15.2 | 14.0-22.3 | 30.0 | | |
| `fire` 30 FPS (misma sesión) | 15 | 7.0 | 8.9 | 15.8 | 14.8-18.2 | 30.0 | 3.6 % / 1.8 % | 33-37 MB / 112-122 MB |
| `dna`, pestaña oculta | 9 | 0.1 | 0.0 | 0.1 | 0.0-0.3 | 0 | 0.1 % | 29-31 MB / 113-120 MB |
| `about:blank` (control) | 12 | 0.2 | 0.3 | 0.5 | 0.0-1.0 | — | — | 34-35 MB / 81-82 MB |

- Razones frente a `fire` de la misma sesión: pestaña 0.90×, GPU 1.04×, suma 0.99×. Llenar o desplazar el registro no cambia el costo.
- Una corrida larga (`dnaL`, sin `fire` en el mismo Chrome) dio más que las otras en sus 5 primeras ventanas (suma 17.7-22.3, luego 15.1 y 14.3); por eso se agregaron `dnaLF` y `dnaFL`, que dieron 13.7-17.0 con el registro lleno o llenándose y `fire` 15.0-18.2 en el mismo Chrome. La primera ventana de cada corrida suele ser la más alta.
- Pestaña oculta: 0 pasos y 0 rAF por segundo, `visibilityState = hidden` al principio y al final.
- Consola: solo `[newtab] renderer: gl (ANGLE (Intel, Intel(R) UHD Graphics …))` (una por carga); sin errores ni excepciones.

**El arnés infla las cifras absolutas y cambia entre sesiones**: esta vez `fire` dio 7.0 / 8.9 / 15.8, frente a 10.6 / 12.0 / 22.6 en la sesión de `pipes` (y 8.8 % de pestaña en el Administrador de tareas del usuario). Por eso solo vale la comparación con `fire` de la misma sesión: `dna` cuesta lo mismo que `fire`.

### Tamaño (`npm run check:size`)

`.wasm` 42.5 KB (43,506 bytes) de 150 KB; creció de 30.7 KB (31,441 bytes) a 42.5 KB, +12,065 bytes, que el revisor atribuye al formateo de enteros con `format!` (≈ 3-6 KB) y a `sin`/`cos` (≈ 3-5 KB). JS + CSS + HTML 53.4 KB (54,694 bytes) de 60 KB (61,440 bytes): margen de 6,746 bytes.

### Conclusión

GL y 2D dan píxeles idénticos (0/255) en todos los tamaños probados y la composición queda centrada en pantallas anchas (55 columnas a cada lado en 1904 px). A 30 FPS `dna` cuesta lo mismo que `fire` (suma 0.99×) y con la pestaña oculta ≈ 0: **el costo es aceptable**. A anchos de unos 1100 px o menos, el panel de KPI tapa parte de la cabecera y de las líneas largas del registro (el panel es translúcido).

## Animaciones nuevas: `synthwave`, `compile_sim`, `pacman_install`, `dashboard`, `pacman`, `mario` (2026-10-12)

Las mediciones se dejaron para el final por decisión del usuario. Reporte completo: [`docs/reports/measure-animations-report.txt`](reports/measure-animations-report.txt); datos crudos: [`docs/reports/data/measure-animations.json`](reports/data/measure-animations.json); capturas de cada modo en [`docs/reports/img/`](reports/img/).

### Método

Mismo arnés que en `pipes` y `dna` (`tools/cdp-harness/`, scripts `measure-anim.mjs`, `analyze.mjs` y `verify-anim.mjs`): Chrome **155** aparte con perfil temporal nuevo por corrida, extensión real cargada con `Extensions.loadUnpacked`, página de 1904×928 a dpr 1 (cuadrícula 211×58), 8 s de calentamiento y ventanas de 10 s, CPU en % de un núcleo. Tres corridas intercaladas con el orden rotado y `fire` en cada una, para comparar contra `fire` de la misma sesión (el arnés infla los absolutos; las conclusiones usan razones).

### Resultados (CPU % de un núcleo, media y rango)

| Modo | Ventanas | Pestaña | GPU | Suma | Razón vs `fire` (pestaña / GPU / suma) | Pasos/s | Memoria privada pestaña / GPU |
|---|---|---|---|---|---|---|---|
| `fire` (control) | 12 | 8.8 (7.9-10.1) | 10.6 (9.5-11.9) | 19.4 (17.4-22.0) | 1.00 | 30.0 | 34-35 MB / 116-134 MB |
| `synthwave` | 9 | 7.9 (6.4-10.9) | 11.3 (8.3-16.2) | 19.2 (15.1-27.1) | 0.90 / 1.06 / **0.99** | 30.0 | 33-36 MB / 122-141 MB |
| `compile_sim` | 12 | 6.5 (5.8-7.9) | 7.2 (5.7-9.3) | 13.7 (11.8-16.3) | 0.74 / 0.68 / **0.71** | 30.0 | 33-38 MB / 124-136 MB |
| `pacman_install` | 12 | 6.7 (5.4-8.0) | 8.3 (4.8-12.6) | 15.0 (10.2-20.6) | 0.76 / 0.78 / **0.77** | 30.0 | 36-37 MB / 125-136 MB |
| `dashboard` (5 FPS) | 9 | 4.7 (4.3-5.2) | 3.0 (2.8-3.4) | 7.8 (7.1-8.5) | 0.54 / 0.29 / **0.40** | 5.0 | 33-35 MB / 130-135 MB |
| `pacman` | 12 | 7.0 (5.5-8.9) | 8.2 (6.1-10.1) | 15.2 (11.6-18.9) | 0.79 / 0.77 / **0.78** | 29.6 | 34-39 MB / 119-139 MB |
| `mario` | 12 | 7.1 (5.9-8.2) | 10.5 (9.3-11.1) | 17.6 (15.2-19.2) | 0.81 / 1.00 / **0.91** | 30.0 | 34-39 MB / 121-139 MB |

- **Ninguna cuesta más que `fire`** y ninguna necesita bajar de FPS. `dashboard` es la más barata (0.40× la suma de `fire`); `synthwave` es la más cara, igual que `fire`.
- Tiempo de tareas / script de la página: `fire` 6.8 % / 3.5 %; `synthwave` 5.0 / 1.7; `compile_sim` 4.1 / 1.4; `pacman_install` 4.3 / 1.4; `dashboard` 3.0 / 0.7; `pacman` 4.7 / 2.1; `mario` 4.8 / 1.6.
- `pacman_install` alterna ventanas de ~10-12 % y de ~17-21 % (la fase de descarga redibuja más). `synthwave` tuvo una ventana puntual de 27.1 %.
- No se descartó ninguna ventana. Dos tuvieron una caída parcial de pasos/s (pacman en la corrida 1: 25.7; `dashboard` sin IPC en la corrida 3: 4.3) y se dejaron incluidas; quitarlas mueve las razones como mucho 0.015.
- `requestAnimationFrame` corre a ~60/s en todos los modos, aunque la animación avance a 30 o 5 pasos/s (el limitador descarta los frames sobrantes). La memoria de GPU crece a lo largo de cada corrida y no se puede atribuir a un modo.

### Con la pestaña oculta

| Modo | Pestaña % | GPU % | Pasos/s | rAF/s | Llamadas a `chrome.system.cpu.getInfo` en 10 s |
|---|---|---|---|---|---|
| `pacman` | 0.02 / 0.01 | 0.23 / 0.01 | 0 | 0 | 0 |
| `mario` | 0.04 / 0.01 | 0.17 / 0.01 | 0 | 0 | 0 |
| `dashboard` | 0.04 / 0.02 | 0.18 / 0.02 | 0 | 0 | 0 (también 0 en `memory` y `storage`) |

Con `dashboard` visible hubo 10 llamadas por ventana de 10 s (una por segundo); oculta no hay ninguna: el temporizador de muestreo se detiene.

### `dashboard`: datos reales

Leídos de las celdas del motor en la página real: `RAM [...] 60.6%  9.5 GB / 15.7 GB`, `DISK   456.6 GB total`, `CPU0` a `CPU11` con su porcentaje por núcleo y el panel BROWSER (Chrome 155, OS Windows, Cores 12, heap, red, batería, idioma). El costo del muestreo de `chrome.system.*` no se puede aislar con este arnés (7.8 con IPC frente a 7.0 sin IPC en la suma, pero 8.6 frente a 9.0 sumando todos los procesos: en sentidos opuestos); la latencia es de 2.5 ms por llamada. El costo de `dashboard` viene sobre todo del bucle `rAF` a 60/s.

### Fidelidad (comparación píxel a píxel GL vs 2D)

Con el `Engine` real (semilla 12345, `set_fps(30)`), el mismo buffer en ambos renderers, también tras `invalidate()` y con una prueba sintética que pinta todas las combinaciones (glifo, color de texto, fondo) vistas más los glifos exigidos:

| Modo | Frames | Puntos de control (211×58) | Diferencia máx. por canal | Píxeles > 2/255 | Sintético (combinaciones / máx.) |
|---|---|---|---|---|---|
| `synthwave` | 600 | 15 | 0 | 0 | 38 / 0 |
| `compile_sim` | 1800 | 36 | 0 | 0 | 254 / 0 |
| `pacman_install` | 1500 | 32 | 0 | 0 | 203 / 0 |
| `dashboard` (datos fijos) | 400 | 11 | 0 | 0 | 137 / 0 |
| `pacman` | 1500 | 33 | 0 | 0 | 155 / 0 |
| `mario` | 1500 | 33 | **1** | 0 | 340 / 1 |

A 40×20 los resultados son idénticos. `pacman`: `ᗤ ᗢ ᗧ ᗣ ᗛ ⍛` aparecen y dan 0/255. `mario`: `▄ ▀ █ ░ ◢ ◣` dan 1/255 en celdas con fondo (cielo `#5c94fc`, suelo `#c84c0c`, negro): redondeo de la mezcla glifo/fondo, dentro de lo esperado.

Consola: en las 33 cargas solo `[newtab] renderer: gl (ANGLE (Intel, Intel(R) UHD Graphics …))`; sin errores ni excepciones.

### Observaciones

- El panel de KPI tapa `WORLD 1-1` en `mario`, y la línea `POWER` y parte del laberinto en `pacman` (cosmético; el usuario ya decidió no tocar la opacidad de los paneles).
- Posible mejora, no aplicada: `dashboard` mantiene `rAF` a 60/s para pintar 5 frames por segundo; un temporizador a 5 Hz bajaría algo más la pestaña.
- Quedan perfiles temporales de Chrome en `C:\ntmeas` que Windows no dejó borrar (basura segura de borrar a mano).

### Tamaños al corte (2026-10-12)

`.wasm` 87.8 KB de 150 KB (con `mario`); JS + CSS + HTML 59.9 KB de 70 KB (el tope subió de 60 a 70 KB al agregar `dashboard`).

### Conclusión

Las seis animaciones nuevas cuestan entre 0.40× y 0.99× lo que `fire` (suma pestaña + GPU) con el mismo renderer, todas se detienen con la pestaña oculta, GL y 2D coinciden (máx. 1/255) y la consola no muestra errores. Con `fire` en 8.8 % de un núcleo en el Administrador de tareas del usuario, las demás deberían quedar por debajo.

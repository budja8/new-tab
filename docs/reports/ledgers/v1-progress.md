# SDD ledger — plan: docs/superpowers/plans/2026-10-08-new-tab-ascii.md

Spec: docs/superpowers/specs/2026-10-08-new-tab-ascii-design.md (alcanzable; actualizado con las 4 desviaciones del plan).
Raíz del proyecto: C:\Users\Daniel Rodriguez\Documents\Tests\owns\new-tab (sin repo git).
Workspace SDD: C:\Users\DANIEL~1\AppData\Local\Temp\claude\C--Users-Daniel-Rodriguez-Documents-Tests-owns-new-tab\551d3df9-9fd5-496e-b351-05fc26dbcc22\scratchpad\sdd

## Rulings previos a la ejecución

- Ruling: sin git (decisión del usuario: "no hagamos ningún commit por ahora porque no hay repo") → no hay worktree, commits ni `review-package`. Los reviewers leen los archivos listados en el reporte del implementador, y el ledger registra tareas completas en vez de SHAs. — Costo si me equivoco: las revisiones ven archivos enteros en vez de diffs (menos precisas); el usuario puede hacer `git init` más adelante.
- Ruling: modelos — implementadores y reviewers en `sonnet` (el plan trae el código completo, pero no se ha compilado nunca: se espera depurar errores de borrow/tipos); revisión final en `opus`. — Costo: más caro que haiku; evita rondas de fallo por turnos extra.
- Ruling: la tarea 1 instala `wasm-pack` y el target `wasm32` en la máquina del usuario. — El plan aprobado lo dice explícitamente (Step 1). — Costo: cambio global en `~/.cargo`, reversible con `cargo uninstall wasm-pack` / `rustup target remove`.
- Ruling: la tarea 11 (verificación manual en Chrome) no la puede ejecutar un subagente (requiere el navegador). Se ejecuta lo automatizable (`npm run test`, `check:size`, `build:wasm`) y se entrega al usuario la lista manual de los pasos 1-6. — Costo: las mediciones de CPU/RAM quedan pendientes del usuario.

## Tabla de pre-vuelo (conflictos entre tareas)

| Par / tarea | Produce → consume | Hallazgo |
|---|---|---|
| T1 → T2,3,4,5,6 | `Rng`, `Grid`, `rgb` (const fn), `CELL_STRIDE`, `NO_COLOR` | Firmas coinciden en todas las tareas. |
| T1 `lib.rs` → T2..T6 | cada tarea reescribe `lib.rs` | T2 deja `animation, grid, rng`; T5 `animation, ascii, grid, rng`; T6 agrega `engine`. Orden alfabético consistente; cada tarea debe conservar los `pub mod` previos. |
| T2 `animation/mod.rs` → T3, T4 | T3/T4 agregan `pub mod rain;`/`starfield;` | Sin conflicto; solo añadir líneas. |
| T2,3,4 → T6 | `FireAnimation::new(w,h,seed)` etc. | Coincide con `Engine::set_animation`. |
| T5 → T6 | `target_dims`, `image_to_packed`, `packed_glyph`, `PACKED_STRIDE` | Coincide con `engine.rs`. |
| T6 → T10 | `Engine`, `cell_stride`, `image_target_dims`, `image_to_ascii` | Coincide con `newtab.js`. |
| T7 → T8 | `applyBatch`, `emptyDay`, `pruneDay`, `summarize`, `cutoffKey`, `dateKey` | `store.js` importa exactamente esos nombres. |
| T7 `keyname.js` → T8 `keycount.js` | `globalThis.NT.keyName` | Coincide; manifest carga `keyname.js` antes (T10). |
| T8 → T10 | `createKpiStore(area, now)` | `newtab.js` la usa con `chrome.storage.local`. |
| T9 → T10 | `diffCells`, `createLimiter`, `loadSettings/saveSettings`, `MAX_COLS/MAX_ROWS/loadImage/saveImage/validateImage` | Coinciden. |
| T6 `check-size` → T10 | cuenta `.js/.css/.html` bajo `extension/` | T10 Step 6 corre `check:size`; si el tope de 50 KB se excede se ajusta código (no el tope). |
| T1 | texto vs. código | `package.json` usa `node --test "tests/**/*.test.js"`; en T1 aún no hay carpeta `tests/` → `test:js` falla hasta T6/T7. Solo se ejecuta `test:rust` en T1–T5. |
| T2 | prueba vs. código | `heat_reaches_the_top_row_over_time`: 8 updates, 40 columnas, semilla 1. Aserción `lit > 0`. Coherente. |
| T3 | prueba vs. código | `lightning_flashes_the_background` espera `lightning_timer == 15` tras el disparo; el código lo arma en `update`. Coherente. |
| T4 | prueba vs. código | `warning_state_shows_the_hud_box` asume 80×24 (HUD en fila 11/12). Cálculo verificado: 38 caracteres, px=21. |
| T5 | prueba vs. código | La aserción `(1,1,200,100) → (182,100)` ya está corregida en el plan. |
| T6 | texto vs. archivos | `wasm-smoke` usa `init({ module_or_path })`; puede fallar según versión de wasm-bindgen → fallback documentado en el paso 9. |
| T6 | `wasm-opt` | `--all-features` puede no ser aceptado → fallback documentado (`wasm-opt = false`). |
| T7,8,9 | pruebas vs. código | Revisadas a mano: contadores de pruebas (9, 17, 9, 6, 6, 5, 6) coinciden con el plan. |
| T10 | sin pruebas automáticas | Declarado en el plan; se cubre con T11 manual. |
| T11 | requiere navegador | Ver ruling arriba. |

Pre-vuelo: sin conflictos que impidan despachar T1.

## Progreso
Task 1: minor (deferred): rng.rs:40 `hi - lo` puede desbordar i32 en rangos > i32::MAX (solo debug; irrelevante en uso real).
Task 1: minor (deferred): rng.rs:45 `range_incl_i32(lo, i32::MAX)` satura y nunca devuelve MAX.
Task 1: minor (deferred): grid.rs `w*h*3` en usize puede desbordar en wasm32 con u16 enormes (65535×65535). → Task 6: `Engine::new`/`resize` deben acotar el tamaño (la cuadrícula real es ≤ unos cientos de celdas). Llevar al dispatch de T6.
Task 1: complete (review clean; controller verificó `cargo test`: 16/16 pasan, wasm32 + wasm-pack 0.15.0 instalados)
Task 2: minor (deferred): la prueba de tamaños degenerados no cubre el cambio de tamaño de no-cero a cero (update(10,6) → update(0,0)); el código lo maneja.
Task 2: complete (review clean; cargo test 21/21, sin warnings)
Task 3: minor (deferred): la prueba de tamaños degenerados no asegura que cayera algún rayo en cada tamaño (depende de la semilla fija; ~12 rayos esperados por tamaño).
Task 3: complete (review clean; cargo test 26/26, sin warnings reportados por el implementador)
Task 4: minor (deferred): la prueba de `speed` no cubre la fórmula de Warning; la de hyperdrive solo comprueba que haya algún glifo de estela (no el cabezal blanco ni la elección de glifo).
Task 4: complete (review clean; cargo test 32/32, sin warnings en todo el crate)
Task 5: minor (deferred): `use crate::grid::rgb;` duplicado en el módulo de pruebas (viene del brief); `max_grid_fits_in_the_storage_budget` solo afirma sobre constantes.
Task 5: complete (review clean; cargo test 43/43, sin warnings)
Task 6: Ruling: agregar a `Engine::new` y `Engine::resize` un tope `MAX_GRID_DIM: u16 = 1024` por lado (más una prueba) — cierra el minor diferido de la T1 (`w*h*3` desbordaba usize en wasm32 con u16 enormes; la cuadrícula real es ≤ unos cientos de celdas, 4K/9px ≈ 426 columnas) — costo si me equivoco: una ventana de más de 9216 px de ancho vería la cuadrícula recortada; es una constante de una línea.
Task 6: ruling aplicado: tope MAX_GRID_DIM=1024 en Engine::new/resize (+1 prueba → 55 pruebas de Rust).
Task 6: Ruling: la prueba `resize_keeps_the_image_and_resizes_the_buffer` del plan tenía una coordenada errónea ((10,4) → debe ser (9,3): (20-1)/2=9, (8-1)/2=3); el implementador corrigió solo la aserción — defecto del plan, sin costo.
Task 6: minor (deferred): `rejects_images_with_wrong_length_or_zero_size` aserta `image.is_none()` cuando nunca hubo imagen; falta el caso "una carga fallida no cambia una imagen ya cargada" (el código es correcto).
Task 6: minor (deferred): `fit` no tiene prueba aislada con proporciones extremas (el análisis lo da por seguro).
Task 6: fix round 1/5 (1 addressed, 0 open — doc de validez de `cells_ptr`/`cells_len`; solo comentarios)
Task 6: complete (review clean tras 1 ronda; controller verificó: cargo test 55/55 sin warnings, test:js 3/3 humo WASM, check:size wasm 27.2 KB/150, js+css+html 12.1 KB/50)
Task 7: minor (deferred): `applyBatch` no limita la longitud de clave/atajo/host ni valida que `batch.keys` sea un objeto plano (un array crearía claves '0','1'). El mensaje solo llega de nuestro content script (background.js verifica sender.id), pero conviene que el final review lo valore.
Task 7: minor (deferred): `cutoffKey` resta 90*24h fijos; cerca de medianoche con cambio de horario (DST) podría caer en el día contiguo (la prueba usa las 15:00).
Task 7: minor (deferred): `pruneTop` devuelve el mapa sin tocar si es pequeño → claves RESERVED ya guardadas sobrevivirían (no ocurre: applyBatch nunca las acepta).
Task 7: minor (deferred): `active_ms` podría contar dos veces un minuto si el reloj retrocede; WPM cae a 0 tras medianoche (recent es por día).
Task 7: minor (deferred): faltan pruebas de `otros` como clave en `batch.keys` y de `keys` no objeto.
Task 7: ⚠️ resuelto: AltGr real en Chrome/Windows → verificación manual T11 (paso 4.3); omisión de `type=password` → código de T8 (revisión de T8).
Task 7: complete (review clean; implementador: npm run test:js 29/29)
Task 8: Ruling: `keycount.js` debe encadenar `.catch(() => {})` a `chrome.runtime.sendMessage(...)` (devuelve una promesa en MV3; el rechazo asíncrono por contexto invalidado/sin receptor no lo atrapa el try/catch síncrono y ensuciaría la consola de cada página) — defecto del plan, mantener también el try/catch para el caso síncrono — costo si me equivoco: ninguno (ignora un error que ya se descartaba).
Task 8: minor (deferred): si `set` funciona pero `removeOld` falla, `record` rechaza aunque el dato se guardó (la poda se reintenta en el siguiente día nuevo).
Task 8: minor (deferred): `get(null)` carga todo el storage una vez por día nuevo (incluye ajustes; la imagen está en IndexedDB, no aquí).
Task 8: minor (deferred): un lote acumulado antes de medianoche se atribuye al día nuevo (se lee `now()` al escribir); el temporizador de 3 s no se limpia tras el flush de `pagehide` (inofensivo); `message.batch` sin validación de forma (limitado por sender.id).
Task 8: ⚠️ resuelto: orden de carga keyname.js → keycount.js y `all_frames:false` → manifest de T10 (el revisor de T10 lo verifica) y T11 manual; semántica real de chrome.* → T11 manual.
Task 8: complete (review clean; implementador: npm run test:js 38/38, node --check OK)
Task 9: Ruling: `diffCells` del plan comparaba 3 valores fijos e ignoraba el parámetro `stride` (la prueba "respeta un stride distinto" del propio plan fallaba); el implementador lo reemplazó por un bucle sobre `stride` valores con salida temprana — defecto del plan; costo si me equivoco: el bucle interno es algo más lento que 3 comparaciones fijas en una ruta caliente (lo valora el reviewer).
Task 9: Ruling: `imagestore.withStore` debe resolver en `tx.oncomplete` y rechazar en `tx.onabort`/`tx.onerror` (no en `req.onsuccess`) — el finding Important del reviewer es real aunque el brief lo escribió así: un fallo de commit (cuota) pasaría como éxito y la UI diría "Imagen guardada" — costo si me equivoco: ninguno funcional (la promesa resuelve unos ms después).
Task 9: minor (deferred): `createLimiter` no protege fps <= 0 / NaN / Infinity (los llamadores usan constantes 30 y 60); sentinel `next !== 0`; `openDb` sin `onblocked`/`onabort`; `diffCells` ignora una celda parcial si length % stride != 0; falta prueba directa de `loadImage` con registro corrupto (no hay IndexedDB en Node).
Task 9: fix round 1/5 (1 addressed, 0 open — imagestore.withStore resuelve en tx.oncomplete y rechaza en onabort/onerror)
Task 9: complete (review clean tras 1 ronda; controller verificó npm run test:js)
Task 10: minor (deferred) — MARCADOS PARA CORREGIR ANTES DE CERRAR (los ve la revisión final): (#2) `Renderer.fit()` no acota cols/rows a 1024 como el Engine → con innerWidth/9 > 1024 (zoom 25-33 % en monitor 2560+ px) `engine.width() !== renderer.cols` y el guard descarta todos los frames: canvas en blanco sin error; (#3) cols/rows se derivan de cellW/cellH pero el tamaño CSS real es round(cellW*dpr)/dpr → a dpr 1.5 el canvas es más ancho que la ventana (se recortan ~5 columnas y la imagen queda descentrada), a dpr 1.25 sobra una franja: derivar cols/rows de pw/dpr y ph/dpr.
Task 10: minor (deferred): (#4) sin listener de `change` en prefers-reduced-motion; (#5) cambio de DPR sin evento resize deja sprites con DPR viejo; (#6) `refreshKpi()` y `saveSettings` en `setKpiVisible` sin catch; (#7) `e.key.toLowerCase()` lanza si e.key es undefined (autofill) y la tecla K repetida parpadea el panel (sin e.repeat); (#8) el panel KPI no se refresca sin eventos (WPM se queda viejo, tras medianoche muestra el día anterior) → recordar en T11; (#9) mensajes de error de IDB en inglés, `createImageBitmap(file)` decodifica a resolución completa (usar resizeWidth/resizeHeight); (#10) el resize en modo estático relee IDB y reejecuta calentamiento en vez de solo `paint()`.
Task 10: ⚠️ resuelto: carga del manifest, CSP/WASM, métricas de glifos, timing real, FPS/CPU → T11 manual. verify10.mjs necesita un argumento (el reviewer no pudo correrlo; el implementador sí lo corrió con su argumento).
Task 10: minor (deferred): el handler de `image-file` muestra "Imagen guardada." tras `await applyMode('image')` aunque esa llamada haya quedado obsoleta (cambio de modo durante la importación); con imagen guardada inválida y reduced-motion cada resize reejecuta el fallback.
Task 10: fix round 1/5 (1 addressed, 0 open — `modeGen` en applyMode: una llamada obsoleta no tiene efectos tras su await; 5 escenarios revisados)
Task 10: complete (review clean tras 1 ronda; controller verificó check:size js+css+html 38.4 KB/50, wasm 27.2 KB/150; implementador: test:js 61, cargo test 55, verify10 ALL CHECKS PASSED)
Task 11: pendiente — parte automatizable (test/check:size/build) + docs/measurements.md; los pasos manuales de Chrome los hace el usuario.
FINAL REVIEW (opus): veredicto "sí, una vez corregidos I-1 e I-2". Hallazgos: I-1 `Renderer.fit()` sin tope de 1024 (canvas en blanco con zoom 25 % en monitor ancho); I-2 cols/rows desde el tamaño CSS de celda vs. `round(cellW*dpr)/dpr` (recorta/deja franja con escala 125 %/150 %). Minors: M-1 top-20 se "bloquea" durante el día (requeriría nota de spec); M-2 HUD de starfield usa ⚠ + U+FE0F (glifo ancho recortado); M-3 el mensaje de fallback a fuego no se ve (#message está dentro de #settings oculto); M-4 asignaciones menores por frame; M-5 el spec dice ResizeObserver pero el código usa `resize` con debounce.
Final: Ruling: una sola ronda de corrección final con I-1 + I-2 (+ prueba pura `gridFor` en Node) — la regla del skill es UN dispatch con la lista completa y UNA re-revisión; M-1..M-4 se difieren porque ninguno bloquea la corrida manual (M-2 y M-3 son cosméticos/raros; M-4 pide medir primero; M-1 pide decisión de spec) — costo si me equivoco: el usuario podría ver el HUD de starfield con un glifo recortado o no ver por qué aparece el fuego si elige "Imagen" sin tener una guardada.
Final: Ruling: M-5 se resuelve actualizando el spec (ResizeObserver → `resize` con debounce de 150 ms, equivalente) en vez de cambiar el código — costo si me equivoco: ninguno.

# SPIKE: diseño del renderer vs CPU (BASE / A buffer de píxeles / B WebGL2)

Fecha 2026-10-09. DESECHABLE: el proyecto no se modificó. Prototipos, datos crudos y capturas en `scratchpad\spike\` (README.txt dice THROWAWAY SPIKE). Resumen condensado por el controlador a partir del reporte del subagente (que no pudo escribir este archivo); los datos crudos completos están en `spike\results-main.json`, `results-a2.json`, `tables-main.md`, `tables-a2.md`, `gpuinfo.json`, `verify-results.json` y 18 capturas en `spike\shots\`.

## Método
- Chrome 154 con `--remote-debugging-pipe`, perfil nuevo y corto por corrida (`C:\ntspike\...`), extensión cargada con `Extensions.loadUnpacked`, modo por `chrome.storage.local`. Ventana de 1920×1080 en el monitor secundario (dpr 1): página real 1904×928 → cuadrícula 211×58 = 12,238 celdas, igual en todas las corridas.
- CPU = % de un núcleo, deltas de `cpuTime` de `SystemInfo.getProcessInfo`; 8 s de calentamiento y ≥ 3 ventanas de 10 s (hasta 6 si la dispersión > 30 %). Corridas intercaladas: ctrl, base, a, b, base15, b, a, base, base15, ctrl.
- Frecuencia entregada medida con un script idéntico en todas las variantes: fire y rain 30.0 pasos/s, starfield 60.0, base15 15.0.

## GPU del arnés
Todo por hardware: Intel UHD (i7-1355U) vía ANGLE D3D11, Skia Ganesh GL; `2d_canvas`, `gpu_compositing`, `rasterization` y `webgl` habilitados; sin SwiftShader. El alto costo de GPU de BASE viene de miles de operaciones 2D pequeñas por frame, no de renderizado por software.

## Resultados (CPU % de un núcleo, media de todas las ventanas: pestaña / GPU / suma)

| Variante | fire | starfield (60 FPS) | rain |
|---|---|---|---|
| BASE (actual) | 47.0 / 48.9 / 96.0 | 20.0 / 22.6 / 42.6 | 10.2 / 12.3 / 22.5 |
| A (1 `putImageData`) | 20.8 / 15.0 / 35.9 | 42.6 / 36.8 / 79.4 (peor) | 14.0 / 14.4 / 28.4 (peor) |
| B (WebGL2) | 7.4 / 9.2 / 16.6 | 13.7 / 18.6 / 32.3 | 6.4 / 8.3 / 14.7 |
| BASE a 15 FPS | 20.2 / 20.5 / 40.7 | — | — |

- Control `about:blank`: < 1 % en pestaña y GPU.
- En estado estable (sin la primera ventana, siempre alta en BASE fire: 55-81 %): BASE fire 39.9/41.9/81.8; B fire 7.0/8.4/15.4.
- B frente a BASE: fire −83 %, starfield −24 %, rain −35 %. Tiempo de JS en fire: 34 % → 1.7 %.
- A pierde en escenas dispersas: el rectángulo sucio es casi toda la pantalla (≈ 7 MB × 60/s) y la memoria del proceso GPU sube a 0.57-0.97 GB. Una variante con teselas (A2) fue peor en todo.
- Bajar fire a 15 FPS recorta ≈ 55 %; B a 30 FPS sigue siendo 2.5 veces más barato que eso. Se pueden combinar.

## Fidelidad visual (comparación píxel a píxel, buffers idénticos y semilla fija)
- A coincide con BASE (diferencia máxima 2/255), sin celdas obsoletas.
- B, primera versión: no equivalente (los glifos salían más delgados en rojos oscuros). Causa: Skia aplica un pre-blend de texto con 8 máscaras de cobertura según la luminancia de 3 bits del color. Corrección: atlas de 8 filas y el shader elige la fila según `fg`. Con eso B es idéntico en fire y rain.
- Diferencias restantes de B: 2 celdas del HUD de starfield (⚡/⚠, que BASE dibuja como emoji a color y B tiñe como máscara) y, en modo imagen, 0.03 % de píxeles con diferencia de hasta 22/255.
- Sin errores ni excepciones de consola en ninguna variante (9 páginas de verificación, 37 escenarios medidos).

## Memoria (MB, final de escenario)
Privada de la pestaña: BASE 42-53, A 40-55, B 34-40. Privada de GPU: BASE 106-119, B 111-114, A 570-965.

## Límites del arnés
CDP conectado e instrumentación inflan todas las variantes por igual; las cifras absolutas no son las del usuario (su Administrador de tareas con fire: 35.1 % pestaña, 60,968 K, GPU 25,677 K). Comparar razones, no valores absolutos. No se midió el monitor principal a 125 %. Una sola máquina (iGPU Intel).

## Riesgos de B para llevarlo a producción
- El atlas del spike es fijo (ASCII 33-126 más `· █ ║ ─ │ ╲ ╱ ▄ ▀ ● ⚠ ⚡`): producción necesita atlas dinámico (codepoint → ranura al verlo por primera vez), ≈ 40-60 líneas.
- Emoji a color (⚡/⚠): ruta RGBA aparte, ≈ 15 líneas (o evitar emojis en el HUD).
- La fidelidad de bordes depende del pre-blend de Skia, verificado solo en Windows / Chrome 154 (cosmético si cambia).
- Pérdida de contexto WebGL y cambio de dpr entre monitores: reconstruir (≈ 20 líneas).
- Sin WebGL2 o con GPU solo por software (SwiftShader) no hay ganancia: mantener el renderer 2D actual como respaldo.
- Tamaño: prototipo +53 líneas / +2.7 KB; producción ≈ +5-7 KB (150-200 líneas) → ≈ 44-46 KB de 50 KB.
- starfield a 60 FPS sigue costando 32 % con B; a 30 FPS rondaría la mitad.

## Recomendación
Adoptar B (WebGL2, una llamada de dibujo por frame), conservando el renderer 2D como respaldo. Rechazar A. Considerar starfield a 30 FPS.

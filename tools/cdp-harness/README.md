# Arnés de medición con Chrome (CDP)

Scripts usados durante el desarrollo para medir CPU/memoria y comparar píxeles con un Chrome aparte. **Son de referencia, no genéricos**: se escribieron para cada medición y tienen rutas y variantes fijas; hay que adaptarlos (ver abajo). Se copiaron aquí porque vivían en una carpeta temporal de la sesión.

| Archivo | Para qué sirve |
|---|---|
| `lib.mjs`, `lib2.mjs` | Utilidades: lanzar Chrome headful con `--remote-debugging-pipe --enable-unsafe-extension-debugging` y un perfil nuevo, adjuntar a un objetivo CDP, evaluar JS en la página, `Browser.setWindowBounds`, abrir la nueva pestaña de la extensión, leer memoria de procesos. `lib2.mjs` es la versión más reciente. |
| `measure-dna.mjs` | Medición de CPU de una animación contra `fire` en la misma sesión (variantes intercaladas, ventanas de 10 s tras 8 s de calentamiento, CPU por `SystemInfo.getProcessInfo` en % de un núcleo). Es la plantilla para medir las animaciones pendientes. |
| `verify-dna.mjs` | Comparación píxel a píxel entre el renderer WebGL2 y el 2D con el `Engine` real y una semilla fija. |
| `measure-anim.mjs`, `analyze.mjs` | Medición de las seis animaciones nuevas (2026-10-12): tres corridas intercaladas con `fire` en cada una, pestaña oculta, capturas. `analyze.mjs` resume los resultados. |
| `verify-anim.mjs`, `vext/` | Fidelidad GL vs 2D de las animaciones nuevas (`vext/` es la página auxiliar de verificación). |
| `page-tests.mjs` | Pruebas de la página real: fallback sin WebGL2, GPU por software, contexto perdido, etc. |
| `diag-sys.mjs` | Diagnóstico de `chrome.system.*` dentro de la página de la extensión. |

## Cómo usarlo

1. Requiere Node 20+ y Chrome instalado (ruta en `CHROME` dentro de `lib2.mjs`: `C:/Program Files/Google/Chrome/Application/chrome.exe`).
2. Construir antes: `npm run build:wasm` en la raíz.
3. Los scripts asumen las rutas de la sesión original (la carpeta de la extensión, perfiles en `C:/nt*`, una copia de `extension/` por variante). Ajusta esas rutas y la lista de modos a medir antes de correrlos.
4. Cada lanzamiento usa un perfil temporal nuevo (borrar perfiles viejos falla con acceso denegado en Windows; es basura segura de borrar a mano).

## Reglas de lectura de resultados

- El arnés **infla las cifras absolutas y cambia entre sesiones**: usa razones contra `fire` de la misma sesión.
- Descarta ventanas con 0 pasos/s o 0 rAF/s (Chrome dejó de dar frames a la ventana).
- La referencia absoluta real es el Administrador de tareas de Chrome (`Shift+Esc`).

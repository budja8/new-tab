# Measurement of the New Tab extension in real headful Chrome 154 (CDP)

Status: DONE (measurements complete; throwaway profile dirs could NOT be deleted, see Leftovers).

## Method
- Script: `scratchpad\measure-chrome\measure.mjs` (Node 24, no deps; NUL-delimited JSON-RPC over fd3/fd4 pipes). Raw output: `results.json`, `run.log`; analysis helper `an.mjs`.
- Chrome spawned with `--remote-debugging-pipe --enable-unsafe-extension-debugging --user-data-dir=<scratch>\p --no-first-run --no-default-browser-check --window-size=1920,1080 --window-position=0,0 --disable-features=CalculateNativeWinOcclusion --disable-backgrounding-occluded-windows`. Extension loaded with `Extensions.loadUnpacked` (id cdcieljoipafbjehooodhmcfhcfmlapd). Only my spawned Chrome was affected (user's Chrome, root PID 1976, never touched; mine exits when its pipe closes).
- Each scenario: open `chrome-extension://<id>/newtab.html`, wait for DOM, `chrome.storage.local.set({settings:{mode,kpiVisible:true}})`, `Page.reload`, `Page.bringToFront`, verify `#mode` value and stored value, 8 s warm-up, then 3 consecutive 10 s windows. CPU% (of one core) = delta(`SystemInfo.getProcessInfo` cpuTime) / delta(wall) x 100. Tab closed between scenarios.
- Tab renderer identification: the renderer PID with the highest CPU delta while only that tab animates (fresh tab/renderer per scenario, so PIDs differ). In the hidden case CPU is ~0 so it cannot be identified this way: that row is simply the top-CPU renderer (25636) and is NOT guaranteed to be the extension's; its memory is indicative only.
- Page metrics from `Performance.getMetrics` on the tab (Task/Script/Layout/RecalcStyle duration deltas per second as % of one thread, JSHeapUsedSize, Nodes).
- Memory: Windows `Get-Process` WorkingSet64 / PrivateMemorySize64 after the windows. Task Manager "memory footprint" is closest to Private. Chrome's per-GPU-memory column is not exposed; GPU-process working set / private bytes are given.

## Caveats
1. The window was NOT 1920x1080: the OS clamped it; page innerWidth x innerHeight = 1540 x 731 (1.13 Mpx vs 2.07 Mpx). On the user's full-screen canvas the pixel count is about 1.8x, so GPU/renderer cost there is plausibly higher than measured.
2. First attempt: window was `visibilityState=hidden` (OS occlusion) giving ~0 CPU; fixed with the occlusion-disabling flags + `Page.bringToFront`/`Browser.setWindowBounds`, and `visible` was verified for each measured scenario.
3. Another failed attempt: profile path over MAX_PATH (260) made LevelDB writes fail (`MANIFEST-000001: Unable to create writable file`); fixed with a short profile dir name `p`. The IndexedDB path (`...\IndexedDB\chrome-extension_<id>_0.indexeddb.leveldb\MANIFEST-...`) still exceeds 260 chars, so IndexedDB fails in this harness; this only affects image mode (artifact of the deep scratchpad path, not of the extension).
4. The user's own Chrome and other apps were running concurrently (noise, mostly in the browser-process column).
5. rAF rate measured: 60.67 Hz, with a separate probe loop evaluated in the fire tab (3 s). This is the DISPLAY's rAF rate, not the page's own frame rate (page not altered). Page Task duration ~59 % per second is consistent with ~60 fps work.

## Results (CPU = % of one core, 3 x 10 s windows, then mean)
| Scenario | Tab renderer CPU | GPU process CPU | Sum all Chrome procs | Renderer WS / Private MB | GPU proc WS / Private MB |
|---|---|---|---|---|---|
| 1 fire visible | 62.88 / 62.33 / 65.01 = **63.4** | 69.90 / 68.87 / 72.00 = **70.3** | 134.2 / 133.9 / 157.2 = 141.7 | 105.9 / 43.8 | 150.4 / 122.1 |
| 2 rain visible | 9.68 / 9.52 / 7.96 = **9.1** | 19.28 / 19.67 / 15.53 = **18.2** | 30.4 / 30.5 / 24.5 = 28.5 | 95.3 / 41.6 | 134.4 / 113.4 |
| 3 starfield visible | 23.14 / 24.33 / 27.46 = **25.0** | 31.46 / 35.42 / 34.69 = **33.9** | 58.0 / 60.3 / 63.4 = 60.6 | 100.4 / 45.6 | 154.0 / 133.7 |
| 5 fire, tab hidden | 0.02 / 0.18 / 0.01 = **0.07** | 0.02 / 0.01 / 0.00 = **0.01** | 0.46 / 0.99 / 0.23 = 0.56 | 83.4 / 29.9 (caveat) | 128.1 / 108.4 |
| 6 control, idle about:blank | 0.07 / 0.01 / 0.53 = 0.2 | 0.76 / 0.04 / 0.01 = 0.27 | 20.96 / 3.28 / 0.88 = 8.4 (window 0 holds 17.7 of browser-process startup noise) | 76.3 / 33.5 | 106.7 / 88.7 |

Variance: window-to-window spread is under 30 % everywhere except the control sum (browser-process startup noise in window 0; windows 1-2 are 3.3 and 0.9). Worst animated case: rain GPU 15.5 vs 19.7 (21 %).

Page metrics per second (% of one thread; heap in MB; DOM Nodes constantly 114):
| Scenario | TaskDuration | ScriptDuration | Layout | RecalcStyle | JS heap |
|---|---|---|---|---|---|
| fire | 59.4 / 59.3 / 61.5 | 27.4 / 27.8 / 28.0 | 0 | 0 | 1.0-1.5 |
| rain | 8.0 / 7.4 / 6.6 | 3.1 / 2.9 / 2.1 | 0 | 0 | 0.8-1.4 |
| starfield | 16.0 / 17.8 / 22.0 | 5.3 / 6.7 / 8.5 | 0 | 0 | 0.8-1.1 |
| hidden fire | 0.02 / 0.14 / 0.01 | 0 | 0 | 0 | 0.7-1.1 |

Reading: zero layout/style cost (canvas only). In fire, script is about 28 % but task time about 59 %, so about half the renderer main-thread time is outside script (canvas drawing / paint / commit), plus the GPU process at about 70 % (raster/composite of a large canvas at 60 fps). GPU process cost exceeds the renderer's in every animated mode. The user's Task Manager figure of 35.1 % is for one row; my renderer row was 63 % at a smaller window, so the difference is unexplained (different refresh rate / GPU / DPR / throttling are possible).

## Scenario checks
- 1/2/3: `#mode` select value equaled fire / rain / starfield after reload; stored settings matched; `visibilityState=visible`.
- 4 image with no saved image: `#mode`='image'; the code path logged `No se pudo leer la imagen guardada` (IndexedDB failure from the MAX_PATH artifact) and per source falls back to `effective='fire'` with the message "No hay una imagen válida guardada: se muestra el fuego." (confirmed from newtab.js; the DOM message was not read). Not measured.
- 5 hidden: a second `about:blank` tab was created and activated; the extension tab reported `visibilityState=hidden`. Renderer and GPU CPU about 0 (whole-Chrome sum 0.2-1.0 %): expectation CONFIRMED.
- 6 control: only an about:blank tab (extension loaded, no extension tab).

## Console errors / exceptions
Only 2 events in the entire run, both `console.error` in scenario 4 (image): `No se pudo leer la imagen guardada` with `UnknownError: Internal error opening backing store for indexedDB.open.` (harness MAX_PATH artifact). No `Runtime.exceptionThrown`, no `Log.entryAdded`, no errors in fire / rain / starfield / hidden.

## Other observations
- Idle blank tab baseline: GPU process private 88.7 MB, renderer private 33.5 MB. Fire adds about +33 MB private to the GPU process and about +10 MB private to the renderer (43.8 MB; user saw 60.97 MB footprint and 25.7 MB GPU).

## Leftovers
- No Chrome process of mine remains (only the user's root chrome PID 1976 exists).
- Throwaway profile dirs could NOT be deleted: `scratchpad\measure-chrome\p`, `profile`, `profile2`, `profile3`, `profile4`, `profile5` (all inside the scratchpad; nothing in the project). Every file under `Default\` returns "Acceso denegado" for Remove-Item, rm, rd, robocopy /MIR and icacls /grant, although ACLs look normal and Restart Manager shows no holder (cause unknown, possibly a security filter driver). Delete `scratchpad\measure-chrome` manually later. Project files untouched.

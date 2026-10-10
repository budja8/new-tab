# pipes: verification and measurement report (2026-10-09)

Status: **DONE_WITH_CONCERNS** (minor: an early hold at 67 % when all 4 pipes die in the same step; 60 FPS costs 2x fire). No project source file changed; only `docs/measurements.md` and the spec were edited. No git.

## 1. Copies and size
- `spike\pipes\` = byte-identical copy of `extension\` (diff -rq at start and end: identical). `spike\pipes30\` = same with `FPS.pipes = 30` (only diff: newtab.js line 10). `spike\verify3\` = copy of pipes + verify.html/verify.js.
- `npm run check:size` exit 0: wasm 30.7 KB = 31,441 B / 150 KB; js+css+html 53.3 KB = 54,630 B / 61,440 B → margin 6,810 B.

## 2. Scripts (all in `scratchpad\spike\`)
- `verify3\verify.js` + `verify-pipes.mjs` (env DSF for emulated dpr): real WASM Engine (seed 12345), real `Renderer` and `GlRenderer`; per-frame cell hash to detect frozen runs; snapshot of both canvases at hold start vs last hold frame; checkpoints compare GL incr vs 2D, GL after invalidate vs incr, fresh GL vs incr, fresh 2D vs incr. Output `verify-pipes.json`, `verify-pipes-dsf1.25.json`.
- `pipes-hold.mjs` (small window, screenshots every ~330 ms + wrapped `engine_step` logging [t, cell hash, ink] per step) → `hold-pipes.json`, `hold-pipes30.json`.
- `pipes-shots.mjs` (full window screenshots at 3 s, 15 s, near-full, hold, restart+3 s) → `shots-pipes.json`, `shots-pipes\page-pipes-*.png`, `shots-pipes\zoom-3s-corner.png`.
- `measure-pipes.mjs` (from measure-final2.mjs; adds ink sampling every 30 steps to locate the cycle phase, fixed 7-window "pipesLong" scenario) → `results-pipes.json`, `results-pipes-long.json`, logs `measure-pipes.log`, `measure-pipes-long.log`; `analyze-pipes.mjs` → `analyze-pipes.log`.
- Profiles under `C:\ntpipes\`.

## 3. Pixel verification
GPU: ANGLE Intel UHD (0xA721) D3D11. dpr 1: 211x58, cell 9x16, canvas 1899x928.
- pipes_fps1 (set_fps(1), hold 4 frames): 5,361 frames, 2 full cycles. Holds at frames 2526-2529 and 5057-5060 (9,793 cells = 80.02 %); reset → 4 cells (4 ●). 29 checkpoints: GL vs 2D max 0/255, 0 px > 2; invalidate/fresh/2D-fresh all 0. Frozen check: last hold frame vs first hold frame 0/255 (2D and GL).
- pipes_fps60 (set_fps(60)): 3,066 frames; hold exactly 240 frames (2526-2765), reset at 2766; 19 checkpoints (incl. hold+1, hold-mid, reset, reset+1, reset+60) all 0/255; frozen 0/255.
- dpr 1.25 emulated (216x58, cell 11x20, canvas 2376x1160): fps1 holds 2590-2593 and 5184-5187 (80.02/80.03 %), fps60 hold 240 frames (2590-2829); 30 + 19 checkpoints all 0/255, 0 px > 2; frozen 0/255.
- Codepoints seen: 2550 2551 2554 2557 255a 255d 25cf (═ ║ ╔ ╗ ╚ ╝ ●); colours: the 7 palette colours. GL atlas slots 7. Console clean.
- Note: the first dpr-1.25 attempt returned no result (page module not loaded 2 s after reload: harness timing); fixed by polling for `window.__verify`, rerun OK.
- PASS (≤ 2/255, 0 px > 8).

## 4. Screenshots (`spike\shots-pipes\`)
- `page-pipes-3s.png` (ink 737): thin double-line pipes in red/blue/yellow/orange, corners join correctly (checked zoomed in `zoom-3s-corner.png`). KPI panel is translucent; pipes visible under it.
- `page-pipes-15s.png` (ink 3,593), `page-pipes-nearfull.png` (~39 s, ink 9,386), `page-pipes-hold.png` (ink 9,793): dense maze of the 7 colours with some ● dots; looks like pipes.
- `page-pipes-restart3s.png` (ink 778): clean background with new pipes.
- `hold-pipes-*.png`, `hold-pipes30-*.png`: small-window frozen states.
- Cosmetic observation: a 1-px background gap between consecutive ═ / ║ cells (font glyph does not fill the 9x16 cell); identical in 2D (pixel diff 0), so not a GL issue.

## 5. Hold behaviour in the real page
Window 420x320 requested; Chrome's minimum gave page 500x168, canvas 495x160, grid 55x10.
60 FPS (copy pipes), 41.6 s, 2,488 steps (59.79 steps/s):
```
start 1.90s  frames 240 frozen 4.049s ink 442 -> 4
start 7.86s  frames 240 frozen 4.009s cycle 5.93s steps/s in hold 59.87
start 13.78s frames 240 frozen 4.000s cycle 5.91s 60.00
start 19.65s frames 240 frozen 4.016s cycle 5.89s 59.75
start 25.60s frames 240 frozen 4.008s cycle 5.93s 59.88
start 31.50s frames 240 frozen 4.000s cycle 5.89s 60.00
start 37.41s frames 240 frozen 4.033s cycle 5.95s 59.50
screenshots: 7 frozen runs of 12-13 equal PNGs, frozen >= 3.56-3.90 s and <= 3.89-4.23 s; raf/s 59.5-60.0, steps/s 59.1-60 during hold
```
30 FPS (copy pipes30), 41.5 s, 1,246 steps (30.02/s):
```
start 3.77s  frames 120 frozen 3.991s
start 11.51s frames 120 frozen 3.991s cycle 7.73s
start 19.27s frames 120 frozen 4.000s cycle 7.77s
start 27.03s frames 120 frozen 4.000s cycle 7.76s
start 34.83s frames 120 frozen 4.008s cycle 7.81s
screenshots: 5 frozen runs, 12-13 equal PNGs, frozen >= 3.54-3.90 s, <= 3.85-4.23 s; raf/s 59.9-60.1, steps/s 29.9-30.1
```
Full size 60 FPS (pipes-shots run): hold at page t 42.51 s (step 2531, 9,793 cells), frozen 4.001 s, 240 frames. Console in all runs: only the `info [newtab] renderer: gl (...)` line.

## 6. CPU / memory (full window 1920x1080 on secondary monitor, dpr 1, page 1904x928, 8 s warm-up, 10 s windows)
Plan A: ctrl, pipes, pipes30, pipes30, pipes, pipes, pipes30, ctrl (each `pipes` run: pipes, fire, pipesHidden). Plan L: pipesL, pipes30, pipesL (pipesL = 7 fixed windows).
Raw (tab/GPU/sum, steps/s, ink start->end, page time):
```
A1-ctrl blank: 0.05/0.03/0.1 | 0.09/1.36/1.5 | 0.9/0.02/0.9
A2-pipes pipes: 17.63/40.13/57.8 st=59.9 ink=2380->4738 t=11-21s | 16.22/35.49/51.7 st=60 4738->7054 | 15.92/35.98/51.9 st=60.04 7054->9310   mem tab 36.7 gpu 127.6
A2-pipes fire: 10.63/11.84/22.5 st=29.95 | 10.65/11.49/22.1 | 10.45/11.65/22.1   mem 32.6/115.1
A2-pipes hidden: 0.04/0.02/0.1 st=0 | 0.2/0.03/0.2 | 0.01/0.01/0.0 vis=hidden
A3-pipes30: 10.53/17.04/27.6 st=29.95 1197->2381 | 8.7/12.25/20.9 | 9.31/12.64/22.0   mem 32.3/112.4
A4-pipes30: 10.58/16.43/27.0 | 9.87/13.47/23.3 | 9.76/13.74/23.5   mem 34/113
A5-pipes pipes: 16.35/37.99/54.3 | 16.85/36.32/53.2 | 13.6/31.48/45.1   mem 37.7/128.2
A5-pipes fire: 10.33/12.29/22.6 | 10.92/13.21/24.1 | 10.09/12.45/22.5   mem 31.9/115
A5-pipes hidden: 0.03/0.02/0.1 | 0.47/0.03/0.5 | 0.01/0.02/0.0
A6-pipes pipes: 17.85/32.97/50.8 st=59.06 | 18.69/31.46/50.2 | 11.87/17.29/29.2 (ink 7071->159: early hold at 8,177 cells 35.8-39.8 s + reset) | 15.81/31.32/47.1 | 15.48/31.04/46.5   mem 34.1/135.3
A6-pipes fire: 11.23/12.22/23.5 | 10.97/11.5/22.5 | 10.57/11.24/21.8   mem 35.3/114.9
A6-pipes hidden: 0.04/0.03/0.1 | 0.2/0.01/0.2 | 0.01/0/0.0
A7-pipes30: 9.39/14.47/23.9 | 8.64/11.54/20.2 | 8.43/11.86/20.3   mem 34.3/113.5
A8-ctrl blank: 0.04/0.02/0.1 | 0.11/1.44/1.6 | 0.89/0.03/0.9
L1-pipesL: 15.72/25.11/40.8 | 13.87/21.82/35.7 | 13.15/20.08/33.2 | 10.23/13.8/24.0 (hold 42.8-46.8 s) | 13.1/21.51/34.6 | 13.06/20.1/33.2 | 0.28/0.05/0.3 st=0 raf=0 (EXCLUDED)   mem 28.5/118.4
L2-pipes30: 12.39/17.94/30.3 | 11.7/15.37/27.1 | 10.79/15.26/26.0   mem 37.3/112.9
L3-pipesL: 17.55/29.56/47.1 | 17.12/26.64/43.8 | 18.82/30.26/49.1 | 12.53/18.37/30.9 (hold 43.0-47.0 s) | 14.52/26.95/41.5 | 14.95/26.71/41.7 | 14.87/23.26/38.1   mem 33.4/121.3
```
Summary (CPU % of one core):
| Case | n | tab | GPU | sum | range | steps/s |
|---|---|---|---|---|---|---|
| pipes 60, growth windows | 21 | 15.8 | 29.3 | 45.1 | 33.2-57.8 | 59.7 |
| pipes 60, windows with hold+restart | 3 | 11.5 | 16.5 | 28.0 | 24.0-30.9 | 59.7 |
| pipes 60, all valid | 24 | 15.2 | 27.7 | 43.0 | 24.0-57.8 | |
| pipes 30 | 12 | 10.0 | 14.3 | 24.3 | 20.2-30.3 | 30.0 |
| fire 30 (same session) | 9 | 10.6 | 12.0 | 22.6 | 21.8-24.1 | 30.0 |
| pipes hidden | 9 | 0.1 | 0.0 | 0.1 | 0.0-0.5 | 0 |
| about:blank | 6 | 0.3 | 0.5 | 0.8 | 0.1-1.6 | |
Ratios vs same-session fire: pipes60 tab 1.5x, GPU 2.4x, sum 2.0x; pipes30 0.94x / 1.19x / 1.08x. Fire this session (10.6/12.0/22.6) is close to the previous session (10.9/13.8/24.8), so the harness did not drift much this time.
Variance: pipes60 tab spread 43 % over all windows (hold windows and batch differences: L runs 33-49 sum vs A runs 45-58); the per-run 3-window spread rule held except where a hold fell inside a window; pipes30 spread 40 % driven by the first window of each run (27-30) vs later windows (20-27). TaskDuration: pipes60 7.4 %, pipes30 4.3 %, fire 5.7 %. Memory private: tab pipes60 28-38 MB, pipes30 32-37, fire 32-35, hidden 28; GPU pipes60 118-135 MB, pipes30 112-114, fire 115. Console: only the gl info line (2 per page: navigate + reload); 0 errors/exceptions.
Cycle coverage: normal windows cover page t 11-41 s = growth from ~19 % to ~76 % of cells (full fill ≈ 42 s); long runs span hold (≈ 43-47 s) and restart.
Excluded: L1 last window (0 steps, 0 rAF: Chrome stopped issuing frames to the window; external, as in the earlier session).

## 7. Documents changed
- `docs/measurements.md`: new section "Animación `pipes` (2026-10-09)" appended at the end (method, fidelity, hold table at 60/30, CPU/memory table, harness-inflation note with extrapolation, size, conclusion recommending 30 FPS). Existing sections untouched.
- Spec: §2 Incluido lists `pipes` ("Tuberías", added 2026-10-09); removed from the out-of-scope list with "(`pipes` ya se portó el 2026-10-09)"; §4 tree adds pipes.rs; Frontera Rust/JS mentions `pipes` and `set_fps`; §5 FPS list adds `pipes` 60 (can drop to 30) + one bullet describing the behaviour; size note adds measured 54,630 B (margin 6,810) and wasm 31,441 B.

## 8. Findings for the implementer / controller
1. Early hold: in one harness run the picture froze at 67 % (8,177 cells) — the code holds when all 4 pipes die in the same step (`full || all dead`). Hold still ≈ 4 s and restart fine. If "always 80 %" is wanted, that branch would need changing; otherwise document it as intended.
2. 60 FPS costs 2.0x fire (GPU 2.4x); 30 FPS ≈ fire. Recommendation: 30 FPS (fill takes ~84 s at full size instead of ~42 s).
3. Cosmetic: 1-px gaps between consecutive ═/║ cells (font metrics), identical in both renderers.

## 9. Cleanup / final checks
- All Chrome instances I launched were closed via taskkill /PID tree (lib close); check: 0 chrome.exe with `ntpipes` in the command line. User's Chrome untouched.
- `C:\ntpipes\` profiles could NOT be deleted (access denied, as before): 17 folders, ~1.2 GB: hcfxh6 hch8bk m0cldci m0cmt8v m0d2uf4 m1cny67 m1d4ykc m2cr4sd m2d67j5 m3csei8 m4ctnmv m5cwu76 m6d0gwg m7d1ph3 scirqa vcanko vcctxh.
- `npm run test`: Rust 66 passed, JS 93 passed (0 fail), exit 0. `npm run check:size`: exit 0 (30.7 KB / 150 KB; 53.3 KB / 60 KB).
- `extension\` identical to `spike\pipes\` (diff -rq).
- Leftovers in scratchpad: `spike\pipes\`, `spike\pipes30\`, `spike\verify3\`, `spike\shots-pipes\`, scripts and JSON/logs listed above, `results-pipes-smoke.json` (smoke test).

# dna: verification and measurement report (2026-10-09)

Status: **DONE_WITH_CONCERNS** (minor, cosmetic: at page widths of about 1100 px or less the translucent KPI panel covers part of the log header and long lines; in a 55-column page it covers the right strand in rows 2-14). No project source file changed; only `docs/measurements.md` and the spec were edited. No git.

## 1. Copies and size
- `spike\dna\` = byte-identical copy of `extension\` (diff -rq at the start and at the end: identical). `spike\verify4\` = copy of dna + verify.html/verify.js.
- `npm run check:size` exit 0: wasm 42.5 KB = 43,506 B / 150 KB (was 31,441 B: +12,065 B); js+css+html 53.4 KB = 54,694 B / 61,440 B → margin 6,746 B.

## 2. Scripts (all in `scratchpad\spike\`)
- `verify4\verify.js` + `verify-dna.mjs` (env DSF=1.25 for emulated dpr): real WASM Engine (seed 12345, set_fps(30), dna), real `Renderer` and `GlRenderer`. Grid size for the narrow and mid scenarios is forced by overriding `window.innerWidth/innerHeight` in the verify page only (the renderers read them in fit()). Per checkpoint: GL incr vs 2D, GL after invalidate vs incr, fresh GL vs incr, fresh 2D vs incr, plus a layout analysis of the cell buffer (ink columns, helix columns, header count/column, log lines, mutations, bridge colours). `sum-dna.mjs` prints the summary. Output: `verify-dna.json`, `verify-dna-dsf1.25.json`.
- `dna-shots.mjs`: real page (copy dna) with external instrumentation (wrapped WASM exports + `__lay()` that reads the cell buffer and the #kpi rect); screenshots at 3/15/45 s at full size, then window widths 1296/1552/1100/1000/516/1920 (page 1280/1536/1084/984/500/1904). Output `shots-dna.json`, `shots-dna\dna-*.png`.
- `measure-dna.mjs` (from measure-pipes.mjs) → `results-dna.json` (`measure-dna.log`), `results-dna-long.json` (`measure-dna-long.log`); `analyze-dna.mjs` → `analyze-dna.log`.
- Profiles under `C:\ntdna\`.
- Doc backups before editing: `sdd-dna\spec-before.md`, `sdd-dna\measurements-before.md`.

## 3. Pixel verification — PASS
GPU: ANGLE Intel UHD (0xA721) D3D11.
| Grid | frames | checkpoints | GL vs 2D | invalidate / fresh GL / fresh 2D |
|---|---|---|---|---|
| 211x58 dpr1 (9x16, canvas 1899x928) | 2,500 | 36 | max 0/255, 0 px >2 | 0/255 |
| 216x58 dpr1.25 (11x20, canvas 2376x1160) | 2,500 | 36 | 0/255 | 0/255 |
| 40x20 dpr1 / dpr1.25 | 600 | 14 + 14 | 0/255 | 0/255 |
| 90x30 dpr1 / dpr1.25 | 1,200 | 23 + 23 | 0/255 | 0/255 |
Checkpoints: f1, 2, 10, 24, 25, 26, 63, 126, every 100, first mutation (f425) and +25, history full (f1300 on 211x58, f600 on 90x30), first scroll (+25), last frame. 2,500 frames = ~20 helix turns (period 125.7 frames) and 100 log lines added; up to 4 mutation lines visible at once.
Codepoints: 52 (ASCII of the log + U+2500 ─, U+2550 ═, U+25CF ●, 'o'); colours: the 8 of dna.rs (gray, green, dark green, magenta, dark magenta, cyan, yellow, red). GL atlas slots 52. Console clean.
Buffer layout:
- 211x58: ink cols 55-155 at every checkpoint → margins 55/55; helix 55-73 (cx 64, amp 9); header once at col 86; log at col 86, normal lines end col 132, mutation lines and header end col 155; log lines 1 at f25, 40 at f1000, 52 (= height-6) from f1300, then scrolls; bridges on rows ≡ 0 mod 3 with correct alternation (row 3 yellow, 6 cyan, ...) at all checkpoints.
- 216x58 (dpr 1.25): ink 57-157, margins 57/58 (shift 46).
- 40x20: helix only, cols 10-30 (cx 20, amp 10), margins 10/9, no header/log.
- 90x30: shift 0, ink 11-89, helix 11-29, header at col 42 clipped at col 89, mutation lines clipped at 89, max 24 lines.
(My analyzer's "helix range" after f25 includes the letter 'o' of "compatibility", so only the pre-log value 55-73 is meaningful; the page-side analyzer excludes columns ≥ header.)

## 4. Screenshots (`spike\shots-dna\`), looked at
- `dna-3s.png` (3 log lines), `dna-15s.png` (17), `dna-45s.png` (52, 3 mutations): helix clearly reads as a rotating double helix (bright ● near side, dim 'o' far side, crossings visible); bridges alternate cyan/yellow; log legible (green/cyan, red mutation lines); header once ("═" looks like "=" at this size); composition visibly centred: ink cols 55-155 → left 495 px, right 500 px to page edge (55/55 cells). KPI panel at cols 183-209 rows 0-14: does NOT overlap the log at 1904 px.
- Other widths (see table §5). `dna-w1904b.png` = back at full size: same 55/55.
- The log survives window resizes (Engine resize keeps the animation state).

## 5. Layout at real sizes (page height 928, dpr 1)
| page width | grid | shift (expected = measured) | ink cols | margins L/R | KPI vs log |
|---|---|---|---|---|---|
| 1904 | 211x58 | 44 | 55-155 | 55/55 | no overlap (KPI cols 183-209) |
| 1536 | 170x58 | 23 | 34-134 | 34/35 | no overlap (KPI 142-168) |
| 1280 | 142x58 | 9 | 20-120 | 20/21 | KPI cols 113-140 rows 0-14 covers header cols 113-120 and the tail of mutation lines when they pass rows 3-14 |
| 1084 | 120x58 | 0 | 11-111 | 11/8 | KPI cols 92-118: header + mutation lines (normal lines end col 88) |
| 984 | 109x58 | 0 | 11-108 | 11/0 (clipped) | KPI cols 80-107: header, mutation and the end of normal lines |
| 500 | 55x58 | narrow | 17-37 helix only | 17/17 | KPI (cols 32-54) covers the right strand in rows 2-14 |
Note: with shift = max(0, (w-101)/2 - 11) the margins are equal (±1) from 123 columns; for 112-122 columns left = 11 and right = 0..10; the log (cols 42-111 at shift 0) is clipped only below **112** columns (the brief said 113; column 111 is the last log column, so a 112-column grid shows it whole). I wrote 112 in the spec.

## 6. CPU / memory (window 1920x1080 secondary monitor, dpr 1, page 1904x928, 8 s warm-up, 10 s windows)
Plan A: ctrl, dna(dna,fire,hidden), dnaB(fire,dna,hidden), dna, dnaL(7 windows), ctrl. Plan L (added because dnaL had a 41 % spread over 7 windows and no same-Chrome fire): ctrl, dnaLF(dnaLong, fire), dnaFL(fire, dnaLong), ctrl.
Raw tab/GPU/sum (steps/s 29.78-30.09 in all visible windows; page t in brackets):
```
A1 ctrl: 0.03/0.01/0.0 | 0.08/0.91/1.0 | 0.58/0.01/0.6
A2 dna:  6.53/10.32/16.9 | 5.9/8.45/14.3 | 5.74/8.14/13.9 (t 11-41, ink 869->2430)  mem tab 35.6 gpu 120.1
A2 fire: 6.68/8.5/15.2 | 6.63/8.17/14.8 | 6.72/8.38/15.1  mem 33.3/121.7
A2 hidden: 0.02/0/0.0 | 0.25/0.06/0.3 | 0.22/0.03/0.3 st=0 raf=0 vis=hidden/hidden  mem 28.8/120.2
A3 fire: 7.78/10.25/18.0 | 7.15/9.05/16.2 | 6.56/8.53/15.1  mem 36.5/113
A3 dna:  6.01/8.97/15.0 | 5.81/8.34/14.1 | 6.13/9/15.1  mem 33.1/120.9
A3 hidden: 0.02/0/0.0 | 0.23/0.05/0.3 | 0.02/0.04/0.1  mem 31/120.2
A4 dna:  6.78/10.55/17.3 | 6/8.66/14.7 | 5.71/8.33/14.0  mem 35.7/120.3
A4 fire: 6.91/8.54/15.4 | 7.58/9.13/16.7 | 7.23/9.01/16.2  mem 33.6/121.2
A4 hidden: 0.02/0/0.0 | 0.23/0.05/0.3 | 0.01/0.02/0.0  mem 30.3/112.6
A5 dnaL: 8.21/12.92/21.1 | 8.67/12.02/20.7 | 8.21/11.54/19.8 | 7.41/10.32/17.7 | 8.91/13.38/22.3 | 6.18/8.92/15.1 | 5.79/8.5/14.3 (t 11-81; log full from ~43 s, ink 2373->2557 in w4 then flat ~2530)  mem 31.1/120.3
A6 ctrl: 0.03/0.01/0.0 | 0.08/0.93/1.0 | 0.52/0.01/0.5
L1 ctrl: 0.02/0.01/0.0 | 0.07/0.89/1.0 | 0.52/0.01/0.5
L2 dnaLong: 6.56/10.45/17.0 | 6.02/8.54/14.6 | 5.67/8.16/13.8 | 5.66/8.29/13.9 | 5.86/8.56/14.4 | 5.8/8.49/14.3 | 5.72/8.5/14.2  mem 31.2/120.1
L2 fire: 6.64/8.4/15.0 | 6.63/8.38/15.0 | 6.49/8.51/15.0  mem 36.4/121.7
L3 fire: 7.65/10.52/18.2 | 6.7/8.64/15.3 | 7.15/9.17/16.3  mem 36.7/112.4
L3 dnaLong: 5.74/8.41/14.2 | 5.73/8.49/14.2 | 5.6/8.14/13.7 | 5.62/8.37/14.0 | 5.63/8.39/14.0 | 5.78/8.36/14.1 | 5.7/8.42/14.1  mem 29.7/121.2
L4 ctrl: 0.02/0.01/0.0 | 0.09/0.91/1.0 | 0.45/0.01/0.5
```
Summary (CPU % of one core):
| Case | n | tab | GPU | sum | range | steps/s | Task% / Script% |
|---|---|---|---|---|---|---|---|
| dna 30, all | 30 | 6.3 | 9.3 | 15.6 | 13.7-22.3 | 30.0 | 2.8 / 0.9 |
| dna, log filling (t 11-41 s) | 18 | 6.4 | 9.4 | 15.8 | 13.7-21.1 | 30.0 | |
| dna, transition (41-51 s) | 3 | 6.2 | 9.0 | 15.2 | 13.9-17.7 | | |
| dna, log full (51-81 s) | 9 | 6.2 | 9.1 | 15.2 | 14.0-22.3 | 30.0 | |
| dna excl. A5 run | 23 | 5.9 | 8.7 | 14.6 | 13.7-17.3 | 30.0 | |
| fire 30 same session | 15 | 7.0 | 8.9 | 15.8 | 14.8-18.2 | 30.0 | 3.6 / 1.8 |
| dna hidden | 9 | 0.1 | 0.0 | 0.1 | 0.0-0.3 | 0 | 0.1 |
| about:blank | 12 | 0.2 | 0.3 | 0.5 | 0.0-1.0 | | |
Ratios vs same-session fire: tab 0.90x, GPU 1.04x, sum 0.99x. Memory private: tab dna 29.7-35.7 MB, fire 33.3-36.7, hidden 28.8-31.0, blank 34-35; GPU process dna 120-121 MB, fire 112-122, blank 81-82. rAF 59.5-60.1/s. Console: only the gl info line (2 per page load: navigate + reload); 0 errors.
Variance: per-run 3-window spreads ≤ 24 % except A5 dnaL (41 % over 7 windows: windows 1-5 at 17.7-22.3, then 15.1/14.3), which is why plan L was added; L2/L3 long runs were 13.7-17.0. The first window of a run is usually the highest.
Harness drift: fire this session 7.0/8.9/15.8 vs 10.6/12.0/22.6 in the pipes session (−30 %); only same-session ratios are meaningful.

## 7. Documents changed
- `docs/measurements.md`: new section "Animación `dna` (2026-10-09)" appended at the end (description, method, fidelity table, screenshot observations, layout table per width, CPU/memory table, harness note, size with the wasm growth attribution, conclusion). Earlier sections untouched.
- Spec `docs/superpowers/specs/2026-10-08-new-tab-ascii-design.md`: §2 Incluido adds `dna` ("ADN", 2026-10-09); removed from the out-of-scope list ("`pipes` y `dna` ya se portaron el 2026-10-09"); §4 tree adds dna.rs; Frontera Rust/JS lists `dna` in set_animation; §5 FPS list adds `dna` 30; new §5 bullet describing dna and layout B (centred on > 55 columns, shift formula, 44 at 211, log clipped below 112 columns, narrow = helix only centred); size note adds 54,694 B (margin 6,746) and wasm 43,506 B.

## 8. Findings for the implementer / controller
1. No functional defect. Pixel-identical GL/2D everywhere; centring exact as designed.
2. Cosmetic: the KPI panel overlaps the log at page widths ≤ ~1280 px (only 8 cells of the header/mutation tails at 1280; more at 1084 and 984) and the helix's right strand at 500 px. Translucent panel, content still visible but dimmed. Decide if acceptable.
3. Doc detail: the log is clipped below 112 columns (not 113); for 112-122 columns shift = 0 gives margins 11 / 0..10 (formula behaviour, as the implementer noted).

## 9. Cleanup / final checks
- All Chrome instances I launched were closed via taskkill /PID tree (lib close); check: 0 chrome.exe with `ntdna` in the command line. User's Chrome untouched.
- `C:\ntdna\` profiles NOT deletable (access denied/locked): 11 folders, ~179 MB: m0edex7 m0erc0k m1eejge m1esgcf m2ehngp m2evgrd m3ekrg7 m3eyh9c m4envab m5epyfu seap4t (the two verify profiles were removed).
- `npm run test`: exit 0, Rust 80 passed, JS 95 passed (0 fail). `npm run check:size`: exit 0 (42.5 KB / 150 KB; 53.4 KB / 60 KB).
- `extension\` identical to `spike\dna\` (diff -rq).
- Leftovers in scratchpad: `spike\dna\`, `spike\verify4\`, `spike\shots-dna\`, scripts (verify-dna.mjs, sum-dna.mjs, dna-shots.mjs, measure-dna.mjs, analyze-dna.mjs), JSON/logs listed above, doc backups in `sdd-dna\`.

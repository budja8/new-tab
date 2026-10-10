# New Tab ASCII Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extensión de Chrome (MV3) que reemplaza la pestaña nueva con un fondo ASCII (animaciones `fire`, `rain`, `starfield` portadas de cwork, o una imagen convertida a ASCII) y un panel de KPI de teclas presionadas dentro de Chrome.

**Architecture:** Un crate de Rust (`core/`) compilado a WASM es dueño del estado de las animaciones y de la conversión de imagen; expone un `Engine` cuyo buffer de celdas (`[carácter, fg, bg]` por celda, `u32`) vive en memoria compartida. JS pinta ese buffer en un `<canvas>` con un atlas de glifos y diff por celda. Un content script cuenta teclas, un service worker las agrega en `chrome.storage.local`, y la pestaña nueva las muestra por eventos.

**Tech Stack:** Rust 1.96 + `wasm-bindgen` + `wasm-pack` (target `web`), JavaScript ES modules sin bundler, Node 24 (`node --test`) para pruebas de lógica pura, Chrome MV3.

**Spec:** `docs/superpowers/specs/2026-10-08-new-tab-ascii-design.md`

**Referencias a portar (solo lectura):** `refs/cwork/src/animation/{fire,rain,starfield}.rs`, `refs/rust-ascii/src/bin/ascii.rs`.

**Convención de rutas:** todos los comandos se ejecutan desde la raíz del proyecto, `C:\Users\Daniel Rodriguez\Documents\Tests\owns\new-tab`.

**Sin git:** el proyecto no es un repositorio, por decisión del usuario. Este plan no tiene pasos de commit; cada tarea termina con una verificación.

## Desviaciones respecto al spec

El plan refina cuatro puntos del spec tras leer el código real. Los cuatro ya están reflejados en el spec.

1. **Carpeta `extension/`**: lo que se carga en Chrome vive en `extension/`, para no incluir `core/target` (cientos de MB) en la extensión.
2. **RNG propio en vez de `rand` + `getrandom`**: un `xorshift64*` de ~20 líneas, sembrado desde JS. Es más pequeño para el tope de 150 KB y reproducible en pruebas.
3. **`image_to_ascii(rgba, w, h, invert)`**: el navegador reduce la imagen (`createImageBitmap` + `OffscreenCanvas`), así que Rust no necesita la crate `image` (que rompería el tope de tamaño). Se guarda empaquetada a 4 bytes por celda (`[índice, r, g, b]`): 200×100 celdas = 80 KB, dentro del tope de 200 KB. Con el formato de 12 bytes serían 240 KB.
4. **CSP `wasm-unsafe-eval`** en el manifest: Chrome lo exige para instanciar WASM en páginas de extensión.

## Global Constraints

- Manifest V3. Permisos: solo `storage`, más content script sobre `<all_urls>`. Sin `history`, sin `tabs` completo, sin `host_permissions`, sin red.
- `.wasm` ≤ 150 KB (`opt-level="z"`, `lto`, `wasm-opt`). JS + CSS + HTML ≤ 50 KB. Imagen ASCII guardada ≤ 200 KB, una sola imagen, cuadrícula máxima 200×100 celdas.
- FPS objetivo iniciales: `fire` 30, `rain` 30, `starfield` 60.
- Pausa total con `visibilitychange`. Con `prefers-reduced-motion`, un frame estático. Con imagen estática, ningún bucle activo. Sin timers sueltos: el panel de KPI se actualiza por eventos.
- Estadísticas: solo conteos (nombre de tecla, nunca texto), solo dominio del sitio, ignorar `type=password`. Se guardan las 20 teclas, 20 atajos y 20 sitios más usados por día (el resto en `otros`); se descartan días de más de 90 días.
- Alcance v1: solo `fire`, `rain` y `starfield`; estadísticas solo dentro de Chrome.

## Review Focus

Entradas que el spec implica pero que ninguna prueba obvia cubriría; cada una tiene su prueba en la tarea indicada.

1. **Cuadrículas diminutas o de tamaño cero** (ventana muy chica, resize extremo): ningún panic en Rust (en WASM un panic mata la pestaña). Tareas 2, 3, 4 y 6.
2. **Imágenes raras** (muy alta, muy ancha, 1×1, transparente, archivo que no es imagen) y **modo `image` sin imagen guardada**: resultado válido dentro del tope, o mensaje de error claro, o regreso a `fire`. Tareas 5, 9 y 10.
3. **Teclados con AltGr** (en español, AltGr+2 escribe `@` y Windows lo reporta como ctrl+alt) y teclas muertas/IME: no cuentan como atajos ni como teclas. Tarea 7.
4. **Nombres hostiles** (`__proto__` como dominio, HTML en un hostname): no contaminan objetos ni se inyectan en el DOM. Tareas 7 y 10.
5. **Escrituras concurrentes y fallos de storage**: el service worker puede recibir varios lotes a la vez; no se deben perder conteos, y un fallo no debe romper la cola. Tarea 8.

## Estructura de archivos

```
new-tab/
├── package.json
├── core/                         # crate Rust → WASM
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                # declara módulos
│       ├── rng.rs                # xorshift64* sembrable
│       ├── grid.rs               # Grid de celdas [ch, fg, bg] (u32) + rgb()
│       ├── ascii.rs              # imagen → ASCII empaquetado, dimensiones objetivo
│       ├── engine.rs             # Engine (wasm-bindgen): animación o imagen estática
│       └── animation/
│           ├── mod.rs            # trait Animation
│           ├── fire.rs
│           ├── rain.rs
│           └── starfield.rs
├── extension/                    # se carga sin empaquetar desde aquí
│   ├── manifest.json
│   ├── newtab.html / newtab.css / newtab.js
│   ├── background.js             # service worker
│   ├── content/{keyname.js, keycount.js}
│   ├── src/{diff.js, limiter.js, renderer.js, keystats.js, store.js, kpi.js, settings.js, imagestore.js}
│   └── pkg/                      # generado por wasm-pack (no editar)
├── scripts/check-size.mjs
├── tests/*.test.js
└── docs/measurements.md          # resultados de medición (tarea 11)
```

---

### Task 1: Toolchain, crate de Rust, RNG y Grid

**Files:**
- Create: `package.json`, `core/Cargo.toml`, `core/src/lib.rs`, `core/src/rng.rs`, `core/src/grid.rs`

**Interfaces:**
- Produces: `rng::Rng` con `new(seed: u64)`, `next_u32()`, `unit() -> f32` (en `[0,1)`), `range_f32(lo, hi)`, `range_i32(lo, hi)` (semiabierto; devuelve `lo` si el rango está vacío), `range_incl_i32(lo, hi)` (cerrado), `chance(p: f32) -> bool`.
- Produces: `grid::{CELL_STRIDE: usize = 3, NO_COLOR: u32 = 0, rgb(r,g,b) -> u32 (const fn), Grid}`. `Grid` tiene `new(w,h)`, `resize(w,h)`, `clear()`, `width()`, `height()`, `data() -> &[u32]`, `get(x: u16, y: u16) -> Option<(char,u32,u32)>`, `set(x: i32, y: i32, ch, fg, bg)` (ignora fuera de rango), `set_str(x: i32, y: i32, s, fg, bg)`, `fill_bg(bg)`.

- [ ] **Step 1: Instalar el toolchain de WASM**

Run:
```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
wasm-pack --version
```
Expected: el target se instala, `cargo install` compila varios minutos, y `wasm-pack --version` imprime una versión. (Hoy no están instalados: `rustup target list --installed` solo muestra `x86_64-pc-windows-msvc`.)

- [ ] **Step 2: Crear `package.json` y `core/Cargo.toml`**

`package.json`:
```json
{
  "name": "newtab-ascii",
  "private": true,
  "type": "module",
  "scripts": {
    "test:js": "node --test \"tests/**/*.test.js\"",
    "test:rust": "cargo test --manifest-path core/Cargo.toml",
    "test": "npm run test:rust && npm run test:js",
    "build:wasm": "wasm-pack build core --target web --release --out-dir ../extension/pkg --out-name newtab_core",
    "check:size": "node scripts/check-size.mjs"
  }
}
```

`core/Cargo.toml`:
```toml
[package]
name = "newtab-core"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wasm-bindgen = "0.2"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true

[package.metadata.wasm-pack.profile.release]
wasm-opt = ["-Oz", "--all-features"]
```

`core/src/lib.rs`:
```rust
pub mod grid;
pub mod rng;
```

- [ ] **Step 3: Escribir las pruebas de `rng.rs` (fallan)**

Crear `core/src/rng.rs` con solo esto:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..32).filter(|_| a.next_u32() == b.next_u32()).count();
        assert!(same < 4);
    }

    #[test]
    fn seed_zero_still_produces_values() {
        let mut r = Rng::new(0);
        assert!((0..16).any(|_| r.next_u32() != 0));
    }

    #[test]
    fn unit_is_in_half_open_interval() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
    }

    #[test]
    fn range_f32_stays_inside_bounds() {
        let mut r = Rng::new(9);
        for _ in 0..10_000 {
            let v = r.range_f32(-500.0, 500.0);
            assert!((-500.0..500.0).contains(&v));
        }
    }

    #[test]
    fn range_i32_is_half_open_and_empty_range_returns_lo() {
        let mut r = Rng::new(3);
        for _ in 0..1_000 {
            let v = r.range_i32(2, 5);
            assert!((2..5).contains(&v));
        }
        // Rangos vacíos o invertidos no deben hacer panic (rand::gen_range sí lo hace).
        assert_eq!(r.range_i32(5, 5), 5);
        assert_eq!(r.range_i32(5, -3), 5);
    }

    #[test]
    fn range_incl_i32_reaches_both_ends() {
        let mut r = Rng::new(11);
        let mut seen = [false; 3];
        for _ in 0..1_000 {
            seen[r.range_incl_i32(0, 2) as usize] = true;
        }
        assert_eq!(seen, [true, true, true]);
    }

    #[test]
    fn chance_extremes() {
        let mut r = Rng::new(5);
        assert!((0..100).all(|_| !r.chance(0.0)));
        assert!((0..100).all(|_| r.chance(1.0)));
    }
}
```

- [ ] **Step 4: Ejecutar para verificar que falla**

Run: `cargo test --manifest-path core/Cargo.toml rng`
Expected: error de compilación `cannot find type Rng in this scope`.

- [ ] **Step 5: Implementar `Rng`**

Insertar al inicio de `core/src/rng.rs`, encima del bloque `#[cfg(test)]`:
```rust
//! Generador pseudoaleatorio xorshift64* sembrable.
//! Sustituye a `rand::thread_rng()` de cwork: no añade dependencias a WASM y
//! con semilla fija las animaciones son reproducibles en pruebas.

pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed ^ 0x9E37_79B9_7F4A_7C15 }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        if x == 0 {
            x = 0x9E37_79B9_7F4A_7C15;
        }
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// Valor en `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    /// Entero en `[lo, hi)`. Si el rango está vacío devuelve `lo`.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u32() % (hi - lo) as u32) as i32
    }

    /// Entero en `[lo, hi]`.
    pub fn range_incl_i32(&mut self, lo: i32, hi: i32) -> i32 {
        self.range_i32(lo, hi.saturating_add(1))
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
}
```

- [ ] **Step 6: Ejecutar para verificar que pasa**

Run: `cargo test --manifest-path core/Cargo.toml rng`
Expected: 8 pruebas pasan.

- [ ] **Step 7: Escribir las pruebas de `grid.rs` (fallan)**

Crear `core/src/grid.rs` con solo esto:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_packs_with_opaque_alpha_marker() {
        assert_eq!(rgb(1, 2, 3), 0xFF01_0203);
        assert_ne!(rgb(0, 0, 0), NO_COLOR);
    }

    #[test]
    fn new_grid_is_all_blank_cells() {
        let g = Grid::new(4, 3);
        assert_eq!(g.data().len(), 4 * 3 * CELL_STRIDE);
        assert_eq!(g.get(0, 0), Some((' ', NO_COLOR, NO_COLOR)));
        assert_eq!(g.get(3, 2), Some((' ', NO_COLOR, NO_COLOR)));
        assert_eq!(g.get(4, 0), None);
    }

    #[test]
    fn set_writes_one_cell() {
        let mut g = Grid::new(4, 3);
        g.set(2, 1, 'x', rgb(9, 8, 7), rgb(1, 1, 1));
        assert_eq!(g.get(2, 1), Some(('x', rgb(9, 8, 7), rgb(1, 1, 1))));
        assert_eq!(g.get(1, 1), Some((' ', NO_COLOR, NO_COLOR)));
    }

    #[test]
    fn set_ignores_out_of_range_coordinates() {
        let mut g = Grid::new(2, 2);
        let before = g.data().to_vec();
        g.set(-1, 0, 'x', NO_COLOR, NO_COLOR);
        g.set(0, -1, 'x', NO_COLOR, NO_COLOR);
        g.set(2, 0, 'x', NO_COLOR, NO_COLOR);
        g.set(0, 2, 'x', NO_COLOR, NO_COLOR);
        g.set(i32::MAX, i32::MIN, 'x', NO_COLOR, NO_COLOR);
        assert_eq!(g.data(), &before[..]);
    }

    #[test]
    fn set_str_clips_on_both_sides() {
        let mut g = Grid::new(4, 1);
        g.set_str(2, 0, "abcd", NO_COLOR, NO_COLOR);
        assert_eq!(g.get(2, 0).unwrap().0, 'a');
        assert_eq!(g.get(3, 0).unwrap().0, 'b');
        let mut g = Grid::new(4, 1);
        g.set_str(-2, 0, "abcd", NO_COLOR, NO_COLOR);
        assert_eq!(g.get(0, 0).unwrap().0, 'c');
        assert_eq!(g.get(1, 0).unwrap().0, 'd');
    }

    #[test]
    fn fill_bg_paints_every_cell() {
        let mut g = Grid::new(3, 2);
        g.set(1, 1, 'x', rgb(1, 1, 1), NO_COLOR);
        g.fill_bg(rgb(5, 5, 5));
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(g.get(x, y), Some((' ', NO_COLOR, rgb(5, 5, 5))));
            }
        }
    }

    #[test]
    fn resize_changes_length_and_blanks() {
        let mut g = Grid::new(2, 2);
        g.set(0, 0, 'x', rgb(1, 1, 1), NO_COLOR);
        g.resize(5, 4);
        assert_eq!(g.data().len(), 5 * 4 * CELL_STRIDE);
        assert_eq!((g.width(), g.height()), (5, 4));
        assert_eq!(g.get(0, 0), Some((' ', NO_COLOR, NO_COLOR)));
    }

    #[test]
    fn zero_sized_grid_is_safe() {
        let mut g = Grid::new(0, 0);
        assert!(g.data().is_empty());
        g.set(0, 0, 'x', NO_COLOR, NO_COLOR);
        g.set_str(0, 0, "abc", NO_COLOR, NO_COLOR);
        g.fill_bg(rgb(1, 1, 1));
        g.clear();
        g.resize(0, 5);
        assert!(g.data().is_empty());
    }
}
```

- [ ] **Step 8: Ejecutar para verificar que falla**

Run: `cargo test --manifest-path core/Cargo.toml grid`
Expected: error de compilación `cannot find type Grid in this scope`.

- [ ] **Step 9: Implementar `Grid`**

Insertar al inicio de `core/src/grid.rs`, encima del bloque `#[cfg(test)]`:
```rust
//! Cuadrícula de celdas compartida con JS.
//! Cada celda ocupa `CELL_STRIDE` u32 consecutivos: `[carácter, fg, bg]`.
//! Un color `0` (`NO_COLOR`) equivale a `Color::Reset` de crossterm: sin color.

pub const CELL_STRIDE: usize = 3;
pub const NO_COLOR: u32 = 0;

/// Color opaco `0xFFRRGGBB`. El byte alto distingue cualquier color de `NO_COLOR`.
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub struct Grid {
    width: u16,
    height: u16,
    data: Vec<u32>,
}

impl Grid {
    pub fn new(width: u16, height: u16) -> Self {
        let mut grid = Self {
            width,
            height,
            data: vec![0; width as usize * height as usize * CELL_STRIDE],
        };
        grid.clear();
        grid
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.data.resize(width as usize * height as usize * CELL_STRIDE, 0);
        self.clear();
    }

    pub fn clear(&mut self) {
        self.fill_bg(NO_COLOR);
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn data(&self) -> &[u32] {
        &self.data
    }

    pub fn get(&self, x: u16, y: u16) -> Option<(char, u32, u32)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * CELL_STRIDE;
        Some((char::from_u32(self.data[i])?, self.data[i + 1], self.data[i + 2]))
    }

    /// Escribe una celda. Las coordenadas fuera de la cuadrícula se ignoran
    /// (cwork usaba `u16` y desbordaba con anchos pequeños).
    pub fn set(&mut self, x: i32, y: i32, ch: char, fg: u32, bg: u32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * CELL_STRIDE;
        self.data[i] = ch as u32;
        self.data[i + 1] = fg;
        self.data[i + 2] = bg;
    }

    pub fn set_str(&mut self, x: i32, y: i32, s: &str, fg: u32, bg: u32) {
        for (i, ch) in s.chars().enumerate() {
            self.set(x.saturating_add(i as i32), y, ch, fg, bg);
        }
    }

    /// Deja todas las celdas en blanco con el fondo dado.
    pub fn fill_bg(&mut self, bg: u32) {
        for cell in self.data.chunks_exact_mut(CELL_STRIDE) {
            cell[0] = ' ' as u32;
            cell[1] = NO_COLOR;
            cell[2] = bg;
        }
    }
}
```

- [ ] **Step 10: Ejecutar para verificar que pasa**

Run: `cargo test --manifest-path core/Cargo.toml`
Expected: 16 pruebas pasan (8 de `rng`, 8 de `grid`), sin warnings de compilación.

---

### Task 2: Animación `fire`

**Files:**
- Create: `core/src/animation/mod.rs`, `core/src/animation/fire.rs`
- Modify: `core/src/lib.rs`

**Interfaces:**
- Consumes: `Grid`, `rgb`, `NO_COLOR` (Task 1), `Rng` (Task 1).
- Produces: `animation::Animation` (trait con `update(&mut self, width: u16, height: u16)` y `draw(&mut self, grid: &mut Grid)`); `animation::fire::FireAnimation::new(width: u16, height: u16, seed: u32) -> Self`.

- [ ] **Step 1: Crear el trait y registrar módulos**

`core/src/animation/mod.rs`:
```rust
pub mod fire;

use crate::grid::Grid;

pub trait Animation {
    /// Avanza el estado un frame. Recibe el tamaño actual y se adapta si cambió.
    fn update(&mut self, width: u16, height: u16);

    /// Dibuja el estado actual en la cuadrícula.
    fn draw(&mut self, grid: &mut Grid);
}
```

`core/src/lib.rs`:
```rust
pub mod animation;
pub mod grid;
pub mod rng;
```

- [ ] **Step 2: Escribir las pruebas de `fire.rs` (fallan)**

Crear `core/src/animation/fire.rs` con solo esto:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_row_is_white_hot_after_first_update() {
        let mut fire = FireAnimation::new(10, 6, 1);
        let mut grid = Grid::new(10, 6);
        fire.update(10, 6);
        fire.draw(&mut grid);
        for x in 0..10 {
            let (ch, fg, _) = grid.get(x, 5).unwrap();
            assert_eq!(ch, 'M');
            assert_eq!(fg, rgb(255, 255, 255));
        }
    }

    #[test]
    fn heat_reaches_the_top_row_over_time() {
        let mut fire = FireAnimation::new(40, 6, 1);
        let mut grid = Grid::new(40, 6);
        for _ in 0..8 {
            fire.update(40, 6);
        }
        fire.draw(&mut grid);
        let lit = (0..40).filter(|&x| grid.get(x, 0).unwrap().0 != ' ').count();
        assert!(lit > 0, "ninguna celda de la fila superior se encendió");
    }

    #[test]
    fn same_seed_gives_same_frame() {
        let render = |seed| {
            let mut fire = FireAnimation::new(20, 8, seed);
            let mut grid = Grid::new(20, 8);
            for _ in 0..10 {
                fire.update(20, 8);
            }
            fire.draw(&mut grid);
            grid.data().to_vec()
        };
        assert_eq!(render(5), render(5));
    }

    #[test]
    fn resize_resets_the_heat_buffer() {
        let mut fire = FireAnimation::new(10, 6, 1);
        fire.update(10, 6);
        fire.update(20, 8);
        assert_eq!(fire.heat.len(), 20 * 8);
    }

    #[test]
    fn degenerate_sizes_do_not_panic() {
        for (w, h) in [(0, 0), (0, 5), (5, 0), (1, 1), (2, 1), (1, 5), (3, 3)] {
            let mut fire = FireAnimation::new(w, h, 9);
            let mut grid = Grid::new(w, h);
            for _ in 0..30 {
                fire.update(w, h);
                fire.draw(&mut grid);
            }
        }
    }
}
```

- [ ] **Step 3: Ejecutar para verificar que falla**

Agregar `pub mod fire;` ya está en `mod.rs`. Run: `cargo test --manifest-path core/Cargo.toml fire`
Expected: error de compilación `cannot find type FireAnimation in this scope`.

- [ ] **Step 4: Implementar `FireAnimation`**

Insertar al inicio de `core/src/animation/fire.rs`, encima del bloque de pruebas:
```rust
//! Port de `refs/cwork/src/animation/fire.rs`.
//! Cambios: RNG propio, `Grid` en vez de `Buffer`/`crossterm::Color`, y la deriva
//! lateral usa módulo (el original desbordaba `usize` cuando `w < 3`).

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};
use crate::rng::Rng;

pub struct FireAnimation {
    heat: Vec<u8>,
    width: u16,
    height: u16,
    rng: Rng,
}

impl FireAnimation {
    pub fn new(width: u16, height: u16, seed: u32) -> Self {
        Self {
            heat: vec![0; width as usize * height as usize],
            width,
            height,
            rng: Rng::new(seed as u64),
        }
    }
}

/// Intensidad de calor → (color, carácter).
fn fire_glyph(heat: u8) -> (u32, char) {
    match heat {
        0 => (NO_COLOR, ' '),
        1..=4 => (rgb(31, 7, 7), '.'),
        5..=8 => (rgb(71, 15, 15), ':'),
        9..=12 => (rgb(111, 23, 23), 'i'),
        13..=16 => (rgb(151, 31, 31), 'l'),
        17..=20 => (rgb(191, 39, 39), 'w'),
        21..=24 => (rgb(231, 47, 47), 'x'),
        25..=28 => (rgb(255, 106, 7), 'W'),
        29..=32 => (rgb(255, 170, 7), '@'),
        33..=35 => (rgb(255, 233, 127), '#'),
        _ => (rgb(255, 255, 255), 'M'),
    }
}

impl Animation for FireAnimation {
    fn update(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.heat = vec![0; width as usize * height as usize];
        }

        let w = self.width as usize;
        let h = self.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        // La fila inferior es la fuente de calor máximo.
        for x in 0..w {
            self.heat[(h - 1) * w + x] = 36;
        }

        // El calor sube, se enfría al azar y deriva hasta 2 columnas a la izquierda.
        for y in 0..(h - 1) {
            for x in 0..w {
                let heat_val = self.heat[(y + 1) * w + x];
                let decay = self.rng.range_incl_i32(0, 2) as u8;
                let wind = self.rng.range_incl_i32(0, 2) as usize;
                let dest_x = (x + w - wind % w) % w;
                self.heat[y * w + dest_x] = heat_val.saturating_sub(decay);
            }
        }
    }

    fn draw(&mut self, grid: &mut Grid) {
        grid.clear();
        let w = self.width as usize;
        let h = self.height as usize;
        for y in 0..h {
            for x in 0..w {
                let (fg, ch) = fire_glyph(self.heat[y * w + x]);
                grid.set(x as i32, y as i32, ch, fg, NO_COLOR);
            }
        }
    }
}
```

- [ ] **Step 5: Ejecutar para verificar que pasa**

Run: `cargo test --manifest-path core/Cargo.toml fire`
Expected: 5 pruebas pasan. Si `heat_reaches_the_top_row_over_time` fallara con la semilla 1 sería un error de lógica, no de azar: tras 5 actualizaciones el calor ya llegó a la fila 0 en 6 filas.

---

### Task 3: Animación `rain`

**Files:**
- Create: `core/src/animation/rain.rs`
- Modify: `core/src/animation/mod.rs` (agregar `pub mod rain;`)

**Interfaces:**
- Consumes: `Animation`, `Grid`, `rgb`, `NO_COLOR`, `Rng`.
- Produces: `animation::rain::RainAnimation::new(width: u16, height: u16, seed: u32) -> Self`.

- [ ] **Step 1: Registrar el módulo**

En `core/src/animation/mod.rs`, debajo de `pub mod fire;`, agregar `pub mod rain;`.

- [ ] **Step 2: Escribir las pruebas de `rain.rs` (fallan)**

Crear `core/src/animation/rain.rs` con solo esto:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn count_char(grid: &Grid, ch: char) -> usize {
        let mut n = 0;
        for y in 0..grid.height() {
            for x in 0..grid.width() {
                if grid.get(x, y).unwrap().0 == ch {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn spawns_one_drop_per_two_columns() {
        let rain = RainAnimation::new(80, 24, 1);
        assert_eq!(rain.drops.len(), 40);
    }

    #[test]
    fn draws_raindrops_after_falling_for_a_while() {
        let mut rain = RainAnimation::new(80, 24, 1);
        let mut grid = Grid::new(80, 24);
        for _ in 0..60 {
            rain.update(80, 24);
        }
        rain.draw(&mut grid);
        assert!(count_char(&grid, '/') > 0);
    }

    #[test]
    fn lightning_flashes_the_background() {
        let mut rain = RainAnimation::new(80, 24, 3);
        let mut grid = Grid::new(80, 24);
        let mut guard = 0;
        while rain.lightning_timer == 0 {
            rain.update(80, 24);
            guard += 1;
            assert!(guard < 200_000, "no cayó ningún rayo");
        }
        assert_eq!(rain.lightning_timer, 15);
        rain.draw(&mut grid);
        assert_eq!(grid.get(0, 0).unwrap().2, rgb(240, 240, 245));
    }

    #[test]
    fn resize_rebuilds_the_drops() {
        let mut rain = RainAnimation::new(80, 24, 1);
        rain.update(40, 24);
        assert_eq!(rain.drops.len(), 20);
    }

    #[test]
    fn degenerate_sizes_do_not_panic_even_with_lightning() {
        // 5000 frames por tamaño: con p = 0.25 % caen ~12 rayos, que es donde
        // rand::gen_range hacía panic con rangos vacíos.
        for (w, h) in [(0, 0), (1, 1), (4, 2), (5, 1), (6, 1), (6, 40), (80, 24)] {
            let mut rain = RainAnimation::new(w, h, 11);
            let mut grid = Grid::new(w, h);
            for _ in 0..5_000 {
                rain.update(w, h);
                rain.draw(&mut grid);
            }
        }
    }
}
```

- [ ] **Step 3: Ejecutar para verificar que falla**

Run: `cargo test --manifest-path core/Cargo.toml rain`
Expected: error de compilación `cannot find type RainAnimation in this scope`.

- [ ] **Step 4: Implementar `RainAnimation`**

Insertar al inicio de `core/src/animation/rain.rs`, encima del bloque de pruebas:
```rust
//! Port de `refs/cwork/src/animation/rain.rs`.
//! Cambios: RNG propio (los rangos vacíos devuelven el límite inferior en vez de
//! hacer panic), `Grid` en vez de `Buffer`, y `Drop` se renombra a `Raindrop`
//! para no ocultar el trait `Drop` de la biblioteca estándar.

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};
use crate::rng::Rng;

struct Raindrop {
    x: f32,
    y: f32,
    speed: f32,
    len: usize,
}

impl Raindrop {
    fn new(width: u16, rng: &mut Rng) -> Self {
        Self {
            // Puede nacer un poco a la izquierda para entrar en diagonal con el viento.
            x: rng.range_f32(-10.0, width as f32 + 5.0),
            y: rng.range_f32(-20.0, 0.0),
            speed: rng.range_f32(0.8, 1.5),
            len: rng.range_i32(2, 5) as usize,
        }
    }
}

struct Splash {
    x: u16,
    y: u16,
    frame: usize,
}

pub struct RainAnimation {
    drops: Vec<Raindrop>,
    splashes: Vec<Splash>,
    lightning_timer: usize,
    lightning_bolt: Vec<(u16, u16)>,
    width: u16,
    height: u16,
    rng: Rng,
}

impl RainAnimation {
    pub fn new(width: u16, height: u16, seed: u32) -> Self {
        let mut rng = Rng::new(seed as u64);
        let drops = (0..width as usize / 2).map(|_| Raindrop::new(width, &mut rng)).collect();
        Self {
            drops,
            splashes: Vec::new(),
            lightning_timer: 0,
            lightning_bolt: Vec::new(),
            width,
            height,
            rng,
        }
    }
}

impl Animation for RainAnimation {
    fn update(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.drops.clear();
            for _ in 0..width as usize / 2 {
                self.drops.push(Raindrop::new(width, &mut self.rng));
            }
            self.splashes.clear();
            self.lightning_timer = 0;
            self.lightning_bolt.clear();
            return;
        }

        // 1. Mover las gotas (viento constante hacia la derecha).
        for drop in &mut self.drops {
            drop.y += drop.speed;
            drop.x += drop.speed * 0.22;

            if drop.y >= self.height as f32 {
                if drop.x >= 0.0 && drop.x < self.width as f32 {
                    self.splashes.push(Splash {
                        x: drop.x as u16,
                        y: self.height.saturating_sub(1),
                        frame: 0,
                    });
                }
                drop.y = self.rng.range_f32(-15.0, 0.0);
                drop.x = self.rng.range_f32(-15.0, self.width as f32 + 5.0);
                drop.speed = self.rng.range_f32(0.8, 1.5);
                drop.len = self.rng.range_i32(2, 5) as usize;
            }
        }

        // 2. Avanzar los chapoteos.
        for splash in &mut self.splashes {
            splash.frame += 1;
        }
        self.splashes.retain(|s| s.frame < 3);

        // 3. Rayos: ~0.25 % de probabilidad por frame.
        if self.lightning_timer > 0 {
            self.lightning_timer -= 1;
            if self.lightning_timer == 0 {
                self.lightning_bolt.clear();
            }
        } else if self.rng.chance(0.0025) {
            self.lightning_timer = 15;
            self.lightning_bolt.clear();

            let w = self.width as i32;
            let h = self.height as i32;
            let mut bolt_x = self.rng.range_i32(5, w - 5);
            let bolt_len = self.rng.range_i32(h / 2, h);

            for y in 0..bolt_len {
                self.lightning_bolt.push((bolt_x.clamp(0, u16::MAX as i32) as u16, y as u16));
                bolt_x += self.rng.range_incl_i32(-1, 1);
                bolt_x = bolt_x.clamp(0, (w - 1).max(0));
            }
        }
    }

    fn draw(&mut self, grid: &mut Grid) {
        // Color de fondo del destello según la fase del rayo.
        let bg_flash = match self.lightning_timer {
            14 | 15 => rgb(240, 240, 245),
            8..=10 => rgb(130, 130, 145),
            4..=7 => rgb(50, 50, 60),
            _ => NO_COLOR,
        };

        grid.clear();
        if bg_flash != NO_COLOR {
            grid.fill_bg(bg_flash);
        }

        // 1. Rayo.
        if !self.lightning_bolt.is_empty() {
            let bolt_color = rgb(255, 255, 215);
            let ch = if self.lightning_timer % 2 == 0 { '█' } else { '║' };
            for &(bx, by) in &self.lightning_bolt {
                grid.set(bx as i32, by as i32, ch, bolt_color, bg_flash);
            }
        }

        // 2. Gotas.
        let rain_color = rgb(60, 90, 160);
        for drop in &self.drops {
            for i in 0..drop.len {
                let ry = drop.y - i as f32;
                let rx = drop.x - (i as f32 * 0.22);
                if ry >= 0.0 && ry < self.height as f32 && rx >= 0.0 && rx < self.width as f32 {
                    let ch = if i == 0 { '/' } else { '·' };
                    grid.set(rx as i32, ry as i32, ch, rain_color, bg_flash);
                }
            }
        }

        // 3. Chapoteos en el suelo.
        let splash_color = rgb(120, 145, 195);
        for splash in &self.splashes {
            let ch = match splash.frame {
                0 => 'v',
                1 => '·',
                _ => ' ',
            };
            grid.set(splash.x as i32, splash.y as i32, ch, splash_color, bg_flash);
        }
    }
}
```

- [ ] **Step 5: Ejecutar para verificar que pasa**

Run: `cargo test --manifest-path core/Cargo.toml rain`
Expected: 5 pruebas pasan. Nota: `lightning_flashes_the_background` depende de que el rayo aparezca en un frame con `lightning_timer == 15`; el bucle se detiene justo cuando el temporizador se arma, y por eso esa aserción es exacta.

---

### Task 4: Animación `starfield`

**Files:**
- Create: `core/src/animation/starfield.rs`
- Modify: `core/src/animation/mod.rs` (agregar `pub mod starfield;`)

**Interfaces:**
- Consumes: `Animation`, `Grid`, `rgb`, `NO_COLOR`, `Rng`.
- Produces: `animation::starfield::StarfieldAnimation::new(width: u16, height: u16, seed: u32) -> Self`.

- [ ] **Step 1: Registrar el módulo**

En `core/src/animation/mod.rs`, debajo de `pub mod rain;`, agregar `pub mod starfield;`.

- [ ] **Step 2: Escribir las pruebas de `starfield.rs` (fallan)**

Crear `core/src/animation/starfield.rs` con solo esto:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn non_blank(grid: &Grid) -> usize {
        let mut n = 0;
        for y in 0..grid.height() {
            for x in 0..grid.width() {
                if grid.get(x, y).unwrap().0 != ' ' {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn normal_mode_draws_stars() {
        let mut sf = StarfieldAnimation::new(80, 24, 1);
        let mut grid = Grid::new(80, 24);
        sf.update(80, 24);
        sf.draw(&mut grid);
        assert!(non_blank(&grid) > 0);
    }

    #[test]
    fn state_machine_walks_normal_warning_hyperdrive() {
        let mut sf = StarfieldAnimation::new(80, 24, 1);
        for _ in 0..350 {
            sf.update(80, 24);
        }
        assert!(matches!(sf.state, State::Normal { timer: 0 }));
        sf.update(80, 24);
        assert!(matches!(sf.state, State::Warning { timer: 100 }));
        for _ in 0..101 {
            sf.update(80, 24);
        }
        assert!(matches!(sf.state, State::Hyperdrive { timer: 200 }));
    }

    #[test]
    fn speed_matches_the_original_curve() {
        let mut sf = StarfieldAnimation::new(80, 24, 1);
        assert_eq!(sf.speed(), 3.5);
        sf.state = State::Hyperdrive { timer: 10 };
        assert_eq!(sf.speed(), 45.0);
        sf.state = State::Decelerating { timer: 80 };
        assert_eq!(sf.speed(), 45.0);
        sf.state = State::Decelerating { timer: 0 };
        assert_eq!(sf.speed(), 3.5);
    }

    #[test]
    fn warning_state_shows_the_hud_box() {
        let mut sf = StarfieldAnimation::new(80, 24, 1);
        let mut grid = Grid::new(80, 24);
        for _ in 0..351 {
            sf.update(80, 24);
        }
        sf.draw(&mut grid);
        // La caja del aviso se dibuja en la fila justo encima del texto (fila 11).
        let boxed = (0..80).any(|x| grid.get(x, 11).unwrap().0 == '▄');
        assert!(boxed);
    }

    #[test]
    fn hyperdrive_draws_streak_glyphs() {
        let mut sf = StarfieldAnimation::new(80, 24, 1);
        let mut grid = Grid::new(80, 24);
        sf.state = State::Hyperdrive { timer: 200 };
        sf.draw(&mut grid);
        let mut streaks = 0;
        for y in 0..24 {
            for x in 0..80 {
                if "─│╲╱".contains(grid.get(x, y).unwrap().0) {
                    streaks += 1;
                }
            }
        }
        assert!(streaks > 0);
    }

    #[test]
    fn degenerate_sizes_do_not_panic_through_every_state() {
        // 1000 frames recorren Normal → Warning → Hyperdrive → Decelerating.
        // El texto del HUD es más ancho que estas cuadrículas: debe recortarse.
        for (w, h) in [(0, 0), (0, 3), (3, 0), (1, 1), (3, 2), (10, 3), (80, 24)] {
            let mut sf = StarfieldAnimation::new(w, h, 5);
            let mut grid = Grid::new(w, h);
            for _ in 0..1_000 {
                sf.update(w, h);
                sf.draw(&mut grid);
            }
        }
    }
}
```

- [ ] **Step 3: Ejecutar para verificar que falla**

Run: `cargo test --manifest-path core/Cargo.toml starfield`
Expected: error de compilación `cannot find type StarfieldAnimation in this scope`.

- [ ] **Step 4: Implementar `StarfieldAnimation`**

Insertar al inicio de `core/src/animation/starfield.rs`, encima del bloque de pruebas:
```rust
//! Port de `refs/cwork/src/animation/starfield.rs`.
//! Cambios: RNG propio, `Grid` en vez de `Buffer`, coordenadas `i32` (el original
//! restaba en `u16` y desbordaba con anchos menores que el texto del HUD), ancho
//! del HUD en caracteres y no en bytes, y la velocidad se calcula en un único
//! método `speed()` (update y draw usaban fórmulas casi duplicadas; en Hyperdrive
//! y Decelerating, los únicos estados donde draw la usa, eran idénticas).

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};
use crate::rng::Rng;

const WHITE: u32 = rgb(255, 255, 255);
const STAR_COLORS: [u32; 4] = [
    rgb(160, 210, 255), // cian
    rgb(255, 255, 255), // blanco
    rgb(255, 180, 255), // violeta
    rgb(255, 255, 180), // amarillo claro
];

#[derive(Clone)]
struct Star {
    x: f32,
    y: f32,
    z: f32,
    color: u32,
}

impl Star {
    fn new(rng: &mut Rng) -> Self {
        Self {
            x: rng.range_f32(-500.0, 500.0),
            y: rng.range_f32(-250.0, 250.0),
            z: rng.range_f32(10.0, 1000.0),
            color: STAR_COLORS[rng.range_i32(0, STAR_COLORS.len() as i32) as usize],
        }
    }

    fn reset(&mut self, rng: &mut Rng) {
        self.x = rng.range_f32(-500.0, 500.0);
        self.y = rng.range_f32(-250.0, 250.0);
        self.z = 1000.0;
    }
}

#[derive(Clone, Copy)]
enum State {
    Normal { timer: usize },
    Warning { timer: usize },
    Hyperdrive { timer: usize },
    Decelerating { timer: usize },
}

pub struct StarfieldAnimation {
    width: u16,
    height: u16,
    tick: usize,
    stars: Vec<Star>,
    state: State,
    rng: Rng,
}

impl StarfieldAnimation {
    pub fn new(width: u16, height: u16, seed: u32) -> Self {
        let mut rng = Rng::new(seed as u64);
        let stars = (0..150).map(|_| Star::new(&mut rng)).collect();
        Self {
            width,
            height,
            tick: 0,
            stars,
            state: State::Normal { timer: 350 },
            rng,
        }
    }

    fn advance_state(&mut self) {
        self.state = match self.state {
            State::Normal { timer } if timer > 0 => State::Normal { timer: timer - 1 },
            State::Normal { .. } => State::Warning { timer: 100 },
            State::Warning { timer } if timer > 0 => State::Warning { timer: timer - 1 },
            State::Warning { .. } => State::Hyperdrive { timer: 200 },
            State::Hyperdrive { timer } if timer > 0 => State::Hyperdrive { timer: timer - 1 },
            State::Hyperdrive { .. } => State::Decelerating { timer: 80 },
            State::Decelerating { timer } if timer > 0 => State::Decelerating { timer: timer - 1 },
            State::Decelerating { .. } => State::Normal { timer: 450 },
        };
    }

    fn speed(&self) -> f32 {
        match self.state {
            State::Normal { .. } => 3.5,
            State::Warning { timer } => {
                let progress = (100 - timer.min(100)) as f32 / 100.0;
                3.5 + progress * progress * 8.0
            }
            State::Hyperdrive { .. } => 45.0,
            State::Decelerating { timer } => {
                let progress = timer.min(80) as f32 / 80.0;
                3.5 + progress * 41.5
            }
        }
    }
}

impl Animation for StarfieldAnimation {
    fn update(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            for star in &mut self.stars {
                *star = Star::new(&mut self.rng);
            }
            self.state = State::Normal { timer: 350 };
            return;
        }

        self.tick += 1;
        self.advance_state();

        let speed = self.speed();
        for star in &mut self.stars {
            star.z -= speed;
            if star.z <= 10.0 {
                star.reset(&mut self.rng);
            }
        }
    }

    fn draw(&mut self, grid: &mut Grid) {
        grid.clear();
        let w = self.width as f32;
        let h = self.height as f32;
        if w == 0.0 || h == 0.0 {
            return;
        }

        let cx = w / 2.0;
        let cy = h / 2.0;
        // Las celdas son más altas que anchas: se ajusta la proyección.
        let scale_x = cx * 0.7;
        let scale_y = cy * 0.7;

        let is_hyperdrive = matches!(self.state, State::Hyperdrive { .. } | State::Decelerating { .. });
        let speed = self.speed();

        // 1. Estrellas.
        for star in &self.stars {
            let px = cx + (star.x / star.z) * scale_x;
            let py = cy + (star.y / star.z) * scale_y;
            if !(px >= 0.0 && px < w && py >= 0.0 && py < h) {
                continue;
            }

            if is_hyperdrive {
                // Estela alineada con el vector de movimiento.
                let prev_z = star.z + speed;
                let prev_px = cx + (star.x / prev_z) * scale_x;
                let prev_py = cy + (star.y / prev_z) * scale_y;
                let dx = px - prev_px;
                let dy = py - prev_py;
                let ch = if dx.abs() > dy.abs() * 2.2 {
                    '─'
                } else if dy.abs() > dx.abs() * 2.2 {
                    '│'
                } else if dx * dy > 0.0 {
                    '╲'
                } else {
                    '╱'
                };

                const STEPS: usize = 4;
                for s in 0..STEPS {
                    let t = s as f32 / (STEPS - 1) as f32;
                    let ix = prev_px + dx * t;
                    let iy = prev_py + dy * t;
                    let col = if s == STEPS - 1 {
                        WHITE
                    } else {
                        rgb((100.0 * t) as u8, (170.0 * t + 80.0) as u8, 255)
                    };
                    grid.set(ix.round() as i32, iy.round() as i32, ch, col, NO_COLOR);
                }
            } else {
                let ch = if star.z > 800.0 {
                    '.'
                } else if star.z > 500.0 {
                    '·'
                } else if star.z > 300.0 {
                    '*'
                } else if star.z > 150.0 {
                    'o'
                } else {
                    '●'
                };
                let color = if star.z < 150.0 {
                    WHITE
                } else if star.z > 600.0 {
                    rgb(100, 100, 120)
                } else {
                    star.color
                };
                grid.set(px as i32, py as i32, ch, color, NO_COLOR);
            }
        }

        // 2. Texto del HUD.
        match self.state {
            State::Warning { timer } if (timer / 10) % 2 == 0 => {
                let prompt = "⚠️  WARNING: HYPERDRIVE INITIATING  ⚠️";
                let len = prompt.chars().count() as i32;
                let px = (self.width as i32 - len) / 2;
                let py = self.height as i32 / 2;
                let box_color = rgb(50, 0, 0);
                for ox in 0..(len + 4) {
                    grid.set(px - 2 + ox, py - 1, '▄', box_color, NO_COLOR);
                    grid.set(px - 2 + ox, py + 1, '▀', box_color, NO_COLOR);
                }
                grid.set_str(px, py, prompt, rgb(255, 50, 50), NO_COLOR);
            }
            State::Hyperdrive { .. } if (self.tick / 15) % 2 == 0 => {
                let prompt = "⚡ HYPERDRIVE ACTIVE ⚡";
                let len = prompt.chars().count() as i32;
                let px = (self.width as i32 - len) / 2;
                grid.set_str(px, 1, prompt, rgb(0, 255, 255), NO_COLOR);
            }
            _ => {}
        }
    }
}
```

- [ ] **Step 5: Ejecutar para verificar que pasa**

Run: `cargo test --manifest-path core/Cargo.toml starfield`
Expected: 6 pruebas pasan. `warning_state_shows_the_hud_box` solo vale para `80×24`: el texto queda en `py = 12` y la caja superior en la fila 11. Si falla la fila, verifica primero que tras 351 actualizaciones el estado sea `Warning { timer: 100 }` (se cubre en `state_machine_walks_normal_warning_hyperdrive`).

---

### Task 5: Conversión imagen → ASCII

**Files:**
- Create: `core/src/ascii.rs`
- Modify: `core/src/lib.rs` (agregar `pub mod ascii;`)

**Interfaces:**
- Consumes: `rgb` (Task 1).
- Produces:
  - `ascii::RAMP: [char; 10]`, `ascii::PACKED_STRIDE: usize = 4`.
  - `ascii::target_dims(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32)`.
  - `ascii::image_to_packed(rgba: &[u8], width: u32, height: u32, invert: bool) -> Vec<u8>` (vacío si los datos no cuadran). Formato: `[índice_en_RAMP, r, g, b]` por celda.
  - `ascii::packed_glyph(packed: &[u8], index: usize) -> Option<(char, u32)>` (carácter y color `rgb`).

- [ ] **Step 1: Registrar el módulo**

`core/src/lib.rs` queda:
```rust
pub mod animation;
pub mod ascii;
pub mod grid;
pub mod rng;
```

- [ ] **Step 2: Escribir las pruebas de `ascii.rs` (fallan)**

Crear `core/src/ascii.rs` con solo esto:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::rgb;

    #[test]
    fn wide_image_fills_the_width() {
        // 1000×500 → ratio 0.5 * 0.55 = 0.275 → 200 × 55
        assert_eq!(target_dims(1000, 500, 200, 100), (200, 55));
    }

    #[test]
    fn tall_image_is_limited_by_height() {
        // 500×1000 → ratio 1.1 → alto 220 > 100 → 100 × 91
        assert_eq!(target_dims(500, 1000, 200, 100), (91, 100));
    }

    #[test]
    fn extreme_aspect_ratios_stay_valid() {
        assert_eq!(target_dims(10_000, 1, 200, 100), (200, 1));
        let (w, h) = target_dims(1, 10_000, 200, 100);
        assert_eq!(h, 100);
        assert!(w >= 1 && w <= 200);
    }

    #[test]
    fn zero_or_tiny_inputs_give_one_cell() {
        assert_eq!(target_dims(0, 100, 200, 100), (1, 1));
        assert_eq!(target_dims(100, 0, 200, 100), (1, 1));
        assert_eq!(target_dims(100, 100, 0, 100), (1, 1));
        // Cuadrada: al ancho máximo le tocarían 110 filas (> 100), así que manda el alto.
        assert_eq!(target_dims(1, 1, 200, 100), (182, 100));
    }

    #[test]
    fn white_maps_to_densest_and_black_to_blank() {
        let rgba = [255, 255, 255, 255, 0, 0, 0, 255];
        let packed = image_to_packed(&rgba, 2, 1, false);
        assert_eq!(packed, vec![9, 255, 255, 255, 0, 0, 0, 0]);
        assert_eq!(packed_glyph(&packed, 0), Some(('@', rgb(255, 255, 255))));
        assert_eq!(packed_glyph(&packed, 1), Some((' ', rgb(0, 0, 0))));
    }

    #[test]
    fn invert_flips_the_ramp_but_keeps_the_color() {
        let rgba = [255, 255, 255, 255];
        let packed = image_to_packed(&rgba, 1, 1, true);
        assert_eq!(packed, vec![0, 255, 255, 255]);
    }

    #[test]
    fn fully_transparent_pixels_become_blank_black() {
        let rgba = [255, 255, 255, 0];
        let packed = image_to_packed(&rgba, 1, 1, false);
        assert_eq!(packed, vec![0, 0, 0, 0]);
    }

    #[test]
    fn semi_transparent_pixels_are_composited_over_black() {
        let rgba = [200, 100, 50, 128];
        let packed = image_to_packed(&rgba, 1, 1, false);
        assert_eq!(&packed[1..], &[100, 50, 25]);
    }

    #[test]
    fn mismatched_buffer_length_returns_empty() {
        assert!(image_to_packed(&[0, 0, 0, 255], 2, 1, false).is_empty());
        assert!(image_to_packed(&[], 0, 0, false).is_empty());
        assert!(image_to_packed(&[0; 8], 1, 1, false).is_empty());
    }

    #[test]
    fn packed_glyph_rejects_bad_indices_and_corrupt_ramp_values() {
        let packed = vec![9, 1, 2, 3];
        assert!(packed_glyph(&packed, 1).is_none());
        assert!(packed_glyph(&packed, usize::MAX).is_none());
        let corrupt = vec![200, 1, 2, 3];
        assert!(packed_glyph(&corrupt, 0).is_none());
    }

    #[test]
    fn max_grid_fits_in_the_storage_budget() {
        // Spec: imagen guardada ≤ 200 KB con cuadrícula máxima 200×100.
        assert!(200 * 100 * PACKED_STRIDE <= 200 * 1024);
    }
}
```
- [ ] **Step 3: Ejecutar para verificar que falla**

Run: `cargo test --manifest-path core/Cargo.toml ascii`
Expected: error de compilación `cannot find function target_dims in this scope`.

- [ ] **Step 4: Implementar `ascii.rs`**

Insertar al inicio de `core/src/ascii.rs`, encima del bloque de pruebas:
```rust
//! Conversión imagen → ASCII (lógica de `refs/rust-ascii/src/bin/ascii.rs`).
//! JS reduce la imagen (`createImageBitmap` + `OffscreenCanvas`) y entrega RGBA ya
//! al tamaño de la cuadrícula; Rust solo mapea brillo → carácter. Así se evita la
//! crate `image`, que no cabría en el tope de 150 KB del `.wasm`.

use crate::grid::rgb;

/// De más oscuro a más brillante (el fondo de la página es oscuro).
pub const RAMP: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

/// Bytes por celda empaquetada: `[índice en RAMP, r, g, b]`.
pub const PACKED_STRIDE: usize = 4;

/// Corrección de aspecto: una celda de texto es ~1.8 veces más alta que ancha.
const CELL_ASPECT: f32 = 0.55;

/// Tamaño de la cuadrícula ASCII que conserva la proporción de la imagen sin
/// pasar de `max_w × max_h`. Siempre devuelve al menos `(1, 1)`.
pub fn target_dims(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || max_w == 0 || max_h == 0 {
        return (1, 1);
    }
    let ratio = src_h as f32 / src_w as f32 * CELL_ASPECT;
    let mut w = max_w;
    let mut h = (w as f32 * ratio).round() as u32;
    if h > max_h {
        h = max_h;
        w = (h as f32 / ratio).round() as u32;
    }
    (w.clamp(1, max_w), h.clamp(1, max_h))
}

/// RGBA (`width * height * 4` bytes) → celdas empaquetadas. Devuelve un vector
/// vacío si las dimensiones no cuadran con el buffer. La transparencia se
/// compone sobre negro. `invert` invierte la rampa de caracteres.
pub fn image_to_packed(rgba: &[u8], width: u32, height: u32, invert: bool) -> Vec<u8> {
    let cells = match (width as usize).checked_mul(height as usize) {
        Some(c) if c > 0 => c,
        _ => return Vec::new(),
    };
    if cells.checked_mul(4) != Some(rgba.len()) {
        return Vec::new();
    }

    let last = RAMP.len() - 1;
    let mut out = Vec::with_capacity(cells * PACKED_STRIDE);
    for px in rgba.chunks_exact(4) {
        let a = px[3] as u32;
        let r = (px[0] as u32 * a / 255) as u8;
        let g = (px[1] as u32 * a / 255) as u8;
        let b = (px[2] as u32 * a / 255) as u8;

        // Brillo percibido (pesos BT.709), igual que rust-ascii.
        let brightness = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
        let mut idx = ((brightness / 255.0) * last as f32).round() as usize;
        idx = idx.min(last);
        if invert {
            idx = last - idx;
        }
        out.extend_from_slice(&[idx as u8, r, g, b]);
    }
    out
}

/// Carácter y color (`rgb`) de la celda `index` de un buffer empaquetado.
pub fn packed_glyph(packed: &[u8], index: usize) -> Option<(char, u32)> {
    let base = index.checked_mul(PACKED_STRIDE)?;
    let end = base.checked_add(PACKED_STRIDE)?;
    let cell = packed.get(base..end)?;
    let ch = *RAMP.get(cell[0] as usize)?;
    Some((ch, rgb(cell[1], cell[2], cell[3])))
}
```

- [ ] **Step 5: Ejecutar para verificar que pasa**

Run: `cargo test --manifest-path core/Cargo.toml ascii`
Expected: 11 pruebas pasan. Comprobaciones aritméticas: `500×1000` da `ratio = 1.1`, alto `220 > 100` → alto 100 y ancho `round(100 / 1.1) = 91`; `(1,1)` en `200×100` da alto 110 → `(182, 100)`.

---

### Task 6: `Engine` (API de WASM), build y verificación de tamaño

**Files:**
- Create: `core/src/engine.rs`, `scripts/check-size.mjs`, `tests/wasm-smoke.test.js`
- Modify: `core/src/lib.rs`

**Interfaces:**
- Consumes: `Animation`, `FireAnimation`, `RainAnimation`, `StarfieldAnimation`, `Grid`, `CELL_STRIDE`, `NO_COLOR`, `ascii::{image_to_packed, packed_glyph, target_dims, PACKED_STRIDE}`.
- Produces (API de WASM, tal como la ve JS desde `extension/pkg/newtab_core.js`):
  - `class Engine { constructor(seed: u32, width: u16, height: u16); set_animation(name: string): boolean; load_image(packed: Uint8Array, width: u32, height: u32): boolean; resize(width: u16, height: u16): void; step(): void; render(): void; cells_ptr(): number; cells_len(): number; width(): number; height(): number }`
  - `cell_stride(): number` (= 3), `image_target_dims(src_w, src_h, max_w, max_h): Uint32Array` (`[w, h]`), `image_to_ascii(rgba: Uint8Array, width, height, invert): Uint8Array`.

- [ ] **Step 1: Registrar el módulo**

`core/src/lib.rs` queda:
```rust
pub mod animation;
pub mod ascii;
pub mod engine;
pub mod grid;
pub mod rng;
```

- [ ] **Step 2: Escribir las pruebas de `engine.rs` (fallan)**

Crear `core/src/engine.rs` con solo esto:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{rgb, CELL_STRIDE};

    fn cell(idx: u8, r: u8, g: u8, b: u8) -> [u8; 4] {
        [idx, r, g, b]
    }

    #[test]
    fn rejects_unknown_animation_and_accepts_known_ones() {
        let mut e = Engine::new(1, 10, 5);
        assert!(!e.set_animation("nope"));
        assert!(e.set_animation("fire"));
        assert!(e.set_animation("rain"));
        assert!(e.set_animation("starfield"));
    }

    #[test]
    fn step_draws_fire_into_the_shared_buffer() {
        let mut e = Engine::new(1, 40, 10);
        assert!(e.set_animation("fire"));
        e.step();
        assert_eq!(e.cells_len(), 40 * 10 * CELL_STRIDE);
        assert_eq!(e.grid.get(0, 9).unwrap().0, 'M');
    }

    #[test]
    fn step_without_animation_is_a_noop() {
        let mut e = Engine::new(1, 10, 5);
        let before = e.grid.data().to_vec();
        e.step();
        assert_eq!(e.grid.data(), &before[..]);
    }

    #[test]
    fn centers_a_small_image() {
        let mut e = Engine::new(1, 10, 5);
        let mut data = Vec::new();
        data.extend_from_slice(&cell(9, 255, 0, 0));
        data.extend_from_slice(&cell(0, 0, 0, 0));
        assert!(e.load_image(&data, 2, 1));
        // offset = ((10-2)/2, (5-1)/2) = (4, 2)
        assert_eq!(e.grid.get(4, 2), Some(('@', rgb(255, 0, 0), NO_COLOR)));
        assert_eq!(e.grid.get(5, 2).unwrap().0, ' ');
        assert_eq!(e.grid.get(3, 2).unwrap().0, ' ');
    }

    #[test]
    fn downscales_an_image_that_does_not_fit() {
        let mut e = Engine::new(1, 10, 5);
        let data: Vec<u8> = (0..20 * 10).flat_map(|_| cell(9, 10, 20, 30)).collect();
        assert!(e.load_image(&data, 20, 10));
        for y in 0..5 {
            for x in 0..10 {
                assert_eq!(e.grid.get(x, y).unwrap().0, '@');
            }
        }
    }

    #[test]
    fn rejects_images_with_wrong_length_or_zero_size() {
        let mut e = Engine::new(1, 10, 5);
        assert!(e.set_animation("fire"));
        assert!(!e.load_image(&[9, 0, 0, 0], 2, 1));
        assert!(!e.load_image(&[], 0, 0));
        assert!(!e.load_image(&[9, 0, 0, 0], u32::MAX, u32::MAX));
        // Una carga fallida no debe quitar la animación activa.
        assert!(e.image.is_none());
        assert!(e.anim.is_some());
    }

    #[test]
    fn resize_keeps_the_image_and_resizes_the_buffer() {
        let mut e = Engine::new(1, 10, 5);
        assert!(e.load_image(&cell(9, 1, 2, 3), 1, 1));
        e.resize(20, 8);
        assert_eq!(e.cells_len(), 20 * 8 * CELL_STRIDE);
        assert_eq!(e.grid.get(10, 4).unwrap().0, '@');
    }

    #[test]
    fn static_image_ignores_step() {
        let mut e = Engine::new(1, 10, 5);
        assert!(e.load_image(&cell(9, 1, 2, 3), 1, 1));
        let before = e.grid.data().to_vec();
        e.step();
        assert_eq!(e.grid.data(), &before[..]);
    }

    #[test]
    fn switching_from_image_to_animation_drops_the_image() {
        let mut e = Engine::new(1, 10, 5);
        assert!(e.load_image(&cell(9, 1, 2, 3), 1, 1));
        assert!(e.set_animation("fire"));
        assert!(e.image.is_none());
    }

    #[test]
    fn zero_sized_engine_never_panics() {
        let mut e = Engine::new(1, 0, 0);
        assert!(e.set_animation("rain"));
        e.step();
        assert!(e.load_image(&cell(9, 1, 2, 3), 1, 1));
        e.resize(0, 7);
        e.render();
        assert_eq!(e.cells_len(), 0);
    }

    #[test]
    fn exported_helpers_delegate_to_ascii() {
        assert_eq!(cell_stride(), CELL_STRIDE);
        assert_eq!(image_target_dims(1000, 500, 200, 100), vec![200, 55]);
        assert_eq!(image_to_ascii(&[255, 255, 255, 255], 1, 1, false), vec![9, 255, 255, 255]);
    }
}
```

- [ ] **Step 3: Ejecutar para verificar que falla**

Run: `cargo test --manifest-path core/Cargo.toml engine`
Expected: error de compilación `cannot find type Engine in this scope`.

- [ ] **Step 4: Implementar `Engine`**

Insertar al inicio de `core/src/engine.rs`, encima del bloque de pruebas:
```rust
//! API de WASM: un `Engine` que muestra una animación o una imagen estática.
//! JS lee el buffer de celdas directamente de la memoria de WASM
//! (`cells_ptr` / `cells_len`), sin serializar nada por frame.

use wasm_bindgen::prelude::*;

use crate::animation::{fire::FireAnimation, rain::RainAnimation, starfield::StarfieldAnimation, Animation};
use crate::ascii::{self, packed_glyph, PACKED_STRIDE};
use crate::grid::{Grid, CELL_STRIDE, NO_COLOR};

struct StaticImage {
    packed: Vec<u8>,
    width: u32,
    height: u32,
}

#[wasm_bindgen]
pub struct Engine {
    grid: Grid,
    anim: Option<Box<dyn Animation>>,
    image: Option<StaticImage>,
    seed: u32,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, width: u16, height: u16) -> Engine {
        Engine {
            grid: Grid::new(width, height),
            anim: None,
            image: None,
            seed,
        }
    }

    /// Activa `fire`, `rain` o `starfield`. Devuelve `false` si el nombre no existe.
    pub fn set_animation(&mut self, name: &str) -> bool {
        let (w, h) = (self.grid.width(), self.grid.height());
        let anim: Box<dyn Animation> = match name {
            "fire" => Box::new(FireAnimation::new(w, h, self.seed)),
            "rain" => Box::new(RainAnimation::new(w, h, self.seed)),
            "starfield" => Box::new(StarfieldAnimation::new(w, h, self.seed)),
            _ => return false,
        };
        self.anim = Some(anim);
        self.image = None;
        self.grid.clear();
        true
    }

    /// Muestra una imagen empaquetada (ver `ascii::image_to_packed`). Devuelve
    /// `false` y no cambia nada si el tamaño no cuadra con los datos.
    pub fn load_image(&mut self, packed: &[u8], width: u32, height: u32) -> bool {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|cells| cells.checked_mul(PACKED_STRIDE));
        match expected {
            Some(n) if n > 0 && n == packed.len() => {}
            _ => return false,
        }
        self.anim = None;
        self.image = Some(StaticImage {
            packed: packed.to_vec(),
            width,
            height,
        });
        self.render();
        true
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.grid.resize(width, height);
        self.render();
    }

    /// Avanza un frame. No hace nada con una imagen estática.
    pub fn step(&mut self) {
        let (w, h) = (self.grid.width(), self.grid.height());
        if let Some(anim) = self.anim.as_mut() {
            anim.update(w, h);
            anim.draw(&mut self.grid);
        }
    }

    /// Redibuja el estado actual sin avanzarlo.
    pub fn render(&mut self) {
        if self.image.is_some() {
            self.draw_image();
        } else if let Some(anim) = self.anim.as_mut() {
            anim.draw(&mut self.grid);
        }
    }

    pub fn cells_ptr(&self) -> *const u32 {
        self.grid.data().as_ptr()
    }

    pub fn cells_len(&self) -> usize {
        self.grid.data().len()
    }

    pub fn width(&self) -> u16 {
        self.grid.width()
    }

    pub fn height(&self) -> u16 {
        self.grid.height()
    }
}

impl Engine {
    /// Dibuja la imagen centrada; si no cabe, la reduce por vecino más cercano.
    fn draw_image(&mut self) {
        self.grid.clear();
        let Some(img) = self.image.as_ref() else { return };
        let (gw, gh) = (self.grid.width() as u32, self.grid.height() as u32);
        if gw == 0 || gh == 0 {
            return;
        }

        let (fw, fh) = fit(img.width, img.height, gw, gh);
        let (ox, oy) = ((gw - fw) / 2, (gh - fh) / 2);
        for dy in 0..fh {
            for dx in 0..fw {
                let sx = (dx as u64 * img.width as u64 / fw as u64) as u32;
                let sy = (dy as u64 * img.height as u64 / fh as u64) as u32;
                let index = sy as usize * img.width as usize + sx as usize;
                if let Some((ch, fg)) = packed_glyph(&img.packed, index) {
                    self.grid.set((ox + dx) as i32, (oy + dy) as i32, ch, fg, NO_COLOR);
                }
            }
        }
    }
}

/// Tamaño con el que cabe una imagen `iw × ih` en `gw × gh` sin ampliarla.
fn fit(iw: u32, ih: u32, gw: u32, gh: u32) -> (u32, u32) {
    if iw <= gw && ih <= gh {
        return (iw, ih);
    }
    let scale = (gw as f32 / iw as f32).min(gh as f32 / ih as f32);
    (
        ((iw as f32 * scale) as u32).clamp(1, gw),
        ((ih as f32 * scale) as u32).clamp(1, gh),
    )
}

/// Número de `u32` por celda en el buffer compartido (para que JS no lo duplique).
#[wasm_bindgen]
pub fn cell_stride() -> usize {
    CELL_STRIDE
}

/// `[ancho, alto]` de la cuadrícula ASCII para una imagen de `src_w × src_h`.
#[wasm_bindgen]
pub fn image_target_dims(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> Vec<u32> {
    let (w, h) = ascii::target_dims(src_w, src_h, max_w, max_h);
    vec![w, h]
}

/// RGBA ya reducido a `width × height` → celdas empaquetadas (vacío si no cuadra).
#[wasm_bindgen]
pub fn image_to_ascii(rgba: &[u8], width: u32, height: u32, invert: bool) -> Vec<u8> {
    ascii::image_to_packed(rgba, width, height, invert)
}
```

- [ ] **Step 5: Ejecutar toda la suite de Rust**

Run: `cargo test --manifest-path core/Cargo.toml`
Expected: todas las pruebas pasan (rng 8, grid 8, fire 5, rain 5, starfield 6, ascii 11, engine 11 = 54), sin warnings.

- [ ] **Step 6: Escribir `scripts/check-size.mjs`**

```js
// Falla si se superan los topes de tamaño del spec (sección 5).
import { readdirSync, statSync } from 'node:fs';
import { extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../extension/', import.meta.url));
const LIMITS = { wasm: 150 * 1024, text: 50 * 1024 };

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)]
  );
}

let wasm = 0;
let text = 0;
for (const file of walk(root)) {
  const ext = extname(file);
  const size = statSync(file).size;
  if (ext === '.wasm') wasm += size;
  else if (['.js', '.css', '.html'].includes(ext)) text += size;
}

const kb = (n) => `${(n / 1024).toFixed(1)} KB`;
console.log(`wasm:         ${kb(wasm)} / ${kb(LIMITS.wasm)}`);
console.log(`js+css+html:  ${kb(text)} / ${kb(LIMITS.text)}`);

const failures = [];
if (wasm === 0) failures.push('No hay .wasm: ejecuta `npm run build:wasm` primero.');
if (wasm > LIMITS.wasm) failures.push('El .wasm supera el tope.');
if (text > LIMITS.text) failures.push('JS+CSS+HTML supera el tope.');
if (failures.length) {
  console.error(failures.join('\n'));
  process.exit(1);
}
```

- [ ] **Step 7: Escribir la prueba de humo JS ↔ WASM (falla: aún no hay build)**

`tests/wasm-smoke.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';

const wasmPath = new URL('../extension/pkg/newtab_core_bg.wasm', import.meta.url);
const glue = '../extension/pkg/newtab_core.js';
const built = existsSync(wasmPath);
const opts = { skip: built ? false : 'falta extension/pkg: ejecuta npm run build:wasm' };

async function load() {
  const mod = await import(glue);
  const wasm = await mod.default({ module_or_path: readFileSync(wasmPath) });
  return { mod, wasm };
}

test('el buffer de celdas se comparte por memoria y respeta el stride', opts, async () => {
  const { mod, wasm } = await load();
  const stride = mod.cell_stride();
  assert.equal(stride, 3);

  const engine = new mod.Engine(1, 20, 10);
  assert.equal(engine.set_animation('fire'), true);
  engine.step();

  assert.equal(engine.cells_len(), 20 * 10 * stride);
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  const bottomLeft = (9 * 20 + 0) * stride;
  assert.equal(String.fromCodePoint(cells[bottomLeft]), 'M');
  assert.equal(cells[bottomLeft + 1], 0xffffffff);
  engine.free();
});

test('una animación desconocida se rechaza sin romper el motor', opts, async () => {
  const { mod } = await load();
  const engine = new mod.Engine(1, 5, 5);
  assert.equal(engine.set_animation('nope'), false);
  engine.free();
});

test('imagen → ASCII empaquetado → motor', opts, async () => {
  const { mod, wasm } = await load();
  const [w, h] = mod.image_target_dims(1000, 500, 200, 100);
  assert.deepEqual([w, h], [200, 55]);

  const packed = mod.image_to_ascii(new Uint8Array([255, 255, 255, 255]), 1, 1, false);
  assert.deepEqual([...packed], [9, 255, 255, 255]);

  const engine = new mod.Engine(1, 5, 3);
  assert.equal(engine.load_image(packed, 1, 1), true);
  const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
  assert.equal(String.fromCodePoint(cells[(1 * 5 + 2) * 3]), '@');
  assert.equal(engine.load_image(new Uint8Array([1, 2, 3]), 1, 1), false);
  engine.free();
});
```

- [ ] **Step 8: Compilar a WASM**

Run: `npm run build:wasm`
Expected: `wasm-pack` descarga `wasm-bindgen` y `wasm-opt` la primera vez, y termina con `Your wasm pkg is ready`. Genera `extension/pkg/newtab_core.js` y `extension/pkg/newtab_core_bg.wasm`.
Si `wasm-opt` falla con un error de características no habilitadas, cambia en `core/Cargo.toml` la línea `wasm-opt = ["-Oz", "--all-features"]` por `wasm-opt = false`, vuelve a compilar y anota el tamaño resultante en `docs/measurements.md` (tarea 11).

- [ ] **Step 9: Verificar contrato JS↔WASM y tamaño**

Run: `npm run test:js`
Expected: las 3 pruebas de `wasm-smoke` pasan. Si Node avisa que pasar bytes directamente a `init` está deprecado, es solo un warning; si la versión instalada rechaza `{ module_or_path }`, cambia la llamada a `mod.default(readFileSync(wasmPath))`.

Run: `npm run check:size`
Expected: imprime `wasm: … / 150.0 KB` y sale con código 0 (JS+CSS+HTML aún solo cuenta el glue de `pkg/`, unos pocos KB).

---

### Task 7: Lógica pura de teclas (`keyname.js`, `keystats.js`)

**Files:**
- Create: `extension/content/keyname.js`, `extension/src/keystats.js`, `tests/keyname.test.js`, `tests/keystats.test.js`

**Interfaces:**
- Produces (`keyname.js`, script clásico): `globalThis.NT.keyName(event) -> { key: string, shortcut: string | null } | null`.
- Produces (`keystats.js`, módulo ES):
  - Constantes `TOP_LIMIT = 20`, `KEEP_DAYS = 90`, `WPM_WINDOW_MIN = 5`, `OTHERS = 'otros'`.
  - `dateKey(ms) -> 'YYYY-MM-DD'` (hora local), `cutoffKey(ms, keepDays = KEEP_DAYS) -> string`.
  - `emptyDay() -> { total, keys, shortcuts, sites, active_ms, recent }`.
  - `applyBatch(day, batch, nowMs) -> day` (muta y devuelve `day`); `batch = { host: string, total: number, keys: {[k]: number}, shortcuts: {[k]: number} }`.
  - `pruneTop(map, limit = TOP_LIMIT) -> map`, `pruneDay(day) -> day`.
  - `wpm(recent, nowMs) -> number`, `topEntries(map, n = 5) -> [string, number][]`.
  - `summarize(day | undefined, nowMs) -> { total, wpm, topKeys, topShortcuts, topSites }`.

- [ ] **Step 1: Escribir las pruebas de `keyname` (fallan)**

`tests/keyname.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import '../extension/content/keyname.js';

const { keyName } = globalThis.NT;
const ev = (o) => ({
  key: '',
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  shiftKey: false,
  getModifierState: () => false,
  ...o,
});

test('una letra simple es una tecla sin atajo', () => {
  assert.deepEqual(keyName(ev({ key: 'e' })), { key: 'e', shortcut: null });
});

test('la mayúscula con shift cuenta como la misma tecla', () => {
  assert.deepEqual(keyName(ev({ key: 'E', shiftKey: true })), { key: 'e', shortcut: null });
});

test('espacio, escape y flechas tienen nombre legible', () => {
  assert.equal(keyName(ev({ key: ' ' })).key, 'space');
  assert.equal(keyName(ev({ key: 'Escape' })).key, 'esc');
  assert.equal(keyName(ev({ key: 'ArrowLeft' })).key, 'left');
  assert.equal(keyName(ev({ key: 'Enter' })).key, 'enter');
  assert.equal(keyName(ev({ key: 'Backspace' })).key, 'backspace');
});

test('los modificadores solos no cuentan', () => {
  for (const key of ['Control', 'Shift', 'Alt', 'Meta', 'AltGraph']) {
    assert.equal(keyName(ev({ key })), null, key);
  }
});

test('ctrl+c es un atajo', () => {
  assert.deepEqual(keyName(ev({ key: 'c', ctrlKey: true })), { key: 'c', shortcut: 'ctrl+c' });
});

test('el orden de modificadores es estable: ctrl, alt, meta, shift', () => {
  assert.equal(keyName(ev({ key: 't', ctrlKey: true, shiftKey: true })).shortcut, 'ctrl+shift+t');
  assert.equal(keyName(ev({ key: 'l', metaKey: true })).shortcut, 'meta+l');
  assert.equal(keyName(ev({ key: 'x', ctrlKey: true, altKey: true, metaKey: true, shiftKey: true })).shortcut, 'ctrl+alt+meta+shift+x');
});

test('AltGr (ctrl+alt en Windows) escribe un carácter y no es un atajo', () => {
  const altGr = ev({ key: '@', ctrlKey: true, altKey: true, getModifierState: (m) => m === 'AltGraph' });
  assert.deepEqual(keyName(altGr), { key: '@', shortcut: null });
});

test('teclas muertas, IME y desconocidas no cuentan', () => {
  for (const key of ['Dead', 'Unidentified', 'Process', '']) {
    assert.equal(keyName(ev({ key })), null, JSON.stringify(key));
  }
  assert.equal(keyName(ev({ key: undefined })), null);
  assert.equal(keyName(ev({ key: 42 })), null);
});

test('nombres absurdamente largos se descartan', () => {
  assert.equal(keyName(ev({ key: 'x'.repeat(50) })), null);
});
```

- [ ] **Step 2: Ejecutar para verificar que falla**

Run: `node --test tests/keyname.test.js`
Expected: FAIL con `Cannot find module` (el archivo no existe).

- [ ] **Step 3: Implementar `keyname.js`**

`extension/content/keyname.js`:
```js
// Script clásico: los content scripts de MV3 no pueden usar `import` estático.
// Expone `NT.keyName` en el ámbito global del script.
(function (root) {
  const MODIFIERS = new Set(['Control', 'Shift', 'Alt', 'Meta', 'AltGraph']);
  const IGNORED = new Set(['Dead', 'Unidentified', 'Process']);
  const NAMES = {
    ' ': 'space',
    Escape: 'esc',
    ArrowUp: 'up',
    ArrowDown: 'down',
    ArrowLeft: 'left',
    ArrowRight: 'right',
  };
  const MAX_NAME = 20;

  /**
   * Convierte un KeyboardEvent en `{ key, shortcut }`, o `null` si no debe contarse.
   * Solo devuelve el *nombre* de la tecla, nunca el texto escrito.
   */
  function keyName(e) {
    const k = e.key;
    if (typeof k !== 'string' || k === '' || MODIFIERS.has(k) || IGNORED.has(k)) return null;

    const key = NAMES[k] ?? k.toLowerCase();
    if (key.length > MAX_NAME) return null;

    // AltGr llega como ctrl+alt en Windows: escribe un carácter, no es un atajo.
    const altGraph = typeof e.getModifierState === 'function' && e.getModifierState('AltGraph');
    let shortcut = null;
    if ((e.ctrlKey || e.altKey || e.metaKey) && !altGraph) {
      const parts = [];
      if (e.ctrlKey) parts.push('ctrl');
      if (e.altKey) parts.push('alt');
      if (e.metaKey) parts.push('meta');
      if (e.shiftKey) parts.push('shift');
      parts.push(key);
      shortcut = parts.join('+');
    }
    return { key, shortcut };
  }

  root.NT = root.NT || {};
  root.NT.keyName = keyName;
})(globalThis);
```

- [ ] **Step 4: Ejecutar para verificar que pasa**

Run: `node --test tests/keyname.test.js`
Expected: 9 pruebas pasan.

- [ ] **Step 5: Escribir las pruebas de `keystats` (fallan)**

`tests/keystats.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  OTHERS,
  applyBatch,
  cutoffKey,
  dateKey,
  emptyDay,
  pruneDay,
  pruneTop,
  summarize,
  topEntries,
  wpm,
} from '../extension/src/keystats.js';

const MIN = 60_000;
const T0 = new Date(2026, 9, 8, 15, 0, 0).getTime(); // 8 oct 2026, 15:00 local
const batch = (o = {}) => ({ host: 'github.com', total: 3, keys: { e: 2, space: 1 }, shortcuts: {}, ...o });

test('dateKey usa la fecha local', () => {
  assert.equal(dateKey(T0), '2026-10-08');
});

test('cutoffKey resta 90 días', () => {
  assert.equal(cutoffKey(T0), '2026-07-10');
});

test('applyBatch acumula total, teclas, atajos y sitios', () => {
  const day = emptyDay();
  applyBatch(day, batch({ shortcuts: { 'ctrl+c': 1 } }), T0);
  applyBatch(day, batch({ host: 'example.com', total: 1, keys: { e: 1 } }), T0);
  assert.equal(day.total, 4);
  assert.deepEqual(day.keys, { e: 3, space: 1 });
  assert.deepEqual(day.shortcuts, { 'ctrl+c': 1 });
  assert.deepEqual(day.sites, { 'github.com': 3, 'example.com': 1 });
});

test('claves peligrosas se ignoran y no contaminan el objeto', () => {
  const day = emptyDay();
  const evil = JSON.parse('{"__proto__": 5, "constructor": 2, "e": 1}');
  applyBatch(day, batch({ host: '__proto__', total: 8, keys: evil }), T0);
  assert.equal(Object.hasOwn(day.keys, '__proto__'), false);
  assert.equal(Object.hasOwn(day.keys, 'constructor'), false);
  assert.equal(Object.hasOwn(day.sites, '__proto__'), false);
  assert.equal(Object.getPrototypeOf(day.keys), Object.prototype);
  assert.equal({}.polluted, undefined);
  assert.deepEqual(day.keys, { e: 1 });
});

test('el hostname reservado "otros" no se mezcla con el agregado', () => {
  const day = emptyDay();
  applyBatch(day, batch({ host: OTHERS }), T0);
  assert.equal(Object.hasOwn(day.sites, OTHERS), false);
});

test('conteos inválidos (NaN, negativos, texto) se ignoran', () => {
  const day = emptyDay();
  applyBatch(day, batch({ total: 'x', keys: { e: 'y' } }), T0);
  applyBatch(day, batch({ total: -4 }), T0);
  applyBatch(day, batch({ total: NaN }), T0);
  applyBatch(day, null, T0);
  applyBatch(day, undefined, T0);
  assert.deepEqual(day, emptyDay());
});

test('active_ms suma un minuto por cada minuto nuevo con actividad', () => {
  const day = emptyDay();
  applyBatch(day, batch(), T0);
  applyBatch(day, batch(), T0 + 10_000); // mismo minuto
  assert.equal(day.active_ms, MIN);
  applyBatch(day, batch(), T0 + 2 * MIN);
  assert.equal(day.active_ms, 2 * MIN);
});

test('wpm: cada 5 teclas son una palabra, promediadas por minuto activo', () => {
  const m = Math.floor(T0 / MIN);
  assert.equal(wpm({ [m]: 50 }, T0), 10);
  assert.equal(wpm({ [m]: 50, [m - 1]: 100 }, T0), 15); // (150/5)/2
  assert.equal(wpm({}, T0), 0);
});

test('wpm ignora minutos fuera de la ventana de 5 minutos', () => {
  const m = Math.floor(T0 / MIN);
  assert.equal(wpm({ [m - 10]: 500, [m]: 25 }, T0), 5);
  assert.equal(wpm({ [m - 10]: 500 }, T0), 0);
});

test('los minutos viejos de recent se podan al llegar un lote nuevo', () => {
  const day = emptyDay();
  applyBatch(day, batch(), T0);
  applyBatch(day, batch(), T0 + 10 * MIN);
  assert.equal(Object.keys(day.recent).length, 1);
});

test('pruneTop conserva el top 20 y agrupa el resto en otros', () => {
  const map = {};
  for (let i = 0; i < 30; i++) map[`k${i}`] = 30 - i; // k0=30 … k29=1
  const out = pruneTop(map);
  assert.equal(Object.keys(out).length, 21);
  assert.equal(out.k0, 30);
  assert.equal(Object.hasOwn(out, 'k20'), false);
  assert.equal(out[OTHERS], 10 + 9 + 8 + 7 + 6 + 5 + 4 + 3 + 2 + 1);
});

test('pruneTop suma el otros previo y no lo cuenta dentro del top', () => {
  const map = { [OTHERS]: 100 };
  for (let i = 0; i < 25; i++) map[`k${i}`] = 100 - i;
  const out = pruneTop(map);
  assert.equal(Object.keys(out).filter((k) => k !== OTHERS).length, 20);
  assert.equal(out[OTHERS], 100 + (80 + 79 + 78 + 77 + 76));
});

test('pruneTop no toca mapas pequeños', () => {
  const map = { a: 2, b: 1, [OTHERS]: 7 };
  assert.deepEqual(pruneTop(map), map);
});

test('pruneDay poda teclas, atajos y sitios', () => {
  const day = emptyDay();
  for (let i = 0; i < 30; i++) {
    day.keys[`k${i}`] = i + 1;
    day.shortcuts[`s${i}`] = i + 1;
    day.sites[`h${i}.com`] = i + 1;
  }
  pruneDay(day);
  for (const map of [day.keys, day.shortcuts, day.sites]) {
    assert.equal(Object.keys(map).length, 21);
  }
});

test('topEntries ordena de mayor a menor y excluye otros', () => {
  const top = topEntries({ a: 1, b: 5, c: 3, [OTHERS]: 99 }, 2);
  assert.deepEqual(top, [['b', 5], ['c', 3]]);
});

test('summarize de un día inexistente devuelve ceros', () => {
  assert.deepEqual(summarize(undefined, T0), {
    total: 0,
    wpm: 0,
    topKeys: [],
    topShortcuts: [],
    topSites: [],
  });
});

test('summarize arma el resumen del panel', () => {
  const day = emptyDay();
  applyBatch(day, batch({ total: 50, keys: { e: 30, a: 20 }, shortcuts: { 'ctrl+c': 2 } }), T0);
  const s = summarize(day, T0);
  assert.equal(s.total, 50);
  assert.equal(s.wpm, 10);
  assert.deepEqual(s.topKeys, [['e', 30], ['a', 20]]);
  assert.deepEqual(s.topShortcuts, [['ctrl+c', 2]]);
  assert.deepEqual(s.topSites, [['github.com', 50]]);
});
```

- [ ] **Step 6: Ejecutar para verificar que falla**

Run: `node --test tests/keystats.test.js`
Expected: FAIL con `Cannot find module '../extension/src/keystats.js'`.

- [ ] **Step 7: Implementar `keystats.js`**

`extension/src/keystats.js`:
```js
// Lógica pura de estadísticas de teclas: sin DOM ni chrome.*, para probarla con Node.

export const TOP_LIMIT = 20;
export const KEEP_DAYS = 90;
export const WPM_WINDOW_MIN = 5;
export const OTHERS = 'otros';

const MIN_MS = 60_000;
const DAY_MS = 86_400_000;
// Claves que nunca se aceptan como nombre de tecla/atajo/sitio.
const RESERVED = new Set(['__proto__', 'constructor', 'prototype', OTHERS]);

export function dateKey(ms) {
  const d = new Date(ms);
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function cutoffKey(ms, keepDays = KEEP_DAYS) {
  return dateKey(ms - keepDays * DAY_MS);
}

export function emptyDay() {
  return { total: 0, keys: {}, shortcuts: {}, sites: {}, active_ms: 0, recent: {} };
}

function count(v) {
  return Number.isFinite(v) && v > 0 ? Math.floor(v) : 0;
}

function bump(map, key, n) {
  if (n === 0 || typeof key !== 'string' || key === '' || RESERVED.has(key)) return;
  map[key] = (Object.hasOwn(map, key) ? map[key] : 0) + n;
}

/** Suma un lote del content script al registro del día. Muta y devuelve `day`. */
export function applyBatch(day, batch, nowMs) {
  const total = count(batch?.total);
  if (total === 0) return day;

  day.total += total;
  for (const [k, v] of Object.entries(batch.keys ?? {})) bump(day.keys, k, count(v));
  for (const [k, v] of Object.entries(batch.shortcuts ?? {})) bump(day.shortcuts, k, count(v));
  bump(day.sites, batch.host, total);

  const minute = Math.floor(nowMs / MIN_MS);
  if (!Object.hasOwn(day.recent, minute)) day.active_ms += MIN_MS;
  day.recent[minute] = (Object.hasOwn(day.recent, minute) ? day.recent[minute] : 0) + total;
  for (const m of Object.keys(day.recent)) {
    if (Number(m) <= minute - WPM_WINDOW_MIN) delete day.recent[m];
  }
  return day;
}

/** Conserva las `limit` entradas más altas; el resto se suma en `otros`. */
export function pruneTop(map, limit = TOP_LIMIT) {
  const entries = Object.entries(map)
    .filter(([k]) => k !== OTHERS)
    .sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1));
  if (entries.length <= limit) return map;

  const kept = Object.fromEntries(entries.slice(0, limit));
  const rest = entries.slice(limit).reduce((sum, [, v]) => sum + v, 0);
  kept[OTHERS] = rest + (Object.hasOwn(map, OTHERS) ? map[OTHERS] : 0);
  return kept;
}

export function pruneDay(day) {
  day.keys = pruneTop(day.keys);
  day.shortcuts = pruneTop(day.shortcuts);
  day.sites = pruneTop(day.sites);
  return day;
}

/** Palabras por minuto: cada 5 teclas = 1 palabra, promedio por minuto activo de los últimos 5. */
export function wpm(recent, nowMs) {
  const minute = Math.floor(nowMs / MIN_MS);
  let keys = 0;
  let active = 0;
  for (let m = minute - WPM_WINDOW_MIN + 1; m <= minute; m++) {
    if (Object.hasOwn(recent, m)) {
      keys += recent[m];
      active += 1;
    }
  }
  return active === 0 ? 0 : Math.round(keys / 5 / active);
}

export function topEntries(map, n = 5) {
  return Object.entries(map)
    .filter(([k]) => k !== OTHERS)
    .sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))
    .slice(0, n);
}

export function summarize(day, nowMs) {
  const d = day ?? emptyDay();
  return {
    total: d.total,
    wpm: wpm(d.recent ?? {}, nowMs),
    topKeys: topEntries(d.keys),
    topShortcuts: topEntries(d.shortcuts),
    topSites: topEntries(d.sites),
  };
}
```

- [ ] **Step 8: Ejecutar para verificar que pasa**

Run: `npm run test:js`
Expected: pasan las pruebas de `keyname` (9) y `keystats` (17), más las 3 de `wasm-smoke` si ya hay build.
Verificación aritmética de la prueba de `cutoffKey`: 8 oct menos 90 días es el 10 jul (8 días hasta el 30 sep, 30 hasta el 31 ago, 31 hasta el 31 jul, 21 más hasta el 10 jul = 90).

---

### Task 8: Almacenamiento, service worker y content script

**Files:**
- Create: `extension/src/store.js`, `extension/background.js`, `extension/content/keycount.js`, `tests/store.test.js`

**Interfaces:**
- Consumes: `applyBatch`, `cutoffKey`, `dateKey`, `emptyDay`, `pruneDay`, `summarize` (Task 7); `NT.keyName` (Task 7).
- Produces: `store.createKpiStore(area, now = Date.now) -> { record(batch): Promise<void>, readToday(): Promise<Summary>, todayKey(): string }`. `area` es cualquier objeto con `get(keys|null)`, `set(obj)` y `remove(keys)` basados en promesas (la forma de `chrome.storage.local`). Claves en el almacenamiento: `day:YYYY-MM-DD`.
- Produces: mensaje `{ type: 'keys', batch }` de `content/keycount.js` a `background.js`.

- [ ] **Step 1: Escribir las pruebas de `store` (fallan)**

`tests/store.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { createKpiStore } from '../extension/src/store.js';

const T0 = new Date(2026, 9, 8, 15, 0, 0).getTime();
const DAY = 86_400_000;
const batch = (o = {}) => ({ host: 'github.com', total: 2, keys: { e: 2 }, shortcuts: {}, ...o });

/** Imita `chrome.storage.local`; `delay` fuerza que las operaciones se intercalen. */
function fakeArea(initial = {}, { delay = 0 } = {}) {
  const data = { ...initial };
  const wait = () => (delay ? new Promise((r) => setTimeout(r, delay)) : Promise.resolve());
  return {
    data,
    failNextSet: false,
    async get(keys) {
      await wait();
      if (keys === null) return structuredClone(data);
      const out = {};
      for (const k of Array.isArray(keys) ? keys : [keys]) {
        if (Object.hasOwn(data, k)) out[k] = structuredClone(data[k]);
      }
      return out;
    },
    async set(obj) {
      await wait();
      if (this.failNextSet) {
        this.failNextSet = false;
        throw new Error('QUOTA_BYTES exceeded');
      }
      Object.assign(data, structuredClone(obj));
    },
    async remove(keys) {
      await wait();
      for (const k of Array.isArray(keys) ? keys : [keys]) delete data[k];
    },
  };
}

test('record guarda el lote bajo la clave del día', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  await store.record(batch());
  assert.equal(area.data['day:2026-10-08'].total, 2);
  assert.equal(store.todayKey(), 'day:2026-10-08');
});

test('varios record concurrentes no pierden conteos', async () => {
  const area = fakeArea({}, { delay: 1 });
  const store = createKpiStore(area, () => T0);
  await Promise.all(Array.from({ length: 10 }, () => store.record(batch({ total: 3, keys: { a: 3 } }))));
  assert.equal(area.data['day:2026-10-08'].total, 30);
  assert.equal(area.data['day:2026-10-08'].keys.a, 30);
});

test('un fallo de storage rechaza ese record pero no rompe la cola', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  area.failNextSet = true;
  await assert.rejects(store.record(batch()), /QUOTA_BYTES/);
  await store.record(batch({ total: 5, keys: { b: 5 } }));
  assert.equal(area.data['day:2026-10-08'].total, 5);
});

test('al empezar un día nuevo se borran los registros de más de 90 días', async () => {
  const area = fakeArea({
    'day:2026-07-01': { total: 1 }, // más viejo que el corte (2026-07-10)
    'day:2026-07-10': { total: 1 }, // justo en el corte: se conserva
    'day:2026-10-07': { total: 1 },
    settings: { mode: 'fire' }, // claves ajenas no se tocan
  });
  const store = createKpiStore(area, () => T0);
  await store.record(batch());
  assert.equal(Object.hasOwn(area.data, 'day:2026-07-01'), false);
  assert.equal(Object.hasOwn(area.data, 'day:2026-07-10'), true);
  assert.equal(Object.hasOwn(area.data, 'day:2026-10-07'), true);
  assert.equal(Object.hasOwn(area.data, 'settings'), true);
});

test('la poda de días viejos solo corre cuando el día es nuevo', async () => {
  const area = fakeArea();
  let fullScans = 0;
  const get = area.get.bind(area);
  area.get = (keys) => {
    if (keys === null) fullScans += 1;
    return get(keys);
  };
  const store = createKpiStore(area, () => T0);
  await store.record(batch());
  await store.record(batch());
  await store.record(batch());
  assert.equal(fullScans, 1);
});

test('el día nuevo arranca de cero', async () => {
  const area = fakeArea();
  let now = T0;
  const store = createKpiStore(area, () => now);
  await store.record(batch());
  now = T0 + DAY;
  await store.record(batch({ total: 7, keys: { z: 7 } }));
  assert.equal(area.data['day:2026-10-08'].total, 2);
  assert.equal(area.data['day:2026-10-09'].total, 7);
});

test('record poda a las 20 entradas más usadas', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  const keys = {};
  for (let i = 0; i < 40; i++) keys[`k${i}`] = 1;
  await store.record(batch({ total: 40, keys }));
  assert.equal(Object.keys(area.data['day:2026-10-08'].keys).length, 21);
});

test('readToday resume el día actual y es cero si no hay datos', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  assert.equal((await store.readToday()).total, 0);
  await store.record(batch({ total: 50, keys: { e: 50 } }));
  const s = await store.readToday();
  assert.equal(s.total, 50);
  assert.equal(s.wpm, 10);
  assert.deepEqual(s.topKeys, [['e', 50]]);
});

test('un lote con dominio hostil no contamina nada', async () => {
  const area = fakeArea();
  const store = createKpiStore(area, () => T0);
  await store.record(batch({ host: '__proto__' }));
  const day = area.data['day:2026-10-08'];
  assert.equal(Object.hasOwn(day.sites, '__proto__'), false);
  assert.equal({}.polluted, undefined);
});
```

- [ ] **Step 2: Ejecutar para verificar que falla**

Run: `node --test tests/store.test.js`
Expected: FAIL con `Cannot find module '../extension/src/store.js'`.

- [ ] **Step 3: Implementar `store.js`**

`extension/src/store.js`:
```js
import { applyBatch, cutoffKey, dateKey, emptyDay, pruneDay, summarize } from './keystats.js';

const PREFIX = 'day:';

/**
 * Persistencia de estadísticas sobre un `area` con la forma de `chrome.storage.local`.
 * Un registro por día (`day:YYYY-MM-DD`). Las escrituras se serializan en una cola
 * porque el service worker puede recibir varios lotes a la vez.
 */
export function createKpiStore(area, now = Date.now) {
  let chain = Promise.resolve();

  const keyFor = (ms) => PREFIX + dateKey(ms);

  async function removeOld(ms) {
    const all = await area.get(null);
    const cutoff = PREFIX + cutoffKey(ms);
    const stale = Object.keys(all).filter((k) => k.startsWith(PREFIX) && k < cutoff);
    if (stale.length) await area.remove(stale);
  }

  async function write(batch) {
    const ms = now();
    const key = keyFor(ms);
    const got = await area.get(key);
    const isNewDay = !Object.hasOwn(got, key);
    const day = pruneDay(applyBatch(isNewDay ? emptyDay() : got[key], batch, ms));
    await area.set({ [key]: day });
    if (isNewDay) await removeOld(ms);
  }

  return {
    todayKey: () => keyFor(now()),

    /** Encola un lote. Rechaza si falla, pero la cola sigue viva para los siguientes. */
    record(batch) {
      const run = chain.then(() => write(batch));
      chain = run.catch(() => {});
      return run;
    },

    async readToday() {
      const ms = now();
      const key = keyFor(ms);
      const got = await area.get(key);
      return summarize(got[key], ms);
    },
  };
}
```

- [ ] **Step 4: Ejecutar para verificar que pasa**

Run: `node --test tests/store.test.js`
Expected: 9 pruebas pasan.

- [ ] **Step 5: Escribir el service worker**

`extension/background.js`:
```js
import { createKpiStore } from './src/store.js';

const store = createKpiStore(chrome.storage.local);

// Los content scripts mandan lotes de conteos; solo se aceptan los de esta extensión.
chrome.runtime.onMessage.addListener((message, sender) => {
  if (sender.id !== chrome.runtime.id) return false;
  if (message && message.type === 'keys') {
    store.record(message.batch).catch((err) => console.error('No se pudo guardar el lote de teclas', err));
  }
  return false;
});
```

- [ ] **Step 6: Escribir el content script**

`extension/content/keycount.js`:
```js
// Cuenta teclas en la página. Solo guarda NOMBRES de tecla y conteos, nunca el
// texto escrito ni su orden. Se carga después de keyname.js (ver manifest.json).
(function () {
  const FLUSH_MS = 3000;
  const host = (location.hostname || 'local').replace(/^www\./, '');

  let pending = null;
  let timer = null;

  function flush() {
    timer = null;
    if (!pending) return;
    const batch = pending;
    pending = null;
    try {
      chrome.runtime.sendMessage({ type: 'keys', batch });
    } catch (_) {
      // El contexto de la extensión se invalida al actualizarla; se descarta el lote.
    }
  }

  function onKeyDown(e) {
    if (!e.isTrusted || e.repeat) return;

    // composedPath()[0] llega hasta el campo real aunque esté dentro de un shadow DOM.
    const target = (e.composedPath && e.composedPath()[0]) || e.target;
    if (target && target.tagName === 'INPUT' && target.type === 'password') return;

    const named = globalThis.NT.keyName(e);
    if (!named) return;

    pending = pending || { host, total: 0, keys: {}, shortcuts: {} };
    pending.total += 1;
    pending.keys[named.key] = (pending.keys[named.key] || 0) + 1;
    if (named.shortcut) {
      pending.shortcuts[named.shortcut] = (pending.shortcuts[named.shortcut] || 0) + 1;
    }
    if (!timer) timer = setTimeout(flush, FLUSH_MS);
  }

  addEventListener('keydown', onKeyDown, true);
  addEventListener('pagehide', flush);
})();
```

- [ ] **Step 7: Verificar**

Run: `npm run test:js`
Expected: todas las pruebas de JS pasan. El service worker y el content script no tienen prueba automática porque dependen de `chrome.*`; se verifican a mano en la tarea 11 (la lógica que contienen ya está cubierta en `keyname`, `keystats` y `store`).

---

### Task 9: Diff, limitador de FPS, ajustes e imagen guardada

**Files:**
- Create: `extension/src/diff.js`, `extension/src/limiter.js`, `extension/src/settings.js`, `extension/src/imagestore.js`, `tests/diff.test.js`, `tests/limiter.test.js`, `tests/settings.test.js`, `tests/imagestore.test.js`

**Interfaces:**
- Produces: `diff.diffCells(prev: Uint32Array | null, cur: Uint32Array, out: Int32Array, stride = 3) -> number` (rellena `out` con los índices de celda cambiados y devuelve cuántos son; `out` debe tener capacidad ≥ `cur.length / stride`).
- Produces: `limiter.createLimiter(fps) -> { ready(nowMs): boolean, reset(): void }`.
- Produces: `settings.{MODES, normalizeSettings(raw), loadSettings(area), saveSettings(area, settings)}`; `Settings = { mode: 'fire'|'rain'|'starfield'|'image', kpiVisible: boolean }`.
- Produces: `imagestore.{MAX_COLS = 200, MAX_ROWS = 100, MAX_IMAGE_BYTES = 204800, validateImage(img), saveImage(img), loadImage(), clearImage()}`; `img = { w: number, h: number, data: Uint8Array }` con `data.length === w*h*4`.

- [ ] **Step 1: Escribir las pruebas de `diff` (fallan)**

`tests/diff.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { diffCells } from '../extension/src/diff.js';

const cells = (...rows) => new Uint32Array(rows.flat());
const run = (prev, cur, stride) => {
  const out = new Int32Array(cur.length);
  const n = diffCells(prev, cur, out, stride);
  return [...out.subarray(0, n)];
};

test('sin frame previo todas las celdas cambian', () => {
  assert.deepEqual(run(null, cells([65, 1, 0], [66, 2, 0], [67, 3, 0])), [0, 1, 2]);
});

test('frames idénticos no cambian nada', () => {
  const a = cells([65, 1, 0], [66, 2, 0]);
  assert.deepEqual(run(a, a.slice()), []);
});

test('detecta cambios en carácter, fg o bg por separado', () => {
  const prev = cells([65, 1, 0], [66, 2, 0], [67, 3, 0], [68, 4, 0]);
  const cur = cells([99, 1, 0], [66, 9, 0], [67, 3, 7], [68, 4, 0]);
  assert.deepEqual(run(prev, cur), [0, 1, 2]);
});

test('si cambia el tamaño del buffer se repinta todo', () => {
  assert.deepEqual(run(cells([65, 1, 0]), cells([65, 1, 0], [66, 2, 0])), [0, 1]);
});

test('respeta un stride distinto', () => {
  const prev = new Uint32Array([1, 2, 3, 4]);
  const cur = new Uint32Array([1, 2, 9, 4]);
  assert.deepEqual(run(prev, cur, 2), [1]);
});

test('un buffer vacío no produce celdas', () => {
  assert.deepEqual(run(null, new Uint32Array(0)), []);
});
```

- [ ] **Step 2: Ejecutar para verificar que falla**

Run: `node --test tests/diff.test.js`
Expected: FAIL con `Cannot find module '../extension/src/diff.js'`.

- [ ] **Step 3: Implementar `diff.js`**

`extension/src/diff.js`:
```js
/**
 * Índices de las celdas que cambiaron entre `prev` y `cur` (equivale al double
 * buffering de cwork). Escribe en `out` y devuelve cuántas son; no asigna memoria.
 * Si `prev` es null o de otro tamaño, todas las celdas cuentan como cambiadas.
 */
export function diffCells(prev, cur, out, stride = 3) {
  const cells = Math.floor(cur.length / stride);
  let n = 0;
  if (!prev || prev.length !== cur.length) {
    for (let i = 0; i < cells; i++) out[n++] = i;
    return n;
  }
  for (let i = 0, j = 0; i < cells; i++, j += stride) {
    if (prev[j] !== cur[j] || prev[j + 1] !== cur[j + 1] || prev[j + 2] !== cur[j + 2]) {
      out[n++] = i;
    }
  }
  return n;
}
```

- [ ] **Step 4: Ejecutar para verificar que pasa**

Run: `node --test tests/diff.test.js`
Expected: 6 pruebas pasan.

- [ ] **Step 5: Escribir las pruebas del limitador (fallan)**

`tests/limiter.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { createLimiter } from '../extension/src/limiter.js';

/** Cuenta cuántos de los ticks de un monitor de `hz` Hz durante 1 s dejan pasar un frame. */
function framesPerSecond(fps, hz, jitterMs = 0) {
  const limiter = createLimiter(fps);
  const step = 1000 / hz;
  let ready = 0;
  for (let k = 0; k * step < 1000; k++) {
    const jitter = jitterMs ? ((k * 7919) % 3 - 1) * jitterMs : 0;
    if (limiter.ready(k * step + jitter)) ready++;
  }
  return ready;
}

test('el primer tick siempre pasa', () => {
  assert.equal(createLimiter(30).ready(1234), true);
});

test('30 FPS en un monitor de 60 Hz deja pasar ~30 frames por segundo', () => {
  const n = framesPerSecond(30, 60);
  assert.ok(n >= 29 && n <= 31, `pasaron ${n}`);
});

test('60 FPS en un monitor de 60 Hz no se reduce a 30 por el jitter de rAF', () => {
  const n = framesPerSecond(60, 60, 1);
  assert.ok(n >= 57, `pasaron ${n}`);
});

test('en un monitor de 144 Hz con 60 FPS no pasa de ~60', () => {
  const n = framesPerSecond(60, 144);
  assert.ok(n >= 58 && n <= 62, `pasaron ${n}`);
});

test('tras una pausa larga no ráfaga frames atrasados', () => {
  const limiter = createLimiter(30);
  assert.equal(limiter.ready(0), true);
  assert.equal(limiter.ready(10_000), true);
  assert.equal(limiter.ready(10_001), false);
  assert.equal(limiter.ready(10_034), true);
});

test('reset permite pintar de inmediato', () => {
  const limiter = createLimiter(30);
  limiter.ready(0);
  assert.equal(limiter.ready(5), false);
  limiter.reset();
  assert.equal(limiter.ready(6), true);
});
```

- [ ] **Step 6: Ejecutar para verificar que falla**

Run: `node --test tests/limiter.test.js`
Expected: FAIL con `Cannot find module '../extension/src/limiter.js'`.

- [ ] **Step 7: Implementar `limiter.js`**

`extension/src/limiter.js`:
```js
// Holgura para el jitter de requestAnimationFrame: sin ella, 60 FPS en un monitor
// de 60 Hz se degradaría a 30 cuando un tick llega 1 ms antes de lo esperado.
const SLACK_MS = 2;

export function createLimiter(fps) {
  const interval = 1000 / fps;
  let next = 0;
  return {
    /** ¿Toca pintar un frame en este tick de rAF? */
    ready(nowMs) {
      if (next !== 0 && nowMs < next - SLACK_MS) return false;
      // Si nos atrasamos más de un frame, re-sincroniza en vez de hacer ráfaga.
      next = next === 0 || nowMs - next > interval ? nowMs + interval : next + interval;
      return true;
    },
    reset() {
      next = 0;
    },
  };
}
```

- [ ] **Step 8: Ejecutar para verificar que pasa**

Run: `node --test tests/limiter.test.js`
Expected: 6 pruebas pasan.

- [ ] **Step 9: Escribir las pruebas de `settings` e `imagestore` (fallan)**

`tests/settings.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { MODES, loadSettings, normalizeSettings, saveSettings } from '../extension/src/settings.js';

test('los valores por defecto son fuego con el panel visible', () => {
  assert.deepEqual(normalizeSettings(undefined), { mode: 'fire', kpiVisible: true });
  assert.deepEqual(normalizeSettings(null), { mode: 'fire', kpiVisible: true });
});

test('acepta cada modo válido', () => {
  for (const mode of MODES) assert.equal(normalizeSettings({ mode }).mode, mode);
});

test('un modo inválido cae a fuego', () => {
  for (const mode of ['pacman', '', 7, null, {}, '__proto__']) {
    assert.equal(normalizeSettings({ mode }).mode, 'fire');
  }
});

test('kpiVisible solo es falso si es exactamente false', () => {
  assert.equal(normalizeSettings({ kpiVisible: false }).kpiVisible, false);
  for (const v of [0, '', null, undefined, 'no']) {
    assert.equal(normalizeSettings({ kpiVisible: v }).kpiVisible, true);
  }
});

test('loadSettings y saveSettings usan la clave "settings" del área', async () => {
  const data = {};
  const area = {
    get: async (k) => (Object.hasOwn(data, k) ? { [k]: data[k] } : {}),
    set: async (o) => Object.assign(data, o),
  };
  assert.deepEqual(await loadSettings(area), { mode: 'fire', kpiVisible: true });
  await saveSettings(area, { mode: 'rain', kpiVisible: false, basura: 1 });
  assert.deepEqual(data.settings, { mode: 'rain', kpiVisible: false });
  assert.deepEqual(await loadSettings(area), { mode: 'rain', kpiVisible: false });
});
```

`tests/imagestore.test.js`:
```js
import test from 'node:test';
import assert from 'node:assert/strict';
import { MAX_COLS, MAX_IMAGE_BYTES, MAX_ROWS, validateImage } from '../extension/src/imagestore.js';

const img = (w, h, data = new Uint8Array(w * h * 4)) => ({ w, h, data });

test('los topes coinciden con el spec', () => {
  assert.equal(MAX_COLS, 200);
  assert.equal(MAX_ROWS, 100);
  assert.equal(MAX_IMAGE_BYTES, 200 * 1024);
});

test('la cuadrícula máxima cabe en el tope de bytes', () => {
  assert.equal(validateImage(img(MAX_COLS, MAX_ROWS)), true);
  assert.ok(MAX_COLS * MAX_ROWS * 4 <= MAX_IMAGE_BYTES);
});

test('acepta una imagen mínima de 1×1', () => {
  assert.equal(validateImage(img(1, 1)), true);
});

test('rechaza datos que no cuadran con las dimensiones', () => {
  assert.equal(validateImage(img(2, 2, new Uint8Array(15))), false);
  assert.equal(validateImage(img(2, 2, new Uint8Array(17))), false);
  assert.equal(validateImage(img(2, 2, new Uint8Array(0))), false);
});

test('rechaza dimensiones inválidas o fuera del tope', () => {
  assert.equal(validateImage(img(0, 5, new Uint8Array(0))), false);
  assert.equal(validateImage(img(-1, 5, new Uint8Array(0))), false);
  assert.equal(validateImage(img(1.5, 2, new Uint8Array(12))), false);
  assert.equal(validateImage(img(MAX_COLS + 1, 1)), false);
  assert.equal(validateImage(img(1, MAX_ROWS + 1)), false);
  assert.equal(validateImage(img(NaN, 2, new Uint8Array(8))), false);
});

test('rechaza valores que no son una imagen', () => {
  assert.equal(validateImage(null), false);
  assert.equal(validateImage(undefined), false);
  assert.equal(validateImage({}), false);
  assert.equal(validateImage({ w: 1, h: 1, data: [0, 0, 0, 0] }), false);
  assert.equal(validateImage({ w: 1, h: 1, data: 'abcd' }), false);
});
```

- [ ] **Step 10: Ejecutar para verificar que falla**

Run: `node --test tests/settings.test.js tests/imagestore.test.js`
Expected: FAIL con `Cannot find module` para ambos.

- [ ] **Step 11: Implementar `settings.js` e `imagestore.js`**

`extension/src/settings.js`:
```js
export const MODES = ['fire', 'rain', 'starfield', 'image'];

export function normalizeSettings(raw) {
  return {
    mode: MODES.includes(raw?.mode) ? raw.mode : 'fire',
    kpiVisible: raw?.kpiVisible !== false,
  };
}

export async function loadSettings(area) {
  const got = await area.get('settings');
  return normalizeSettings(got.settings);
}

export function saveSettings(area, settings) {
  return area.set({ settings: normalizeSettings(settings) });
}
```

`extension/src/imagestore.js`:
```js
// Una sola imagen ASCII ya convertida (4 bytes por celda), guardada en IndexedDB.

export const MAX_COLS = 200;
export const MAX_ROWS = 100;
export const MAX_IMAGE_BYTES = 200 * 1024;

const DB_NAME = 'newtab-ascii';
const STORE = 'image';
const KEY = 'current';

/** `img = { w, h, data }` donde `data` son `w*h*4` bytes empaquetados por `image_to_ascii`. */
export function validateImage(img) {
  if (!img || !(img.data instanceof Uint8Array)) return false;
  const { w, h, data } = img;
  if (!Number.isInteger(w) || !Number.isInteger(h)) return false;
  if (w < 1 || h < 1 || w > MAX_COLS || h > MAX_ROWS) return false;
  return data.length === w * h * 4 && data.length <= MAX_IMAGE_BYTES;
}

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function withStore(mode, fn) {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const req = fn(db.transaction(STORE, mode).objectStore(STORE));
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  } finally {
    db.close();
  }
}

export async function saveImage(img) {
  if (!validateImage(img)) throw new Error('Imagen inválida o demasiado grande');
  await withStore('readwrite', (s) => s.put({ w: img.w, h: img.h, data: img.data }, KEY));
}

/** Devuelve la imagen guardada, o `null` si no hay o está corrupta. */
export async function loadImage() {
  const img = await withStore('readonly', (s) => s.get(KEY));
  return validateImage(img) ? img : null;
}

export async function clearImage() {
  await withStore('readwrite', (s) => s.delete(KEY));
}
```

- [ ] **Step 12: Ejecutar para verificar que pasa**

Run: `npm run test:js`
Expected: pasan todas las pruebas de JS (keyname 9, keystats 17, store 9, diff 6, limiter 6, settings 5, imagestore 6, más wasm-smoke 3).

---

### Task 10: Renderer, panel de KPI y página de nueva pestaña

**Files:**
- Create: `extension/src/renderer.js`, `extension/src/kpi.js`, `extension/newtab.html`, `extension/newtab.css`, `extension/newtab.js`, `extension/manifest.json`

**Interfaces:**
- Consumes: `Engine`, `cell_stride`, `image_target_dims`, `image_to_ascii` (Task 6); `diffCells` (Task 9); `createLimiter` (Task 9); `createKpiStore` (Task 8); `loadSettings`/`saveSettings` (Task 9); `loadImage`/`saveImage`/`validateImage`/`MAX_COLS`/`MAX_ROWS` (Task 9).
- Produces: `Renderer { constructor(canvas, { stride, cellW = 9, cellH = 16, fontSize = 14, font }); fit() -> { cols, rows }; invalidate(); draw(cells: Uint32Array, cols, rows) }` y `kpi.renderKpi(root: HTMLElement, summary)`.

Estos archivos dependen de `canvas`, DOM y `chrome.*`, así que no tienen prueba automática. La lógica que podía probarse (diff, limitador, ajustes, validación de imagen, estadísticas) ya está cubierta; esta tarea se verifica a mano en la tarea 11.

- [ ] **Step 1: Escribir `renderer.js`**

`extension/src/renderer.js`:
```js
import { diffCells } from './diff.js';

const DEFAULT_FG = 0xffcccccc;
const MAX_SPRITES = 2048;

const css = (c) => `rgb(${(c >> 16) & 255},${(c >> 8) & 255},${c & 255})`;

/**
 * Pinta el buffer de celdas de WASM en un canvas. Cada (carácter, color) se dibuja
 * una sola vez en un sprite y después cada celda es un `drawImage`, que es mucho
 * más barato que `fillText`. Solo se repintan las celdas que cambiaron.
 */
export class Renderer {
  constructor(canvas, { stride = 3, cellW = 9, cellH = 16, fontSize = 14, font = 'Consolas, "Cascadia Mono", "Courier New", monospace' } = {}) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.stride = stride;
    this.cellW = cellW;
    this.cellH = cellH;
    this.fontSize = fontSize;
    this.font = font;
    this.sprites = new Map();
    this.prev = null;
    this.scratch = new Int32Array(0);
    this.cols = 0;
    this.rows = 0;
    this.dpr = 1;
    this.pw = cellW;
    this.ph = cellH;
  }

  /** Ajusta el canvas a la ventana y devuelve el tamaño de la cuadrícula. */
  fit() {
    this.dpr = window.devicePixelRatio || 1;
    this.pw = Math.max(1, Math.round(this.cellW * this.dpr));
    this.ph = Math.max(1, Math.round(this.cellH * this.dpr));
    this.cols = Math.max(1, Math.floor(window.innerWidth / this.cellW));
    this.rows = Math.max(1, Math.floor(window.innerHeight / this.cellH));

    this.canvas.width = this.cols * this.pw;
    this.canvas.height = this.rows * this.ph;
    this.canvas.style.width = `${(this.cols * this.pw) / this.dpr}px`;
    this.canvas.style.height = `${(this.rows * this.ph) / this.dpr}px`;

    this.sprites.clear();
    this.prev = null;
    return { cols: this.cols, rows: this.rows };
  }

  /** Obliga a repintar todo en el siguiente `draw`. */
  invalidate() {
    this.prev = null;
  }

  sprite(ch, fg) {
    // 4 bits por canal: acota la cantidad de sprites distintos (fotos con miles de colores).
    const q = (fg || DEFAULT_FG) & 0xf0f0f0;
    const key = ch * 0x1000000 + q;
    let sprite = this.sprites.get(key);
    if (sprite) return sprite;

    if (this.sprites.size >= MAX_SPRITES) this.sprites.clear();
    sprite = new OffscreenCanvas(this.pw, this.ph);
    const g = sprite.getContext('2d');
    g.font = `${Math.round(this.fontSize * this.dpr)}px ${this.font}`;
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    g.fillStyle = css(q | (q >> 4));
    g.fillText(String.fromCodePoint(ch), this.pw / 2, this.ph / 2);
    this.sprites.set(key, sprite);
    return sprite;
  }

  /** `cells` es la vista sobre la memoria de WASM: `[ch, fg, bg]` por celda. */
  draw(cells, cols, rows) {
    if (cols !== this.cols || rows !== this.rows || cells.length !== cols * rows * this.stride) return;

    if (this.scratch.length < cols * rows) this.scratch = new Int32Array(cols * rows);
    const changed = diffCells(this.prev, cells, this.scratch, this.stride);
    const ctx = this.ctx;

    for (let k = 0; k < changed; k++) {
      const i = this.scratch[k];
      const j = i * this.stride;
      const x = (i % cols) * this.pw;
      const y = Math.floor(i / cols) * this.ph;
      const ch = cells[j];
      const fg = cells[j + 1];
      const bg = cells[j + 2];

      ctx.clearRect(x, y, this.pw, this.ph);
      if (bg !== 0) {
        ctx.fillStyle = css(bg);
        ctx.fillRect(x, y, this.pw, this.ph);
      }
      if (ch !== 32 && ch !== 0) ctx.drawImage(this.sprite(ch, fg), x, y);
    }

    if (!this.prev || this.prev.length !== cells.length) this.prev = new Uint32Array(cells.length);
    this.prev.set(cells);
  }
}
```

- [ ] **Step 2: Escribir `kpi.js`**

`extension/src/kpi.js`:
```js
// Solo `textContent`: los nombres de sitio vienen de páginas web y nunca deben
// interpretarse como HTML.

const fmt = (n) => n.toLocaleString('es-MX');

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function stat(label, value) {
  const box = el('div', 'stat');
  box.append(el('span', 'stat-value', fmt(value)), el('span', 'stat-label', label));
  return box;
}

function list(title, entries) {
  const section = el('section', 'list');
  section.append(el('h2', '', title));
  if (entries.length === 0) {
    section.append(el('p', 'empty', '—'));
    return section;
  }
  const ul = el('ul');
  for (const [name, n] of entries) {
    const li = el('li');
    li.append(el('span', 'name', name), el('span', 'count', fmt(n)));
    ul.append(li);
  }
  section.append(ul);
  return section;
}

export function renderKpi(root, summary) {
  const row = el('div', 'stats');
  row.append(stat('teclas hoy', summary.total), stat('WPM', summary.wpm));
  root.replaceChildren(
    row,
    list('Teclas', summary.topKeys),
    list('Atajos', summary.topShortcuts),
    list('Sitios', summary.topSites)
  );
}
```

- [ ] **Step 3: Escribir `newtab.html` y `newtab.css`**

`extension/newtab.html`:
```html
<!doctype html>
<html lang="es">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Nueva pestaña</title>
  <link rel="stylesheet" href="newtab.css">
</head>
<body>
  <canvas id="bg" aria-hidden="true"></canvas>
  <aside id="kpi" aria-label="Estadísticas de teclado"></aside>
  <button id="gear" type="button" aria-label="Ajustes" aria-expanded="false" aria-controls="settings">⚙</button>
  <section id="settings" hidden>
    <label>Fondo
      <select id="mode">
        <option value="fire">Fuego</option>
        <option value="rain">Lluvia</option>
        <option value="starfield">Campo de estrellas</option>
        <option value="image">Imagen ASCII</option>
      </select>
    </label>
    <label>Imagen
      <input id="image-file" type="file" accept="image/*">
    </label>
    <label class="check"><input id="kpi-visible" type="checkbox"> Mostrar estadísticas (tecla K)</label>
    <p id="message" role="status"></p>
  </section>
  <script type="module" src="newtab.js"></script>
</body>
</html>
```

`extension/newtab.css`:
```css
:root {
  color-scheme: dark;
  --bg: #0b0d10;
  --fg: #d7dde4;
  --muted: #7f8a96;
  --accent: #ff8a3d;
  --panel: rgba(11, 13, 16, 0.78);
  --line: rgba(255, 255, 255, 0.12);
}

* { box-sizing: border-box; }

html, body {
  margin: 0;
  height: 100%;
  overflow: hidden;
  background: var(--bg);
  color: var(--fg);
  font: 13px/1.4 Consolas, "Cascadia Mono", "Courier New", monospace;
}

#bg { position: fixed; top: 0; left: 0; display: block; }

/* Sin backdrop-filter: obligaría a la GPU a difuminar el canvas en cada frame. */
#kpi, #settings {
  position: fixed;
  padding: 12px 14px;
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 8px;
}

#kpi { top: 16px; right: 16px; width: 240px; }
#kpi[hidden] { display: none; }

.stats { display: flex; gap: 16px; margin-bottom: 10px; }
.stat { display: flex; flex-direction: column; }
.stat-value { font-size: 22px; font-weight: 700; color: var(--accent); }
.stat-label { color: var(--muted); }

.list h2 { margin: 10px 0 4px; font-size: 11px; font-weight: 400; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
.list ul { margin: 0; padding: 0; list-style: none; }
.list li { display: flex; justify-content: space-between; gap: 8px; }
.list .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.list .count { color: var(--muted); }
.list .empty { margin: 0; color: var(--muted); }

#gear {
  position: fixed;
  right: 16px;
  bottom: 16px;
  width: 36px;
  height: 36px;
  padding: 0;
  font-size: 18px;
  color: var(--fg);
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 50%;
  cursor: pointer;
}
#gear:hover, #gear:focus-visible { border-color: var(--accent); outline: none; }

#settings { right: 16px; bottom: 60px; display: grid; gap: 10px; width: 280px; }
#settings[hidden] { display: none; }
#settings label { display: grid; gap: 4px; color: var(--muted); }
#settings label.check { display: flex; align-items: center; gap: 8px; }
#settings select, #settings input[type="file"] { color: var(--fg); font: inherit; }
#message { min-height: 1.4em; margin: 0; color: var(--accent); }

@media (max-width: 640px) {
  #kpi { top: 8px; right: 8px; width: 200px; }
}
```

- [ ] **Step 4: Escribir `newtab.js`**

`extension/newtab.js`:
```js
import init, { Engine, cell_stride, image_target_dims, image_to_ascii } from './pkg/newtab_core.js';
import { MAX_COLS, MAX_ROWS, loadImage, saveImage, validateImage } from './src/imagestore.js';
import { renderKpi } from './src/kpi.js';
import { createLimiter } from './src/limiter.js';
import { Renderer } from './src/renderer.js';
import { loadSettings, saveSettings } from './src/settings.js';
import { createKpiStore } from './src/store.js';

const FPS = { fire: 30, rain: 30, starfield: 60 };
const MAX_FILE_BYTES = 20 * 1024 * 1024;
const REDUCED_MOTION_WARMUP = 120;

const $ = (id) => document.getElementById(id);

function say(text) {
  $('message').textContent = text;
}

async function main() {
  const wasm = await init();
  const area = chrome.storage.local;
  const settings = await loadSettings(area);
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');

  const renderer = new Renderer($('bg'), { stride: cell_stride() });
  const size = renderer.fit();
  const engine = new Engine((Math.random() * 0xffffffff) >>> 0, size.cols, size.rows);

  let rafId = 0;
  let animated = false;
  let limiter = createLimiter(30);

  // La vista se recrea en cada frame: la memoria de WASM puede crecer y soltar el buffer anterior.
  const paint = () => {
    const cells = new Uint32Array(wasm.memory.buffer, engine.cells_ptr(), engine.cells_len());
    renderer.draw(cells, engine.width(), engine.height());
  };

  function frame(now) {
    rafId = requestAnimationFrame(frame);
    if (!limiter.ready(now)) return;
    engine.step();
    paint();
  }

  function start() {
    if (!rafId && !document.hidden) rafId = requestAnimationFrame(frame);
  }

  function stop() {
    if (rafId) cancelAnimationFrame(rafId);
    rafId = 0;
  }

  async function applyMode(mode) {
    stop();
    animated = false;
    let effective = mode;

    if (mode === 'image') {
      let img = null;
      try {
        img = await loadImage();
      } catch (err) {
        console.error('No se pudo leer la imagen guardada', err);
      }
      if (!img || !engine.load_image(img.data, img.w, img.h)) {
        effective = 'fire';
        say('No hay una imagen válida guardada: se muestra el fuego.');
      }
    }

    if (effective !== 'image') engine.set_animation(effective);
    renderer.invalidate();

    if (effective === 'image') {
      paint(); // Imagen estática: se pinta una vez y no hay bucle.
      return;
    }
    if (reducedMotion.matches) {
      for (let i = 0; i < REDUCED_MOTION_WARMUP; i++) engine.step();
      paint(); // Con prefers-reduced-motion se muestra un solo frame.
      return;
    }
    animated = true;
    limiter = createLimiter(FPS[effective]);
    start();
  }

  document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
      stop();
    } else if (animated) {
      limiter.reset();
      start();
    }
  });

  let resizeTimer = 0;
  window.addEventListener('resize', () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      const next = renderer.fit();
      engine.resize(next.cols, next.rows);
      if (animated) renderer.invalidate();
      else applyMode(settings.mode);
    }, 150);
  });

  // --- Panel de KPI (se actualiza por eventos, sin polling) ---
  const kpiStore = createKpiStore(area);
  const kpiEl = $('kpi');
  const refreshKpi = async () => renderKpi(kpiEl, await kpiStore.readToday());
  chrome.storage.onChanged.addListener((changes, areaName) => {
    if (areaName === 'local' && Object.keys(changes).some((k) => k.startsWith('day:'))) refreshKpi();
  });

  function setKpiVisible(visible) {
    settings.kpiVisible = visible;
    kpiEl.hidden = !visible;
    $('kpi-visible').checked = visible;
    saveSettings(area, settings);
  }

  // --- Ajustes ---
  const gear = $('gear');
  const panel = $('settings');
  gear.addEventListener('click', () => {
    panel.hidden = !panel.hidden;
    gear.setAttribute('aria-expanded', String(!panel.hidden));
  });

  $('mode').value = settings.mode;
  $('mode').addEventListener('change', async (e) => {
    settings.mode = e.target.value;
    say('');
    await saveSettings(area, settings);
    await applyMode(settings.mode);
  });

  $('kpi-visible').addEventListener('change', (e) => setKpiVisible(e.target.checked));
  document.addEventListener('keydown', (e) => {
    const typing = /^(INPUT|SELECT|TEXTAREA)$/.test(e.target.tagName);
    if (!typing && !e.ctrlKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === 'k') {
      setKpiVisible(kpiEl.hidden);
    }
  });

  $('image-file').addEventListener('change', async (e) => {
    const file = e.target.files[0];
    e.target.value = '';
    if (!file) return;
    try {
      say('Convirtiendo…');
      await importImage(file);
      settings.mode = 'image';
      $('mode').value = 'image';
      await saveSettings(area, settings);
      await applyMode('image');
      say('Imagen guardada.');
    } catch (err) {
      say(err.message || 'No se pudo convertir la imagen.');
    }
  });

  kpiEl.hidden = !settings.kpiVisible;
  $('kpi-visible').checked = settings.kpiVisible;
  await applyMode(settings.mode);
  await refreshKpi();
}

/** Reduce la imagen en el navegador, la convierte a ASCII con WASM y la guarda. */
async function importImage(file) {
  if (!file.type.startsWith('image/')) throw new Error('El archivo no es una imagen.');
  if (file.size > MAX_FILE_BYTES) throw new Error('La imagen pesa más de 20 MB.');

  let bitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    throw new Error('No se pudo leer la imagen (formato no compatible).');
  }
  try {
    const [w, h] = image_target_dims(bitmap.width, bitmap.height, MAX_COLS, MAX_ROWS);
    const canvas = new OffscreenCanvas(w, h);
    const g = canvas.getContext('2d', { willReadFrequently: true });
    g.drawImage(bitmap, 0, 0, w, h);
    const rgba = g.getImageData(0, 0, w, h).data;
    const packed = image_to_ascii(new Uint8Array(rgba.buffer, rgba.byteOffset, rgba.byteLength), w, h, false);
    const img = { w, h, data: packed };
    if (!validateImage(img)) throw new Error('No se pudo convertir la imagen.');
    await saveImage(img);
  } finally {
    bitmap.close();
  }
}

main().catch((err) => {
  console.error(err);
  say('Error al iniciar: ' + (err.message || err));
});
```

- [ ] **Step 5: Escribir `manifest.json`**

`extension/manifest.json`:
```json
{
  "manifest_version": 3,
  "name": "New Tab ASCII",
  "version": "0.1.0",
  "description": "Pestaña nueva con fondo ASCII animado (Rust + WebAssembly) y estadísticas de teclas dentro de Chrome.",
  "chrome_url_overrides": { "newtab": "newtab.html" },
  "background": { "service_worker": "background.js", "type": "module" },
  "permissions": ["storage"],
  "content_scripts": [
    {
      "matches": ["<all_urls>"],
      "js": ["content/keyname.js", "content/keycount.js"],
      "run_at": "document_start",
      "all_frames": false
    }
  ],
  "content_security_policy": {
    "extension_pages": "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'"
  }
}
```

- [ ] **Step 6: Verificar tamaño y pruebas**

Run: `npm run build:wasm && npm run check:size && npm run test`
Expected: el build termina, `check:size` imprime ambos totales por debajo de sus topes y sale con código 0, y pasan todas las pruebas (Rust y JS).
Si `js+css+html` supera 50 KB, el desglose lo da `Get-ChildItem extension -Recurse -Include *.js,*.css,*.html | Sort-Object Length -Descending`; lo habitual es que el glue de `extension/pkg/newtab_core.js` sea la parte grande.

---

### Task 11: Verificación manual en Chrome y mediciones

**Files:**
- Create: `docs/measurements.md`

Esta tarea cierra el ciclo con comprobaciones que solo se pueden hacer en el navegador real. No se escribe código nuevo; si algo falla, se corrige en la tarea dueña del archivo y se repite la comprobación.

- [ ] **Step 1: Cargar la extensión**

1. Abre `chrome://extensions`, activa **Modo de desarrollador**.
2. **Cargar descomprimida** → selecciona `C:\Users\Daniel Rodriguez\Documents\Tests\owns\new-tab\extension`.
3. Expected: la extensión aparece sin errores. Pulsa **Errores** si el botón aparece en rojo y corrige antes de seguir. Los errores típicos: falta `extension/pkg/` (compila con `npm run build:wasm`), o el CSP bloquea WASM (revisa `wasm-unsafe-eval`).

- [ ] **Step 2: Comprobar los tres fondos**

Abre una pestaña nueva y, en ⚙ **Ajustes → Fondo**, prueba `Fuego`, `Lluvia` y `Campo de estrellas`.
Expected:
- Cada fondo anima a pantalla completa y el panel de KPI se ve arriba a la derecha.
- Redimensiona la ventana (incluida muy pequeña): no se rompe ni aparece una pantalla en blanco.
- En `starfield`, tras unos 6 segundos aparece el aviso `HYPERDRIVE`; en `rain` caen rayos de vez en cuando.

- [ ] **Step 3: Comprobar la imagen**

En ⚙ **Imagen**, elige una foto normal. Expected: se muestra en ASCII a color, estática.
Luego prueba, y verifica que cada caso dé un mensaje claro o un resultado válido:
- Un `.txt` renombrado a `.png` → mensaje de formato no compatible.
- Una imagen enorme (>20 MB) → mensaje del límite.
- Una imagen muy ancha y otra muy alta → se ven completas, sin pasar de 200×100.
- Una imagen PNG con fondo transparente → se ve sobre el fondo oscuro.
- Abre una pestaña nueva tras cerrar la anterior: la imagen sigue ahí (IndexedDB) y no hay ningún bucle activo.

- [ ] **Step 4: Comprobar las estadísticas**

1. Abre una página normal (por ejemplo `https://example.com`) y escribe en un campo de texto, usa `Ctrl+C`, y escribe en un campo `password`.
2. Abre una pestaña nueva.
Expected: el panel muestra el total del día, el WPM, las teclas y los atajos más usados, y el dominio (`example.com`). **Las teclas escritas en el campo `password` no aparecen en el conteo.** Ninguna tecla tiene el texto completo, solo nombres sueltos.
3. En un teclado en español, pulsa `AltGr+2` (`@`): debe contar como la tecla `@`, no como un atajo `ctrl+alt+@`.
4. Pulsa `K` en la pestaña nueva: el panel se oculta y se recuerda al recargar.
5. Cambia de pestaña y vuelve: el contador del panel se actualiza al volver (por el evento de storage), sin que haya timers en segundo plano.

- [ ] **Step 5: Comprobar la pausa y el ahorro**

Con `Fuego` activo, abre `Shift+Esc` (Administrador de tareas de Chrome):
- Con la pestaña nueva visible, anota CPU y memoria.
- Cambia a otra pestaña y espera 10 s: la CPU de la pestaña nueva debe bajar a ~0 (el bucle está cancelado).
- Con `prefers-reduced-motion` activado en Windows (Configuración → Accesibilidad → Efectos visuales → Efectos de animación: desactivado), recarga la pestaña: se ve un solo frame estático y no hay bucle.

- [ ] **Step 6: Medir y registrar**

Crea `docs/measurements.md` con la tabla siguiente y rellénala con lo que midas en `Shift+Esc` (pestaña nueva visible y con otra ventana en primer plano, 30 s de calentamiento, promedio de 3 lecturas). Compara con la referencia de cwork medida antes:

```markdown
# Mediciones

Máquina: 12 núcleos, Windows 11. Chrome: <versión>. Fecha: <fecha>.

## Referencia: cwork.exe en release, 60 FPS (solo el proceso, sin el host de consola)

| Animación | CPU | RAM |
|---|---|---|
| fire | 0.93 % | 8.4 MB |
| rain | 0.19 % | 7.6 MB |
| starfield | 0.18 % | 7.6 MB |

## Extensión (proceso de la pestaña, Administrador de tareas de Chrome)

| Fondo | FPS objetivo | CPU visible | CPU en otra pestaña | RAM |
|---|---|---|---|---|
| fire | 30 | | | |
| rain | 30 | | | |
| starfield | 60 | | | |
| imagen estática | — | | | |

## Tamaños (`npm run check:size`)

| Pieza | Medido | Tope |
|---|---|---|
| .wasm | | 150 KB |
| js+css+html | | 50 KB |
| imagen guardada (máx. 200×100) | 80 KB | 200 KB |

## Conclusiones y ajustes de FPS
```
Expected: las tres filas de animación quedan por debajo de la hipótesis del spec (1 a 5 % de CPU, 30 a 60 MB de RAM). Si alguna la supera, el primer ajuste es bajar su FPS en `FPS` de `extension/newtab.js` y repetir la medición. Anota la decisión en "Conclusiones".

- [ ] **Step 7: Verificación final**

Run: `npm run test && npm run check:size`
Expected: todas las pruebas pasan (Rust 54, JS 61) y `check:size` termina con código 0.

---

## Self-review

**Cobertura del spec**
- Objetivos y criterios de éxito (spec §1): fondo + panel (T10), cero consumo con la pestaña oculta (T10 `visibilitychange`, T11 paso 5), imagen estática sin bucle (T10 `applyMode`, T11 paso 3), topes de tamaño (T6 `check-size`, T9 `validateImage`, T11), solo conteos (T7, T8).
- Alcance v1 (§2): `fire` T2, `rain` T3, `starfield` T4, imagen T5/T6/T10, panel de KPI T7/T8/T10, selector de modo y atajo `K` T10. Fuera de alcance: no se planifica nada de sistema completo, heatmap, histórico visual ni las otras 8 animaciones.
- Arquitectura (§4): estructura de carpetas (con `extension/`, ver Desviaciones), frontera Rust/JS por `Engine` (T6), adaptaciones de portado (T1–T4).
- Renderizado (§5): bucle, atlas de glifos, diff, `ResizeObserver` (se usó `resize` con *debounce*, equivalente para una página que ocupa toda la ventana), pausa, FPS por animación, `prefers-reduced-motion`, sin timers sueltos (T9, T10); presupuesto de tamaño (T6, T11).
- KPI (§6): captura sin texto, `password` ignorado, lotes (T8), almacenamiento por día con top 20 y 90 días (T7, T8), métricas del panel (T7, T10), permisos mínimos (T10 manifest).
- Pruebas (§7): Rust con semilla fija, JS de lógica pura, manual guiado y medición (T11), verificación de tamaño (T6).
- Riesgos (§8): costo real en Chrome (T11), service worker reiniciado (cola y reintento en T8), fuente monoespaciada (pila de fuentes en T10; el ancho de celda es fijo, 9×16 px).

**Placeholders:** ninguno. Las únicas celdas vacías son las de `docs/measurements.md`, que se rellenan con datos reales en T11.

**Consistencia de tipos:** `Engine`, `cell_stride`, `image_target_dims`, `image_to_ascii` tienen la misma firma en T6 (Rust y prueba de humo) y en T10 (`newtab.js`). `diffCells(prev, cur, out, stride)` es igual en T9 y T10. `createKpiStore(area, now)` es igual en T8 y T10. `validateImage`/`MAX_COLS`/`MAX_ROWS` coinciden en T9 y T10. `applyBatch(day, batch, nowMs)` coincide en T7 y T8.

**Review Focus:** (1) cuadrículas diminutas: T2, T3, T4 y T6 tienen pruebas de tamaños `0×0`, `1×1`, `5×1`, etc. (2) imágenes raras: T5 (`target_dims` extremos, transparencia, buffers que no cuadran), T6 (`load_image` con dimensiones absurdas), T9 (`validateImage`), T10 (mensajes de error y *fallback* a `fire`) y T11 (casos manuales). (3) AltGr y teclas muertas: T7. (4) nombres hostiles: T7 (`__proto__`, `constructor`, `otros`), T8 (host `__proto__`), T10 (`textContent`). (5) escrituras concurrentes y fallo de storage: T8.

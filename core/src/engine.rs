//! API de WASM: un `Engine` que muestra una animación o una imagen estática.
//! JS lee el buffer de celdas directamente de la memoria de WASM
//! (`cells_ptr` / `cells_len`), sin serializar nada por frame.

use wasm_bindgen::prelude::*;

use crate::animation::{compile_sim::CompileSimAnimation, dashboard::DashboardAnimation, dna::DnaAnimation, fire::FireAnimation, mario::MarioAnimation, pacman::PacmanAnimation, pacman_install::PacmanInstallAnimation, pipes::PipesAnimation, rain::RainAnimation, starfield::StarfieldAnimation, synthwave::SynthwaveAnimation, Animation};
use crate::ascii::{self, packed_glyph, PACKED_STRIDE};
use crate::grid::{Grid, CELL_STRIDE, NO_COLOR};

/// Tope por lado de la cuadrícula: evita que `w*h*3` desborde `usize` en wasm32 (las reales miden unos cientos de celdas).
const MAX_GRID_DIM: u16 = 1024;

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
    fps: u32,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, width: u16, height: u16) -> Engine {
        Engine {
            grid: Grid::new(width.min(MAX_GRID_DIM), height.min(MAX_GRID_DIM)),
            anim: None,
            image: None,
            seed,
            fps: 30,
        }
    }

    /// FPS objetivo del bucle de JS; se reenvía a la animación activa y a las futuras.
    pub fn set_fps(&mut self, fps: u32) {
        self.fps = fps.max(1);
        if let Some(anim) = self.anim.as_mut() {
            anim.set_fps(self.fps);
        }
    }

    /// Activa `fire`, `rain`, `starfield`, `pipes`, `dna`, `synthwave`, `compile_sim`, `pacman_install`, `pacman`, `mario` o `dashboard`. Devuelve `false` si el nombre no existe.
    pub fn set_animation(&mut self, name: &str) -> bool {
        let (w, h) = (self.grid.width(), self.grid.height());
        let mut anim: Box<dyn Animation> = match name {
            "pipes" => Box::new(PipesAnimation::new(w, h, self.seed)),
            "fire" => Box::new(FireAnimation::new(w, h, self.seed)),
            "rain" => Box::new(RainAnimation::new(w, h, self.seed)),
            "starfield" => Box::new(StarfieldAnimation::new(w, h, self.seed)),
            "dna" => Box::new(DnaAnimation::new(w, h, self.seed)),
            "synthwave" => Box::new(SynthwaveAnimation::new(w, h)),
            "compile_sim" => Box::new(CompileSimAnimation::new(w, h, self.seed)),
            "pacman_install" => Box::new(PacmanInstallAnimation::new(w, h, self.seed)),
            "pacman" => Box::new(PacmanAnimation::new(w, h, self.seed)),
            "mario" => Box::new(MarioAnimation::new(w, h, self.seed)),
            "dashboard" => Box::new(DashboardAnimation::new()),
            _ => return false,
        };
        anim.set_fps(self.fps);
        self.anim = Some(anim);
        self.image = None;
        self.grid.clear();
        true
    }

    /// Datos numéricos del navegador para la animación activa (no-op si no los usa).
    /// Layout de `dashboard`: ver `DashboardAnimation::feed`.
    pub fn feed(&mut self, data: &[f32]) {
        if let Some(anim) = self.anim.as_mut() {
            anim.feed(data);
        }
    }

    /// Texto del navegador ("Etiqueta: valor" por línea) para la animación activa.
    pub fn feed_text(&mut self, text: &str) {
        if let Some(anim) = self.anim.as_mut() {
            anim.feed_text(text);
        }
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
        self.grid.resize(width.min(MAX_GRID_DIM), height.min(MAX_GRID_DIM));
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

    /// Puntero al buffer de celdas en la memoria de WASM. Solo es válido hasta
    /// la siguiente llamada que pueda realocar el buffer (`resize`,
    /// `set_animation`, `load_image`) o hasta que la memoria de WASM crezca
    /// (lo que además desprende `wasm.memory.buffer`). JS debe volver a leer
    /// `wasm.memory.buffer`, `cells_ptr()` y `cells_len()` y reconstruir el
    /// `Uint32Array` en cada frame, sin guardar la vista.
    pub fn cells_ptr(&self) -> *const u32 {
        self.grid.data().as_ptr()
    }

    /// Longitud en `u32` del buffer; mismas reglas de validez que `cells_ptr`.
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
        assert!(e.set_animation("dna"));
        assert!(e.set_animation("synthwave"));
        assert!(e.set_animation("compile_sim"));
        assert!(e.set_animation("pacman_install"));
        assert!(e.set_animation("pacman"));
        assert!(e.set_animation("mario"));
        assert!(e.set_animation("dashboard"));
    }

    #[test]
    fn feed_and_feed_text_are_noops_on_other_animations() {
        let mut e = Engine::new(1, 20, 10);
        e.feed(&[1.0, 2.0]); // sin animación
        e.feed_text("A: b");
        assert!(e.set_animation("fire"));
        e.feed(&[f32::NAN; 20]);
        e.feed_text("A: b
C: d");
        e.step();
        assert_eq!(e.cells_len(), 20 * 10 * CELL_STRIDE);
    }

    #[test]
    fn dashboard_shows_fed_clock_through_the_engine() {
        let mut e = Engine::new(1, 100, 30);
        assert!(e.set_animation("dashboard"));
        e.feed(&[12.0, 34.0, 56.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        e.feed_text("OS: test");
        e.step();
        assert_eq!(e.grid.get(33, 1).unwrap().0, '█');
    }

    #[test]
    fn dna_steps_100_times_on_a_normal_grid() {
        let mut e = Engine::new(1, 40, 20);
        assert!(e.set_animation("dna"));
        e.set_fps(60);
        for _ in 0..100 {
            e.step();
        }
        assert_eq!(e.cells_len(), 40 * 20 * CELL_STRIDE);
        let glyphs = (0..40).flat_map(|x| (0..20).map(move |y| (x, y))).filter(|&(x, y)| matches!(e.grid.get(x, y).unwrap().0, '●' | 'o')).count();
        assert!(glyphs > 0);
    }

    #[test]
    fn accepts_pipes_and_survives_extreme_fps() {
        let mut e = Engine::new(1, 40, 12);
        assert!(e.set_animation("pipes"));
        e.set_fps(0);
        e.set_fps(u32::MAX);
        assert!(e.set_animation("pipes"));
        for _ in 0..50 {
            e.step();
        }
        e.set_fps(60);
        e.step();
        assert_eq!(e.cells_len(), 40 * 12 * CELL_STRIDE);
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
        // offset = ((20-1)/2, (8-1)/2) = (9, 3)
        assert_eq!(e.grid.get(9, 3).unwrap().0, '@');
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
    fn switching_between_mario_and_other_animations_keeps_the_buffer_valid() {
        let mut e = Engine::new(1, 80, 24);
        for name in ["pacman", "mario", "fire", "mario"] {
            assert!(e.set_animation(name));
            e.set_fps(60);
            for _ in 0..20 {
                e.step();
            }
            assert_eq!(e.cells_len(), 80 * 24 * CELL_STRIDE);
        }
        e.resize(15, 9);
        e.step();
        assert_eq!(e.cells_len(), 15 * 9 * CELL_STRIDE);
    }

    #[test]
    fn zero_sized_engine_never_panics() {
        let mut e = Engine::new(1, 0, 0);
        assert!(e.set_animation("rain"));
        e.step();
        assert!(e.set_animation("pipes"));
        for _ in 0..10 {
            e.step();
        }
        assert!(e.set_animation("dna"));
        for _ in 0..30 {
            e.step();
        }
        assert!(e.set_animation("synthwave"));
        for _ in 0..10 {
            e.step();
        }
        assert!(e.set_animation("compile_sim"));
        for _ in 0..10 {
            e.step();
        }
        assert!(e.set_animation("pacman_install"));
        for _ in 0..10 {
            e.step();
        }
        assert!(e.set_animation("pacman"));
        for _ in 0..10 {
            e.step();
        }
        assert!(e.set_animation("mario"));
        for _ in 0..10 {
            e.step();
        }
        assert!(e.set_animation("dashboard"));
        e.feed(&[1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 2.0, 2.0, 0.5, 0.5]);
        e.feed_text("A: b");
        for _ in 0..10 {
            e.step();
        }
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

    #[test]
    fn grid_size_is_clamped_to_max_grid_dim() {
        let mut e = Engine::new(1, u16::MAX, u16::MAX);
        assert_eq!(e.width(), 1024);
        assert_eq!(e.height(), 1024);
        assert_eq!(e.cells_len(), 1024 * 1024 * CELL_STRIDE);
        e.resize(u16::MAX, 3);
        assert_eq!(e.width(), 1024);
        assert_eq!(e.height(), 3);
    }
}

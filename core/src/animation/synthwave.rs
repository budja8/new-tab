//! Port de `refs/cwork/src/animation/synthwave.rs`.
//! Cambios: `Grid` en vez de `Buffer`, aritmética `i32` (el original restaba en
//! `u16` y dividía por `height - 1 - cy`, que puede ser 0), `tick` en `u64` con la
//! fase de las ondas calculada en `f64` (con `f32` se perdía precisión tras horas)
//! y título ASCII centrado por número de caracteres.

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};

const TITLE: &str = "** SYNTHWAVE HORIZON **";
const TITLE_COLOR: u32 = rgb(255, 255, 255);
const GRID_COLOR: u32 = rgb(50, 15, 90);
const HORIZONTAL_LINES: i32 = 7;
const RADIAL_LINES: i32 = 14;

/// Fila del horizonte: el 46 % de la altura, redondeado.
fn horizon_row(h: i32) -> i32 {
    (h as f32 * 0.46).round() as i32
}

struct Wave {
    amp: f32,
    speed: f64,
    freq: f64,
    color: u32,
    offset: f64,
}

const WAVES: [Wave; 3] = [
    Wave { amp: 0.6, speed: 0.02, freq: 0.05, color: rgb(120, 0, 150), offset: 0.0 },
    Wave { amp: 0.9, speed: -0.035, freq: 0.08, color: rgb(235, 20, 140), offset: 2.5 },
    Wave { amp: 1.3, speed: 0.055, freq: 0.12, color: rgb(0, 255, 240), offset: 5.0 },
];

fn draw_sun(grid: &mut Grid, w: i32, cy: i32, cx: i32, radius: i32) {
    for y in (cy - radius).max(0)..cy {
        let dy = cy - y; // 1..=radius
        let is_gap = match dy {
            0..=2 => true,
            3..=5 => dy % 3 == 0,
            6..=11 => dy % 4 == 0,
            _ => dy % 6 == 0,
        };
        if is_gap {
            continue;
        }
        let half_width = ((radius * radius - dy * dy).max(0) as f32).sqrt() * 2.2;
        let x_start = (cx as f32 - half_width).round() as i32;
        let x_end = (cx as f32 + half_width).round() as i32;
        let ratio = dy as f32 / radius as f32;
        let g = (220.0 * (1.0 - ratio) + 30.0).clamp(0.0, 255.0) as u8;
        let b = (50.0 * (1.0 - ratio)).clamp(0.0, 255.0) as u8;
        let color = rgb(255, g, b);
        for x in x_start.max(0)..=x_end.min(w - 1) {
            grid.set(x, y, '█', color, NO_COLOR);
        }
    }
}

fn draw_grid_lines(grid: &mut Grid, w: i32, h: i32, cy: i32, cx: i32) {
    let grid_height = h - cy;
    for i in 1..=HORIZONTAL_LINES {
        let progress = i as f32 / HORIZONTAL_LINES as f32;
        let ly = cy + (grid_height as f32 * progress * progress).round() as i32;
        if ly < h {
            for x in 0..w {
                grid.set(x, ly, '═', GRID_COLOR, NO_COLOR);
            }
        }
    }

    let dy = h - 1 - cy;
    for j in 0..=RADIAL_LINES {
        let progress = j as f32 / RADIAL_LINES as f32;
        let target_x = (w as f32 * progress).round() as i32;
        let dx = target_x - cx;
        // Con una sola fila bajo el horizonte no hay perspectiva: dy == 0.
        let slope = if dy > 0 { dx as f32 / dy as f32 } else { 0.0 };
        let ch = if slope.abs() < 0.22 {
            '│'
        } else if slope > 0.0 {
            '╲'
        } else {
            '╱'
        };
        for y in cy..h {
            let t = if dy > 0 { (y - cy) as f32 / dy as f32 } else { 0.0 };
            let rx = (cx as f32 + dx as f32 * t).round() as i32;
            grid.set(rx, y, ch, GRID_COLOR, NO_COLOR);
        }
    }
}

fn draw_wave(grid: &mut Grid, w: i32, h: i32, cy: i32, tick: u64, wave: &Wave) {
    let mut prev_y: Option<i32> = None;
    for x in 0..w {
        // Fase en f64: con f32 se pierde precisión tras horas de ejecución.
        let t = (tick as f64 * wave.speed + x as f64 * wave.freq + wave.offset) as f32;
        let sine = t.sin() * 0.7 + (t * 2.3).cos() * 0.2 + (t * 4.7).sin() * 0.1;
        let wave_y = (cy as f32 + sine * (h as f32 * 0.12) * wave.amp).round() as i32;

        if wave_y >= 0 && wave_y < h {
            match prev_y {
                Some(py) => {
                    let (start, end) = (py.min(wave_y), py.max(wave_y));
                    let ch = if start == end { '─' } else { '│' };
                    for y in start..=end {
                        grid.set(x, y, ch, wave.color, NO_COLOR);
                    }
                }
                None => grid.set(x, wave_y, '─', wave.color, NO_COLOR),
            }
            prev_y = Some(wave_y);
        } else {
            prev_y = None;
        }
    }
}

pub struct SynthwaveAnimation {
    width: u16,
    height: u16,
    tick: u64,
}

impl SynthwaveAnimation {
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height, tick: 0 }
    }
}

impl Animation for SynthwaveAnimation {
    fn update(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.tick += 1;
    }

    fn draw(&mut self, grid: &mut Grid) {
        grid.clear();
        let (w, h) = (self.width as i32, self.height as i32);
        if w <= 0 || h <= 0 {
            return;
        }
        let cy = horizon_row(h);
        let cx = (w as f32 / 2.0).round() as i32;
        let radius = (h as f32 * 0.36) as i32;

        draw_sun(grid, w, cy, cx, radius);
        draw_grid_lines(grid, w, h, cy, cx);
        for wave in &WAVES {
            draw_wave(grid, w, h, cy, self.tick, wave);
        }

        let title_len = TITLE.chars().count() as i32;
        grid.set_str((w - title_len) / 2, 1, TITLE, TITLE_COLOR, NO_COLOR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(w: u16, h: u16, updates: u32) -> Grid {
        let mut a = SynthwaveAnimation::new(w, h);
        let mut grid = Grid::new(w, h);
        for _ in 0..updates {
            a.update(w, h);
        }
        a.draw(&mut grid);
        grid
    }

    fn count_in_row(grid: &Grid, y: u16, ch: char) -> usize {
        (0..grid.width()).filter(|&x| grid.get(x, y).unwrap().0 == ch).count()
    }

    #[test]
    fn horizon_row_is_46_percent_of_height() {
        assert_eq!(horizon_row(24), 11);
        assert_eq!(horizon_row(50), 23);
        assert_eq!(horizon_row(0), 0);
    }

    #[test]
    fn tiny_and_degenerate_sizes_never_panic() {
        for w in 0..=6u16 {
            for h in 0..=6u16 {
                let mut a = SynthwaveAnimation::new(w, h);
                let mut grid = Grid::new(w, h);
                for _ in 0..40 {
                    a.update(w, h);
                    a.draw(&mut grid);
                }
            }
        }
        for (w, h) in [(211, 50), (300, 100), (1024, 3), (3, 1024), (1, 1024), (1024, 1), (80, 24)] {
            let mut a = SynthwaveAnimation::new(w, h);
            let mut grid = Grid::new(w, h);
            for _ in 0..5 {
                a.update(w, h);
                a.draw(&mut grid);
            }
        }
    }

    #[test]
    fn huge_tick_does_not_panic() {
        let mut a = SynthwaveAnimation::new(80, 24);
        a.tick = u64::MAX - 1;
        let mut grid = Grid::new(80, 24);
        a.update(80, 24);
        a.draw(&mut grid);
    }

    #[test]
    fn sun_is_above_the_horizon_with_gradient() {
        let grid = scene(80, 24, 1);
        // cy = 11, radio = 8, dy = 6 -> fila 5, ratio 0.75.
        assert_eq!(grid.get(40, 5), Some(('█', rgb(255, 85, 12), NO_COLOR)));
        // Los huecos por dy (1, 2 y 3) no se dibujan: filas 10, 9 y 8 sin sol.
        for y in [10u16, 9, 8] {
            assert_eq!(count_in_row(&grid, y, '█'), 0, "fila {y}");
        }
        // Nada de sol bajo el horizonte.
        for y in 11..24 {
            assert_eq!(count_in_row(&grid, y, '█'), 0, "fila {y}");
        }
        let big = scene(211, 50, 1);
        assert_eq!(big.get(106, 10).unwrap().0, '█'); // cy = 23, dy = 13
        assert_eq!(big.get(106, 11).unwrap().0, ' '); // dy = 12 es hueco
    }

    #[test]
    fn grid_has_horizontal_lines_at_quadratic_rows() {
        let g = scene(80, 24, 1);
        // gh = 13; i = 6 -> 11 + round(13 * 36/49) = 21; i = 7 queda fuera.
        assert!(count_in_row(&g, 21, '═') > 40);
        let g = scene(211, 50, 1);
        // gh = 27; i = 6 -> 23 + 20 = 43; i = 5 -> 23 + round(27*25/49) = 37.
        assert!(count_in_row(&g, 43, '═') > 100);
        assert!(count_in_row(&g, 37, '═') > 100);
        assert_eq!(g.get(0, 43).unwrap().1, rgb(50, 15, 90));
    }

    #[test]
    fn radial_lines_use_vertical_and_slanted_glyphs() {
        let g = scene(211, 50, 1);
        let mut seen = std::collections::HashSet::new();
        for y in 30..50 {
            for x in 0..211 {
                seen.insert(g.get(x, y).unwrap().0);
            }
        }
        for ch in ['│', '╲', '╱'] {
            assert!(seen.contains(&ch), "falta {ch}");
        }
    }

    #[test]
    fn title_is_centered_ascii_on_row_one() {
        for (w, h) in [(80u16, 24u16), (211, 50)] {
            let g = scene(w, h, 3);
            let len = TITLE.chars().count() as u16;
            let x0 = (w - len) / 2;
            for (i, ch) in TITLE.chars().enumerate() {
                assert_eq!(g.get(x0 + i as u16, 1), Some((ch, rgb(255, 255, 255), NO_COLOR)));
            }
        }
        assert!(TITLE.is_ascii());
        assert_eq!(TITLE, "** SYNTHWAVE HORIZON **");
    }

    #[test]
    fn title_wider_than_grid_is_clipped() {
        let g = scene(10, 12, 2);
        assert_eq!(g.get(0, 1).unwrap().0, 'T'); // recorta "** SYNTHWAVE HORIZON **" por ambos lados
    }

    #[test]
    fn background_has_no_color() {
        let g = scene(80, 24, 1);
        for y in 0..24 {
            for x in 0..80 {
                assert_eq!(g.get(x, y).unwrap().2, NO_COLOR);
            }
        }
        assert_eq!(g.get(0, 0), Some((' ', NO_COLOR, NO_COLOR)));
    }

    #[test]
    fn waves_move_between_frames_and_scene_is_deterministic() {
        let a = scene(80, 24, 1);
        let b = scene(80, 24, 30);
        assert_ne!(a.data(), b.data());
        assert_eq!(scene(80, 24, 30).data(), b.data());
        assert_eq!(scene(211, 50, 7).data(), scene(211, 50, 7).data());
    }

    #[test]
    fn waves_draw_in_their_colors() {
        let g = scene(80, 24, 5);
        let mut colors = std::collections::HashSet::new();
        for y in 0..24 {
            for x in 0..80 {
                colors.insert(g.get(x, y).unwrap().1);
            }
        }
        for c in [rgb(120, 0, 150), rgb(235, 20, 140), rgb(0, 255, 240)] {
            assert!(colors.contains(&c));
        }
    }
}

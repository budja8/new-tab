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

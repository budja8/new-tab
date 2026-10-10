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

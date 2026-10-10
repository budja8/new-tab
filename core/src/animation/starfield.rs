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
// Texto del HUD en ASCII puro: los emojis se dibujan a color y no encajan en una celda monoespaciada.
const WARNING_PROMPT: &str = "!! WARNING: HYPERDRIVE INITIATING !!";
const HYPERDRIVE_PROMPT: &str = ">> HYPERDRIVE ACTIVE <<";

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
                let prompt = WARNING_PROMPT;
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
                let prompt = HYPERDRIVE_PROMPT;
                let len = prompt.chars().count() as i32;
                let px = (self.width as i32 - len) / 2;
                grid.set_str(px, 1, prompt, rgb(0, 255, 255), NO_COLOR);
            }
            _ => {}
        }
    }
}

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

    #[test]
    fn hud_prompts_are_plain_ascii() {
        // Los emojis se dibujan a color y no caben en una celda monoespaciada.
        assert!(WARNING_PROMPT.is_ascii());
        assert!(HYPERDRIVE_PROMPT.is_ascii());
    }
}

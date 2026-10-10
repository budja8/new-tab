//! Port de `refs/cwork/src/animation/pacman.rs`.
//! Cambios respecto al original:
//! - RNG propio (`Rng`) en vez de `rand`, `Grid` en vez de `Buffer`.
//! - Ticks lógicos a 60 por segundo independientes del FPS: `set_fps` ajusta
//!   los pasos por `update` (como `compile_sim`). Pac-Man y los fantasmas se
//!   mueven cada 6 ticks, la muerte dura 30 y el reaparecer 20, en tiempo real.
//! - Toda la aritmética y las coordenadas son `i32`; los accesos a las tablas
//!   pasan por `idx`/`get` (el original usaba `u16`/`usize` que desbordan y
//!   `gen_range` con rangos vacíos, que hace panic en WASM).
//! - Tamaño mínimo `MIN_W` x `MIN_H`: por debajo no hay laberinto y solo se
//!   dibuja el HUD (recortado). Tampoco hay laberinto por encima de `MAX_CELLS`.
//! - DESVIACIÓN: `energizer_timer` baja en cada MOVIMIENTO (cada 6 ticks) y no
//!   en cada tick. El original lo decrementaba cada tick aunque su comentario
//!   y su HUD (`POWER: ceil(timer/10) s`) suponen 60 movimientos = 6 s.
//! - Salvaguarda: 90 s sin comer nada pasan al nivel siguiente (sin bonus).
//! - Al generar un laberinto (resize, nivel, game over) el energizante se apaga.
//! - Los fantasmas se ocultan durante toda la animación de muerte (el original
//!   solo comparaba con `Dying { tick: 0 }`) y nunca entran en un muro.
//! - Se quitan los emoji del título y del pie, y lo de `q`/`Esc` del pie.
//! - Colores de crossterm: Yellow = rgb(255,255,0), Cyan = rgb(0,255,255),
//!   Green = rgb(0,255,0), White = rgb(255,255,255).
//! - Los bordes `╔╗╚╝═║` solo se pintan si hay un muro en la columna 1/`w-2` o
//!   la fila 3/`h-2`; como en el original, el generador nunca los crea.

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};
use crate::rng::Rng;

/// Todos los símbolos en un único sitio, para cambiarlos fácilmente a ASCII.
struct Glyphs {
    pac_up: char,
    pac_down: char,
    pac_left: char,
    pac_right: char,
    pac_closed: char,
    /// Fotogramas de la animación de muerte (se avanza cada 5 ticks).
    pac_death: [char; 6],
    life: char,
    ghost: char,
    ghost_dead: char,
    dot: char,
    energizer: char,
    wall_block: char,
    wall_h: char,
    wall_v: char,
    corner_tl: char,
    corner_tr: char,
    corner_bl: char,
    corner_br: char,
    divider: char,
}

const GLYPHS: Glyphs = Glyphs {
    pac_up: 'ᗣ',
    pac_down: 'ᗢ',
    pac_left: 'ᗧ',
    pac_right: 'ᗤ',
    pac_closed: 'O',
    pac_death: ['ᗤ', 'ᗢ', 'ᗧ', 'ᗣ', 'x', ' '],
    life: 'ᗤ',
    ghost: 'ᗛ',
    ghost_dead: '⍛',
    dot: '·',
    energizer: '●',
    wall_block: '█',
    wall_h: '═',
    wall_v: '║',
    corner_tl: '╔',
    corner_tr: '╗',
    corner_bl: '╚',
    corner_br: '╝',
    divider: '═',
};

const TITLE: &str = "RETRO PAC-MAN ARCADE";
const FOOTER: &str = "AUTOPILOT RUNNING";

const YELLOW: u32 = rgb(255, 255, 0);
const CYAN: u32 = rgb(0, 255, 255);
const GREEN: u32 = rgb(0, 255, 0);
const WHITE: u32 = rgb(255, 255, 255);
const GRAY: u32 = rgb(120, 120, 120);
const POWER_COLOR: u32 = rgb(255, 100, 0);
const DIVIDER_COLOR: u32 = rgb(0, 0, 150);
const WALL_COLOR: u32 = rgb(33, 33, 222);
const DOT_COLOR: u32 = rgb(255, 184, 151);
const FRIGHTENED: u32 = rgb(71, 71, 255);
const GHOST_COLORS: [(u32, &str); 4] = [
    (rgb(255, 0, 0), "Blinky"),
    (rgb(255, 184, 255), "Pinky"),
    (rgb(0, 255, 255), "Inky"),
    (rgb(255, 184, 81), "Clyde"),
];

/// Ticks lógicos por segundo del original.
const TICKS_PER_SECOND: u64 = 60;
const DEFAULT_FPS: u32 = 30;
/// Ticks lógicos entre dos movimientos (10 movimientos por segundo).
const MOVE_EVERY: u64 = 6;
const DYING_TICKS: u32 = 30;
const RESPAWN_TICKS: u32 = 20;
/// Movimientos que dura un energizante (6 s reales).
const ENERGIZER_MOVES: u32 = 60;
/// 90 s reales sin comer nada en el nivel actual.
const IDLE_LIMIT: u32 = 90 * TICKS_PER_SECOND as u32;
const MIN_W: i32 = 12;
const MIN_H: i32 = 10;
/// Tope de celdas con laberinto (el `Grid` de la extensión llega a 1024x1024).
const MAX_CELLS: i64 = 2048 * 2048;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

const DIRS: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

impl Dir {
    fn opposite(self) -> Self {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dot {
    None,
    Normal,
    Energizer,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GameState {
    Playing,
    Dying { tick: u32 },
    Respawning { tick: u32 },
}

#[derive(Clone, Debug)]
struct Ghost {
    x: i32,
    y: i32,
    color: u32,
    dir: Dir,
    name: &'static str,
    dead: bool,
}

/// Pasos lógicos (a 60/s) por llamada a `update` para un FPS dado.
fn steps_for(fps: u32) -> u32 {
    let fps = fps.max(1) as u64;
    ((TICKS_PER_SECOND + fps / 2) / fps).max(1) as u32
}

fn can_play(width: i32, height: i32) -> bool {
    width >= MIN_W && height >= MIN_H && (width as i64) * (height as i64) <= MAX_CELLS
}

pub struct PacmanAnimation {
    width: u16,
    height: u16,
    tick: u64,
    pacman_x: i32,
    pacman_y: i32,
    pacman_dir: Dir,
    walls: Vec<bool>,
    dots: Vec<Dot>,
    dots_left: usize,
    ghosts: Vec<Ghost>,
    score: u32,
    lives: u32,
    stage: u32,
    energizer_timer: u32,
    state: GameState,
    /// Ticks lógicos sin comer nada en el nivel actual.
    idle_ticks: u32,
    rng: Rng,
    steps: u32,
}

impl PacmanAnimation {
    pub fn new(width: u16, height: u16, seed: u32) -> Self {
        let mut s = Self {
            width,
            height,
            tick: 0,
            pacman_x: 0,
            pacman_y: 0,
            pacman_dir: Dir::Right,
            walls: Vec::new(),
            dots: Vec::new(),
            dots_left: 0,
            ghosts: Vec::new(),
            score: 0,
            lives: 3,
            stage: 1,
            energizer_timer: 0,
            state: GameState::Playing,
            idle_ticks: 0,
            rng: Rng::new(seed as u64),
            steps: steps_for(DEFAULT_FPS),
        };
        s.generate_maze();
        s
    }

    fn w(&self) -> i32 {
        self.width as i32
    }

    fn h(&self) -> i32 {
        self.height as i32
    }

    /// Hay laberinto jugable (tamaño suficiente y tablas generadas).
    fn playable(&self) -> bool {
        can_play(self.w(), self.h()) && self.walls.len() == self.width as usize * self.height as usize && self.dots.len() == self.walls.len()
    }

    /// Índice de la celda `(x, y)` en las tablas, o `None` si queda fuera.
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.w() || y >= self.h() {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    /// Celda transitable: dentro de la rejilla y sin muro.
    fn is_free(&self, x: i32, y: i32) -> bool {
        match self.idx(x, y).and_then(|i| self.walls.get(i)) {
            Some(wall) => !*wall,
            None => false,
        }
    }

    fn dot_at(&self, x: i32, y: i32) -> Dot {
        self.idx(x, y).and_then(|i| self.dots.get(i)).copied().unwrap_or(Dot::None)
    }

    fn set_wall(&mut self, x: i32, y: i32, v: bool) {
        if let Some(i) = self.idx(x, y) {
            if let Some(c) = self.walls.get_mut(i) {
                *c = v;
            }
        }
    }

    #[cfg(test)]
    fn set_dot(&mut self, x: i32, y: i32, d: Dot) {
        if let Some(i) = self.idx(x, y) {
            if let Some(c) = self.dots.get_mut(i) {
                *c = d;
            }
        }
        self.recount_dots();
    }

    fn recount_dots(&mut self) {
        self.dots_left = self.dots.iter().filter(|d| **d != Dot::None).count();
    }

    fn plaza(&self) -> (i32, i32) {
        (self.w() / 2, (self.h() + 3) / 2)
    }

    fn pacman_start(&self) -> (i32, i32) {
        (self.w() / 2, self.h() - 3)
    }

    fn clear_area_dots(&mut self, cx: i32, cy: i32, radius: i32) {
        for y in cy - radius..=cy + radius {
            for x in cx - radius..=cx + radius {
                if let Some(c) = self.idx(x, y).and_then(|i| self.dots.get_mut(i)) {
                    *c = Dot::None;
                }
            }
        }
    }

    fn clear_area_walls(&mut self, cx: i32, cy: i32, radius: i32) {
        for y in cy - radius..=cy + radius {
            for x in cx - radius..=cx + radius {
                self.set_wall(x, y, false);
            }
        }
    }

    /// Coloca a Pac-Man y a los 4 fantasmas en su posición inicial.
    fn place_actors(&mut self) {
        let (px, py) = self.pacman_start();
        self.pacman_x = px;
        self.pacman_y = py;
        self.pacman_dir = Dir::Right;
        let (cx, cy) = self.plaza();
        self.ghosts.clear();
        for (i, &(color, name)) in GHOST_COLORS.iter().enumerate() {
            self.ghosts.push(Ghost { x: cx - 2 + i as i32, y: cy, color, dir: Dir::Up, name, dead: false });
        }
    }

    /// Genera laberinto, puntos, Pac-Man y fantasmas. Conserva puntaje, vidas y
    /// nivel. Sin tamaño mínimo no deja nada (solo se dibuja el HUD).
    fn generate_maze(&mut self) {
        self.idle_ticks = 0;
        self.energizer_timer = 0;
        self.state = GameState::Playing;
        self.walls.clear();
        self.dots.clear();
        self.ghosts.clear();
        self.dots_left = 0;
        self.pacman_x = 0;
        self.pacman_y = 0;
        self.pacman_dir = Dir::Right;
        let (w, h) = (self.w(), self.h());
        if !can_play(w, h) {
            return;
        }
        let n = w as usize * h as usize;
        self.walls.resize(n, false);
        self.dots.resize(n, Dot::None);

        // 1. Borde, y fila 2 como divisor del HUD.
        for x in 0..w {
            self.set_wall(x, 0, true);
            self.set_wall(x, 2, true);
            self.set_wall(x, h - 1, true);
        }
        for y in 0..h {
            self.set_wall(0, y, true);
            self.set_wall(w - 1, y, true);
        }

        // 2. Bloques interiores aleatorios (margen de 2 con cualquier muro).
        let num_blocks = (w as i64 * h as i64 / 120).clamp(2, 40);
        for _ in 0..num_blocks {
            let bw = self.rng.range_incl_i32(4, 8);
            let bh = self.rng.range_incl_i32(2, 3);
            let max_x = w - (bw + 2);
            let max_y = h - (bh + 2);
            if max_x <= 2 || max_y <= 3 {
                continue;
            }
            let bx = self.rng.range_i32(2, max_x);
            let by = self.rng.range_i32(3, max_y);
            let mut can_place = true;
            'check: for y in by - 2..by + bh + 2 {
                for x in bx - 2..bx + bw + 2 {
                    if self.idx(x, y).is_some() && !self.is_free(x, y) {
                        can_place = false;
                        break 'check;
                    }
                }
            }
            if can_place {
                for y in by..by + bh {
                    for x in bx..bx + bw {
                        self.set_wall(x, y, true);
                    }
                }
            }
        }

        // 3. Puntos en todo pasillo libre bajo la línea 2 (1,5 % energizantes).
        for y in 3..h - 1 {
            for x in 1..w - 1 {
                if self.is_free(x, y) {
                    let d = if self.rng.chance(0.015) { Dot::Energizer } else { Dot::Normal };
                    if let Some(c) = self.idx(x, y).and_then(|i| self.dots.get_mut(i)) {
                        *c = d;
                    }
                }
            }
        }

        // Inicio de Pac-Man y plaza central de los fantasmas.
        let (px, py) = self.pacman_start();
        self.clear_area_dots(px, py, 2);
        self.clear_area_walls(px, py, 1);
        let (cx, cy) = self.plaza();
        self.clear_area_dots(cx, cy, 3);
        self.clear_area_walls(cx, cy, 2);
        self.place_actors();
        self.recount_dots();
    }

    /// Casilla siguiente en `dir`: horizontal con wrap entre `x = 1` y `w - 2`,
    /// vertical limitado a las filas 3..=`h - 2`.
    fn get_next_pos(&self, x: i32, y: i32, dir: Dir) -> Option<(i32, i32)> {
        let (w, h) = (self.w(), self.h());
        match dir {
            Dir::Up => (y > 3).then_some((x, y - 1)),
            Dir::Down => (y < h - 2).then_some((x, y + 1)),
            Dir::Left => Some(if x <= 1 { (w - 2, y) } else { (x - 1, y) }),
            Dir::Right => Some(if x >= w - 2 { (1, y) } else { (x + 1, y) }),
        }
    }

    fn get_pos_ahead(&self, x: i32, y: i32, dir: Dir, dist: i32) -> (i32, i32) {
        let (mut cx, mut cy) = (x, y);
        for _ in 0..dist {
            match self.get_next_pos(cx, cy, dir) {
                Some((nx, ny)) => {
                    cx = nx;
                    cy = ny;
                }
                None => break,
            }
        }
        (cx, cy)
    }

    /// Piloto automático: puntúa las 4 direcciones y elige la mejor.
    fn autopilot_dir(&self) -> Dir {
        let mut best_dir = self.pacman_dir;
        let mut best_score = -999_999.0f32;
        for dir in DIRS {
            let Some((nx, ny)) = self.get_next_pos(self.pacman_x, self.pacman_y, dir) else {
                continue;
            };
            if !self.is_free(nx, ny) {
                continue;
            }
            let mut score = 10.0f32;
            if dir == self.pacman_dir {
                score += 5.0;
            }
            if dir == self.pacman_dir.opposite() {
                score -= 15.0;
            }
            // Puntos hasta 5 casillas por delante.
            let (mut cx, mut cy) = (nx, ny);
            for step in 1..=5 {
                let Some((sx, sy)) = self.get_next_pos(cx, cy, dir) else {
                    break;
                };
                if !self.is_free(sx, sy) {
                    break;
                }
                match self.dot_at(sx, sy) {
                    Dot::Normal => score += 4.0 / step as f32,
                    Dot::Energizer => score += 20.0 / step as f32,
                    Dot::None => {}
                }
                cx = sx;
                cy = sy;
            }
            // Cazar fantasmas asustados o huir de los peligrosos.
            for g in &self.ghosts {
                let dist = (g.x - nx).abs() + (g.y - ny).abs();
                if dist < 8 && !g.dead {
                    if self.energizer_timer > 0 {
                        score += 60.0 / (dist as f32 + 1.0);
                    } else {
                        score -= 120.0 / (dist as f32 + 1.0);
                    }
                }
            }
            if score > best_score {
                best_score = score;
                best_dir = dir;
            }
        }
        best_dir
    }

    /// Casilla objetivo del fantasma `i` (vivo).
    fn ghost_target(&mut self, i: usize) -> (i32, i32) {
        let (w, h) = (self.w(), self.h());
        let (px, py, pdir) = (self.pacman_x, self.pacman_y, self.pacman_dir);
        let Some(g) = self.ghosts.get(i) else {
            return (px, py);
        };
        let (gx, gy, name) = (g.x, g.y, g.name);
        if self.energizer_timer > 0 {
            return (if i % 2 == 0 { 2 } else { w - 3 }, if i < 2 { 3 } else { h - 3 });
        }
        match name {
            "Pinky" => self.get_pos_ahead(px, py, pdir, 4),
            "Inky" => {
                if self.rng.chance(0.3) {
                    (px, py)
                } else {
                    (2, 3)
                }
            }
            "Clyde" => {
                if (gx - px).abs() + (gy - py).abs() > 8 {
                    (px, py)
                } else {
                    (2, h - 3)
                }
            }
            _ => (px, py),
        }
    }

    /// Mueve el fantasma `i` una casilla hacia `(tx, ty)` (distancia euclídea
    /// al cuadrado). No invierte salvo que esté muerto, asustado o sin salida,
    /// y nunca entra en un muro.
    fn ghost_move_towards(&mut self, i: usize, tx: i32, ty: i32) {
        let Some(g) = self.ghosts.get(i) else {
            return;
        };
        let (gx, gy, gdir, dead) = (g.x, g.y, g.dir, g.dead);
        let can_reverse = dead || self.energizer_timer > 0;
        let mut valid: Vec<(Dir, i32, i32)> = Vec::with_capacity(4);
        for dir in DIRS {
            if dir == gdir.opposite() && !can_reverse {
                continue;
            }
            if let Some((nx, ny)) = self.get_next_pos(gx, gy, dir) {
                if self.is_free(nx, ny) {
                    valid.push((dir, nx, ny));
                }
            }
        }
        if valid.is_empty() {
            let rev = gdir.opposite();
            if let Some((nx, ny)) = self.get_next_pos(gx, gy, rev) {
                if self.is_free(nx, ny) {
                    valid.push((rev, nx, ny));
                }
            }
        }
        let mut best: Option<(Dir, i32, i32)> = None;
        let mut min_dist = i64::MAX;
        for &(dir, nx, ny) in &valid {
            let dx = (nx - tx) as i64;
            let dy = (ny - ty) as i64;
            let d = dx * dx + dy * dy;
            if d < min_dist {
                min_dist = d;
                best = Some((dir, nx, ny));
            }
        }
        if let (Some((dir, nx, ny)), Some(g)) = (best, self.ghosts.get_mut(i)) {
            g.x = nx;
            g.y = ny;
            g.dir = dir;
        }
    }

    fn update_ghosts(&mut self) {
        let (cx, cy) = self.plaza();
        for i in 0..self.ghosts.len() {
            let Some(g) = self.ghosts.get(i) else {
                continue;
            };
            if g.dead {
                if (g.x - cx).abs() + (g.y - cy).abs() <= 1 {
                    if let Some(g) = self.ghosts.get_mut(i) {
                        g.dead = false;
                    }
                } else {
                    self.ghost_move_towards(i, cx, cy);
                }
                continue;
            }
            let (tx, ty) = self.ghost_target(i);
            self.ghost_move_towards(i, tx, ty);
        }
    }

    /// Nivel nuevo: `stage + 1` y laberinto nuevo.
    fn next_level(&mut self) {
        self.stage = self.stage.saturating_add(1);
        self.generate_maze();
    }

    /// Un movimiento de Pac-Man y de los fantasmas (cada `MOVE_EVERY` ticks).
    fn move_actors(&mut self) {
        // DESVIACIÓN: el energizante baja por movimiento, no por tick.
        self.energizer_timer = self.energizer_timer.saturating_sub(1);

        self.pacman_dir = self.autopilot_dir();
        if let Some((nx, ny)) = self.get_next_pos(self.pacman_x, self.pacman_y, self.pacman_dir) {
            if self.is_free(nx, ny) {
                self.pacman_x = nx;
                self.pacman_y = ny;
                let eaten = self.idx(nx, ny).and_then(|i| self.dots.get_mut(i)).map(|c| std::mem::replace(c, Dot::None));
                match eaten {
                    Some(Dot::Normal) => {
                        self.score = self.score.saturating_add(10);
                        self.dots_left = self.dots_left.saturating_sub(1);
                        self.idle_ticks = 0;
                    }
                    Some(Dot::Energizer) => {
                        self.score = self.score.saturating_add(50);
                        self.dots_left = self.dots_left.saturating_sub(1);
                        self.energizer_timer = ENERGIZER_MOVES;
                        self.idle_ticks = 0;
                    }
                    _ => {}
                }
            }
        }

        self.update_ghosts();

        let (px, py) = (self.pacman_x, self.pacman_y);
        for i in 0..self.ghosts.len() {
            let powered = self.energizer_timer > 0;
            let Some(g) = self.ghosts.get_mut(i) else {
                continue;
            };
            if g.x == px && g.y == py && !g.dead {
                if powered {
                    g.dead = true;
                    self.score = self.score.saturating_add(200);
                } else {
                    self.state = GameState::Dying { tick: 0 };
                    return;
                }
            }
        }

        if self.dots_left == 0 {
            self.score = self.score.saturating_add(1000);
            self.next_level();
        }
    }

    /// Un tick lógico (60 por segundo).
    fn step(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if !self.playable() {
            return;
        }
        self.idle_ticks = self.idle_ticks.saturating_add(1);
        if self.idle_ticks >= IDLE_LIMIT {
            // Salvaguarda: 90 s sin comer nada, siguiente nivel sin bonus.
            self.next_level();
            return;
        }
        match self.state {
            GameState::Playing => {
                if self.tick % MOVE_EVERY == 0 {
                    self.move_actors();
                }
            }
            GameState::Dying { tick } => {
                let tick = tick + 1;
                if tick < DYING_TICKS {
                    self.state = GameState::Dying { tick };
                } else if self.lives > 1 {
                    self.lives -= 1;
                    self.state = GameState::Respawning { tick: 0 };
                } else {
                    // Game over: reinicio completo.
                    self.score = 0;
                    self.lives = 3;
                    self.stage = 1;
                    self.generate_maze();
                }
            }
            GameState::Respawning { tick } => {
                let tick = tick + 1;
                if tick < RESPAWN_TICKS {
                    self.state = GameState::Respawning { tick };
                } else {
                    self.place_actors();
                    self.energizer_timer = 0;
                    self.state = GameState::Playing;
                }
            }
        }
    }

    fn put(grid: &mut Grid, x: i32, y: i32, ch: char, fg: u32) {
        grid.set(x, y, ch, fg, NO_COLOR);
    }

    fn put_str(grid: &mut Grid, x: i32, y: i32, s: &str, fg: u32) {
        grid.set_str(x, y, s, fg, NO_COLOR);
    }
}

impl Animation for PacmanAnimation {
    fn update(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.generate_maze();
            return;
        }
        for _ in 0..self.steps {
            self.step();
        }
    }

    fn draw(&mut self, grid: &mut Grid) {
        grid.clear();
        let (w, h) = (self.w(), self.h());
        let hud_x = (w - 18).max(0);

        // HUD
        Self::put_str(grid, 1, 0, TITLE, YELLOW);
        Self::put_str(grid, (w / 2 - 4).max(0), 0, &format!("STAGE: {}", self.stage), CYAN);
        Self::put_str(grid, hud_x, 0, &format!("SCORE: {:06}", self.score), GREEN);
        let mut lives = String::from("LIVES: ");
        for _ in 0..self.lives {
            lives.push(GLYPHS.life);
            lives.push(' ');
        }
        Self::put_str(grid, 1, 1, &lives, YELLOW);
        if self.energizer_timer > 0 {
            let secs = (self.energizer_timer + 9) / 10;
            Self::put_str(grid, hud_x, 1, &format!("POWER: {secs}s"), POWER_COLOR);
        }
        for x in 0..w {
            Self::put(grid, x, 2, GLYPHS.divider, DIVIDER_COLOR);
        }

        if !self.playable() {
            return;
        }

        // Laberinto y puntos.
        for y in 3..h - 1 {
            for x in 1..w - 1 {
                if !self.is_free(x, y) {
                    let border = x == 1 || x == w - 2 || y == 3 || y == h - 2;
                    let ch = if !border {
                        GLYPHS.wall_block
                    } else if x == 1 && y == 3 {
                        GLYPHS.corner_tl
                    } else if x == w - 2 && y == 3 {
                        GLYPHS.corner_tr
                    } else if x == 1 && y == h - 2 {
                        GLYPHS.corner_bl
                    } else if x == w - 2 && y == h - 2 {
                        GLYPHS.corner_br
                    } else if y == 3 || y == h - 2 {
                        GLYPHS.wall_h
                    } else {
                        GLYPHS.wall_v
                    };
                    Self::put(grid, x, y, ch, WALL_COLOR);
                } else {
                    match self.dot_at(x, y) {
                        Dot::Normal => Self::put(grid, x, y, GLYPHS.dot, DOT_COLOR),
                        Dot::Energizer => {
                            let color = if (self.tick / 15) % 2 == 0 { WHITE } else { DOT_COLOR };
                            Self::put(grid, x, y, GLYPHS.energizer, color);
                        }
                        Dot::None => {}
                    }
                }
            }
        }

        // Pac-Man.
        match self.state {
            GameState::Playing | GameState::Respawning { .. } => {
                let show = match self.state {
                    GameState::Respawning { tick } => (tick / 5) % 2 == 0,
                    _ => true,
                };
                if show {
                    let ch = if (self.tick / 3) % 2 == 0 {
                        match self.pacman_dir {
                            Dir::Up => GLYPHS.pac_up,
                            Dir::Down => GLYPHS.pac_down,
                            Dir::Left => GLYPHS.pac_left,
                            Dir::Right => GLYPHS.pac_right,
                        }
                    } else {
                        GLYPHS.pac_closed
                    };
                    Self::put(grid, self.pacman_x, self.pacman_y, ch, YELLOW);
                }
            }
            GameState::Dying { tick } => {
                let frame = ((tick / 5) as usize).min(GLYPHS.pac_death.len() - 1);
                Self::put(grid, self.pacman_x, self.pacman_y, GLYPHS.pac_death[frame], YELLOW);
            }
        }

        // Fantasmas (ocultos mientras Pac-Man muere).
        if !matches!(self.state, GameState::Dying { .. }) {
            for g in &self.ghosts {
                let (ch, color) = if g.dead {
                    (GLYPHS.ghost_dead, GRAY)
                } else if self.energizer_timer > 0 {
                    let flash = self.energizer_timer < 20 && (self.tick / 8) % 2 == 0;
                    (GLYPHS.ghost, if flash { WHITE } else { FRIGHTENED })
                } else {
                    (GLYPHS.ghost, g.color)
                };
                Self::put(grid, g.x, g.y, ch, color);
            }
        }

        // Pie centrado por número de caracteres.
        let n = FOOTER.chars().count() as i32;
        Self::put_str(grid, ((w - n) / 2).max(0), h - 1, FOOTER, GRAY);
    }

    fn set_fps(&mut self, fps: u32) {
        self.steps = steps_for(fps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn anim(w: u16, h: u16, seed: u32, fps: u32) -> PacmanAnimation {
        let mut a = PacmanAnimation::new(w, h, seed);
        a.set_fps(fps);
        a
    }

    /// Arena vacía (solo muros de borde y divisor), sin fantasmas, a 60 FPS.
    /// Un punto "centinela" en una celda de muro (0,0) evita el fin de nivel.
    fn arena(w: u16, h: u16) -> PacmanAnimation {
        let mut a = anim(w, h, 1, 60);
        let (wi, hi) = (a.w(), a.h());
        for y in 0..hi {
            for x in 0..wi {
                let border = x == 0 || x == wi - 1 || y <= 2 || y == hi - 1;
                a.set_wall(x, y, border);
            }
        }
        for d in a.dots.iter_mut() {
            *d = Dot::None;
        }
        a.set_dot(0, 0, Dot::Normal);
        a.ghosts.clear();
        a.energizer_timer = 0;
        a
    }

    fn ghost(x: i32, y: i32, dir: Dir, name: &'static str) -> Ghost {
        let color = GHOST_COLORS.iter().find(|g| g.1 == name).map_or(WHITE, |g| g.0);
        Ghost { x, y, color, dir, name, dead: false }
    }

    fn run(a: &mut PacmanAnimation, n: u32) {
        for _ in 0..n {
            a.step();
        }
    }

    fn row_text(g: &Grid, x: i32, y: u16, n: usize) -> String {
        (0..n).filter_map(|i| g.get((x + i as i32) as u16, y)).map(|c| c.0).collect()
    }

    fn drawn(a: &mut PacmanAnimation) -> Grid {
        let mut g = Grid::new(a.width, a.height);
        a.draw(&mut g);
        g
    }

    // ---------- tabla de símbolos y ritmo ----------

    #[test]
    fn glyph_table_has_the_original_symbols() {
        assert_eq!((GLYPHS.pac_up, GLYPHS.pac_down, GLYPHS.pac_left, GLYPHS.pac_right), ('ᗣ', 'ᗢ', 'ᗧ', 'ᗤ'));
        assert_eq!(GLYPHS.pac_closed, 'O');
        assert_eq!(GLYPHS.pac_death, ['ᗤ', 'ᗢ', 'ᗧ', 'ᗣ', 'x', ' ']);
        assert_eq!((GLYPHS.life, GLYPHS.ghost, GLYPHS.ghost_dead), ('ᗤ', 'ᗛ', '⍛'));
        assert_eq!((GLYPHS.dot, GLYPHS.energizer, GLYPHS.wall_block), ('·', '●', '█'));
        assert_eq!((GLYPHS.corner_tl, GLYPHS.corner_tr, GLYPHS.corner_bl, GLYPHS.corner_br), ('╔', '╗', '╚', '╝'));
        assert_eq!((GLYPHS.wall_h, GLYPHS.wall_v, GLYPHS.divider), ('═', '║', '═'));
    }

    #[test]
    fn steps_per_update_follow_fps() {
        assert_eq!(steps_for(60), 1);
        assert_eq!(steps_for(30), 2);
        assert_eq!(steps_for(20), 3);
        assert_eq!(steps_for(0), 60);
        assert_eq!(steps_for(u32::MAX), 1);
        assert_eq!(PacmanAnimation::new(1, 1, 1).steps, 2);
    }

    // ---------- laberinto ----------

    fn reachable_free_cells(a: &PacmanAnimation) -> (usize, usize) {
        let (w, h) = (a.w(), a.h());
        let mut seen = vec![false; (w * h) as usize];
        let mut q = VecDeque::new();
        let start = (a.pacman_x, a.pacman_y);
        seen[(start.1 * w + start.0) as usize] = true;
        q.push_back(start);
        let mut reached = 0;
        while let Some((x, y)) = q.pop_front() {
            reached += 1;
            for d in DIRS {
                if let Some((nx, ny)) = a.get_next_pos(x, y, d) {
                    let i = (ny * w + nx) as usize;
                    if a.is_free(nx, ny) && !seen[i] {
                        seen[i] = true;
                        q.push_back((nx, ny));
                    }
                }
            }
        }
        let mut free = 0;
        for y in 3..h - 1 {
            for x in 1..w - 1 {
                if a.is_free(x, y) {
                    free += 1;
                }
            }
        }
        (reached, free)
    }

    #[test]
    fn maze_is_valid_for_many_seeds_and_sizes() {
        for &(w, h) in &[(80u16, 24u16), (211, 58), (12, 10), (40, 15), (13, 11), (100, 12)] {
            for seed in 1..=25u32 {
                let a = anim(w, h, seed, 30);
                let (wi, hi) = (w as i32, h as i32);
                assert!(a.playable());
                for x in 0..wi {
                    assert!(!a.is_free(x, 0) && !a.is_free(x, 2) && !a.is_free(x, hi - 1), "{w}x{h} s{seed}");
                }
                for y in 0..hi {
                    assert!(!a.is_free(0, y) && !a.is_free(wi - 1, y));
                }
                let (px, py) = (wi / 2, hi - 3);
                assert_eq!((a.pacman_x, a.pacman_y, a.pacman_dir), (px, py, Dir::Right));
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        assert!(a.is_free(px + dx, py + dy), "inicio libre");
                    }
                }
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        assert_eq!(a.dot_at(px + dx, py + dy), Dot::None);
                    }
                }
                let (cx, cy) = ((wi) / 2, (hi + 3) / 2);
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        assert!(a.is_free(cx + dx, cy + dy), "plaza libre");
                    }
                }
                for dy in -3..=3 {
                    for dx in -3..=3 {
                        assert_eq!(a.dot_at(cx + dx, cy + dy), Dot::None);
                    }
                }
                assert_eq!(a.ghosts.len(), 4);
                for (i, g) in a.ghosts.iter().enumerate() {
                    assert_eq!((g.x, g.y, g.dir, g.dead), (cx - 2 + i as i32, cy, Dir::Up, false));
                    assert!(a.is_free(g.x, g.y));
                }
                assert_eq!(a.ghosts.iter().map(|g| g.name).collect::<Vec<_>>(), ["Blinky", "Pinky", "Inky", "Clyde"]);
                // Puntos solo en celdas libres de la zona de juego.
                for y in 0..hi {
                    for x in 0..wi {
                        if a.dot_at(x, y) != Dot::None {
                            assert!(a.is_free(x, y) && (3..hi - 1).contains(&y) && (1..wi - 1).contains(&x));
                        }
                    }
                }
                let total = a.dots.iter().filter(|d| **d != Dot::None).count();
                assert_eq!(total, a.dots_left);
                assert!(total > 0);
                // Sin celdas libres selladas: todo es alcanzable (con wrap horizontal).
                let (reached, free) = reachable_free_cells(&a);
                assert_eq!(reached, free, "{w}x{h} seed {seed}: celdas selladas");
            }
        }
    }

    #[test]
    fn maze_places_inner_blocks_of_the_right_size() {
        let mut inner = 0;
        for seed in 1..=10u32 {
            let a = anim(80, 24, seed, 30);
            for y in 3..23 {
                for x in 1..79 {
                    if !a.is_free(x, y) {
                        inner += 1;
                    }
                }
            }
        }
        // 80*24/120 = 16 intentos por laberinto, bloques de 8..=24 celdas.
        assert!(inner >= 10 * 8, "demasiado pocos muros: {inner}");
        assert!(inner <= 10 * 16 * 24, "demasiados muros: {inner}");
    }

    #[test]
    fn energizers_are_about_one_and_a_half_percent() {
        let a = anim(211, 58, 3, 30);
        let en = a.dots.iter().filter(|d| **d == Dot::Energizer).count();
        let dots = a.dots_left;
        assert!(en > 0);
        let pct = en as f32 * 100.0 / dots as f32;
        assert!((0.5..3.0).contains(&pct), "{pct}");
    }

    #[test]
    fn same_seed_same_maze_different_seed_differs() {
        assert!(anim(80, 24, 5, 30).playable());
        assert_eq!(anim(80, 24, 5, 30).walls, anim(80, 24, 5, 30).walls);
        assert_eq!(anim(80, 24, 5, 30).dots, anim(80, 24, 5, 30).dots);
        assert_ne!(anim(80, 24, 5, 30).walls, anim(80, 24, 6, 30).walls);
    }

    #[test]
    fn too_small_grids_have_no_maze() {
        for &(w, h) in &[(0u16, 0u16), (1, 1), (11, 30), (30, 9), (11, 9), (1, 1024), (1024, 1)] {
            let a = anim(w, h, 1, 30);
            assert!(!a.playable());
            assert!(a.walls.is_empty() && a.dots.is_empty() && a.ghosts.is_empty(), "{w}x{h}");
        }
        assert!(anim(12, 10, 1, 30).playable());
        assert!(anim(1024, 1024, 1, 30).playable());
    }

    // ---------- movimiento ----------

    #[test]
    fn next_pos_wraps_horizontally_and_stops_vertically() {
        let a = anim(40, 20, 1, 30);
        assert_eq!(a.get_next_pos(10, 10, Dir::Up), Some((10, 9)));
        assert_eq!(a.get_next_pos(10, 10, Dir::Down), Some((10, 11)));
        assert_eq!(a.get_next_pos(10, 10, Dir::Left), Some((9, 10)));
        assert_eq!(a.get_next_pos(10, 10, Dir::Right), Some((11, 10)));
        assert_eq!(a.get_next_pos(10, 4, Dir::Up), Some((10, 3)));
        assert_eq!(a.get_next_pos(10, 3, Dir::Up), None);
        assert_eq!(a.get_next_pos(10, 17, Dir::Down), Some((10, 18)));
        assert_eq!(a.get_next_pos(10, 18, Dir::Down), None);
        assert_eq!(a.get_next_pos(1, 10, Dir::Left), Some((38, 10)));
        assert_eq!(a.get_next_pos(38, 10, Dir::Right), Some((1, 10)));
        assert_eq!(a.get_next_pos(2, 10, Dir::Left), Some((1, 10)));
        assert_eq!(a.get_next_pos(37, 10, Dir::Right), Some((38, 10)));
    }

    // ---------- piloto automático ----------

    #[test]
    fn autopilot_goes_towards_nearby_dots() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.pacman_dir = Dir::Right;
        assert_eq!(a.autopilot_dir(), Dir::Right, "sin nada, sigue recto");
        for y in 4..=6 {
            a.set_dot(10, y, Dot::Normal);
        }
        assert_eq!(a.autopilot_dir(), Dir::Up);
    }

    #[test]
    fn autopilot_prefers_energizers_and_avoids_reversing() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.pacman_dir = Dir::Right;
        a.set_dot(10, 6, Dot::Energizer);
        assert_eq!(a.autopilot_dir(), Dir::Up);
        let mut b = arena(30, 15);
        b.pacman_x = 10;
        b.pacman_y = 8;
        b.pacman_dir = Dir::Right;
        b.set_dot(8, 8, Dot::Normal);
        b.set_dot(7, 8, Dot::Normal);
        // izquierda: 10 - 15 (reversa) + 4/1 + 4/2 < 15 de seguir recto
        assert_eq!(b.autopilot_dir(), Dir::Right);
    }

    #[test]
    fn autopilot_flees_from_a_dangerous_ghost() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.pacman_dir = Dir::Right;
        a.ghosts.push(ghost(13, 8, Dir::Left, "Blinky"));
        let d = a.autopilot_dir();
        assert_eq!(d, Dir::Up);
        assert_ne!(d, Dir::Right);
        // un fantasma muerto no asusta
        a.ghosts[0].dead = true;
        assert_eq!(a.autopilot_dir(), Dir::Right);
    }

    #[test]
    fn autopilot_chases_frightened_ghosts_but_not_dead_ones() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.pacman_dir = Dir::Up;
        a.energizer_timer = 30;
        a.ghosts.push(ghost(13, 8, Dir::Left, "Blinky"));
        assert_eq!(a.autopilot_dir(), Dir::Right);
        a.ghosts[0].dead = true;
        assert_eq!(a.autopilot_dir(), Dir::Up);
    }

    // ---------- fantasmas ----------

    #[test]
    fn each_ghost_picks_its_expected_target() {
        let mut a = arena(40, 20);
        a.pacman_x = 20;
        a.pacman_y = 10;
        a.pacman_dir = Dir::Right;
        for name in ["Blinky", "Pinky", "Inky", "Clyde"] {
            a.ghosts.push(ghost(30, 12, Dir::Left, name));
        }
        assert_eq!(a.ghost_target(0), (20, 10));
        assert_eq!(a.ghost_target(1), (24, 10));
        let (mut pac, mut corner) = (0, 0);
        for _ in 0..1000 {
            match a.ghost_target(2) {
                (20, 10) => pac += 1,
                (2, 3) => corner += 1,
                t => panic!("objetivo inesperado {t:?}"),
            }
        }
        assert!((200..400).contains(&pac), "Inky persigue ~30 %: {pac}");
        assert_eq!(pac + corner, 1000);
        // Clyde lejos (dist 12 > 8) persigue; cerca huye a su esquina.
        assert_eq!(a.ghost_target(3), (20, 10));
        a.ghosts[3].x = 18;
        a.ghosts[3].y = 10;
        assert_eq!(a.ghost_target(3), (2, 17));
        // Pinky ve 4 casillas adelante con wrap.
        a.pacman_x = 37;
        assert_eq!(a.ghost_target(1), (3, 10));
    }

    #[test]
    fn frightened_ghosts_flee_to_their_corners() {
        let mut a = arena(40, 20);
        a.energizer_timer = 10;
        for name in ["Blinky", "Pinky", "Inky", "Clyde"] {
            a.ghosts.push(ghost(20, 10, Dir::Up, name));
        }
        assert_eq!(a.ghost_target(0), (2, 3));
        assert_eq!(a.ghost_target(1), (37, 3));
        assert_eq!(a.ghost_target(2), (2, 17));
        assert_eq!(a.ghost_target(3), (37, 17));
    }

    #[test]
    fn ghosts_do_not_reverse_unless_dead_frightened_or_cornered() {
        let mut a = arena(30, 15);
        a.ghosts.push(ghost(10, 8, Dir::Right, "Blinky"));
        a.ghost_move_towards(0, 2, 8);
        // Izquierda está prohibida: Arriba y Abajo empatan (gana el primero).
        assert_eq!((a.ghosts[0].x, a.ghosts[0].y, a.ghosts[0].dir), (10, 7, Dir::Up));
        // Asustado: puede invertir.
        let mut b = arena(30, 15);
        b.energizer_timer = 5;
        b.ghosts.push(ghost(10, 8, Dir::Right, "Blinky"));
        b.ghost_move_towards(0, 2, 8);
        assert_eq!((b.ghosts[0].x, b.ghosts[0].dir), (9, Dir::Left));
        // Muerto: puede invertir.
        let mut c = arena(30, 15);
        c.ghosts.push(ghost(10, 8, Dir::Right, "Blinky"));
        c.ghosts[0].dead = true;
        c.ghost_move_towards(0, 2, 8);
        assert_eq!((c.ghosts[0].x, c.ghosts[0].dir), (9, Dir::Left));
    }

    #[test]
    fn ghosts_never_enter_walls_and_reverse_only_in_dead_ends() {
        let mut a = arena(30, 15);
        for (x, y) in [(10, 7), (10, 9), (11, 8)] {
            a.set_wall(x, y, true);
        }
        a.ghosts.push(ghost(10, 8, Dir::Right, "Blinky"));
        a.ghost_move_towards(0, 25, 8);
        assert_eq!((a.ghosts[0].x, a.ghosts[0].y, a.ghosts[0].dir), (9, 8, Dir::Left));
        // Encerrado por completo: se queda quieto.
        let mut b = arena(30, 15);
        for (x, y) in [(10, 7), (10, 9), (11, 8), (9, 8)] {
            b.set_wall(x, y, true);
        }
        b.ghosts.push(ghost(10, 8, Dir::Right, "Blinky"));
        b.ghost_move_towards(0, 25, 8);
        assert_eq!((b.ghosts[0].x, b.ghosts[0].y), (10, 8));
        // Sigue recto hacia el objetivo en pasillo libre.
        let mut c = arena(30, 15);
        c.ghosts.push(ghost(10, 8, Dir::Right, "Blinky"));
        c.ghost_move_towards(0, 25, 8);
        assert_eq!((c.ghosts[0].x, c.ghosts[0].dir), (11, Dir::Right));
    }

    #[test]
    fn dead_ghosts_return_to_the_plaza_and_revive() {
        let mut a = arena(30, 15);
        let (cx, cy) = a.plaza();
        let mut g = ghost(5, 4, Dir::Up, "Pinky");
        g.dead = true;
        a.ghosts.push(g);
        let mut moves = 0;
        while a.ghosts[0].dead {
            a.update_ghosts();
            moves += 1;
            assert!(moves < 60, "no vuelve a la plaza");
        }
        let g = &a.ghosts[0];
        assert!((g.x - cx).abs() + (g.y - cy).abs() <= 1);
        assert!(moves > 3, "tarda en llegar: {moves}");
    }

    // ---------- reglas de juego ----------

    #[test]
    fn eating_a_dot_scores_10() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.set_dot(11, 8, Dot::Normal);
        a.tick = 5;
        a.step();
        assert_eq!((a.pacman_x, a.pacman_y), (11, 8));
        assert_eq!(a.score, 10);
        assert_eq!(a.dot_at(11, 8), Dot::None);
        assert_eq!(a.energizer_timer, 0);
    }

    #[test]
    fn pacman_moves_only_every_six_ticks() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        run(&mut a, 5);
        assert_eq!((a.pacman_x, a.pacman_y), (10, 8));
        run(&mut a, 1);
        assert_eq!((a.pacman_x, a.pacman_y), (11, 8));
        run(&mut a, 5);
        assert_eq!(a.pacman_x, 11);
        run(&mut a, 1);
        assert_eq!(a.pacman_x, 12);
    }

    #[test]
    fn energizer_gives_50_and_lasts_60_moves_which_is_360_ticks() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.set_dot(11, 8, Dot::Energizer);
        a.tick = 5;
        a.step();
        assert_eq!(a.score, 50);
        assert_eq!(a.energizer_timer, 60);
        run(&mut a, 359);
        assert_eq!(a.energizer_timer, 1);
        a.step();
        assert_eq!(a.energizer_timer, 0);
    }

    #[test]
    fn energizer_lasts_the_same_real_time_at_30_fps() {
        let mut a = arena(30, 15);
        a.set_fps(30);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.set_dot(11, 8, Dot::Energizer);
        a.tick = 4; // el primer update (ticks 5 y 6) incluye el movimiento
        a.update(30, 15);
        assert_eq!(a.energizer_timer, 60);
        for _ in 0..179 {
            a.update(30, 15);
        }
        assert_eq!(a.energizer_timer, 1);
        a.update(30, 15);
        assert_eq!(a.energizer_timer, 0);
    }

    #[test]
    fn eating_a_frightened_ghost_gives_200() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.energizer_timer = 30;
        a.set_dot(11, 8, Dot::Normal);
        a.ghosts.push(ghost(12, 8, Dir::Left, "Blinky"));
        a.tick = 5;
        a.step();
        assert_eq!(a.score, 10 + 200);
        assert!(a.ghosts[0].dead);
        assert_eq!(a.state, GameState::Playing);
    }

    #[test]
    fn touching_a_ghost_without_power_kills_pacman() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        // Pasillo sin más salida que la derecha: el piloto no puede huir.
        for (x, y) in [(10, 7), (10, 9), (9, 8)] {
            a.set_wall(x, y, true);
        }
        a.set_dot(11, 8, Dot::Normal);
        a.ghosts.push(ghost(12, 8, Dir::Left, "Blinky"));
        a.tick = 5;
        a.step();
        assert_eq!(a.state, GameState::Dying { tick: 0 });
        assert_eq!(a.lives, 3);
    }

    #[test]
    fn death_sequence_takes_30_then_20_ticks_and_costs_a_life() {
        let mut a = arena(30, 15);
        a.score = 70;
        a.pacman_x = 3;
        a.pacman_y = 5;
        a.state = GameState::Dying { tick: 0 };
        run(&mut a, 29);
        assert_eq!(a.state, GameState::Dying { tick: 29 });
        assert_eq!(a.lives, 3);
        a.step();
        assert_eq!(a.lives, 2);
        assert_eq!(a.state, GameState::Respawning { tick: 0 });
        run(&mut a, 19);
        assert_eq!(a.state, GameState::Respawning { tick: 19 });
        a.energizer_timer = 7;
        a.step();
        assert_eq!(a.state, GameState::Playing);
        assert_eq!((a.pacman_x, a.pacman_y, a.pacman_dir), (15, 12, Dir::Right));
        assert_eq!(a.energizer_timer, 0);
        assert_eq!(a.score, 70);
    }

    #[test]
    fn respawn_puts_ghosts_back_in_the_plaza() {
        let mut a = anim(80, 24, 2, 60);
        a.ghosts[1].x = 3;
        a.ghosts[1].y = 4;
        a.ghosts[1].dead = true;
        a.state = GameState::Respawning { tick: 19 };
        a.step();
        assert_eq!(a.state, GameState::Playing);
        let (cx, cy) = a.plaza();
        for (i, g) in a.ghosts.iter().enumerate() {
            assert_eq!((g.x, g.y, g.dir, g.dead), (cx - 2 + i as i32, cy, Dir::Up, false));
        }
    }

    #[test]
    fn three_deaths_restart_the_whole_game() {
        let mut a = anim(80, 24, 2, 60);
        a.score = 900;
        a.stage = 4;
        for expected_lives in [2u32, 1] {
            a.state = GameState::Dying { tick: 29 };
            a.step();
            assert_eq!(a.lives, expected_lives);
            assert_eq!(a.state, GameState::Respawning { tick: 0 });
        }
        a.state = GameState::Dying { tick: 29 };
        a.step();
        assert_eq!((a.score, a.lives, a.stage), (0, 3, 1));
        assert_eq!(a.state, GameState::Playing);
        assert!(a.dots_left > 0);
    }

    #[test]
    fn clearing_the_level_gives_1000_and_a_new_maze() {
        let mut a = arena(30, 15);
        a.set_dot(0, 0, Dot::None);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.set_dot(11, 8, Dot::Normal);
        a.score = 5;
        a.tick = 5;
        a.step();
        assert_eq!(a.score, 5 + 10 + 1000);
        assert_eq!(a.stage, 2);
        assert_eq!(a.lives, 3);
        assert!(a.dots_left > 1);
        assert_eq!(a.ghosts.len(), 4);
        assert_eq!((a.pacman_x, a.pacman_y), (15, 12));
        assert_eq!(a.state, GameState::Playing);
    }

    #[test]
    fn idle_guard_advances_the_level_after_90_real_seconds_without_bonus() {
        let mut a = arena(30, 15);
        a.score = 40;
        a.lives = 2;
        run(&mut a, 5399);
        assert_eq!(a.stage, 1);
        a.step();
        assert_eq!((a.stage, a.score, a.lives), (2, 40, 2));
        assert_eq!(a.idle_ticks, 0);
        assert_eq!(a.ghosts.len(), 4);
        assert!(a.dots_left > 1);
        // Mismo tiempo real a 30 FPS: 2700 updates de 2 pasos.
        let mut b = arena(30, 15);
        b.set_fps(30);
        for _ in 0..2699 {
            b.update(30, 15);
        }
        assert_eq!(b.stage, 1);
        b.update(30, 15);
        assert_eq!((b.stage, b.score), (2, 0));
    }

    #[test]
    fn eating_resets_the_idle_counter() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.set_dot(11, 8, Dot::Normal);
        run(&mut a, 6);
        assert_eq!(a.score, 10);
        assert_eq!(a.idle_ticks, 0);
        run(&mut a, 5399);
        assert_eq!(a.stage, 1);
        a.step();
        assert_eq!(a.stage, 2);
    }

    #[test]
    fn resize_regenerates_and_keeps_score_lives_and_stage() {
        let mut a = anim(40, 20, 3, 30);
        a.score = 120;
        a.lives = 2;
        a.stage = 3;
        a.state = GameState::Dying { tick: 7 };
        let tick = a.tick;
        a.update(50, 22);
        assert_eq!((a.width, a.height), (50, 22));
        assert_eq!(a.walls.len(), 50 * 22);
        assert_eq!(a.state, GameState::Playing);
        assert_eq!((a.score, a.lives, a.stage), (120, 2, 3));
        assert_eq!(a.tick, tick, "el resize sale del update");
        assert_eq!(a.ghosts.len(), 4);
        let walls = a.walls.clone();
        for _ in 0..100 {
            a.update(50, 22);
        }
        assert_eq!(a.walls, walls, "sin resize el laberinto no se regenera");
    }

    // ---------- dibujo ----------

    #[test]
    fn hud_has_the_exact_text_positions_and_colors() {
        let mut a = anim(80, 24, 1, 30);
        a.score = 1234;
        a.stage = 3;
        a.lives = 2;
        let g = drawn(&mut a);
        assert_eq!(row_text(&g, 1, 0, TITLE.chars().count()), "RETRO PAC-MAN ARCADE");
        assert_eq!(g.get(1, 0).unwrap().1, YELLOW);
        assert_eq!(row_text(&g, 36, 0, 8), "STAGE: 3");
        assert_eq!(g.get(36, 0).unwrap().1, CYAN);
        assert_eq!(row_text(&g, 62, 0, 13), "SCORE: 001234");
        assert_eq!(g.get(62, 0).unwrap().1, GREEN);
        assert_eq!(row_text(&g, 1, 1, 11), "LIVES: ᗤ ᗤ ");
        assert_eq!(g.get(1, 1).unwrap().1, YELLOW);
        assert_eq!(g.get(62, 1).unwrap().0, ' ');
        for x in 0..80 {
            assert_eq!(g.get(x, 2), Some(('═', DIVIDER_COLOR, NO_COLOR)));
        }
        // 17 caracteres centrados: (80 - 17) / 2 = 31.
        assert_eq!(row_text(&g, 31, 23, 17), "AUTOPILOT RUNNING");
        assert_eq!(g.get(31, 23).unwrap().1, GRAY);
        assert_eq!(g.get(30, 23).unwrap().0, ' ');
        assert_eq!(g.get(48, 23).unwrap().0, ' ');
        for y in 0..24 {
            for x in 0..80 {
                assert_ne!(g.get(x, y).unwrap().0, '🕹');
            }
        }
    }

    #[test]
    fn power_indicator_shows_ceil_of_timer_over_ten_seconds() {
        let mut a = anim(80, 24, 1, 30);
        for (timer, text) in [(60u32, "POWER: 6s"), (25, "POWER: 3s"), (20, "POWER: 2s"), (1, "POWER: 1s")] {
            a.energizer_timer = timer;
            let g = drawn(&mut a);
            assert_eq!(row_text(&g, 62, 1, 9), text);
            assert_eq!(g.get(62, 1).unwrap().1, POWER_COLOR);
        }
        a.energizer_timer = 0;
        let g = drawn(&mut a);
        assert_eq!(g.get(62, 1).unwrap().0, ' ');
    }

    #[test]
    fn maze_cells_use_their_glyphs_and_colors() {
        let mut a = arena(30, 15);
        a.set_wall(5, 6, true);
        a.set_dot(7, 6, Dot::Normal);
        a.set_dot(8, 6, Dot::Energizer);
        a.tick = 0;
        let g = drawn(&mut a);
        assert_eq!(g.get(5, 6), Some(('█', WALL_COLOR, NO_COLOR)));
        assert_eq!(g.get(7, 6), Some(('·', DOT_COLOR, NO_COLOR)));
        assert_eq!(g.get(8, 6), Some(('●', WHITE, NO_COLOR)));
        a.tick = 15;
        let g = drawn(&mut a);
        assert_eq!(g.get(8, 6), Some(('●', DOT_COLOR, NO_COLOR)));
        // Bordes
        a.set_wall(1, 3, true);
        a.set_wall(28, 3, true);
        a.set_wall(1, 13, true);
        a.set_wall(28, 13, true);
        a.set_wall(10, 3, true);
        a.set_wall(1, 8, true);
        let g = drawn(&mut a);
        assert_eq!(g.get(1, 3).unwrap().0, '╔');
        assert_eq!(g.get(28, 3).unwrap().0, '╗');
        assert_eq!(g.get(1, 13).unwrap().0, '╚');
        assert_eq!(g.get(28, 13).unwrap().0, '╝');
        assert_eq!(g.get(10, 3).unwrap().0, '═');
        assert_eq!(g.get(1, 8).unwrap().0, '║');
    }

    #[test]
    fn pacman_glyph_follows_direction_and_mouth() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        for (dir, ch) in [(Dir::Up, 'ᗣ'), (Dir::Down, 'ᗢ'), (Dir::Left, 'ᗧ'), (Dir::Right, 'ᗤ')] {
            a.pacman_dir = dir;
            a.tick = 0;
            assert_eq!(drawn(&mut a).get(10, 8), Some((ch, YELLOW, NO_COLOR)));
            a.tick = 2;
            assert_eq!(drawn(&mut a).get(10, 8).unwrap().0, ch);
            a.tick = 3;
            assert_eq!(drawn(&mut a).get(10, 8), Some(('O', YELLOW, NO_COLOR)));
            a.tick = 6;
            assert_eq!(drawn(&mut a).get(10, 8).unwrap().0, ch);
        }
    }

    #[test]
    fn death_animation_and_blinking_respawn() {
        let mut a = arena(30, 15);
        a.pacman_x = 10;
        a.pacman_y = 8;
        a.ghosts.push(ghost(14, 8, Dir::Up, "Blinky"));
        let frames = ['ᗤ', 'ᗢ', 'ᗧ', 'ᗣ', 'x', ' '];
        for (tick, want) in [(0u32, 0usize), (4, 0), (5, 1), (10, 2), (15, 3), (20, 4), (25, 5), (29, 5)] {
            a.state = GameState::Dying { tick };
            let g = drawn(&mut a);
            assert_eq!(g.get(10, 8).unwrap().0, frames[want], "tick {tick}");
            assert_eq!(g.get(14, 8).unwrap().0, ' ', "fantasmas ocultos en Dying (tick {tick})");
        }
        for (tick, shown) in [(0u32, true), (4, true), (5, false), (9, false), (10, true), (15, false)] {
            a.state = GameState::Respawning { tick };
            a.tick = 0;
            let g = drawn(&mut a);
            assert_eq!(g.get(10, 8).unwrap().0 != ' ', shown, "respawn tick {tick}");
            assert_eq!(g.get(14, 8).unwrap().0, 'ᗛ');
        }
    }

    #[test]
    fn ghost_glyphs_and_colors() {
        let mut a = arena(40, 15);
        a.ghosts.push(ghost(10, 8, Dir::Up, "Blinky"));
        a.ghosts.push(ghost(12, 8, Dir::Up, "Pinky"));
        a.ghosts.push(ghost(14, 8, Dir::Up, "Inky"));
        a.ghosts.push(ghost(16, 8, Dir::Up, "Clyde"));
        let g = drawn(&mut a);
        assert_eq!(g.get(10, 8), Some(('ᗛ', rgb(255, 0, 0), NO_COLOR)));
        assert_eq!(g.get(12, 8).unwrap().1, rgb(255, 184, 255));
        assert_eq!(g.get(14, 8).unwrap().1, rgb(0, 255, 255));
        assert_eq!(g.get(16, 8).unwrap().1, rgb(255, 184, 81));
        // asustados
        a.energizer_timer = 40;
        assert_eq!(drawn(&mut a).get(10, 8), Some(('ᗛ', rgb(71, 71, 255), NO_COLOR)));
        // parpadeo con timer < 20 y (tick/8)%2 == 0
        a.energizer_timer = 19;
        a.tick = 0;
        assert_eq!(drawn(&mut a).get(10, 8).unwrap().1, WHITE);
        a.tick = 8;
        assert_eq!(drawn(&mut a).get(10, 8).unwrap().1, rgb(71, 71, 255));
        a.energizer_timer = 20;
        a.tick = 0;
        assert_eq!(drawn(&mut a).get(10, 8).unwrap().1, rgb(71, 71, 255));
        // muertos
        a.ghosts[0].dead = true;
        assert_eq!(drawn(&mut a).get(10, 8), Some(('⍛', rgb(120, 120, 120), NO_COLOR)));
    }

    #[test]
    fn tiny_grids_only_draw_a_clipped_hud() {
        let mut a = anim(8, 5, 1, 30);
        let g = drawn(&mut a);
        // SCORE cae en x = 0 (como en el original) y pisa el título.
        assert_eq!(row_text(&g, 0, 0, 8), "SCORE: 0");
        assert_eq!(row_text(&g, 1, 1, 7), "LIVES: ");
        assert_eq!(g.get(0, 2).unwrap().0, '═');
        for y in 3..5 {
            for x in 0..8 {
                assert_eq!(g.get(x, y).unwrap().0, ' ');
            }
        }
    }

    // ---------- tiempo real, determinismo y robustez ----------

    #[test]
    fn same_real_time_gives_same_state_at_20_30_and_60_fps() {
        let run_secs = |fps: u32| {
            let mut a = anim(80, 24, 7, fps);
            for _ in 0..fps * 25 {
                a.update(80, 24);
            }
            (a.tick, a.pacman_x, a.pacman_y, a.score, a.lives, a.stage, a.dots_left, a.ghosts.iter().map(|g| (g.x, g.y, g.dead)).collect::<Vec<_>>())
        };
        let base = run_secs(60);
        assert_eq!(base.0, 1500);
        assert_eq!(run_secs(30), base);
        assert_eq!(run_secs(20), base);
    }

    #[test]
    fn the_autopilot_actually_plays() {
        let mut a = anim(80, 24, 7, 60);
        for _ in 0..60 * 60 {
            a.update(80, 24);
        }
        assert!(a.score > 0 || a.stage > 1 || a.lives < 3);
    }

    #[test]
    fn degenerate_sizes_never_panic() {
        let mut sizes: Vec<(u16, u16)> = Vec::new();
        for w in 0..=14u16 {
            for h in 0..=14u16 {
                sizes.push((w, h));
            }
        }
        sizes.extend([(1024, 3), (3, 1024), (1, 1024), (1024, 1), (11, 9), (12, 10), (211, 58), (1024, 1024)]);
        for (w, h) in sizes {
            for fps in [30u32, 60, 0, u32::MAX] {
                let cells = w as usize * h as usize;
                let (updates, draw_every) = if cells > 100_000 { (if fps == 0 { 30 } else { 400 }, 53) } else if cells > 5_000 { (1_500, 31) } else { (3_000, 1) };
                let mut a = anim(w, h, 9, fps);
                let mut grid = Grid::new(w, h);
                for i in 0..updates {
                    a.update(w, h);
                    if i % draw_every == 0 {
                        a.draw(&mut grid);
                    }
                }
            }
        }
    }

    #[test]
    fn resize_between_updates_never_panics() {
        let mut a = anim(80, 24, 4, 30);
        let mut grid = Grid::new(80, 24);
        for i in 0..3_000u32 {
            let (w, h) = (((i * 7) % 130) as u16, ((i * 13) % 40) as u16);
            a.update(w, h);
            grid.resize(w, h);
            a.draw(&mut grid);
        }
    }

    #[test]
    fn same_seed_same_frames_different_seed_differs() {
        let run = |seed: u32| {
            let mut a = anim(80, 24, seed, 30);
            let mut grid = Grid::new(80, 24);
            let mut frames = Vec::new();
            for _ in 0..600 {
                a.update(80, 24);
                a.draw(&mut grid);
                frames.push(grid.data().to_vec());
            }
            frames
        };
        assert_eq!(run(4), run(4));
        assert_ne!(run(4), run(5));
    }
}

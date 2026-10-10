//! Port de `refs/cwork/src/animation/pipes.rs`.
//! Cambios: RNG propio, `Grid` en vez de `Buffer`, aritmética `i32` (el original
//! restaba en `u16` y hacía panic con `gen_range` en cuadrículas pequeñas), las
//! tuberías reclaman su celda inicial al crearse, y en lugar de limpiar al 55 %
//! la animación se llena hasta el 80 %, se queda quieta unos segundos y reinicia.

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};
use crate::rng::Rng;

const PIPE_COUNT: usize = 4;
const TURN_CHANCE: f32 = 0.18;
const FILL_PERCENT: usize = 80;
const HOLD_SECONDS: u32 = 4;
const DEFAULT_FPS: u32 = 30;
const SPAWN_TRIES: usize = 50;

const COLORS: [u32; 7] = [
    rgb(255, 85, 85),
    rgb(85, 255, 85),
    rgb(255, 255, 85),
    rgb(95, 135, 255),
    rgb(255, 95, 255),
    rgb(85, 255, 255),
    rgb(255, 165, 0),
];

type CanvasCell = (char, u32);
const BLANK: CanvasCell = (' ', NO_COLOR);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

const DIRS: [Direction; 4] = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];

impl Direction {
    fn perpendicular(self) -> [Direction; 2] {
        match self {
            Direction::Up | Direction::Down => [Direction::Left, Direction::Right],
            Direction::Left | Direction::Right => [Direction::Up, Direction::Down],
        }
    }

    fn straight_glyph(self) -> char {
        match self {
            Direction::Up | Direction::Down => '║',
            Direction::Left | Direction::Right => '═',
        }
    }
}

/// Glifo de la celda que se abandona al ir de `from` a `to`.
fn connection_glyph(from: Direction, to: Direction) -> char {
    if from == to {
        return from.straight_glyph();
    }
    match (from, to) {
        (Direction::Right, Direction::Up) => '╝',
        (Direction::Right, Direction::Down) => '╗',
        (Direction::Left, Direction::Up) => '╚',
        (Direction::Left, Direction::Down) => '╔',
        (Direction::Down, Direction::Left) => '╝',
        (Direction::Down, Direction::Right) => '╚',
        (Direction::Up, Direction::Left) => '╗',
        (Direction::Up, Direction::Right) => '╔',
        _ => '═',
    }
}

#[derive(Clone, Copy)]
struct Pipe {
    x: i32,
    y: i32,
    dir: Direction,
    color: u32,
    dead: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Running,
    Holding,
}

pub struct PipesAnimation {
    pipes: Vec<Pipe>,
    occupied: Vec<bool>,
    canvas: Vec<CanvasCell>,
    occupied_count: usize,
    width: u16,
    height: u16,
    rng: Rng,
    state: State,
    hold_frames: u32,
    hold_left: u32,
    #[cfg(test)]
    cycles: u32,
}

fn hold_frames_for(fps: u32) -> u32 {
    fps.max(1).saturating_mul(HOLD_SECONDS)
}

impl PipesAnimation {
    pub fn new(width: u16, height: u16, seed: u32) -> Self {
        let mut anim = Self {
            pipes: Vec::new(),
            occupied: Vec::new(),
            canvas: Vec::new(),
            occupied_count: 0,
            width,
            height,
            rng: Rng::new(seed as u64),
            state: State::Running,
            hold_frames: hold_frames_for(DEFAULT_FPS),
            hold_left: 0,
            #[cfg(test)]
            cycles: 0,
        };
        anim.rebuild();
        anim
    }

    /// Reserva los buffers al tamaño actual, vacíos, y recrea las tuberías.
    fn rebuild(&mut self) {
        let size = self.width as usize * self.height as usize;
        self.occupied = vec![false; size];
        self.canvas = vec![BLANK; size];
        self.reset_grid();
    }

    fn reset_grid(&mut self) {
        self.occupied.fill(false);
        self.canvas.fill(BLANK);
        self.occupied_count = 0;
        self.state = State::Running;
        self.pipes.clear();
        for _ in 0..PIPE_COUNT {
            let pipe = self.spawn_pipe();
            self.pipes.push(pipe);
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    fn is_free(&self, x: i32, y: i32) -> bool {
        self.index(x, y).map_or(false, |i| !self.occupied[i])
    }

    /// Crea una tubería; si encuentra una celda libre en el interior la reclama
    /// (dibuja `●`), si no queda muerta.
    fn spawn_pipe(&mut self) -> Pipe {
        let color = COLORS[self.rng.range_i32(0, COLORS.len() as i32) as usize];
        let mut pipe = Pipe { x: 0, y: 0, dir: Direction::Up, color, dead: true };

        let (w, h) = (self.width as i32, self.height as i32);
        if w > 4 && h > 4 {
            for _ in 0..SPAWN_TRIES {
                let x = self.rng.range_i32(2, w - 2);
                let y = self.rng.range_i32(2, h - 2);
                if self.is_free(x, y) {
                    pipe.x = x;
                    pipe.y = y;
                    pipe.dead = false;
                    break;
                }
            }
        }
        pipe.dir = DIRS[self.rng.range_i32(0, DIRS.len() as i32) as usize];

        if !pipe.dead {
            if let Some(i) = self.index(pipe.x, pipe.y) {
                self.occupied[i] = true;
                self.canvas[i] = ('●', color);
                self.occupied_count += 1;
            }
        }
        pipe
    }

    fn next_free(&self, x: i32, y: i32, dir: Direction) -> Option<(i32, i32)> {
        let (nx, ny) = match dir {
            Direction::Up => (x, y - 1),
            Direction::Down => (x, y + 1),
            Direction::Left => (x - 1, y),
            Direction::Right => (x + 1, y),
        };
        if self.is_free(nx, ny) {
            Some((nx, ny))
        } else {
            None
        }
    }

    fn step_pipe(&mut self, mut pipe: Pipe) -> Pipe {
        let mut dir = pipe.dir;
        if self.rng.chance(TURN_CHANCE) {
            let perp = pipe.dir.perpendicular();
            dir = perp[self.rng.range_i32(0, 2) as usize];
        }

        let mut dest = self.next_free(pipe.x, pipe.y, dir);
        if dest.is_none() {
            let mut fallbacks = Vec::with_capacity(3);
            if dir != pipe.dir {
                fallbacks.push(pipe.dir);
            }
            for d in pipe.dir.perpendicular() {
                if d != dir {
                    fallbacks.push(d);
                }
            }
            for d in fallbacks {
                if let Some(p) = self.next_free(pipe.x, pipe.y, d) {
                    dir = d;
                    dest = Some(p);
                    break;
                }
            }
        }

        match dest {
            Some((nx, ny)) => {
                if let Some(old) = self.index(pipe.x, pipe.y) {
                    self.canvas[old] = (connection_glyph(pipe.dir, dir), pipe.color);
                }
                if let Some(new) = self.index(nx, ny) {
                    self.occupied[new] = true;
                    self.canvas[new] = (dir.straight_glyph(), pipe.color);
                    self.occupied_count += 1;
                }
                pipe.x = nx;
                pipe.y = ny;
                pipe.dir = dir;
            }
            None => pipe.dead = true,
        }
        pipe
    }
}

impl Animation for PipesAnimation {
    fn update(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.rebuild();
            return;
        }

        if self.state == State::Holding {
            self.hold_left = self.hold_left.saturating_sub(1);
            if self.hold_left == 0 {
                self.reset_grid();
                #[cfg(test)]
                {
                    self.cycles += 1;
                }
            }
            return;
        }

        // Atasco = las PIPE_COUNT tuberías estaban muertas al empezar, todas
        // intentaron reaparecer y ninguna encontró celda libre. Las que mueren
        // en esta actualización no cuentan: reaparecen en la siguiente.
        let (mut attempts, mut failures) = (0, 0);
        for i in 0..self.pipes.len() {
            let pipe = self.pipes[i];
            self.pipes[i] = if pipe.dead {
                attempts += 1;
                let respawned = self.spawn_pipe();
                if respawned.dead {
                    failures += 1;
                }
                respawned
            } else {
                self.step_pipe(pipe)
            };
        }

        let size = self.width as usize * self.height as usize;
        let full = size > 0 && self.occupied_count * 100 >= size * FILL_PERCENT;
        let stuck = attempts == PIPE_COUNT && failures == PIPE_COUNT;
        if full || stuck {
            self.state = State::Holding;
            self.hold_left = self.hold_frames;
        }
    }

    fn draw(&mut self, grid: &mut Grid) {
        grid.clear();
        let w = self.width as usize;
        if w == 0 {
            return;
        }
        for (i, &(ch, fg)) in self.canvas.iter().enumerate() {
            if ch != ' ' {
                grid.set((i % w) as i32, (i / w) as i32, ch, fg, NO_COLOR);
            }
        }
    }

    fn set_fps(&mut self, fps: u32) {
        self.hold_frames = hold_frames_for(fps);
        self.hold_left = self.hold_left.min(self.hold_frames);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLYPHS: [char; 7] = ['║', '═', '╔', '╗', '╚', '╝', '●'];

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

    fn assert_invariant(p: &PipesAnimation) {
        let occ = p.occupied.iter().filter(|&&b| b).count();
        let ink = p.canvas.iter().filter(|c| c.0 != ' ').count();
        assert_eq!(p.occupied_count, occ);
        assert_eq!(p.occupied_count, ink);
    }

    #[test]
    fn draws_only_pipe_glyphs() {
        let mut p = PipesAnimation::new(80, 24, 1);
        let mut grid = Grid::new(80, 24);
        for _ in 0..200 {
            p.update(80, 24);
        }
        p.draw(&mut grid);
        assert!(non_blank(&grid) > 0);
        for y in 0..24 {
            for x in 0..80 {
                let (ch, fg, bg) = grid.get(x, y).unwrap();
                if ch != ' ' {
                    assert!(GLYPHS.contains(&ch), "glifo inesperado {ch:?}");
                    assert_ne!(fg, NO_COLOR);
                    assert_eq!(bg, NO_COLOR);
                }
            }
        }
    }

    #[test]
    fn occupancy_invariant_holds_every_update() {
        let mut p = PipesAnimation::new(80, 24, 7);
        assert_invariant(&p);
        for _ in 0..300 {
            p.update(80, 24);
            assert_invariant(&p);
        }
    }

    #[test]
    fn creation_claims_distinct_start_cells() {
        let p = PipesAnimation::new(80, 24, 3);
        assert_eq!(p.occupied_count, 4);
        assert_eq!(p.canvas.iter().filter(|c| c.0 == '●').count(), 4);
    }

    #[test]
    fn fills_holds_unchanged_then_resets() {
        let mut p = PipesAnimation::new(80, 24, 5);
        p.set_fps(1);
        let mut guard = 0;
        while p.state != State::Holding {
            p.update(80, 24);
            guard += 1;
            assert!(guard < 200_000, "nunca entró en Holding");
        }
        let size = 80 * 24;
        assert!(p.occupied_count * 100 >= size * 80 || p.pipes.iter().all(|x| x.dead));
        assert_eq!(p.hold_left, 4);
        let frozen = p.canvas.clone();
        let count = p.occupied_count;
        for _ in 0..3 {
            p.update(80, 24);
            assert_eq!(p.canvas, frozen);
            assert_eq!(p.occupied_count, count);
            assert_eq!(p.state, State::Holding);
        }
        p.update(80, 24);
        assert_eq!(p.state, State::Running);
        assert_eq!(p.occupied_count, 4);
        assert_eq!(p.canvas.iter().filter(|c| c.0 != ' ').count(), p.occupied_count);
        assert_invariant(&p);
    }

    #[test]
    fn set_fps_controls_hold_length_and_clamps_countdown() {
        let mut p = PipesAnimation::new(80, 24, 1);
        assert_eq!(p.hold_frames, 120);
        p.set_fps(60);
        assert_eq!(p.hold_frames, 240);
        p.set_fps(0);
        assert_eq!(p.hold_frames, 4);
        p.set_fps(u32::MAX);
        assert_eq!(p.hold_frames, u32::MAX);

        p.set_fps(30);
        let mut guard = 0;
        while p.state != State::Holding {
            p.update(80, 24);
            guard += 1;
            assert!(guard < 200_000);
        }
        assert_eq!(p.hold_left, 120);
        p.update(80, 24);
        assert_eq!(p.hold_left, 119);
        p.set_fps(60); // alargar no reinicia ni alarga la cuenta atrás
        assert_eq!(p.hold_left, 119);
        p.set_fps(1); // acortar la recorta
        assert_eq!(p.hold_left, 4);
        assert_eq!(p.state, State::Holding);
    }

    #[test]
    fn resize_rebuilds_buffers_and_returns_to_running() {
        let mut p = PipesAnimation::new(80, 24, 2);
        p.set_fps(1);
        let mut guard = 0;
        while p.state != State::Holding {
            p.update(80, 24);
            guard += 1;
            assert!(guard < 200_000);
        }
        p.update(40, 12);
        assert_eq!(p.state, State::Running);
        assert_eq!(p.occupied.len(), 40 * 12);
        assert_eq!(p.canvas.len(), 40 * 12);
        assert_eq!((p.width, p.height), (40, 12));
        assert_invariant(&p);
    }

    #[test]
    fn degenerate_sizes_do_not_panic_and_never_get_stuck() {
        for (w, h) in [(0, 0), (0, 5), (5, 0), (1, 1), (4, 4), (3, 10), (10, 3)] {
            let mut p = PipesAnimation::new(w, h, 11);
            let mut grid = Grid::new(w, h);
            for _ in 0..3_000 {
                p.update(w, h);
                p.draw(&mut grid);
                assert_invariant(&p);
            }
            assert!(p.cycles >= 1, "{w}x{h} se quedó atascada");
        }
    }

    #[test]
    fn small_grid_completes_a_fill_hold_reset_cycle() {
        let mut p = PipesAnimation::new(10, 8, 9);
        p.set_fps(1);
        for _ in 0..20_000 {
            p.update(10, 8);
            if p.cycles >= 1 {
                break;
            }
        }
        assert!(p.cycles >= 1);
        assert_eq!(p.state, State::Running);
    }

    #[test]
    fn full_size_grid_holds_only_at_eighty_percent() {
        let (w, h) = (211u16, 58u16);
        let size = w as usize * h as usize;
        let mut failures = Vec::new();
        for seed in 1..=200u32 {
            let mut p = PipesAnimation::new(w, h, seed);
            p.set_fps(1);
            let mut n = 0;
            while p.state != State::Holding {
                p.update(w, h);
                n += 1;
                assert!(n < 20_000, "seed {seed}: nunca entró en Holding");
            }
            if p.occupied_count * 100 < size * 80 {
                failures.push((seed, p.occupied_count * 100 / size));
            }
        }
        assert!(failures.is_empty(), "hold anticipado (semilla, % ocupación): {failures:?}");
    }

    #[test]
    fn pipes_dying_in_the_same_update_do_not_trigger_hold() {
        // Se bloquea el tablero a mano (todo ocupado) con las cuatro tuberías
        // vivas; el contador queda bajo el 80 % para aislar la regla de atasco.
        let mut p = PipesAnimation::new(10, 8, 1);
        p.occupied.fill(true);
        for (i, pipe) in p.pipes.iter_mut().enumerate() {
            pipe.x = 5;
            pipe.y = 2 + i as i32 % 4;
            pipe.dead = false;
        }
        p.update(10, 8);
        assert_eq!(p.state, State::Running);
        assert!(p.pipes.iter().all(|x| x.dead));
        // En la siguiente actualización intentan reaparecer y todas fallan: ahora sí.
        p.update(10, 8);
        assert_eq!(p.state, State::Holding);
    }

    #[test]
    fn same_seed_same_frames_different_seed_differs() {
        let frame = |seed: u32| {
            let mut p = PipesAnimation::new(80, 24, seed);
            let mut grid = Grid::new(80, 24);
            for _ in 0..500 {
                p.update(80, 24);
            }
            p.draw(&mut grid);
            grid.data().to_vec()
        };
        assert_eq!(frame(4), frame(4));
        assert_ne!(frame(4), frame(5));
    }
}

//! Port de `refs/cwork/src/animation/mario.rs`.
//! Cambios respecto al original:
//! - RNG propio (`Rng`) en vez de `rand`, `Grid` en vez de `Buffer`.
//! - Las constantes de física (scroll 0,4, gravedad 0,085, salto -1,45...) son
//!   por tick a 60 ticks/s. `set_fps` ajusta los pasos lógicos por `update`
//!   (como pacman/`compile_sim`): el tiempo real es el mismo a 20, 30 y 60 FPS.
//!   El resize se detecta una sola vez por `update`, antes del bucle de pasos.
//! - Coordenadas y alturas en `i32` (el original usaba `u16` y `floor_y - 6`,
//!   `height - 1`, `height - 2` desbordan en pantallas chicas); cero `gen_range`
//!   con rango vacío ni indexado sin guardia.
//! - Tamaño mínimo `MIN_W` x `MIN_H`: por debajo no hay juego (ni nubes, ni
//!   tuberías, ni goombas, ni bloques, ni Mario) y solo se dibuja cielo (si
//!   cabe), suelo y barra al pie, con recorte seguro.
//! - Los `Vec` de elementos están acotados (`MAX_ELEMS`, `MAX_COINS`): a anchos
//!   grandes no crecen sin límite.
//! - DESVIACIÓN: el golpe de bloque `?` es por CRUCE. El original exigía
//!   `round(y) - 3 == y_del_bloque` exacto en el mismo tick, pero Mario sube más
//!   de una fila por tick y podía saltárselo. Ahora el bloque se golpea si, en un
//!   paso en que Mario sube, la fila de la cabeza pasó de estar por debajo del
//!   bloque (`> qby`) a estar a su altura o por encima (`<= qby`), con la misma
//!   condición horizontal `|mx - qbx| <= 2`.
//! - En el resize `mario_vy` también se pone a 0 (el original lo conservaba con
//!   el estado ya en `Running`, y la comprobación de pisotón lo usa). Se omite el
//!   `scroll_offset` del original, que solo se escribía y nunca se leía.
//! - Se quitan los emoji del contador (`🪙` pasa a `●`) y del pie, y lo de `q`.
//! - Pie centrado por número de caracteres (el original usaba bytes).

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};
use crate::rng::Rng;

// ---------- colores (idénticos a los originales) ----------

const SKY: u32 = rgb(92, 148, 252);
const GROUND: u32 = rgb(200, 76, 12);
const BRICK_SHADOW: u32 = rgb(0, 0, 0);
const BLACK: u32 = rgb(0, 0, 0);
const GREEN: u32 = rgb(0, 168, 0);
const PEACH: u32 = rgb(252, 188, 176);
const RED: u32 = rgb(228, 0, 0);
const BLUE: u32 = rgb(0, 0, 168);
const YELLOW: u32 = rgb(252, 228, 24);
const WHITE: u32 = rgb(255, 255, 255);
const GRAY: u32 = rgb(120, 120, 120);
const BROWN: u32 = rgb(140, 60, 0);
/// Amarillo apagado del parpadeo del bloque `?`.
const YELLOW_DIM: u32 = rgb(212, 156, 0);

// ---------- símbolos, todos en un único sitio ----------

const BUSH_LEFT: char = '◢';
const BUSH_MID: char = '█';
const BUSH_RIGHT: char = '◣';
const COIN_FACE: char = '●';
const COIN_EDGE: char = '│';
const PIPE_BODY: char = '║';
const PIPE_LIP: &str = "▄██▄";
const GOOMBA_FLAT: char = '▄';
const BRICK_SHADE: char = '░';
const BRICK_SOLID: char = '█';
const QBLOCK_ACTIVE: &str = "[?]";
const QBLOCK_SPENT: &str = "[ ]";
const QBLOCK_MARK: char = '?';

/// Fila de un sprite: texto, color de `█`/`▄` y color del resto de símbolos.
type Row = (&'static str, u32, u32);

const CLOUD_SMALL: [Row; 2] = [(" ▄█▄ ", WHITE, WHITE), ("▀███▀", WHITE, WHITE)];
const CLOUD_LARGE: [Row; 2] = [(" ▄███▄ ", WHITE, WHITE), ("▀█████▀", WHITE, WHITE)];
const GOOMBA_FRAMES: [[Row; 2]; 2] = [
    [(" ▄▄ ", BROWN, BROWN), ("▀  ▀", PEACH, PEACH)],
    [(" ▄▄ ", BROWN, BROWN), (" ▀▀ ", PEACH, PEACH)],
];
const MARIO_RUN: [[Row; 3]; 3] = [
    [(" ▄▀▄ ", RED, PEACH), (" ▄█▄ ", RED, BLUE), (" █ █ ", BLUE, BROWN)],
    [(" ▄▀▄ ", RED, PEACH), ("▄██▄ ", RED, BLUE), (" ▀ █ ", BLUE, BROWN)],
    [(" ▄▀▄ ", RED, PEACH), (" ▄██▄", RED, BLUE), (" █ ▀ ", BLUE, BROWN)],
];
const MARIO_JUMP: [Row; 3] = [(" ▄▀▄▄", RED, PEACH), (" ░█▀ ", BLUE, WHITE), (" ▄▀  ", BLUE, BROWN)];
const MARIO_DEAD: [Row; 3] = [(" ▄▀▄ ", RED, PEACH), (" ▄█▄ ", RED, BLUE), (" ▀ ▀ ", BLACK, BLACK)];

const FOOTER: &str = "AUTOPILOT ACTIVE | JUMPING & STOMPING";
const WORLD: &str = "WORLD  1-1";

// ---------- ritmo y física (por tick a 60 ticks/s) ----------

const TICKS_PER_SECOND: u64 = 60;
const DEFAULT_FPS: u32 = 30;
const SCROLL_SPEED: f32 = 0.40;
const CLOUD_SCROLL: f32 = 0.05;
const BUSH_SCROLL: f32 = 0.18;
const GOOMBA_EXTRA: f32 = 0.08;
const JUMP_VY: f32 = -1.45;
const GRAVITY: f32 = 0.085;
const STOMP_VY: f32 = -0.75;
const COIN_VY: f32 = -0.65;
const COIN_GRAVITY: f32 = 0.12;
const COIN_TICKS: u32 = 18;
const DYING_TICKS: u32 = 45;
const GOOMBA_DEAD_TICKS: u32 = 25;
const STOMP_SCORE: u32 = 100;
const BLOCK_SCORE: u32 = 200;
const MIN_W: i32 = 20;
const MIN_H: i32 = 12;
/// Tope de elementos por `Vec` de escenario o de juego.
/// Distancia (columnas pasado el borde derecho) del primer trío del nivel; el
/// original usaba 10, 22 y 35 fijos.
const FIRST_PIPE_DIST: (f32, f32) = (10.0, 30.0);
const FIRST_BLOCK_DIST: (f32, f32) = (15.0, 45.0);
const FIRST_GOOMBA_DIST: (f32, f32) = (25.0, 80.0);
const MAX_ELEMS: usize = 128;
/// Tope de monedas emergentes simultáneas.
const MAX_COINS: usize = 32;

struct Pipe {
    x: f32,
    h: i32,
}

struct Goomba {
    x: f32,
    y: f32,
    frame: usize,
    dead: bool,
    dead_timer: u32,
}

struct QBlock {
    x: f32,
    y: i32,
    hit: bool,
    bounce_offset: i32,
}

struct PopCoin {
    x: f32,
    y: f32,
    vy: f32,
    timer: u32,
}

struct Cloud {
    x: f32,
    y: i32,
    size: i32,
}

struct Bush {
    x: f32,
    y: i32,
    w: i32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum MarioState {
    Running,
    Jumping,
    Dying { tick: u32 },
}

/// Pasos lógicos (a 60/s) por llamada a `update` para un FPS dado.
fn steps_for(fps: u32) -> u32 {
    let fps = fps.max(1) as u64;
    ((TICKS_PER_SECOND + fps / 2) / fps).max(1) as u32
}

fn can_play(width: i32, height: i32) -> bool {
    width >= MIN_W && height >= MIN_H
}

pub struct MarioAnimation {
    width: u16,
    height: u16,
    tick: u64,
    mario_y: f32,
    mario_vy: f32,
    state: MarioState,
    run_frame: usize,
    score: u32,
    coins: u32,
    clouds: Vec<Cloud>,
    bushes: Vec<Bush>,
    pipes: Vec<Pipe>,
    goombas: Vec<Goomba>,
    q_blocks: Vec<QBlock>,
    pop_coins: Vec<PopCoin>,
    floor_y: i32,
    rng: Rng,
    steps: u32,
}

impl MarioAnimation {
    pub fn new(width: u16, height: u16, seed: u32) -> Self {
        let floor_y = height as i32 - 4;
        let mut s = Self {
            width,
            height,
            tick: 0,
            mario_y: floor_y as f32,
            mario_vy: 0.0,
            state: MarioState::Running,
            run_frame: 0,
            score: 0,
            coins: 0,
            clouds: Vec::new(),
            bushes: Vec::new(),
            pipes: Vec::new(),
            goombas: Vec::new(),
            q_blocks: Vec::new(),
            pop_coins: Vec::new(),
            floor_y,
            rng: Rng::new(seed as u64),
            steps: steps_for(DEFAULT_FPS),
        };
        s.init_level();
        s
    }

    fn w(&self) -> i32 {
        self.width as i32
    }

    fn h(&self) -> i32 {
        self.height as i32
    }

    /// Hay juego (tamaño suficiente).
    fn playable(&self) -> bool {
        can_play(self.w(), self.h())
    }

    /// Mario corre fijo en el primer cuarto de la pantalla.
    fn mario_x(&self) -> i32 {
        self.w() / 4
    }

    /// Mario de vuelta en el suelo, quieto.
    fn reset_mario(&mut self) {
        self.mario_y = self.floor_y as f32;
        self.mario_vy = 0.0;
    }

    /// Regenera todo el escenario. Sin tamaño mínimo no deja nada.
    fn init_level(&mut self) {
        self.clouds.clear();
        self.bushes.clear();
        self.pipes.clear();
        self.goombas.clear();
        self.q_blocks.clear();
        self.pop_coins.clear();
        if !self.playable() {
            return;
        }
        let w = self.w() as f32;
        let floor_y = self.floor_y;

        let mut cx = 10.0f32;
        while cx < w && self.clouds.len() < MAX_ELEMS {
            let y = self.rng.range_incl_i32(4, 8);
            let size = self.rng.range_incl_i32(1, 2);
            self.clouds.push(Cloud { x: cx, y, size });
            cx += self.rng.range_f32(30.0, 50.0);
        }

        let mut bx = 5.0f32;
        while bx < w && self.bushes.len() < MAX_ELEMS {
            let bw = self.rng.range_incl_i32(3, 6);
            self.bushes.push(Bush { x: bx, y: floor_y, w: bw });
            bx += self.rng.range_f32(20.0, 40.0);
        }

        // Desviación del original: allí el primer trío estaba fijo (tubería a `w+10`,
        // bloque a `w+22`, goomba a `w+35`) y, tras cada muerte, el reinicio repetía
        // la misma escena y la misma muerte. Aquí las distancias se sortean en cada
        // reinicio, con rangos que contienen los valores originales.
        let h = self.rng.range_incl_i32(2, 3);
        let pipe_x = self.rng.range_f32(FIRST_PIPE_DIST.0, FIRST_PIPE_DIST.1);
        let block_x = self.rng.range_f32(FIRST_BLOCK_DIST.0, FIRST_BLOCK_DIST.1);
        let goomba_x = self.rng.range_f32(FIRST_GOOMBA_DIST.0, FIRST_GOOMBA_DIST.1);
        self.pipes.push(Pipe { x: w + pipe_x, h });
        self.q_blocks.push(QBlock { x: w + block_x, y: floor_y - 6, hit: false, bounce_offset: 0 });
        self.goombas.push(Goomba { x: w + goomba_x, y: floor_y as f32 - 1.0, frame: 0, dead: false, dead_timer: 0 });
    }

    fn trigger_jump(&mut self) {
        if self.state == MarioState::Running {
            self.state = MarioState::Jumping;
            self.mario_vy = JUMP_VY;
        }
    }

    /// Mueve el escenario, hace aparecer elementos nuevos y retira los que
    /// salieron por la izquierda.
    fn scroll_world(&mut self) {
        let w = self.w() as f32;
        let floor_y = self.floor_y;

        for c in &mut self.clouds {
            c.x -= CLOUD_SCROLL;
        }
        if self.clouds.len() < MAX_ELEMS && self.clouds.last().map_or(true, |c| c.x < w - 15.0) {
            let y = self.rng.range_incl_i32(4, 8);
            let size = self.rng.range_incl_i32(1, 2);
            self.clouds.push(Cloud { x: w + 10.0, y, size });
        }
        self.clouds.retain(|c| c.x > -15.0);

        for b in &mut self.bushes {
            b.x -= BUSH_SCROLL;
        }
        if self.bushes.len() < MAX_ELEMS && self.bushes.last().map_or(true, |b| b.x < w - 20.0) {
            let dx = self.rng.range_f32(5.0, 25.0);
            let bw = self.rng.range_incl_i32(3, 6);
            self.bushes.push(Bush { x: w + dx, y: floor_y, w: bw });
        }
        self.bushes.retain(|b| b.x > -10.0);

        for p in &mut self.pipes {
            p.x -= SCROLL_SPEED;
        }
        if self.pipes.len() < MAX_ELEMS && self.pipes.last().map_or(true, |p| p.x < w - 22.0) {
            let dx = self.rng.range_f32(8.0, 28.0);
            let h = self.rng.range_incl_i32(2, 3);
            self.pipes.push(Pipe { x: w + dx, h });
        }
        self.pipes.retain(|p| p.x > -6.0);

        for qb in &mut self.q_blocks {
            qb.x -= SCROLL_SPEED;
            // El bloque golpeado sube una fila y vuelve a su sitio.
            if qb.hit && qb.bounce_offset != 0 {
                qb.bounce_offset += 1;
                if qb.bounce_offset > 2 {
                    qb.bounce_offset = 0;
                }
            }
        }
        if self.q_blocks.len() < MAX_ELEMS && self.q_blocks.last().map_or(true, |q| q.x < w - 18.0) {
            let dx = self.rng.range_f32(12.0, 32.0);
            self.q_blocks.push(QBlock { x: w + dx, y: floor_y - 6, hit: false, bounce_offset: 0 });
        }
        self.q_blocks.retain(|q| q.x > -5.0);

        for pc in &mut self.pop_coins {
            pc.x -= SCROLL_SPEED;
            pc.y += pc.vy;
            pc.vy += COIN_GRAVITY;
            pc.timer += 1;
        }
        self.pop_coins.retain(|pc| pc.timer < COIN_TICKS);

        let tick = self.tick;
        for g in &mut self.goombas {
            // Los goombas caminan más rápido que el scroll.
            g.x -= SCROLL_SPEED + GOOMBA_EXTRA;
            if g.dead {
                g.dead_timer += 1;
            } else if tick % 8 == 0 {
                g.frame = (g.frame + 1) % 2;
            }
        }
        if self.goombas.len() < MAX_ELEMS && self.goombas.last().map_or(true, |g| g.x < w - 15.0) {
            let dx = self.rng.range_f32(15.0, 35.0);
            self.goombas.push(Goomba { x: w + dx, y: floor_y as f32 - 1.0, frame: 0, dead: false, dead_timer: 0 });
        }
        self.goombas.retain(|g| g.x > -5.0 && (!g.dead || g.dead_timer < GOOMBA_DEAD_TICKS));
    }

    /// Piloto automático: salta ante tuberías, goombas vivos y bloques `?`.
    fn autopilot(&mut self, mario_x: f32) {
        let mut nearest = 999.0f32;
        let mut dangerous = false;
        for p in &self.pipes {
            let dist = p.x - mario_x;
            if dist > -2.0 && dist < nearest {
                nearest = dist;
                dangerous = true;
            }
        }
        for g in &self.goombas {
            if !g.dead {
                let dist = g.x - mario_x;
                if dist > -1.0 && dist < nearest {
                    nearest = dist;
                    dangerous = true;
                }
            }
        }
        let jump_for_block = self.q_blocks.iter().any(|q| {
            let dist = q.x - mario_x;
            !q.hit && (-1.0..3.5).contains(&dist)
        });
        if jump_for_block {
            self.trigger_jump();
        }
        if dangerous && (0.0..6.0).contains(&nearest) {
            self.trigger_jump();
        }
    }

    /// Un tick lógico (60 por segundo).
    fn step(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        if !self.playable() {
            return;
        }

        if let MarioState::Dying { tick } = self.state {
            let tick = tick + 1;
            // Salto clásico de la muerte.
            if tick < 15 {
                self.mario_y -= 0.8;
            } else if tick < 35 {
                self.mario_y += 0.8;
            }
            if tick >= DYING_TICKS {
                self.score = 0;
                self.coins = 0;
                self.reset_mario();
                self.init_level();
                self.state = MarioState::Running;
            } else {
                self.state = MarioState::Dying { tick };
            }
            return;
        }

        self.scroll_world();

        // Física de Mario.
        let mario_x = self.mario_x();
        let prev_head = round_i32(self.mario_y) - 3;
        let rising = self.state == MarioState::Jumping && self.mario_vy < 0.0;
        if self.state == MarioState::Jumping {
            self.mario_y += self.mario_vy;
            self.mario_vy += GRAVITY;
            let floor = self.floor_y as f32;
            if self.mario_y >= floor {
                self.mario_y = floor;
                self.mario_vy = 0.0;
                self.state = MarioState::Running;
            }
        } else if self.tick % 5 == 0 {
            self.run_frame = (self.run_frame + 1) % 3;
        }

        self.autopilot(mario_x as f32);

        // Colisiones.
        let mx = mario_x;
        let my = round_i32(self.mario_y);

        // Golpe de bloque desde abajo. DESVIACIÓN (ver cabecera): por cruce de
        // fila, no por igualdad exacta de la fila de la cabeza.
        let head = my - 3;
        if rising && self.state == MarioState::Jumping {
            for qb in &mut self.q_blocks {
                if qb.hit {
                    continue;
                }
                let qbx = round_i32(qb.x);
                if (mx - qbx).abs() <= 2 && prev_head > qb.y && head <= qb.y {
                    qb.hit = true;
                    qb.bounce_offset = -1;
                    self.coins = self.coins.saturating_add(1);
                    self.score = self.score.saturating_add(BLOCK_SCORE);
                    if self.pop_coins.len() < MAX_COINS {
                        self.pop_coins.push(PopCoin { x: qb.x + 1.0, y: qb.y as f32 - 1.2, vy: COIN_VY, timer: 0 });
                    }
                }
            }
        }

        // Goombas: pisotón o muerte.
        for g in &mut self.goombas {
            if g.dead {
                continue;
            }
            let gx = round_i32(g.x);
            let gy = round_i32(g.y);
            if (mx - gx).abs() <= 1 && (my - gy).abs() <= 1 {
                if self.mario_vy > 0.0 && my < gy {
                    g.dead = true;
                    g.dead_timer = 0;
                    self.score = self.score.saturating_add(STOMP_SCORE);
                    self.mario_vy = STOMP_VY;
                    self.state = MarioState::Jumping;
                } else {
                    self.state = MarioState::Dying { tick: 0 };
                    return;
                }
            }
        }

        // Tuberías: correr contra una es morir.
        if self.state == MarioState::Running {
            for p in &self.pipes {
                let px = round_i32(p.x);
                if mx + 2 >= px && mx <= px + 3 && my >= self.floor_y - p.h {
                    self.state = MarioState::Dying { tick: 0 };
                    return;
                }
            }
        }
    }

    /// Dibuja un sprite con la regla de color del original: `█`/`▄` con el
    /// primer color y el resto con el segundo. Solo filas en `y_min..y_max`.
    fn draw_sprite(grid: &mut Grid, x0: i32, y0: i32, rows: &[Row], y_min: i32, y_max: i32) {
        for (dy, &(text, f1, f2)) in rows.iter().enumerate() {
            let y = y0.saturating_add(dy as i32);
            if y < y_min || y >= y_max {
                continue;
            }
            for (dx, ch) in text.chars().enumerate() {
                if ch != ' ' {
                    let fg = if ch == '█' || ch == '▄' { f1 } else { f2 };
                    grid.set(x0.saturating_add(dx as i32), y, ch, fg, SKY);
                }
            }
        }
    }

    fn draw_text_row(grid: &mut Grid, x: i32, y: i32, text: &str, fg: u32, bg: u32) {
        grid.set_str(x, y, text, fg, bg);
    }

    /// HUD, escenario, elementos de juego y Mario (todo sobre el cielo).
    fn draw_world(&self, grid: &mut Grid) {
        let (w, floor_y) = (self.w(), self.floor_y);

        // HUD.
        Self::draw_text_row(grid, 3, 1, &format!("MARIO  {:06}", self.score), WHITE, SKY);
        Self::draw_text_row(grid, (w / 2 - 4).max(0), 1, &format!("{COIN_FACE} x{:02}", self.coins), YELLOW, SKY);
        Self::draw_text_row(grid, (w - 13).max(0), 1, WORLD, WHITE, SKY);

        // Arbustos.
        for bush in &self.bushes {
            let bx = round_i32(bush.x);
            for dx in 0..bush.w {
                let ch = if dx == 0 {
                    BUSH_LEFT
                } else if dx == bush.w - 1 {
                    BUSH_RIGHT
                } else {
                    BUSH_MID
                };
                grid.set(bx.saturating_add(dx), bush.y - 1, ch, GREEN, SKY);
            }
        }

        // Nubes.
        for cloud in &self.clouds {
            let rows: &[Row] = if cloud.size == 1 { &CLOUD_SMALL } else { &CLOUD_LARGE };
            Self::draw_sprite(grid, round_i32(cloud.x), cloud.y, rows, i32::MIN, floor_y);
        }

        // Bloques `?`.
        for qb in &self.q_blocks {
            let qbx = round_i32(qb.x);
            let qby = qb.y + qb.bounce_offset;
            if qbx >= -2 && qbx < w {
                if qb.hit {
                    Self::draw_text_row(grid, qbx, qby, QBLOCK_SPENT, GRAY, SKY);
                } else {
                    let block_color = if (self.tick / 8) % 2 == 0 { YELLOW } else { YELLOW_DIM };
                    for (dx, ch) in QBLOCK_ACTIVE.chars().enumerate() {
                        let fg = if ch == QBLOCK_MARK { WHITE } else { block_color };
                        grid.set(qbx + dx as i32, qby, ch, fg, SKY);
                    }
                }
            }
        }

        // Monedas emergentes.
        for pc in &self.pop_coins {
            let (x, y) = (round_i32(pc.x), round_i32(pc.y));
            if y >= 0 && y < floor_y {
                let ch = if (pc.timer / 3) % 2 == 0 { COIN_FACE } else { COIN_EDGE };
                grid.set(x, y, ch, YELLOW, SKY);
            }
        }

        // Tuberías: cuerpo `║║` y labio.
        for pipe in &self.pipes {
            let px = round_i32(pipe.x);
            if px >= -4 && px < w {
                for y in (floor_y - pipe.h + 1)..floor_y {
                    grid.set(px + 1, y, PIPE_BODY, GREEN, SKY);
                    grid.set(px + 2, y, PIPE_BODY, GREEN, SKY);
                }
                let lip_y = floor_y - pipe.h;
                if lip_y >= 0 {
                    for (dx, ch) in PIPE_LIP.chars().enumerate() {
                        grid.set(px + dx as i32, lip_y, ch, GREEN, SKY);
                    }
                }
            }
        }

        // Goombas.
        for g in &self.goombas {
            let (gx, gy) = (round_i32(g.x), round_i32(g.y));
            if gx >= -2 && gx < w {
                if g.dead {
                    grid.set(gx, gy, GOOMBA_FLAT, BROWN, SKY);
                    grid.set(gx + 1, gy, GOOMBA_FLAT, BROWN, SKY);
                } else {
                    let rows = &GOOMBA_FRAMES[g.frame.min(1)];
                    Self::draw_sprite(grid, gx - 1, gy - 1, rows, 0, floor_y);
                }
            }
        }

        // Mario.
        let mx = self.mario_x();
        let my = round_i32(self.mario_y);
        match self.state {
            MarioState::Running => {
                let rows = &MARIO_RUN[self.run_frame.min(2)];
                Self::draw_sprite(grid, mx - 2, my - 2, rows, 0, floor_y);
            }
            MarioState::Jumping => Self::draw_sprite(grid, mx - 2, my - 2, &MARIO_JUMP, 0, floor_y),
            MarioState::Dying { .. } => Self::draw_sprite(grid, mx - 2, my - 2, &MARIO_DEAD, i32::MIN, self.h()),
        }
    }
}

/// `f32` a `i32` redondeando como `f32::round` (los valores están acotados).
fn round_i32(v: f32) -> i32 {
    v.round() as i32
}

impl Animation for MarioAnimation {
    fn update(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.floor_y = height as i32 - 4;
            self.reset_mario();
            self.init_level();
            self.state = MarioState::Running;
            return;
        }
        for _ in 0..self.steps {
            self.step();
        }
    }

    fn draw(&mut self, grid: &mut Grid) {
        grid.clear();
        let (w, h) = (self.w(), self.h());
        let floor_y = self.floor_y;

        // Cielo.
        for y in 0..floor_y.max(0) {
            for x in 0..w {
                grid.set(x, y, ' ', NO_COLOR, SKY);
            }
        }

        if self.playable() {
            self.draw_world(grid);
        }

        // Suelo de ladrillos.
        for y in floor_y.max(0)..(h - 1).max(0) {
            for x in 0..w {
                let edge = (x + y * 3) % 4 == 0;
                let joint = y == floor_y || y == h - 2;
                if edge || joint {
                    grid.set(x, y, BRICK_SHADE, BRICK_SHADOW, GROUND);
                } else {
                    grid.set(x, y, BRICK_SOLID, GROUND, GROUND);
                }
            }
        }

        // Barra al pie.
        if h > 0 {
            for x in 0..w {
                grid.set(x, h - 1, ' ', NO_COLOR, BLACK);
            }
            let n = FOOTER.chars().count() as i32;
            Self::draw_text_row(grid, ((w - n) / 2).max(0), h - 1, FOOTER, WHITE, BLACK);
        }
    }

    fn set_fps(&mut self, fps: u32) {
        self.steps = steps_for(fps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(w: u16, h: u16, seed: u32, fps: u32) -> MarioAnimation {
        let mut a = MarioAnimation::new(w, h, seed);
        a.set_fps(fps);
        a
    }

    /// Deja un solo elemento lejos (fuera de pantalla) en cada `Vec`: no se
    /// generan más y no interfieren. Los elementos de prueba se insertan al
    /// principio para que el "último" siga siendo el lejano.
    fn quiet(a: &mut MarioAnimation) {
        let far = a.w() as f32 + 100.0;
        let fy = a.floor_y;
        a.clouds = vec![Cloud { x: far, y: 5, size: 1 }];
        a.bushes = vec![Bush { x: far, y: fy, w: 4 }];
        a.pipes = vec![Pipe { x: far, h: 2 }];
        a.goombas = vec![Goomba { x: far, y: fy as f32 - 1.0, frame: 0, dead: false, dead_timer: 0 }];
        a.q_blocks = vec![QBlock { x: far, y: fy - 6, hit: false, bounce_offset: 0 }];
        a.pop_coins.clear();
    }

    /// Partida 80x24 a 60 FPS sin elementos cercanos (floor_y 20, Mario en x = 20).
    fn game() -> MarioAnimation {
        let mut a = anim(80, 24, 1, 60);
        quiet(&mut a);
        a
    }

    fn goomba(x: f32) -> Goomba {
        Goomba { x, y: 19.0, frame: 0, dead: false, dead_timer: 0 }
    }

    fn qblock(x: f32) -> QBlock {
        QBlock { x, y: 14, hit: false, bounce_offset: 0 }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn drawn(a: &mut MarioAnimation) -> Grid {
        let mut g = Grid::new(a.width, a.height);
        a.draw(&mut g);
        g
    }

    fn cell(g: &Grid, x: i32, y: i32) -> (char, u32, u32) {
        g.get(x as u16, y as u16).unwrap()
    }

    fn row_text(g: &Grid, x: i32, y: i32, n: usize) -> String {
        (0..n as i32).map(|i| cell(g, x + i, y).0).collect()
    }

    fn lens(a: &MarioAnimation) -> [usize; 6] {
        [a.clouds.len(), a.bushes.len(), a.pipes.len(), a.goombas.len(), a.q_blocks.len(), a.pop_coins.len()]
    }

    /// Estado observable aplanado a bits (comparable entre ejecuciones).
    fn snap(a: &MarioAnimation) -> Vec<u32> {
        let mut v = vec![a.tick as u32, a.mario_y.to_bits(), a.mario_vy.to_bits(), a.score, a.coins, a.run_frame as u32];
        v.push(match a.state {
            MarioState::Running => 0,
            MarioState::Jumping => 1,
            MarioState::Dying { tick } => 2 + tick,
        });
        for c in &a.clouds {
            v.extend([c.x.to_bits(), c.y as u32, c.size as u32]);
        }
        for b in &a.bushes {
            v.extend([b.x.to_bits(), b.y as u32, b.w as u32]);
        }
        for p in &a.pipes {
            v.extend([p.x.to_bits(), p.h as u32]);
        }
        for g in &a.goombas {
            v.extend([g.x.to_bits(), g.y.to_bits(), g.frame as u32, g.dead as u32, g.dead_timer]);
        }
        for q in &a.q_blocks {
            v.extend([q.x.to_bits(), q.y as u32, q.hit as u32, q.bounce_offset as u32]);
        }
        for c in &a.pop_coins {
            v.extend([c.x.to_bits(), c.y.to_bits(), c.vy.to_bits(), c.timer]);
        }
        v
    }

    // ---------- constantes, símbolos y ritmo ----------

    #[test]
    fn colors_and_symbols_are_the_original_ones() {
        assert_eq!(SKY, rgb(92, 148, 252));
        assert_eq!(GROUND, rgb(200, 76, 12));
        assert_eq!((BRICK_SHADOW, BLACK), (rgb(0, 0, 0), rgb(0, 0, 0)));
        assert_eq!((GREEN, PEACH, RED, BLUE), (rgb(0, 168, 0), rgb(252, 188, 176), rgb(228, 0, 0), rgb(0, 0, 168)));
        assert_eq!((YELLOW, WHITE, GRAY, BROWN), (rgb(252, 228, 24), rgb(255, 255, 255), rgb(120, 120, 120), rgb(140, 60, 0)));
        assert_eq!((BUSH_LEFT, BUSH_MID, BUSH_RIGHT), ('◢', '█', '◣'));
        assert_eq!((COIN_FACE, COIN_EDGE, PIPE_BODY, GOOMBA_FLAT), ('●', '│', '║', '▄'));
        assert_eq!((BRICK_SHADE, BRICK_SOLID, QBLOCK_MARK), ('░', '█', '?'));
        assert_eq!((PIPE_LIP, QBLOCK_ACTIVE, QBLOCK_SPENT), ("▄██▄", "[?]", "[ ]"));
        assert_eq!((FOOTER, WORLD), ("AUTOPILOT ACTIVE | JUMPING & STOMPING", "WORLD  1-1"));
    }

    #[test]
    fn steps_per_update_follow_fps() {
        assert_eq!(steps_for(60), 1);
        assert_eq!(steps_for(30), 2);
        assert_eq!(steps_for(20), 3);
        assert_eq!(steps_for(0), 60);
        assert_eq!(steps_for(u32::MAX), 1);
        assert_eq!(MarioAnimation::new(1, 1, 1).steps, 2);
    }

    // ---------- nivel inicial ----------

    #[test]
    fn initial_level_has_the_expected_positions() {
        for seed in 1..=20u32 {
            let a = anim(80, 24, seed, 30);
            assert_eq!(a.floor_y, 20);
            assert_eq!((a.mario_y, a.mario_vy, a.state, a.score, a.coins), (20.0, 0.0, MarioState::Running, 0, 0));
            assert_eq!(a.mario_x(), 20);
            assert_eq!(a.pipes.len(), 1);
            assert!((90.0..=110.0).contains(&a.pipes[0].x), "{}", a.pipes[0].x);
            assert!((2..=3).contains(&a.pipes[0].h));
            assert_eq!(a.q_blocks.len(), 1);
            assert_eq!((a.q_blocks[0].y, a.q_blocks[0].hit, a.q_blocks[0].bounce_offset), (14, false, 0));
            assert!((95.0..=125.0).contains(&a.q_blocks[0].x), "{}", a.q_blocks[0].x);
            assert_eq!(a.goombas.len(), 1);
            assert_eq!((a.goombas[0].y, a.goombas[0].dead, a.goombas[0].frame), (19.0, false, 0));
            assert!((105.0..=160.0).contains(&a.goombas[0].x), "{}", a.goombas[0].x);
            assert!(a.pop_coins.is_empty());
            // Nubes: desde x = 10, cada 30..50, y 4..=8, tamaño 1..=2.
            assert_eq!(a.clouds[0].x, 10.0);
            for c in &a.clouds {
                assert!((4..=8).contains(&c.y) && (1..=2).contains(&c.size));
                assert!(c.x < 80.0);
            }
            for p in a.clouds.windows(2) {
                let d = p[1].x - p[0].x;
                assert!((30.0..50.0).contains(&d), "{d}");
            }
            assert!(a.clouds.last().unwrap().x + 50.0 >= 80.0);
            // Arbustos: desde x = 5, cada 20..40, ancho 3..=6, sobre el suelo.
            assert_eq!(a.bushes[0].x, 5.0);
            for b in &a.bushes {
                assert!((3..=6).contains(&b.w) && b.y == 20 && b.x < 80.0);
            }
            for p in a.bushes.windows(2) {
                let d = p[1].x - p[0].x;
                assert!((20.0..40.0).contains(&d), "{d}");
            }
            assert!(a.bushes.last().unwrap().x + 40.0 >= 80.0);
        }
        assert_eq!(anim(100, 30, 1, 30).floor_y, 26);
        assert!((110.0..=130.0).contains(&anim(100, 30, 1, 30).pipes[0].x));
        assert_eq!(anim(100, 30, 1, 30).q_blocks[0].y, 20);
    }

    // ---------- scroll, aparición y retiro ----------

    #[test]
    fn one_tick_scrolls_each_layer_at_its_speed() {
        let mut a = anim(80, 24, 5, 60);
        let (c, b, p, g, q) = (a.clouds[0].x, a.bushes[0].x, a.pipes[0].x, a.goombas[0].x, a.q_blocks[0].x);
        a.update(80, 24);
        assert_eq!(a.tick, 1);
        assert!(close(a.clouds[0].x, c - 0.05));
        assert!(close(a.bushes[0].x, b - 0.18));
        assert!(close(a.pipes[0].x, p - 0.40));
        assert!(close(a.goombas[0].x, g - 0.48));
        assert!(close(a.q_blocks[0].x, q - 0.40));
    }

    #[test]
    fn new_elements_spawn_under_the_original_conditions_and_ranges() {
        // (umbral, borde sin aparición, borde con aparición): `last.x` previo.
        let mut a = game();
        a.pipes[0].x = 80.0 - 22.0 + 0.5;
        a.bushes[0].x = 80.0 - 20.0 + 0.3;
        a.clouds[0].x = 80.0 - 15.0 + 0.1;
        a.q_blocks[0].x = 80.0 - 18.0 + 0.5;
        a.goombas[0].x = 80.0 - 15.0 + 0.6;
        a.step();
        assert_eq!(lens(&a), [1, 1, 1, 1, 1, 0], "todavía no");
        let mut a = game();
        a.pipes[0].x = 80.0 - 22.0 + 0.3;
        a.bushes[0].x = 80.0 - 20.0 + 0.1;
        a.clouds[0].x = 80.0 - 15.0 + 0.03;
        a.q_blocks[0].x = 80.0 - 18.0 + 0.3;
        a.goombas[0].x = 80.0 - 15.0 + 0.3;
        a.step();
        assert_eq!(lens(&a), [2, 2, 2, 2, 2, 0]);
        let (c, b, p, q, g) = (&a.clouds[1], &a.bushes[1], &a.pipes[1], &a.q_blocks[1], &a.goombas[1]);
        assert_eq!(c.x, 90.0);
        assert!((4..=8).contains(&c.y) && (1..=2).contains(&c.size));
        assert!((85.0..105.0).contains(&b.x) && (3..=6).contains(&b.w) && b.y == 20);
        assert!((88.0..108.0).contains(&p.x) && (2..=3).contains(&p.h));
        assert!((92.0..112.0).contains(&q.x) && q.y == 14 && !q.hit && q.bounce_offset == 0);
        assert!((95.0..115.0).contains(&g.x) && g.y == 19.0 && !g.dead && g.frame == 0 && g.dead_timer == 0);
    }

    #[test]
    fn empty_lists_spawn_immediately() {
        let mut a = game();
        a.clouds.clear();
        a.bushes.clear();
        a.pipes.clear();
        a.goombas.clear();
        a.q_blocks.clear();
        a.step();
        assert_eq!(lens(&a), [1, 1, 1, 1, 1, 0]);
    }

    #[test]
    fn elements_are_removed_past_the_original_thresholds() {
        let mut a = game();
        a.clouds.insert(0, Cloud { x: -14.9, y: 5, size: 1 });
        a.clouds.insert(0, Cloud { x: -14.97, y: 5, size: 1 });
        a.bushes.insert(0, Bush { x: -9.7, y: 20, w: 3 });
        a.bushes.insert(0, Bush { x: -9.9, y: 20, w: 3 });
        a.pipes.insert(0, Pipe { x: -5.5, h: 2 });
        a.pipes.insert(0, Pipe { x: -5.7, h: 2 });
        a.q_blocks.insert(0, qblock(-4.5));
        a.q_blocks.insert(0, qblock(-4.7));
        a.goombas.insert(0, goomba(-4.4));
        a.goombas.insert(0, goomba(-4.6));
        let mut dead_old = goomba(30.0);
        dead_old.dead = true;
        dead_old.dead_timer = 24;
        a.goombas.insert(0, dead_old);
        let mut dead_new = goomba(30.0);
        dead_new.dead = true;
        dead_new.dead_timer = 23;
        a.goombas.insert(0, dead_new);
        a.pop_coins.push(PopCoin { x: 40.0, y: 10.0, vy: 0.0, timer: 17 });
        a.pop_coins.push(PopCoin { x: 40.0, y: 10.0, vy: 0.0, timer: 16 });
        a.step();
        assert_eq!(a.clouds.len(), 2);
        assert_eq!(a.bushes.len(), 2);
        assert_eq!(a.pipes.len(), 2);
        assert_eq!(a.q_blocks.len(), 2);
        // Quedan: el de -4.4 (sigue), el muerto con timer 23 -> 24, el lejano.
        assert_eq!(a.goombas.len(), 3);
        assert_eq!(a.pop_coins.len(), 1);
        assert_eq!(a.pop_coins[0].timer, 17);
    }

    #[test]
    fn pop_coin_moves_with_gravity() {
        let mut a = game();
        a.pop_coins.push(PopCoin { x: 30.0, y: 10.0, vy: COIN_VY, timer: 0 });
        a.step();
        let c = &a.pop_coins[0];
        assert!(close(c.x, 29.6) && close(c.y, 9.35) && close(c.vy, -0.53));
        assert_eq!(c.timer, 1);
    }

    #[test]
    fn goomba_frame_toggles_every_eight_ticks_and_dead_ones_count_up() {
        let mut a = game();
        a.goombas.insert(0, goomba(60.0));
        let mut d = goomba(60.0);
        d.dead = true;
        a.goombas.insert(1, d);
        for _ in 0..7 {
            a.step();
        }
        assert_eq!(a.goombas[0].frame, 0);
        a.step();
        assert_eq!(a.goombas[0].frame, 1);
        assert_eq!(a.goombas[1].dead_timer, 8);
        assert_eq!(a.goombas[1].frame, 0);
    }

    // ---------- piloto automático ----------

    fn jumped(a: &MarioAnimation) -> bool {
        a.state == MarioState::Jumping && a.mario_vy == JUMP_VY
    }

    #[test]
    fn autopilot_jumps_before_a_pipe_but_not_too_early() {
        let mut a = game();
        a.pipes.insert(0, Pipe { x: 20.0 + 6.3 + 0.4, h: 2 });
        a.step();
        assert_eq!(a.state, MarioState::Running, "dist 6.3 todavía no");
        let mut a = game();
        a.pipes.insert(0, Pipe { x: 20.0 + 5.8 + 0.4, h: 2 });
        a.step();
        assert!(jumped(&a), "dist 5.8");
    }

    #[test]
    fn autopilot_jumps_before_a_live_goomba_only() {
        let mut a = game();
        a.goombas.insert(0, goomba(20.0 + 5.0 + 0.48));
        a.step();
        assert!(jumped(&a));
        let mut a = game();
        let mut g = goomba(20.0 + 3.0 + 0.48);
        g.dead = true;
        a.goombas.insert(0, g);
        a.step();
        assert_eq!(a.state, MarioState::Running);
    }

    #[test]
    fn autopilot_jumps_under_an_unhit_block_only() {
        let mut a = game();
        a.q_blocks.insert(0, qblock(20.0 + 3.3 + 0.4));
        a.step();
        assert!(jumped(&a));
        let mut a = game();
        a.q_blocks.insert(0, qblock(20.0 + 3.7 + 0.4));
        a.step();
        assert_eq!(a.state, MarioState::Running);
        let mut a = game();
        let mut q = qblock(20.0 + 1.0 + 0.4);
        q.hit = true;
        a.q_blocks.insert(0, q);
        a.step();
        assert_eq!(a.state, MarioState::Running);
    }

    #[test]
    fn autopilot_does_not_jump_without_obstacles() {
        let mut a = game();
        for _ in 0..300 {
            a.step();
            assert_eq!(a.state, MarioState::Running);
            assert_eq!(a.mario_y, 20.0);
        }
    }

    #[test]
    fn autopilot_clears_a_pipe_without_dying() {
        let mut a = game();
        a.pipes.insert(0, Pipe { x: 30.0, h: 3 });
        let mut jumps = 0;
        let mut was_air = false;
        for _ in 0..140 {
            a.step();
            assert!(!matches!(a.state, MarioState::Dying { .. }));
            let air = a.state == MarioState::Jumping;
            if air && !was_air {
                jumps += 1;
            }
            was_air = air;
        }
        assert_eq!(jumps, 1);
        assert_eq!(a.score, 0);
    }

    // ---------- físicas ----------

    #[test]
    fn jump_follows_the_original_physics_and_lands_on_the_floor() {
        let mut a = game();
        a.state = MarioState::Jumping;
        a.mario_vy = JUMP_VY;
        a.step();
        assert!(close(a.mario_y, 18.55) && close(a.mario_vy, -1.365));
        let mut steps = 1;
        while a.state == MarioState::Jumping {
            a.step();
            steps += 1;
            assert!(steps < 100);
        }
        assert_eq!((a.mario_y, a.mario_vy), (20.0, 0.0));
        assert_eq!(steps, 36);
    }

    #[test]
    fn run_frame_advances_every_five_ticks_over_three_frames() {
        let mut a = game();
        for _ in 0..4 {
            a.step();
        }
        assert_eq!(a.run_frame, 0);
        a.step();
        assert_eq!(a.run_frame, 1);
        for _ in 0..10 {
            a.step();
        }
        assert_eq!(a.run_frame, 0);
    }

    // ---------- colisiones ----------

    #[test]
    fn stomping_a_goomba_scores_100_and_bounces() {
        let mut a = game();
        a.state = MarioState::Jumping;
        a.mario_y = 17.5;
        a.mario_vy = 0.5;
        a.goombas.insert(0, goomba(20.48));
        a.step();
        assert!(a.goombas[0].dead);
        assert_eq!(a.goombas[0].dead_timer, 0);
        assert_eq!(a.score, 100);
        assert_eq!((a.mario_vy, a.state), (-0.75, MarioState::Jumping));
        a.step();
        assert_eq!(a.score, 100, "un goomba muerto no puntúa dos veces");
        assert!(!matches!(a.state, MarioState::Dying { .. }));
    }

    #[test]
    fn running_into_a_goomba_kills_mario() {
        let mut a = game();
        a.goombas.insert(0, goomba(20.5));
        a.step();
        assert_eq!(a.state, MarioState::Dying { tick: 0 });
        assert_eq!(a.score, 0);
    }

    #[test]
    fn landing_beside_a_pipe_kills_mario_but_jumping_over_it_does_not() {
        let mut a = game();
        a.pipes.insert(0, Pipe { x: 19.4, h: 3 });
        a.step();
        assert_eq!(a.state, MarioState::Dying { tick: 0 });
        let mut b = game();
        b.pipes.insert(0, Pipe { x: 19.4, h: 3 });
        b.state = MarioState::Jumping;
        b.mario_y = 10.0;
        b.step();
        assert_eq!(b.state, MarioState::Jumping);
    }

    fn block_hit_case(state_y: f32, vy: f32, block_x: f32, hit: bool) -> MarioAnimation {
        let mut a = game();
        let mut q = qblock(block_x);
        q.hit = hit;
        a.q_blocks.insert(0, q);
        a.state = MarioState::Jumping;
        a.mario_y = state_y;
        a.mario_vy = vy;
        a.step();
        a
    }

    #[test]
    fn block_is_hit_when_the_head_crosses_its_row() {
        // Cabeza 15 -> 13 con bloque en la fila 14: el original no lo golpeaba.
        let a = block_hit_case(18.0, -2.0, 20.4, false);
        assert!(a.q_blocks[0].hit);
        assert_eq!(a.q_blocks[0].bounce_offset, -1);
        assert_eq!((a.coins, a.score), (1, 200));
        assert_eq!(a.pop_coins.len(), 1);
        let c = &a.pop_coins[0];
        assert!(close(c.x, 21.0) && close(c.y, 12.8) && c.vy == COIN_VY && c.timer == 0);
        // Caso de igualdad exacta (cabeza 15 -> 14).
        let b = block_hit_case(18.0, -1.0, 20.4, false);
        assert_eq!((b.coins, b.score), (1, 200));
    }

    #[test]
    fn block_is_not_hit_without_a_crossing_or_overlap() {
        // Cayendo.
        assert_eq!(block_hit_case(14.0, 0.5, 20.4, false).coins, 0);
        // Cabeza ya por encima del bloque.
        assert_eq!(block_hit_case(15.0, -1.0, 20.4, false).coins, 0);
        // Demasiado lejos en horizontal (|20 - 23| = 3).
        assert_eq!(block_hit_case(18.0, -2.0, 23.4, false).coins, 0);
        // Se queda por debajo (cabeza 15 -> 15).
        assert_eq!(block_hit_case(18.0, -0.2, 20.4, false).coins, 0);
        // Ya golpeado.
        let a = block_hit_case(18.0, -2.0, 20.4, true);
        assert_eq!((a.coins, a.score, a.pop_coins.len()), (0, 0, 0));
    }

    #[test]
    fn block_is_hit_at_the_real_jump_speed() {
        let mut a = game();
        a.q_blocks.insert(0, qblock(24.7));
        let mut steps = 0;
        while a.coins == 0 {
            a.step();
            steps += 1;
            assert!(steps < 60, "el piloto no golpeó el bloque");
            assert!(!matches!(a.state, MarioState::Dying { .. }));
        }
        assert_eq!(a.score, 200);
        assert!(a.q_blocks[0].hit);
        assert_eq!(a.state, MarioState::Jumping);
    }

    #[test]
    fn bounce_offset_returns_to_zero_after_a_tick() {
        let mut a = block_hit_case(18.0, -2.0, 20.4, false);
        assert_eq!(a.q_blocks[0].bounce_offset, -1);
        a.step();
        assert_eq!(a.q_blocks[0].bounce_offset, 0);
        a.step();
        assert_eq!(a.q_blocks[0].bounce_offset, 0);
    }

    // ---------- muerte y reinicio ----------

    #[test]
    fn death_leap_lasts_45_ticks_then_everything_resets() {
        let mut a = game();
        a.score = 500;
        a.coins = 3;
        a.state = MarioState::Dying { tick: 0 };
        let pipe_x = a.pipes[0].x;
        for _ in 0..14 {
            a.step();
        }
        assert_eq!(a.state, MarioState::Dying { tick: 14 });
        assert!(close(a.mario_y, 20.0 - 14.0 * 0.8));
        a.step();
        assert!(close(a.mario_y, 20.0 - 14.0 * 0.8 + 0.8), "tick 15 ya baja");
        for _ in 0..19 {
            a.step();
        }
        assert_eq!(a.state, MarioState::Dying { tick: 34 });
        assert!(close(a.mario_y, 20.0 - 11.2 + 16.0));
        for _ in 0..10 {
            a.step();
        }
        assert_eq!(a.state, MarioState::Dying { tick: 44 });
        assert!(close(a.mario_y, 24.8), "quieto entre 35 y 44");
        assert_eq!((a.score, a.coins), (500, 3));
        assert_eq!(a.pipes[0].x, pipe_x, "el nivel no se mueve al morir");
        a.step();
        assert_eq!(a.state, MarioState::Running);
        assert_eq!((a.score, a.coins, a.mario_y, a.mario_vy), (0, 0, 20.0, 0.0));
        assert_eq!(lens(&a), [a.clouds.len(), a.bushes.len(), 1, 1, 1, 0]);
        assert!((90.0..=110.0).contains(&a.pipes[0].x) && (95.0..=125.0).contains(&a.q_blocks[0].x) && (105.0..=160.0).contains(&a.goombas[0].x));
        assert!(!a.q_blocks[0].hit);
    }

    // ---------- resize ----------

    #[test]
    fn resize_regenerates_the_level_and_leaves_the_update() {
        let mut a = anim(80, 24, 3, 60);
        a.score = 77;
        a.mario_vy = 0.3;
        a.state = MarioState::Dying { tick: 7 };
        a.update(100, 30);
        assert_eq!((a.width, a.height, a.floor_y), (100, 30, 26));
        assert_eq!(a.state, MarioState::Running);
        assert_eq!((a.mario_y, a.mario_vy), (26.0, 0.0));
        assert_eq!(a.tick, 0, "el resize sale del update sin avanzar");
        assert_eq!(a.q_blocks[0].y, 20);
        assert!((110.0..=130.0).contains(&a.pipes[0].x) && (115.0..=145.0).contains(&a.q_blocks[0].x) && (125.0..=180.0).contains(&a.goombas[0].x));
        assert_eq!(a.score, 77);
        a.update(100, 30);
        assert_eq!(a.tick, 1);
    }

    #[test]
    fn resize_is_detected_once_per_update_not_per_step() {
        let mut a = anim(80, 24, 3, 20);
        a.update(90, 24);
        assert_eq!(a.tick, 0);
        a.update(90, 24);
        assert_eq!(a.tick, 3);
    }

    // ---------- dibujo ----------

    #[test]
    fn hud_has_the_exact_text_positions_and_colors() {
        let mut a = anim(80, 24, 1, 30);
        let g = drawn(&mut a);
        assert_eq!(row_text(&g, 3, 1, 13), "MARIO  000000");
        assert_eq!(cell(&g, 3, 1), ('M', WHITE, SKY));
        assert_eq!(row_text(&g, 36, 1, 5), "● x00");
        assert_eq!(cell(&g, 36, 1), ('●', YELLOW, SKY));
        assert_eq!(row_text(&g, 67, 1, 10), "WORLD  1-1");
        assert_eq!(cell(&g, 67, 1), ('W', WHITE, SKY));
        a.score = 1234;
        a.coins = 12;
        let g = drawn(&mut a);
        assert_eq!(row_text(&g, 3, 1, 13), "MARIO  001234");
        assert_eq!(row_text(&g, 36, 1, 5), "● x12");
        for y in 0..24 {
            for x in 0..80 {
                let ch = cell(&g, x, y).0;
                assert!((ch as u32) < 0x1_0000, "emoji en ({x},{y}): {ch}");
                assert_ne!(ch, '🪙');
                assert_ne!(ch, '🍄');
            }
        }
    }

    #[test]
    fn footer_is_centered_by_chars_on_a_black_row() {
        let mut a = anim(80, 24, 1, 30);
        let g = drawn(&mut a);
        let n = FOOTER.chars().count();
        assert_eq!(n, 37);
        assert_eq!(row_text(&g, 21, 23, n), FOOTER);
        assert_eq!(cell(&g, 21, 23), ('A', WHITE, BLACK));
        for x in 0..80 {
            assert_eq!(cell(&g, x, 23).2, BLACK, "fondo negro en x = {x}");
        }
        assert_eq!(cell(&g, 20, 23), (' ', NO_COLOR, BLACK));
        assert_eq!(cell(&g, 58, 23), (' ', NO_COLOR, BLACK));
        let text: String = (0..80).map(|x| cell(&g, x, 23).0).collect();
        assert!(!text.contains('q') && !text.contains("Quit"));
    }

    #[test]
    fn sky_and_ground_have_the_original_colors_and_brick_rule() {
        let mut a = game();
        let g = drawn(&mut a);
        for x in 0..80 {
            assert_eq!(cell(&g, x, 10), (' ', NO_COLOR, SKY));
        }
        assert_eq!(cell(&g, 0, 0), (' ', NO_COLOR, SKY));
        assert_eq!(cell(&g, 79, 19).2, SKY);
        for y in 20..23 {
            for x in 0..80 {
                let joint = y == 20 || y == 22;
                let edge = (x + y * 3) % 4 == 0;
                let want = if joint || edge { ('░', BRICK_SHADOW, GROUND) } else { ('█', GROUND, GROUND) };
                assert_eq!(cell(&g, x, y), want, "({x},{y})");
            }
        }
    }

    fn expect(g: &Grid, cells: &[(i32, i32, char, u32)]) {
        for &(x, y, ch, fg) in cells {
            let c = cell(g, x, y);
            assert_eq!((c.0, c.1), (ch, fg), "({x},{y})");
            assert_eq!(c.2, SKY, "fondo en ({x},{y})");
        }
    }

    fn mario_at(state: MarioState, frame: usize) -> Grid {
        let mut a = game();
        a.state = state;
        a.mario_y = 15.0;
        a.run_frame = frame;
        drawn(&mut a)
    }

    #[test]
    fn running_mario_has_three_distinct_frames() {
        let g = mario_at(MarioState::Running, 0);
        expect(&g, &[(19, 13, '▄', RED), (20, 13, '▀', PEACH), (21, 13, '▄', RED), (19, 14, '▄', RED), (20, 14, '█', RED), (21, 14, '▄', RED), (19, 15, '█', BLUE), (21, 15, '█', BLUE)]);
        assert_eq!(cell(&g, 20, 15).0, ' ');
        let g = mario_at(MarioState::Running, 1);
        expect(&g, &[(18, 14, '▄', RED), (19, 14, '█', RED), (20, 14, '█', RED), (21, 14, '▄', RED), (19, 15, '▀', BROWN), (21, 15, '█', BLUE)]);
        assert_eq!(cell(&g, 22, 14).0, ' ');
        let g = mario_at(MarioState::Running, 2);
        expect(&g, &[(19, 14, '▄', RED), (20, 14, '█', RED), (21, 14, '█', RED), (22, 14, '▄', RED), (19, 15, '█', BLUE), (21, 15, '▀', BROWN)]);
        assert_eq!(cell(&g, 18, 14).0, ' ');
    }

    #[test]
    fn jumping_and_dying_sprites() {
        let g = mario_at(MarioState::Jumping, 0);
        expect(&g, &[(19, 13, '▄', RED), (20, 13, '▀', PEACH), (21, 13, '▄', RED), (22, 13, '▄', RED), (19, 14, '░', WHITE), (20, 14, '█', BLUE), (21, 14, '▀', WHITE), (19, 15, '▄', BLUE), (20, 15, '▀', BROWN)]);
        let g = mario_at(MarioState::Dying { tick: 0 }, 0);
        expect(&g, &[(20, 13, '▀', PEACH), (20, 14, '█', RED), (19, 15, '▀', BLACK), (21, 15, '▀', BLACK)]);
    }

    #[test]
    fn running_mario_feet_row_hides_under_the_ground() {
        let mut a = game();
        let g = drawn(&mut a);
        assert_eq!(cell(&g, 20, 19).0, '█', "torso sobre el suelo");
        assert_eq!(cell(&g, 20, 20).2, GROUND, "la fila de las piernas queda bajo el suelo");
    }

    #[test]
    fn pipe_goomba_block_coin_bush_and_cloud_glyphs() {
        let mut a = game();
        a.pipes.insert(0, Pipe { x: 40.0, h: 3 });
        a.goombas.insert(0, goomba(50.0));
        let mut flat = goomba(60.0);
        flat.dead = true;
        a.goombas.insert(0, flat);
        let mut g2 = goomba(70.0);
        g2.frame = 1;
        a.goombas.insert(0, g2);
        a.q_blocks.insert(0, qblock(5.0));
        let mut spent = qblock(10.0);
        spent.hit = true;
        a.q_blocks.insert(0, spent);
        a.pop_coins.push(PopCoin { x: 30.0, y: 10.0, vy: 0.0, timer: 0 });
        a.pop_coins.push(PopCoin { x: 32.0, y: 10.0, vy: 0.0, timer: 3 });
        a.bushes.insert(0, Bush { x: 5.0, y: 20, w: 4 });
        a.clouds.insert(0, Cloud { x: 10.0, y: 5, size: 1 });
        a.clouds.insert(0, Cloud { x: 40.0, y: 5, size: 2 });
        a.tick = 0;
        let g = drawn(&mut a);
        // Tubería h=3: cuerpo en las filas 18 y 19, labio en la 17.
        expect(&g, &[(41, 18, '║', GREEN), (42, 19, '║', GREEN), (40, 17, '▄', GREEN), (41, 17, '█', GREEN), (42, 17, '█', GREEN), (43, 17, '▄', GREEN)]);
        assert_eq!(cell(&g, 40, 18).0, ' ');
        // Goomba vivo frame 0, frame 1 y aplastado.
        expect(&g, &[(50, 18, '▄', BROWN), (51, 18, '▄', BROWN), (49, 19, '▀', PEACH), (52, 19, '▀', PEACH), (70, 18, '▄', BROWN), (70, 19, '▀', PEACH), (71, 19, '▀', PEACH), (60, 19, '▄', BROWN), (61, 19, '▄', BROWN)]);
        assert_eq!(cell(&g, 50, 19).0, ' ');
        assert_eq!(cell(&g, 69, 19).0, ' ');
        assert_eq!(cell(&g, 60, 18).0, ' ');
        // Bloques.
        expect(&g, &[(5, 14, '[', YELLOW), (6, 14, '?', WHITE), (7, 14, ']', YELLOW), (10, 14, '[', GRAY), (11, 14, ' ', GRAY), (12, 14, ']', GRAY)]);
        // Monedas.
        expect(&g, &[(30, 10, '●', YELLOW), (32, 10, '│', YELLOW)]);
        // Arbusto.
        expect(&g, &[(5, 19, '◢', GREEN), (6, 19, '█', GREEN), (7, 19, '█', GREEN), (8, 19, '◣', GREEN)]);
        // Nubes: pequeña en (10, 5) y grande en (40, 5).
        expect(&g, &[(11, 5, '▄', WHITE), (12, 5, '█', WHITE), (13, 5, '▄', WHITE), (10, 6, '▀', WHITE), (12, 6, '█', WHITE), (14, 6, '▀', WHITE)]);
        expect(&g, &[(41, 5, '▄', WHITE), (43, 5, '█', WHITE), (45, 5, '▄', WHITE), (40, 6, '▀', WHITE), (44, 6, '█', WHITE), (46, 6, '▀', WHITE)]);
        a.tick = 8;
        let g = drawn(&mut a);
        expect(&g, &[(5, 14, '[', YELLOW_DIM), (6, 14, '?', WHITE), (7, 14, ']', YELLOW_DIM)]);
        a.pop_coins[0].timer = 6;
        assert_eq!(cell(&drawn(&mut a), 30, 10).0, '●');
    }

    #[test]
    fn bounced_block_is_drawn_one_row_up() {
        let mut a = game();
        let mut q = qblock(5.0);
        q.bounce_offset = -1;
        a.q_blocks.insert(0, q);
        let g = drawn(&mut a);
        assert_eq!(cell(&g, 6, 13).0, '?');
        assert_eq!(cell(&g, 6, 14).0, ' ');
    }

    // ---------- tamaño mínimo ----------

    #[test]
    fn below_the_minimum_size_there_is_no_game() {
        for &(w, h) in &[(0u16, 0u16), (1, 1), (19, 11), (19, 30), (30, 11), (1, 1024), (1024, 1), (5, 40)] {
            let mut a = anim(w, h, 1, 30);
            assert_eq!(lens(&a), [0; 6], "{w}x{h}");
            for _ in 0..50 {
                a.update(w, h);
            }
            assert_eq!(lens(&a), [0; 6]);
            assert_eq!((a.mario_vy, a.state), (0.0, MarioState::Running));
            let g = drawn(&mut a);
            let fy = h as i32 - 4;
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    let c = cell(&g, x, y);
                    if y == h as i32 - 1 {
                        assert_eq!(c.2, BLACK);
                    } else if y >= fy {
                        assert_eq!(c.2, GROUND, "suelo ({x},{y})");
                    } else {
                        assert_eq!(c, (' ', NO_COLOR, SKY), "cielo ({x},{y})");
                    }
                }
            }
        }
        assert!(anim(20, 12, 1, 30).playable());
        assert_eq!(lens(&anim(20, 12, 1, 30))[2], 1);
    }

    #[test]
    fn the_footer_is_clipped_not_overflowed_on_narrow_grids() {
        let mut a = anim(19, 12, 1, 30);
        let g = drawn(&mut a);
        assert_eq!(row_text(&g, 0, 11, 19), "AUTOPILOT ACTIVE | ");
        let mut a = anim(5, 3, 1, 30);
        let g = drawn(&mut a);
        assert_eq!(row_text(&g, 0, 2, 5), "AUTOP");
    }

    // ---------- tiempo real, determinismo y robustez ----------

    #[test]
    fn same_real_time_gives_same_state_at_20_30_and_60_fps() {
        let run_secs = |fps: u32, w: u16, h: u16| {
            let mut a = anim(w, h, 7, fps);
            for _ in 0..fps * 40 {
                a.update(w, h);
            }
            snap(&a)
        };
        for &(w, h) in &[(80u16, 24u16), (211, 58)] {
            let base = run_secs(60, w, h);
            assert_eq!(base[0], 2400);
            assert_eq!(run_secs(30, w, h), base);
            assert_eq!(run_secs(20, w, h), base);
        }
    }

    /// Con el primer trío fijo del original, Mario moría siempre en el primer
    /// goomba (el salto sobre la tubería dura 36 ticks y el goomba llegaba justo
    /// al aterrizar) y el reinicio repetía la misma escena: ~75 muertes en 300 s.
    /// Con el trío sorteado en cada reinicio mueren ~15-20 veces, hay golpes de
    /// bloque y pisotones, y las escenas de reinicio cambian.
    #[test]
    fn the_autopilot_plays_and_each_restart_is_a_different_scene() {
        for seed in 1..=8u32 {
            let mut a = anim(211, 58, seed, 30);
            let (mut deaths, mut hits, mut stomps, mut prev_coins, mut prev_score) = (0u32, 0u32, 0u32, 0u32, 0u32);
            let mut was_dying = false;
            let mut restarts: Vec<(i32, i32)> = Vec::new();
            for _ in 0..30 * 300 {
                let dying = matches!(a.state, MarioState::Dying { .. });
                if dying && !was_dying {
                    deaths += 1;
                }
                if !dying && was_dying {
                    (prev_coins, prev_score) = (0, 0); // el reinicio pone puntaje y monedas a 0
                    restarts.push((a.pipes[0].x as i32, a.goombas[0].x as i32));
                }
                was_dying = dying;
                a.update(211, 58);
                if a.coins > prev_coins {
                    hits += a.coins - prev_coins;
                }
                if a.score >= prev_score + STOMP_SCORE && a.score < prev_score + BLOCK_SCORE {
                    stomps += 1;
                }
                (prev_coins, prev_score) = (a.coins, a.score);
            }
            assert!(deaths < 40, "seed {seed}: {deaths} muertes en 300 s (con el trío fijo eran ~75)");
            assert!(hits >= 5, "seed {seed}: {hits} golpes de bloque");
            assert!(stomps >= 1, "seed {seed}: ningún pisotón");
            assert!(restarts.len() >= 5, "seed {seed}: {} reinicios", restarts.len());
            let distinct_goombas = restarts.iter().map(|r| r.1).collect::<std::collections::BTreeSet<_>>();
            assert!(distinct_goombas.len() >= 4, "seed {seed}: reinicios casi iguales {restarts:?}");
        }
    }

    #[test]
    fn the_first_trio_is_drawn_again_at_each_restart_within_the_documented_ranges() {
        let mut a = anim(80, 24, 7, 30);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..40 {
            a.init_level();
            let (p, b, g) = (a.pipes[0].x - 80.0, a.q_blocks[0].x - 80.0, a.goombas[0].x - 80.0);
            assert!((FIRST_PIPE_DIST.0..=FIRST_PIPE_DIST.1).contains(&p), "{p}");
            assert!((FIRST_BLOCK_DIST.0..=FIRST_BLOCK_DIST.1).contains(&b), "{b}");
            assert!((FIRST_GOOMBA_DIST.0..=FIRST_GOOMBA_DIST.1).contains(&g), "{g}");
            seen.insert(g as i32);
        }
        assert!(seen.len() >= 15, "el goomba apenas cambia: {seen:?}");
    }

    #[test]
    fn element_lists_stay_bounded_at_wide_sizes_and_over_time() {
        for &(w, h, steps) in &[(1024u16, 24u16, 60_000u32), (4000, 24, 20_000), (u16::MAX, 24, 3_000)] {
            let mut a = anim(w, h, 5, 60);
            let check = |a: &MarioAnimation| {
                for n in [a.clouds.len(), a.bushes.len(), a.pipes.len(), a.goombas.len(), a.q_blocks.len()] {
                    assert!(n <= MAX_ELEMS, "{w}x{h}: {n}");
                }
                assert!(a.pop_coins.len() <= MAX_COINS);
            };
            check(&a);
            for i in 0..steps {
                a.update(w, h);
                if i % 97 == 0 {
                    check(&a);
                }
            }
            check(&a);
            assert!(a.mario_y.is_finite() && a.mario_vy.is_finite());
        }
    }

    #[test]
    fn degenerate_sizes_never_panic() {
        let mut sizes: Vec<(u16, u16)> = Vec::new();
        for w in 0..=22u16 {
            for h in 0..=14u16 {
                sizes.push((w, h));
            }
        }
        sizes.extend([(1024, 3), (3, 1024), (1, 1024), (1024, 1), (19, 11), (20, 12), (211, 58), (1024, 1024)]);
        for (w, h) in sizes {
            for fps in [30u32, 60, 0, u32::MAX] {
                let cells = w as usize * h as usize;
                let (updates, draw_every) = if cells > 100_000 { (if fps == 0 { 3 } else { 20 }, 7) } else if cells > 5_000 { (800, 31) } else { (3_000, 7) };
                let mut a = anim(w, h, 9, fps);
                let mut grid = Grid::new(w, h);
                for i in 0..updates {
                    a.update(w, h);
                    if i % draw_every == 0 {
                        a.draw(&mut grid);
                    }
                }
                a.draw(&mut grid);
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
            for _ in 0..400 {
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

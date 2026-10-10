//! Port de `refs/cwork/src/animation/compile_sim.rs`.
//! Cambios: RNG propio, `Grid` en vez de `Buffer`, ticks lógicos a 60 por
//! segundo independientes del FPS (`set_fps` ajusta los pasos por `update`),
//! porcentajes enteros (sin formatear flotantes en WASM), aritmética `i32` y
//! bloque de log centrado horizontalmente en rejillas anchas.

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};
use crate::rng::Rng;

const DEPS: &[&str] = &[
    "libc v0.2.155",
    "cfg-if v1.0.0",
    "once_cell v1.19.0",
    "log v0.4.21",
    "bitflags v2.6.0",
    "serde v1.0.204",
    "serde_json v1.0.120",
    "proc-macro2 v1.0.86",
    "quote v1.0.36",
    "syn v2.0.71",
    "openssl-sys v0.9.102",
    "tokio v1.38.1",
    "hyper v1.4.1",
    "axum v0.7.5",
];

const APP_FILES: &[&str] = &[
    "src/buffer.rs",
    "src/animation/mod.rs",
    "src/animation/fire.rs",
    "src/animation/pipes.rs",
    "src/animation/rain.rs",
    "src/animation/compile_sim.rs",
    "src/main.rs",
];

const GREEN: u32 = rgb(92, 180, 35);
const WHITE: u32 = rgb(235, 235, 235);
const GRAY: u32 = rgb(130, 130, 130);
const RED: u32 = rgb(255, 85, 85);
const YELLOW: u32 = rgb(255, 220, 50);
const CYAN: u32 = rgb(0, 210, 230);

/// Longitud en caracteres de la línea más larga posible: la de `error[E0599]`
/// (106); la `^^^` del warning mide 103 y la línea activa más larga 60.
const BLOCK_W: i32 = 106;
/// Ticks lógicos por segundo del original.
const TICKS_PER_SECOND: u64 = 60;
const DEFAULT_FPS: u32 = 30;

type Line = (String, u32);

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Download { dep_idx: usize, tick: u32 },
    CompileDeps { dep_idx: usize, progress: f32, tick: u32 },
    CompileApp { file_idx: usize, progress: f32, tick: u32, has_error: bool },
    ErrorEvent { tick: u32 },
    CleanAndRetry { tick: u32 },
    Linking { tick: u32 },
    Finished { tick: u32 },
}

/// Pasos lógicos (a 60/s) por llamada a `update` para un FPS dado.
fn steps_for(fps: u32) -> u32 {
    let fps = fps.max(1) as u64;
    ((TICKS_PER_SECOND + fps / 2) / fps).max(1) as u32
}

/// Barra `[███---]` de `width` columnas en total (corchetes incluidos).
fn make_progress_bar(width: usize, progress: f32) -> String {
    let bar_width = width.saturating_sub(2);
    let fill = ((bar_width as f32 * progress.max(0.0)) as usize).min(bar_width);
    format!("[{}{}]", "█".repeat(fill), "-".repeat(bar_width - fill))
}

fn build_line(bar_width: usize, progress: f32, name: &str) -> Line {
    let pct = (progress * 100.0).round() as i32;
    let pb = make_progress_bar(bar_width, progress);
    (format!("  Build {pct:>3}% {pb} {name}"), CYAN)
}

pub struct CompileSimAnimation {
    history: Vec<Line>,
    state: State,
    width: u16,
    height: u16,
    rng: Rng,
    steps: u32,
}

impl CompileSimAnimation {
    pub fn new(width: u16, height: u16, seed: u32) -> Self {
        Self {
            history: Vec::new(),
            state: State::Download { dep_idx: 0, tick: 0 },
            width,
            height,
            rng: Rng::new(seed as u64),
            steps: steps_for(DEFAULT_FPS),
        }
    }

    fn add_log(&mut self, text: String, fg: u32) {
        self.history.push((text, fg));
        let max = if self.height > 5 { self.height as usize - 5 } else { 5 };
        if self.history.len() > max {
            let excess = self.history.len() - max;
            self.history.drain(..excess);
        }
    }

    /// Un tick lógico de la máquina de estados.
    fn step(&mut self) {
        let mut logs: Vec<Line> = Vec::new();
        let mut next_state = None;
        let mut clear_history = false;

        match &mut self.state {
            State::Download { dep_idx, tick } => {
                *tick += 1;
                if *tick == 1 {
                    if let Some(dep) = DEPS.get(*dep_idx) {
                        logs.push((format!("  Downloading {dep} from crates.io..."), GRAY));
                    }
                }
                if *tick >= 10 {
                    *dep_idx += 1;
                    *tick = 0;
                    if *dep_idx >= DEPS.len() {
                        next_state = Some(State::CompileDeps { dep_idx: 0, progress: 0.0, tick: 0 });
                    }
                }
            }
            State::CompileDeps { dep_idx, progress, tick } => {
                *tick = tick.saturating_add(1);
                *progress += self.rng.range_f32(0.08, 0.22);
                if *progress >= 1.0 {
                    if let Some(dep) = DEPS.get(*dep_idx) {
                        logs.push((format!("{:>12} {dep}", "Compiling"), GREEN));
                    }
                    *dep_idx += 1;
                    *progress = 0.0;
                    *tick = 0;
                    if *dep_idx >= DEPS.len() {
                        next_state = Some(State::CompileApp { file_idx: 0, progress: 0.0, tick: 0, has_error: true });
                    }
                }
            }
            State::CompileApp { file_idx, progress, tick, has_error } => {
                *tick = tick.saturating_add(1);
                *progress += 0.05;
                if *progress >= 1.0 {
                    if let Some(file) = APP_FILES.get(*file_idx) {
                        let pct = (*file_idx + 1) * 100 / APP_FILES.len();
                        logs.push((format!("   [{pct:>2}%] Compiling cwork/{file}"), WHITE));
                        if *file_idx == 2 {
                            logs.push((format!("warning: unused variable 'unused_decay' in cwork/{file}"), YELLOW));
                            logs.push(("   --> src/animation/fire.rs:45:13".to_string(), GRAY));
                            logs.push(("    |".to_string(), GRAY));
                            logs.push((" 45 |         let unused_decay = 0;".to_string(), GRAY));
                            logs.push(("    |             ^^^^^^^^^^^^ help: if this is intentional, prefix with an underscore: `_unused_decay`".to_string(), GRAY));
                        }
                    }
                    if *file_idx + 1 >= APP_FILES.len() && *has_error {
                        next_state = Some(State::ErrorEvent { tick: 0 });
                    } else {
                        *file_idx += 1;
                        *progress = 0.0;
                        *tick = 0;
                        if *file_idx >= APP_FILES.len() {
                            next_state = Some(State::Linking { tick: 0 });
                        }
                    }
                }
            }
            State::ErrorEvent { tick } => {
                *tick += 1;
                if *tick == 5 {
                    logs.push(("error[E0599]: no method named `handle_reset_safely` found for struct `PipesAnimation` in the current scope".to_string(), RED));
                } else if *tick == 12 {
                    logs.push(("   --> src/main.rs:89:14".to_string(), WHITE));
                    logs.push(("    |".to_string(), GRAY));
                    logs.push((" 89 |         anim.handle_reset_safely(&mut current_buffer);".to_string(), WHITE));
                    logs.push(("    |              ^^^^^^^^^^^^^^^^^^^ method not found in `PipesAnimation`".to_string(), RED));
                } else if *tick == 22 {
                    logs.push(("error: could not compile `cwork` (bin \"cwork\") due to 1 previous error".to_string(), RED));
                    next_state = Some(State::CleanAndRetry { tick: 0 });
                }
            }
            State::CleanAndRetry { tick } => {
                *tick += 1;
                if *tick == 20 {
                    logs.push((":: Applying hot patch to `src/main.rs`...".to_string(), CYAN));
                } else if *tick == 35 {
                    logs.push(("   cargo clean".to_string(), GRAY));
                } else if *tick == 45 {
                    logs.push(("   cargo check --release".to_string(), GRAY));
                    next_state = Some(State::CompileApp { file_idx: 0, progress: 0.0, tick: 0, has_error: false });
                }
            }
            State::Linking { tick } => {
                *tick += 1;
                if *tick == 10 {
                    logs.push((format!("{:>12} release target(s) in 14.56s", "Linking"), GREEN));
                } else if *tick == 25 {
                    logs.push((format!("{:>12} release [optimized] target(s) finished", "Finished"), GREEN));
                    next_state = Some(State::Finished { tick: 0 });
                }
            }
            State::Finished { tick } => {
                *tick += 1;
                if *tick == 180 {
                    clear_history = true;
                    next_state = Some(State::Download { dep_idx: 0, tick: 0 });
                }
            }
        }

        if clear_history {
            self.history.clear();
        }
        for (text, fg) in logs {
            self.add_log(text, fg);
        }
        if let Some(ns) = next_state {
            self.state = ns;
        }
    }

    /// Texto y color de la línea activa, si el estado tiene una.
    fn active_line(&self) -> Option<Line> {
        let bar_width = if self.width > 50 { 20 } else { 10 };
        match self.state {
            State::Download { dep_idx, .. } => DEPS.get(dep_idx).map(|dep| {
                let pb = make_progress_bar(15, 0.5);
                (format!("  Fetch [ {pb} ] Downloading {dep}"), GRAY)
            }),
            State::CompileDeps { dep_idx, progress, .. } => DEPS.get(dep_idx).map(|dep| build_line(bar_width, progress, dep)),
            State::CompileApp { file_idx, progress, .. } => APP_FILES.get(file_idx).map(|f| build_line(bar_width, progress, f)),
            State::CleanAndRetry { tick } => Some(if tick < 35 {
                ("  Patching source code...".to_string(), CYAN)
            } else {
                ("  Cleaning cargo targets...".to_string(), GRAY)
            }),
            State::Linking { .. } => Some(("  Linking target executables...".to_string(), GREEN)),
            State::Finished { .. } => Some(("  Target up-to-date. Waiting for changes...".to_string(), GREEN)),
            State::ErrorEvent { .. } => None,
        }
    }
}

impl Animation for CompileSimAnimation {
    fn update(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        for _ in 0..self.steps {
            self.step();
        }
    }

    fn draw(&mut self, grid: &mut Grid) {
        grid.clear();
        let height = self.height as i32;
        let margin = ((self.width as i32 - BLOCK_W) / 2).max(1);

        for (i, (text, fg)) in self.history.iter().enumerate() {
            let y = i as i32;
            if y >= height {
                break;
            }
            grid.set_str(margin, y, text, *fg, NO_COLOR);
        }

        let start_y = self.history.len() as i32 + 1;
        if start_y >= height {
            return;
        }
        if let Some((text, fg)) = self.active_line() {
            grid.set_str(margin, start_y, &text, fg, NO_COLOR);
        }
    }

    fn set_fps(&mut self, fps: u32) {
        self.steps = steps_for(fps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(w: u16, h: u16, seed: u32, fps: u32) -> CompileSimAnimation {
        let mut a = CompileSimAnimation::new(w, h, seed);
        a.set_fps(fps);
        a
    }

    fn row_text(grid: &Grid, x: i32, y: u16, n: usize) -> String {
        (0..n).filter_map(|i| grid.get((x + i as i32) as u16, y)).map(|c| c.0).collect()
    }

    fn app_pass(e: &mut Vec<(String, u32)>) {
        let pcts = [14, 28, 42, 57, 71, 85, 100];
        for (i, f) in APP_FILES.iter().enumerate() {
            e.push((format!("   [{:>2}%] Compiling cwork/{f}", pcts[i]), WHITE));
            if i == 2 {
                e.push(("warning: unused variable 'unused_decay' in cwork/src/animation/fire.rs".to_string(), YELLOW));
                e.push(("   --> src/animation/fire.rs:45:13".to_string(), GRAY));
                e.push(("    |".to_string(), GRAY));
                e.push((" 45 |         let unused_decay = 0;".to_string(), GRAY));
                e.push(("    |             ^^^^^^^^^^^^ help: if this is intentional, prefix with an underscore: `_unused_decay`".to_string(), GRAY));
            }
        }
    }

    fn expected_log() -> Vec<(String, u32)> {
        let mut e = Vec::new();
        for d in DEPS {
            e.push((format!("  Downloading {d} from crates.io..."), GRAY));
        }
        for d in DEPS {
            e.push((format!("   Compiling {d}").replace("   Compiling", &format!("{:>12}", "Compiling")), GREEN));
        }
        app_pass(&mut e);
        e.push(("error[E0599]: no method named `handle_reset_safely` found for struct `PipesAnimation` in the current scope".to_string(), RED));
        e.push(("   --> src/main.rs:89:14".to_string(), WHITE));
        e.push(("    |".to_string(), GRAY));
        e.push((" 89 |         anim.handle_reset_safely(&mut current_buffer);".to_string(), WHITE));
        e.push(("    |              ^^^^^^^^^^^^^^^^^^^ method not found in `PipesAnimation`".to_string(), RED));
        e.push(("error: could not compile `cwork` (bin \"cwork\") due to 1 previous error".to_string(), RED));
        e.push((":: Applying hot patch to `src/main.rs`...".to_string(), CYAN));
        e.push(("   cargo clean".to_string(), GRAY));
        e.push(("   cargo check --release".to_string(), GRAY));
        app_pass(&mut e);
        e.push(("     Linking release target(s) in 14.56s".to_string(), GREEN));
        e.push(("    Finished release [optimized] target(s) finished".to_string(), GREEN));
        e
    }

    #[test]
    fn full_sequence_has_exact_texts_colors_and_order() {
        let expected = expected_log();
        let mut a = anim(100, 400, 1, 60);
        let mut updates = 0;
        while a.history.len() < expected.len() {
            a.update(100, 400);
            updates += 1;
            assert!(updates < 20_000, "la secuencia no termina");
        }
        assert_eq!(a.history, expected);
        assert!(matches!(a.state, State::Finished { tick: 0 }));
        // Una línea más de 10 ticks de descarga por dep + compilación de deps >= 13 ticks por dep.
        assert!(updates > 14 * 10 + 14 * 5 + 7 * 20 * 2);
    }

    #[test]
    fn finished_pause_is_180_logical_ticks_then_history_restarts() {
        let mut a = anim(100, 400, 1, 60);
        while !matches!(a.state, State::Finished { .. }) {
            a.update(100, 400);
        }
        for _ in 0..179 {
            a.update(100, 400);
        }
        assert!(!a.history.is_empty());
        a.update(100, 400);
        assert!(a.history.is_empty());
        assert!(matches!(a.state, State::Download { dep_idx: 0, tick: 0 }));
        a.update(100, 400);
        assert_eq!(a.history.len(), 1);
        assert_eq!(a.history[0].0, format!("  Downloading {} from crates.io...", DEPS[0]));
    }

    #[test]
    fn warning_and_error_have_exact_text_and_color() {
        let expected = expected_log();
        let mut a = anim(100, 400, 3, 60);
        while a.history.len() < expected.len() {
            a.update(100, 400);
        }
        let find = |p: &str| a.history.iter().find(|l| l.0.starts_with(p)).cloned().unwrap();
        assert_eq!(find("warning:"), ("warning: unused variable 'unused_decay' in cwork/src/animation/fire.rs".to_string(), rgb(255, 220, 50)));
        assert_eq!(
            find("error[E0599]"),
            ("error[E0599]: no method named `handle_reset_safely` found for struct `PipesAnimation` in the current scope".to_string(), rgb(255, 85, 85))
        );
        assert_eq!(find("error: could").1, rgb(255, 85, 85));
        assert_eq!(find(":: Applying").1, rgb(0, 210, 230));
    }

    #[test]
    fn error_happens_once_and_second_pass_has_no_error() {
        let expected = expected_log();
        let mut a = anim(100, 400, 5, 60);
        while a.history.len() < expected.len() {
            a.update(100, 400);
        }
        assert_eq!(a.history.iter().filter(|l| l.0.starts_with("error[")).count(), 1);
        assert_eq!(a.history.iter().filter(|l| l.0.contains("Compiling cwork/src/main.rs")).count(), 2);
        assert_eq!(a.history.iter().filter(|l| l.0.starts_with("warning:")).count(), 2);
    }

    #[test]
    fn block_width_is_the_longest_possible_line() {
        let mut longest = expected_log().iter().map(|l| l.0.chars().count()).max().unwrap();
        // Líneas activas posibles.
        let mut a = anim(100, 400, 1, 60);
        for _ in 0..3000 {
            a.update(100, 400);
            if let Some((t, _)) = a.active_line() {
                longest = longest.max(t.chars().count());
            }
        }
        assert_eq!(longest as i32, BLOCK_W);
        assert_eq!(BLOCK_W, 106);
    }

    #[test]
    fn download_phase_logs_one_dep_each_10_ticks() {
        let mut a = anim(100, 40, 1, 60);
        for t in 1..=10 {
            a.update(100, 40);
            assert_eq!(a.history.len(), 1, "tick {t}");
        }
        for _ in 0..10 {
            a.update(100, 40);
        }
        assert_eq!(a.history.len(), 2);
        assert_eq!(a.history[1].0, "  Downloading cfg-if v1.0.0 from crates.io...");
    }

    #[test]
    fn history_is_capped_at_height_minus_five_or_five() {
        for (h, cap) in [(24u16, 19usize), (40, 35), (10, 5), (6, 1), (5, 5), (3, 5), (0, 5)] {
            let mut a = anim(100, h, 2, 60);
            let mut max_seen = 0;
            for _ in 0..4000 {
                a.update(100, h);
                assert!(a.history.len() <= cap, "altura {h}: {}", a.history.len());
                max_seen = max_seen.max(a.history.len());
            }
            assert_eq!(max_seen, cap, "altura {h}");
        }
    }

    #[test]
    fn active_line_is_drawn_at_history_len_plus_one() {
        let mut a = anim(80, 24, 1, 60);
        let mut grid = Grid::new(80, 24);
        for _ in 0..500 {
            a.update(80, 24);
            a.draw(&mut grid);
            let y = a.history.len() as u16 + 1;
            match a.active_line() {
                Some((text, fg)) => {
                    let n = text.chars().count();
                    assert_eq!(row_text(&grid, 1, y, n), text);
                    assert_eq!(grid.get(1, y).unwrap().1, fg);
                }
                None => assert!(matches!(a.state, State::ErrorEvent { .. })),
            }
            assert_eq!(grid.get(1, y - 1).unwrap().0, ' ');
            // Las líneas de historial en sus filas.
            for (i, (t, c)) in a.history.iter().enumerate() {
                let n = t.chars().count().min(20);
                assert_eq!(row_text(&grid, 1, i as u16, n), t.chars().take(n).collect::<String>());
                assert_eq!(grid.get(1, i as u16).unwrap().1, *c);
            }
        }
    }

    #[test]
    fn active_line_is_skipped_when_it_does_not_fit() {
        // Altura 3: con 2 líneas de historial la fila activa sería la 3 (fuera).
        let mut a = anim(80, 3, 1, 60);
        let mut grid = Grid::new(80, 3);
        for _ in 0..30 {
            a.update(80, 3);
        }
        assert_eq!(a.history.len(), 3);
        a.draw(&mut grid);
        for y in 0..3u16 {
            assert!(row_text(&grid, 1, y, 10).contains("Download"));
        }
        let mut a = anim(80, 8, 1, 60);
        let mut grid = Grid::new(80, 8);
        for _ in 0..100 {
            a.update(80, 8);
        }
        assert_eq!(a.history.len(), 3);
        a.draw(&mut grid);
        assert!(row_text(&grid, 1, 4, 20).contains("Fetch") || row_text(&grid, 1, 4, 20).contains("Build"));
    }

    #[test]
    fn progress_is_shown_in_whole_percent_and_bar_is_bounded() {
        let mut a = anim(40, 40, 1, 60);
        while !matches!(a.state, State::CompileApp { .. }) {
            a.update(40, 40);
        }
        for k in 1..=19u32 {
            a.update(40, 40);
            let (text, fg) = a.active_line().unwrap();
            let pct = k * 5;
            let fill = (8.0 * (k as f32 * 0.05)) as usize;
            let want = format!("  Build {pct:>3}% [{}{}] src/buffer.rs", "█".repeat(fill), "-".repeat(8 - fill));
            assert_eq!(text, want, "k={k}");
            assert_eq!(fg, CYAN);
        }
        // Barra de 20 columnas en anchos > 50, de 10 en el resto.
        let mut wide = anim(120, 40, 1, 60);
        while !matches!(wide.state, State::CompileApp { .. }) {
            wide.update(120, 40);
        }
        assert_eq!(wide.active_line().unwrap().0.chars().filter(|&c| c == '-').count(), 18);
        // Bordes de la barra.
        assert_eq!(make_progress_bar(10, 0.0), "[--------]");
        assert_eq!(make_progress_bar(10, 1.0), "[████████]");
        assert_eq!(make_progress_bar(10, 5.0), "[████████]");
        assert_eq!(make_progress_bar(10, -1.0), "[--------]");
        assert_eq!(make_progress_bar(10, f32::NAN), "[--------]");
        assert_eq!(make_progress_bar(1, 0.5), "[]");
        assert_eq!(make_progress_bar(0, 0.5), "[]");
        assert_eq!(make_progress_bar(15, 0.5), "[██████-------]");
    }

    #[test]
    fn download_active_line_uses_fixed_bar() {
        let a = anim(80, 24, 1, 60);
        assert_eq!(a.active_line().unwrap(), ("  Fetch [ [██████-------] ] Downloading libc v0.2.155".to_string(), GRAY));
    }

    #[test]
    fn deps_progress_steps_are_random_between_008_and_022() {
        let mut a = anim(80, 40, 7, 60);
        while !matches!(a.state, State::CompileDeps { .. }) {
            a.update(80, 40);
        }
        let mut last = 0.0f32;
        let mut seen = 0;
        while let State::CompileDeps { dep_idx: 0, progress, .. } = a.state {
            let step = progress - last;
            if progress > 0.0 {
                assert!((0.079..0.221).contains(&step), "paso {step}");
                seen += 1;
            }
            last = progress;
            a.update(80, 40);
        }
        assert!(seen >= 4, "solo {seen} pasos");
    }

    #[test]
    fn steps_per_update_follow_fps() {
        assert_eq!(steps_for(60), 1);
        assert_eq!(steps_for(30), 2);
        assert_eq!(steps_for(20), 3);
        assert_eq!(steps_for(120), 1);
        assert_eq!(steps_for(0), 60);
        assert_eq!(steps_for(u32::MAX), 1);
        assert_eq!(CompileSimAnimation::new(1, 1, 1).steps, 2);
    }

    /// Llamadas a `update` desde que aparece "Finished" hasta que se limpia el historial.
    fn calls_in_pause(fps: u32) -> u32 {
        let mut a = anim(100, 400, 1, fps);
        while !a.history.last().map_or(false, |l| l.0.trim_start().starts_with("Finished")) {
            a.update(100, 400);
        }
        let mut calls = 0;
        loop {
            let before = a.history.len();
            a.update(100, 400);
            calls += 1;
            if a.history.len() < before {
                return calls;
            }
            assert!(calls < 1000);
        }
    }

    #[test]
    fn the_final_pause_lasts_three_seconds_at_30_and_60_fps() {
        assert_eq!(calls_in_pause(60), 180);
        assert_eq!(calls_in_pause(30), 90);
        assert_eq!(calls_in_pause(20), 60);
    }

    #[test]
    fn resize_is_saved_by_update() {
        let mut a = anim(80, 24, 1, 30);
        a.update(40, 10);
        assert_eq!((a.width, a.height), (40, 10));
    }

    #[test]
    fn block_is_centered_on_wide_grids_and_starts_at_column_one_on_narrow_ones() {
        // Ancho 211: margen (211 - 106) / 2 = 52.
        let mut a = anim(211, 30, 1, 60);
        let mut grid = Grid::new(211, 30);
        for _ in 0..30 {
            a.update(211, 30);
        }
        a.draw(&mut grid);
        let first = &a.history[0].0;
        assert_eq!(row_text(&grid, 52, 0, first.chars().count()), *first);
        assert_eq!(grid.get(51, 0).unwrap().0, ' ');
        assert_eq!(grid.get(52, 0).unwrap().1, GRAY);
        let (active, _) = a.active_line().unwrap();
        assert_eq!(row_text(&grid, 52, a.history.len() as u16 + 1, active.chars().count()), active);
        // Ancho 106 y menores: margen 1.
        for w in [106u16, 107, 80, 20] {
            let mut a = anim(w, 30, 1, 60);
            let mut grid = Grid::new(w, 30);
            for _ in 0..30 {
                a.update(w, 30);
            }
            a.draw(&mut grid);
            assert_eq!(grid.get(0, 0).unwrap().0, ' ', "ancho {w}");
            assert_eq!(grid.get(1, 0).unwrap().0, ' ');
            assert_eq!(grid.get(3, 0).unwrap().0, 'D', "ancho {w}");
        }
        // Un ancho justo en el umbral: 108 -> margen 1; 110 -> margen 2.
        let mut a = anim(110, 30, 1, 60);
        let mut grid = Grid::new(110, 30);
        a.update(110, 30);
        a.draw(&mut grid);
        assert_eq!(grid.get(2, 0).unwrap().0, ' ');
        assert_eq!(grid.get(4, 0).unwrap().0, 'D');
    }

    #[test]
    fn nothing_is_clipped_on_a_grid_wide_enough() {
        let expected = expected_log();
        let w = 130u16;
        let mut a = anim(w, 400, 1, 60);
        let mut grid = Grid::new(w, 400);
        while a.history.len() < expected.len() {
            a.update(w, 400);
        }
        a.draw(&mut grid);
        let m = ((w as i32 - BLOCK_W) / 2).max(1);
        for (i, (t, _)) in a.history.iter().enumerate() {
            assert_eq!(row_text(&grid, m, i as u16, t.chars().count()), *t);
        }
    }

    #[test]
    fn degenerate_sizes_never_panic() {
        let mut sizes: Vec<(u16, u16)> = Vec::new();
        for w in 0..=6u16 {
            for h in 0..=6u16 {
                sizes.push((w, h));
            }
        }
        sizes.extend([(1024, 3), (3, 1024), (1, 1024), (1024, 1), (50, 24), (51, 24), (211, 58)]);
        for (w, h) in sizes {
            for fps in [30u32, 60, 0, u32::MAX] {
                let mut a = anim(w, h, 9, fps);
                let mut grid = Grid::new(w, h);
                let draw_every = if (w as usize * h as usize) > 10_000 { 97 } else { 1 };
                for i in 0..1_500 {
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
        for i in 0..2_000u32 {
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
            for _ in 0..300 {
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

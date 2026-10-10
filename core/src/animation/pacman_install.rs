//! Port de `refs/cwork/src/animation/pacman_install.rs`.
//! Cambios: RNG propio, `Grid` en vez de `Buffer`, ticks lógicos a 60 por
//! segundo independientes del FPS (`set_fps` ajusta los pasos por `update`),
//! números sin formato de flotantes (décimas enteras, redondeo al más cercano
//! con mitades hacia arriba; porcentajes enteros), bloque centrado
//! horizontalmente y lista de paquetes partida en varias líneas de log
//! (alineadas bajo el primer paquete, como hace pacman real) para que ninguna
//! línea supere `BLOCK_W`.

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};
use crate::rng::Rng;

struct Package {
    name: &'static str,
    new_ver: &'static str,
    size_mb: f32,
}

const PACKAGES: &[Package] = &[
    Package { name: "linux", new_ver: "6.13.2-1", size_mb: 182.4 },
    Package { name: "rustc", new_ver: "1.96.0-1", size_mb: 84.1 },
    Package { name: "cargo", new_ver: "1.96.0-1", size_mb: 8.7 },
    Package { name: "neovim", new_ver: "0.10.4-1", size_mb: 12.3 },
    Package { name: "hyprland", new_ver: "0.47.0-1", size_mb: 15.2 },
    Package { name: "firefox", new_ver: "135.0-1", size_mb: 72.5 },
    Package { name: "alacritty", new_ver: "0.15.1-1", size_mb: 3.8 },
    Package { name: "postgresql", new_ver: "17.2-1", size_mb: 18.5 },
    Package { name: "cmatrix-ascii", new_ver: "1.0.0-1", size_mb: 1.2 },
];

const CHECK_NAMES: [&str; 4] = [
    "checking keys",
    "checking package integrity",
    "loading package files",
    "checking for file conflicts",
];

const CYAN: u32 = rgb(23, 147, 209);
const WHITE: u32 = rgb(240, 240, 240);
const GREEN: u32 = rgb(50, 220, 50);
const GRAY: u32 = rgb(120, 120, 120);
const YELLOW: u32 = rgb(220, 220, 50);

/// Longitud de la línea más larga posible: la de una descarga activa con su
/// barra de 24 columnas (texto de 51 + 2 de separación + 24 = 77); el resto
/// de líneas (log, prechecks, upgrade) mide 62 como máximo.
const BLOCK_W: i32 = 77;
/// Ticks lógicos por segundo del original.
const TICKS_PER_SECOND: u64 = 60;
const DEFAULT_FPS: u32 = 30;
const FULL_BAR: &str = "[####################]";

type Line = (String, u32);
/// Fragmento de una fila activa: (desplazamiento dentro del bloque, texto, color).
type Seg = (i32, String, u32);

#[derive(Clone, PartialEq, Debug)]
struct ActiveDownload {
    pkg_idx: usize,
    downloaded: f32,
    speed: f32,
}

#[derive(Clone, PartialEq, Debug)]
enum State {
    SyncDb { sub_step: usize, tick: u32 },
    CalculateUpgrade { tick: u32 },
    Prompt { tick: u32 },
    Downloading { active: Vec<ActiveDownload>, completed: Vec<usize> },
    PreChecks { sub_step: usize, tick: u32 },
    Upgrading { package_idx: usize, progress: f32 },
    PostChecks { sub_step: usize, tick: u32 },
    Finished { tick: u32 },
}

/// Pasos lógicos (a 60/s) por llamada a `update` para un FPS dado.
fn steps_for(fps: u32) -> u32 {
    let fps = fps.max(1) as u64;
    ((TICKS_PER_SECOND + fps / 2) / fps).max(1) as u32
}

/// Barra `[###---]` de `width` columnas en total (corchetes incluidos).
fn make_progress_bar(width: usize, progress: f32) -> String {
    let bar_width = width.saturating_sub(2);
    let fill = ((bar_width as f32 * progress.max(0.0)) as usize).min(bar_width);
    format!("[{}{}]", "#".repeat(fill), "-".repeat(bar_width - fill))
}

/// Valor en décimas, redondeado al más cercano (mitades hacia arriba);
/// negativos y NaN dan 0.
fn tenths(x: f32) -> u32 {
    (x * 10.0).round() as u32
}

/// Décimas como `E.D` con `width` columnas en total (equivale a `{:>W.1}`).
fn dec(t: u32, width: usize) -> String {
    format!("{:>w$}.{}", t / 10, t % 10, w = width.saturating_sub(2))
}

/// Porcentaje entero acotado a 0..=100.
fn percent(progress: f32) -> i32 {
    ((progress * 100.0).round() as i32).clamp(0, 100)
}

fn download_info(name: &str, downloaded: f32, size: f32, speed: f32, progress: f32) -> String {
    format!(
        " {:<15} {} / {} MiB  {} MiB/s {:>3}%",
        name,
        dec(tenths(downloaded), 5),
        dec(tenths(size), 5),
        dec(tenths(speed), 4),
        percent(progress)
    )
}

/// Incremento de progreso por tick al actualizar un paquete.
fn upgrade_increment(size_mb: f32) -> f32 {
    (15.0 / size_mb).clamp(0.01, 0.15)
}

fn margin(width: u16) -> i32 {
    ((width as i32 - BLOCK_W) / 2).max(1)
}

/// Lista de paquetes partida en líneas de como máximo `BLOCK_W` columnas; las
/// siguientes a la primera se alinean bajo el primer paquete.
fn package_lines() -> Vec<String> {
    let prefix = format!("Packages ({})  ", PACKAGES.len());
    let indent = " ".repeat(prefix.len());
    let mut lines = Vec::new();
    let mut cur = prefix;
    let mut empty = true;
    for pkg in PACKAGES {
        let item = format!("{}-{}", pkg.name, pkg.new_ver);
        if !empty && cur.len() + 2 + item.len() > BLOCK_W as usize {
            lines.push(std::mem::replace(&mut cur, indent.clone()));
            empty = true;
        }
        if !empty {
            cur.push_str("  ");
        }
        cur.push_str(&item);
        empty = false;
    }
    lines.push(cur);
    lines
}

pub struct PacmanInstallAnimation {
    history: Vec<Line>,
    state: State,
    width: u16,
    height: u16,
    rng: Rng,
    steps: u32,
}

impl PacmanInstallAnimation {
    pub fn new(width: u16, height: u16, seed: u32) -> Self {
        Self {
            history: Vec::new(),
            state: State::SyncDb { sub_step: 0, tick: 0 },
            width,
            height,
            rng: Rng::new(seed as u64),
            steps: steps_for(DEFAULT_FPS),
        }
    }

    fn add_log(&mut self, text: String, fg: u32) {
        self.history.push((text, fg));
        let max = if self.height > 6 { self.height as usize - 6 } else { 5 };
        if self.history.len() > max {
            let excess = self.history.len() - max;
            self.history.drain(..excess);
        }
    }

    /// Un tick lógico de la máquina de estados.
    fn step(&mut self) {
        let mut logs: Vec<Line> = Vec::new();
        let mut update_last: Option<Line> = None;
        let mut next_state = None;
        let mut clear_history = false;

        match &mut self.state {
            State::SyncDb { sub_step, tick } => {
                *tick = tick.saturating_add(1);
                if *tick == 1 {
                    logs.push((":: Synchronizing package databases...".to_string(), CYAN));
                }
                if *tick == 12 && *sub_step == 0 {
                    logs.push((" core is up to date".to_string(), WHITE));
                    *sub_step = 1;
                    *tick = 2;
                } else if *tick == 22 && *sub_step == 1 {
                    logs.push((" extra is up to date".to_string(), WHITE));
                    *sub_step = 2;
                    *tick = 2;
                } else if *tick == 32 && *sub_step == 2 {
                    logs.push((" community is up to date".to_string(), WHITE));
                    *sub_step = 3;
                    *tick = 2;
                } else if *tick == 40 && *sub_step == 3 {
                    next_state = Some(State::CalculateUpgrade { tick: 0 });
                }
            }
            State::CalculateUpgrade { tick } => {
                *tick = tick.saturating_add(1);
                if *tick == 1 {
                    logs.push((":: Starting full system upgrade...".to_string(), CYAN));
                } else if *tick == 15 {
                    logs.push(("resolving dependencies...".to_string(), GRAY));
                } else if *tick == 28 {
                    logs.push(("looking for conflicting packages...".to_string(), GRAY));
                } else if *tick == 40 {
                    logs.push((String::new(), WHITE));
                    for line in package_lines() {
                        logs.push((line, WHITE));
                    }
                    logs.push((String::new(), WHITE));
                    logs.push(("Total Download Size:   398.7 MiB".to_string(), WHITE));
                    logs.push(("Total Installed Size: 1104.2 MiB".to_string(), WHITE));
                    logs.push(("Net Upgrade Size:       88.4 MiB".to_string(), WHITE));
                    logs.push((String::new(), WHITE));
                    next_state = Some(State::Prompt { tick: 0 });
                }
            }
            State::Prompt { tick } => {
                *tick = tick.saturating_add(1);
                if *tick == 10 {
                    logs.push((":: Proceed with installation? [Y/n] y".to_string(), WHITE));
                    let first = self.rng.range_f32(15.0, 30.0);
                    let second = self.rng.range_f32(10.0, 20.0);
                    next_state = Some(State::Downloading {
                        active: vec![
                            ActiveDownload { pkg_idx: 0, downloaded: 0.0, speed: first },
                            ActiveDownload { pkg_idx: 1, downloaded: 0.0, speed: second },
                        ],
                        completed: Vec::new(),
                    });
                }
            }
            State::Downloading { active, completed } => {
                let mut done: Vec<usize> = Vec::new();
                for (i, dl) in active.iter_mut().enumerate() {
                    let size = PACKAGES.get(dl.pkg_idx).map_or(0.0, |p| p.size_mb);
                    dl.downloaded += dl.speed / 60.0;
                    dl.speed = (dl.speed + self.rng.range_f32(-1.5, 1.5)).clamp(5.0, 45.0);
                    if dl.downloaded >= size {
                        dl.downloaded = size;
                        done.push(i);
                    }
                }
                // Índices ascendentes: se recorren al revés para no desplazarlos.
                for i in done.into_iter().rev() {
                    if i < active.len() {
                        let dl = active.remove(i);
                        if let Some(pkg) = PACKAGES.get(dl.pkg_idx) {
                            logs.push((format!(" download completed: {}", pkg.name), GREEN));
                        }
                        completed.push(dl.pkg_idx);
                    }
                }
                while active.len() < 2 && active.len() + completed.len() < PACKAGES.len() {
                    let next = (0..PACKAGES.len()).find(|idx| !completed.contains(idx) && !active.iter().any(|a| a.pkg_idx == *idx));
                    match next {
                        Some(pkg_idx) => {
                            let speed = self.rng.range_f32(8.0, 35.0);
                            active.push(ActiveDownload { pkg_idx, downloaded: 0.0, speed });
                        }
                        None => break,
                    }
                }
                if completed.len() == PACKAGES.len() && active.is_empty() {
                    logs.push((":: Retrieving packages...".to_string(), CYAN));
                    next_state = Some(State::PreChecks { sub_step: 0, tick: 0 });
                }
            }
            State::PreChecks { sub_step, tick } => {
                *tick = tick.saturating_add(1);
                let name = CHECK_NAMES.get(*sub_step).copied().unwrap_or("");
                if *tick == 1 {
                    logs.push((format!("({}/4) {} [--------------------]   0%", *sub_step + 1, name), WHITE));
                } else if *tick == 12 {
                    update_last = Some((format!("({}/4) {} {} 100%", *sub_step + 1, name, FULL_BAR), GREEN));
                    *sub_step += 1;
                    *tick = 0;
                    if *sub_step >= CHECK_NAMES.len() {
                        next_state = Some(State::Upgrading { package_idx: 0, progress: 0.0 });
                    }
                }
            }
            State::Upgrading { package_idx, progress } => match PACKAGES.get(*package_idx) {
                Some(pkg) => {
                    *progress += upgrade_increment(pkg.size_mb);
                    if *progress >= 1.0 {
                        logs.push((format!("({}/{}) upgrading {:<18} {} 100%", *package_idx + 1, PACKAGES.len(), pkg.name, FULL_BAR), GREEN));
                        *package_idx += 1;
                        *progress = 0.0;
                        if *package_idx >= PACKAGES.len() {
                            next_state = Some(State::PostChecks { sub_step: 0, tick: 0 });
                        }
                    }
                }
                None => next_state = Some(State::PostChecks { sub_step: 0, tick: 0 }),
            },
            State::PostChecks { sub_step, tick } => {
                *tick = tick.saturating_add(1);
                if *tick == 1 && *sub_step == 0 {
                    logs.push((":: Running post-transaction hooks...".to_string(), CYAN));
                }
                if *tick == 15 && *sub_step == 0 {
                    logs.push((" (1/3) Arming ConditionNeedsUpdate...".to_string(), WHITE));
                    *sub_step = 1;
                    *tick = 2;
                } else if *tick == 30 && *sub_step == 1 {
                    logs.push((" (2/3) Updating icon theme cache...".to_string(), WHITE));
                    *sub_step = 2;
                    *tick = 2;
                } else if *tick == 45 && *sub_step == 2 {
                    logs.push((" (3/3) Updating terminal database...".to_string(), WHITE));
                    *sub_step = 3;
                    *tick = 2;
                } else if *tick == 60 && *sub_step == 3 {
                    next_state = Some(State::Finished { tick: 0 });
                }
            }
            State::Finished { tick } => {
                *tick = tick.saturating_add(1);
                if *tick == 1 {
                    logs.push((":: System upgrade finished successfully!".to_string(), GREEN));
                } else if *tick == 120 {
                    clear_history = true;
                    next_state = Some(State::SyncDb { sub_step: 0, tick: 0 });
                }
            }
        }

        if clear_history {
            self.history.clear();
        }
        for (text, fg) in logs {
            self.add_log(text, fg);
        }
        if let Some(line) = update_last {
            if let Some(last) = self.history.last_mut() {
                *last = line;
            }
        }
        if let Some(ns) = next_state {
            self.state = ns;
        }
    }

    /// Filas activas bajo el historial; cada una es una lista de fragmentos.
    fn active_rows(&self) -> Vec<Vec<Seg>> {
        match &self.state {
            State::Downloading { active, .. } => {
                let mut rows = vec![vec![(0, ":: Retrieving packages...".to_string(), CYAN)]];
                let bar_width = if self.width > 60 { 24 } else { 12 };
                let m = margin(self.width);
                for dl in active {
                    let Some(pkg) = PACKAGES.get(dl.pkg_idx) else { continue };
                    let progress = if pkg.size_mb > 0.0 { dl.downloaded / pkg.size_mb } else { 1.0 };
                    let info = download_info(pkg.name, dl.downloaded, pkg.size_mb, dl.speed, progress);
                    let pb_off = info.chars().count() as i32 + 2;
                    let mut row = vec![(0, info, WHITE)];
                    if m + pb_off + (bar_width as i32) < self.width as i32 {
                        row.push((pb_off, make_progress_bar(bar_width, progress), CYAN));
                    }
                    rows.push(row);
                }
                rows
            }
            State::PreChecks { sub_step, .. } => match CHECK_NAMES.get(*sub_step) {
                Some(name) => vec![vec![(0, format!("({}/4) {} {}", sub_step + 1, name, make_progress_bar(20, 0.5)), YELLOW)]],
                None => Vec::new(),
            },
            State::Upgrading { package_idx, progress } => match PACKAGES.get(*package_idx) {
                Some(pkg) => vec![vec![(
                    0,
                    format!(
                        "({}/{}) upgrading {:<18} {} {:>3}%",
                        package_idx + 1,
                        PACKAGES.len(),
                        pkg.name,
                        make_progress_bar(20, *progress),
                        percent(*progress)
                    ),
                    YELLOW,
                )]],
                None => Vec::new(),
            },
            _ => Vec::new(),
        }
    }
}

impl Animation for PacmanInstallAnimation {
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
        let m = margin(self.width);

        for (i, (text, fg)) in self.history.iter().enumerate() {
            let y = i as i32;
            if y >= height {
                break;
            }
            grid.set_str(m, y, text, *fg, NO_COLOR);
        }

        let start_y = self.history.len() as i32 + 1;
        for (r, row) in self.active_rows().iter().enumerate() {
            let y = start_y + r as i32;
            if y >= height {
                break;
            }
            for (off, text, fg) in row {
                grid.set_str(m + off, y, text, *fg, NO_COLOR);
            }
        }

        if let State::Downloading { completed, .. } = &self.state {
            let summary = format!(" Download progress: {} / {} completed", completed.len(), PACKAGES.len());
            grid.set_str(m, height - 1, &summary, GRAY, NO_COLOR);
        }
    }

    fn set_fps(&mut self, fps: u32) {
        self.steps = steps_for(fps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(w: u16, h: u16, seed: u32, fps: u32) -> PacmanInstallAnimation {
        let mut a = PacmanInstallAnimation::new(w, h, seed);
        a.set_fps(fps);
        a
    }

    fn row_text(grid: &Grid, x: i32, y: u16, n: usize) -> String {
        (0..n).filter_map(|i| grid.get((x + i as i32) as u16, y)).map(|c| c.0).collect()
    }

    fn state_name(s: &State) -> &'static str {
        match s {
            State::SyncDb { .. } => "sync",
            State::CalculateUpgrade { .. } => "calc",
            State::Prompt { .. } => "prompt",
            State::Downloading { .. } => "download",
            State::PreChecks { .. } => "pre",
            State::Upgrading { .. } => "upgrade",
            State::PostChecks { .. } => "post",
            State::Finished { .. } => "finished",
        }
    }

    const PKG_LINES: [&str; 3] = [
        "Packages (9)  linux-6.13.2-1  rustc-1.96.0-1  cargo-1.96.0-1  neovim-0.10.4-1",
        "              hyprland-0.47.0-1  firefox-135.0-1  alacritty-0.15.1-1",
        "              postgresql-17.2-1  cmatrix-ascii-1.0.0-1",
    ];

    /// Corre hasta que aparece el mensaje final; devuelve el historial.
    fn run_full(seed: u32) -> Vec<Line> {
        let mut a = anim(100, 400, seed, 60);
        let mut n = 0;
        while !a.history.last().map_or(false, |l| l.0.starts_with(":: System upgrade finished")) {
            a.update(100, 400);
            n += 1;
            assert!(n < 50_000, "la secuencia no termina");
        }
        a.history
    }

    fn expected_log(order: &[&str]) -> Vec<Line> {
        let mut e: Vec<Line> = vec![
            (":: Synchronizing package databases...".into(), CYAN),
            (" core is up to date".into(), WHITE),
            (" extra is up to date".into(), WHITE),
            (" community is up to date".into(), WHITE),
            (":: Starting full system upgrade...".into(), CYAN),
            ("resolving dependencies...".into(), GRAY),
            ("looking for conflicting packages...".into(), GRAY),
            (String::new(), WHITE),
        ];
        for l in PKG_LINES {
            e.push((l.into(), WHITE));
        }
        e.push((String::new(), WHITE));
        e.push(("Total Download Size:   398.7 MiB".into(), WHITE));
        e.push(("Total Installed Size: 1104.2 MiB".into(), WHITE));
        e.push(("Net Upgrade Size:       88.4 MiB".into(), WHITE));
        e.push((String::new(), WHITE));
        e.push((":: Proceed with installation? [Y/n] y".into(), WHITE));
        for n in order {
            e.push((format!(" download completed: {n}"), GREEN));
        }
        e.push((":: Retrieving packages...".into(), CYAN));
        let checks = ["checking keys", "checking package integrity", "loading package files", "checking for file conflicts"];
        for (i, c) in checks.iter().enumerate() {
            e.push((format!("({}/4) {c} [####################] 100%", i + 1), GREEN));
        }
        for (i, p) in PACKAGES.iter().enumerate() {
            e.push((format!("({}/9) upgrading {:<18} [####################] 100%", i + 1, p.name), GREEN));
        }
        e.push((":: Running post-transaction hooks...".into(), CYAN));
        e.push((" (1/3) Arming ConditionNeedsUpdate...".into(), WHITE));
        e.push((" (2/3) Updating icon theme cache...".into(), WHITE));
        e.push((" (3/3) Updating terminal database...".into(), WHITE));
        e.push((":: System upgrade finished successfully!".into(), GREEN));
        e
    }

    #[test]
    fn full_sequence_has_exact_texts_colors_and_order() {
        for seed in [1u32, 2, 99] {
            let hist = run_full(seed);
            let order: Vec<&str> = hist
                .iter()
                .filter_map(|l| l.0.strip_prefix(" download completed: "))
                .map(|n| PACKAGES.iter().find(|p| p.name == n).expect("paquete conocido").name)
                .collect();
            let mut sorted = order.clone();
            sorted.sort_unstable();
            let mut all: Vec<&str> = PACKAGES.iter().map(|p| p.name).collect();
            all.sort_unstable();
            assert_eq!(sorted, all, "cada paquete se completa una vez");
            assert_eq!(hist, expected_log(&order), "semilla {seed}");
        }
    }

    #[test]
    fn states_follow_the_original_order() {
        let mut a = anim(100, 400, 3, 60);
        let mut names: Vec<&str> = Vec::new();
        for _ in 0..6000 {
            a.update(100, 400);
            let n = state_name(&a.state);
            if names.last() != Some(&n) {
                names.push(n);
            }
        }
        assert_eq!(&names[..8], ["sync", "calc", "prompt", "download", "pre", "upgrade", "post", "finished"]);
        assert_eq!(names[8], "sync");
    }

    #[test]
    fn early_ticks_match_the_original() {
        let mut a = anim(100, 400, 1, 60);
        let mut lens = Vec::new();
        for _ in 0..150 {
            a.step();
            lens.push(a.history.len());
        }
        let first_at = |n: usize| lens.iter().position(|&l| l >= n).map(|i| i + 1);
        assert_eq!(first_at(1), Some(1));
        assert_eq!(first_at(2), Some(12));
        assert_eq!(first_at(3), Some(32));
        assert_eq!(first_at(4), Some(62));
        assert_eq!(first_at(5), Some(101));
        assert_eq!(first_at(6), Some(115));
        assert_eq!(first_at(7), Some(128));
        assert_eq!(first_at(8), Some(140));
        assert_eq!(lens[138], 7);
        assert_eq!(lens[139], 16);
        assert_eq!(a.history.len(), 17);
        assert_eq!(a.history[16].0, ":: Proceed with installation? [Y/n] y");
        assert!(matches!(a.state, State::Downloading { .. }));
    }

    #[test]
    fn number_formatting_uses_tenths_without_float_formatting() {
        assert_eq!(tenths(182.4), 1824);
        assert_eq!(tenths(84.1), 841);
        assert_eq!(tenths(1.25), 13);
        assert_eq!(tenths(0.04), 0);
        assert_eq!(tenths(0.06), 1);
        assert_eq!(tenths(-3.0), 0);
        assert_eq!(tenths(f32::NAN), 0);
        assert_eq!(dec(1824, 5), "182.4");
        assert_eq!(dec(12, 5), "  1.2");
        assert_eq!(dec(0, 5), "  0.0");
        assert_eq!(dec(450, 4), "45.0");
        assert_eq!(dec(50, 4), " 5.0");
        assert_eq!(dec(99999, 5), "9999.9");
        assert_eq!(percent(0.5), 50);
        assert_eq!(percent(0.995), 100);
        assert_eq!(percent(0.004), 0);
        assert_eq!(percent(2.0), 100);
        assert_eq!(percent(-1.0), 0);
        assert_eq!(percent(f32::NAN), 0);
        assert_eq!(download_info("linux", 91.2, 182.4, 22.5, 0.5), " linux            91.2 / 182.4 MiB  22.5 MiB/s  50%");
        assert_eq!(
            download_info("cmatrix-ascii", 0.0, 1.2, 5.0, 0.0),
            " cmatrix-ascii     0.0 /   1.2 MiB   5.0 MiB/s   0%"
        );
        assert_eq!(download_info("linux", 91.2, 182.4, 22.5, 0.5).chars().count(), 51);
    }

    #[test]
    fn progress_bars_are_bounded() {
        assert_eq!(make_progress_bar(10, 0.0), "[--------]");
        assert_eq!(make_progress_bar(10, 1.0), "[########]");
        assert_eq!(make_progress_bar(10, 5.0), "[########]");
        assert_eq!(make_progress_bar(10, -1.0), "[--------]");
        assert_eq!(make_progress_bar(10, f32::NAN), "[--------]");
        assert_eq!(make_progress_bar(1, 0.5), "[]");
        assert_eq!(make_progress_bar(0, 0.5), "[]");
        assert_eq!(make_progress_bar(20, 0.5), "[#########---------]");
    }

    #[test]
    fn package_list_wraps_in_order_within_the_block() {
        let lines = package_lines();
        assert_eq!(lines, PKG_LINES);
        assert!(lines.iter().all(|l| l.len() as i32 <= BLOCK_W));
        let joined = lines.join(" ");
        let mut last = 0;
        for p in PACKAGES {
            let item = format!("{}-{}", p.name, p.new_ver);
            let pos = joined.find(&item).expect("el paquete aparece");
            assert!(pos >= last);
            last = pos;
        }
        assert!(lines[0].starts_with("Packages (9)  "));
    }

    #[test]
    fn block_width_is_the_longest_possible_line() {
        let mut a = anim(211, 400, 1, 60);
        let mut longest = 0usize;
        for _ in 0..8000 {
            a.update(211, 400);
            for row in a.active_rows() {
                for (off, t, _) in row {
                    longest = longest.max(off as usize + t.chars().count());
                }
            }
        }
        for l in run_full(1) {
            longest = longest.max(l.0.chars().count());
        }
        assert_eq!(longest as i32, BLOCK_W);
        assert_eq!(BLOCK_W, 77);
        // Con 12 columnas de barra (ancho <= 60) tampoco se supera.
        assert!(51 + 2 + 12 <= BLOCK_W);
    }

    #[test]
    fn history_is_capped_at_height_minus_six_or_five() {
        for (h, cap) in [(24u16, 18usize), (10, 4), (7, 1), (6, 5), (5, 5), (0, 5)] {
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
    fn downloads_keep_two_active_and_follow_the_original_rules() {
        let mut a = anim(100, 400, 5, 60);
        while !matches!(a.state, State::Downloading { .. }) {
            a.step();
        }
        if let State::Downloading { active, completed } = &a.state {
            assert!(completed.is_empty());
            assert_eq!((active[0].pkg_idx, active[1].pkg_idx), (0, 1));
            assert!((15.0..30.0).contains(&active[0].speed));
            assert!((10.0..20.0).contains(&active[1].speed));
        }
        let mut prev_done = 0;
        while let State::Downloading { active, completed } = &a.state {
            assert!(active.len() <= 2);
            if completed.len() < PACKAGES.len() - 1 {
                assert_eq!(active.len(), 2);
            }
            for dl in active {
                assert!((5.0..=45.0).contains(&dl.speed));
                assert!(dl.downloaded <= PACKAGES[dl.pkg_idx].size_mb);
            }
            assert!(completed.len() >= prev_done);
            prev_done = completed.len();
            a.step();
        }
        assert_eq!(prev_done, PACKAGES.len() - 1);
        assert!(matches!(a.state, State::PreChecks { sub_step: 0, tick: 0 }));
    }

    #[test]
    fn summary_is_drawn_on_the_last_row() {
        let mut a = anim(80, 24, 1, 60);
        let mut grid = Grid::new(80, 24);
        while !matches!(a.state, State::Downloading { .. }) {
            a.update(80, 24);
        }
        let mut seen = [false; 10];
        while let State::Downloading { completed, .. } = &a.state {
            let n = completed.len();
            a.draw(&mut grid);
            let want = format!(" Download progress: {n} / 9 completed");
            assert_eq!(row_text(&grid, 1, 23, want.len()), want);
            assert_eq!(grid.get(1, 23).unwrap().1, GRAY);
            seen[n] = true;
            a.update(80, 24);
        }
        assert!(seen[0] && seen[5]);
    }

    #[test]
    fn prechecks_update_the_last_line_to_green_100_percent() {
        let mut a = anim(100, 400, 1, 60);
        while !matches!(a.state, State::PreChecks { .. }) {
            a.step();
        }
        a.step();
        assert_eq!(a.history.last().unwrap(), &("(1/4) checking keys [--------------------]   0%".to_string(), WHITE));
        let n = a.history.len();
        for _ in 0..10 {
            a.step();
        }
        assert_eq!(a.history.last().unwrap().1, WHITE);
        a.step();
        assert_eq!(a.history.len(), n);
        assert_eq!(a.history.last().unwrap(), &("(1/4) checking keys [####################] 100%".to_string(), GREEN));
        a.step();
        assert_eq!(a.history.len(), n + 1);
        for _ in 0..(11 + 12 + 12) {
            a.step();
        }
        assert!(matches!(a.state, State::Upgrading { package_idx: 0, .. }));
        assert_eq!(a.history.last().unwrap().0, "(4/4) checking for file conflicts [####################] 100%");
    }

    #[test]
    fn upgrade_progress_steps_and_active_rows() {
        assert!((upgrade_increment(182.4) - 0.0822).abs() < 0.001);
        assert_eq!(upgrade_increment(1.2), 0.15);
        assert_eq!(upgrade_increment(100_000.0), 0.01);
        let mut a = anim(100, 400, 1, 60);
        a.state = State::Upgrading { package_idx: 2, progress: 0.0 };
        for _ in 0..6 {
            a.step();
        }
        assert!(a.history.is_empty());
        a.step();
        assert_eq!(a.history[0], (format!("(3/9) upgrading cargo{}[####################] 100%", " ".repeat(14)), GREEN));
        assert!(matches!(a.state, State::Upgrading { package_idx: 3, .. }));
        a.state = State::Upgrading { package_idx: 1, progress: 0.5 };
        let rows = a.active_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], (0, format!("(2/9) upgrading rustc{} [#########---------]  50%", " ".repeat(13)), YELLOW));
        a.state = State::PreChecks { sub_step: 2, tick: 3 };
        assert_eq!(a.active_rows()[0][0], (0, "(3/4) loading package files [#########---------]".to_string(), YELLOW));
        // Estados en el límite no indexan fuera de rango.
        a.state = State::Upgrading { package_idx: PACKAGES.len(), progress: 0.0 };
        assert!(a.active_rows().is_empty());
        a.step();
        assert!(matches!(a.state, State::PostChecks { .. }));
        a.state = State::PreChecks { sub_step: 4, tick: 0 };
        assert!(a.active_rows().is_empty());
        a.step();
    }

    #[test]
    fn download_bar_is_drawn_only_when_it_fits() {
        let mut a = anim(80, 24, 1, 60);
        let mut grid = Grid::new(80, 24);
        for _ in 0..200 {
            a.update(80, 24);
        }
        assert!(matches!(a.state, State::Downloading { .. }));
        a.draw(&mut grid);
        let y = a.history.len() as u16 + 2;
        // margen 1 + texto 51 + 2 = columna 54; 54 + 24 < 80.
        assert_eq!(grid.get(54, y).unwrap().0, '[');
        assert_eq!(grid.get(54 + 23, y).unwrap().0, ']');
        assert_eq!(grid.get(54, y).unwrap().1, CYAN);
        assert_eq!(grid.get(1, y).unwrap().1, WHITE);
        assert_eq!(row_text(&grid, 1, y - 1, 25), ":: Retrieving packages...");
        // Ancho 70: la barra de 24 no cabe y se omite.
        let mut grid = Grid::new(70, 24);
        a.update(70, 24);
        a.draw(&mut grid);
        let y = a.history.len() as u16 + 2;
        assert!((0..70u16).all(|x| grid.get(x, y).unwrap().0 != '['));
        // Ancho 50: barra de 12 columnas, tampoco cabe a partir de la columna 54.
        for w in [50u16, 60] {
            let mut grid = Grid::new(w, 24);
            a.update(w, 24);
            a.draw(&mut grid);
            assert!((0..w).all(|x| grid.get(x, y).unwrap().0 != '['));
        }
        assert!(make_progress_bar(12, 0.5).chars().count() == 12);
    }

    #[test]
    fn finished_pause_is_120_logical_ticks_then_history_restarts() {
        let mut a = anim(100, 400, 1, 60);
        while !matches!(a.state, State::Finished { .. }) {
            a.step();
        }
        assert!(matches!(a.state, State::Finished { tick: 0 }));
        for _ in 0..119 {
            a.step();
        }
        assert!(!a.history.is_empty());
        a.step();
        assert!(a.history.is_empty());
        assert!(matches!(a.state, State::SyncDb { sub_step: 0, tick: 0 }));
        a.step();
        assert_eq!(a.history.len(), 1);
        assert_eq!(a.history[0].0, ":: Synchronizing package databases...");
    }

    #[test]
    fn steps_per_update_follow_fps() {
        assert_eq!(steps_for(60), 1);
        assert_eq!(steps_for(30), 2);
        assert_eq!(steps_for(20), 3);
        assert_eq!(steps_for(0), 60);
        assert_eq!(steps_for(u32::MAX), 1);
        assert_eq!(PacmanInstallAnimation::new(1, 1, 1).steps, 2);
    }

    /// Llamadas a `update` desde que aparece el mensaje final hasta que se limpia el historial.
    fn calls_in_pause(fps: u32) -> u32 {
        let mut a = anim(100, 400, 1, fps);
        while !a.history.last().map_or(false, |l| l.0.starts_with(":: System upgrade finished")) {
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
    fn the_final_pause_lasts_two_seconds_at_20_30_and_60_fps() {
        // Entre el mensaje (tick 1) y la limpieza (tick 120) hay 119 ticks.
        assert_eq!(calls_in_pause(60), 119);
        for (fps, steps) in [(30u32, 2i32), (20, 3)] {
            let calls = calls_in_pause(fps) as i32;
            assert!((calls * steps - 119).abs() < steps, "fps {fps}: {calls} llamadas");
        }
    }

    #[test]
    fn same_tick_count_gives_same_state_at_any_fps() {
        let run = |fps: u32, updates: u32| {
            let mut a = anim(90, 30, 11, fps);
            for _ in 0..updates {
                a.update(90, 30);
            }
            (a.history, a.state)
        };
        let base = run(60, 600);
        assert_eq!(run(30, 300), base);
        assert_eq!(run(20, 200), base);
    }

    #[test]
    fn resize_is_saved_by_update() {
        let mut a = anim(80, 24, 1, 30);
        a.update(40, 10);
        assert_eq!((a.width, a.height), (40, 10));
    }

    #[test]
    fn block_is_centered_on_wide_grids_and_starts_at_column_one_on_narrow_ones() {
        // Ancho 211: margen (211 - 77) / 2 = 67.
        let mut a = anim(211, 30, 1, 60);
        let mut grid = Grid::new(211, 30);
        for _ in 0..30 {
            a.update(211, 30);
        }
        a.draw(&mut grid);
        let first = &a.history[0].0;
        assert_eq!(row_text(&grid, 67, 0, first.chars().count()), *first);
        assert_eq!(grid.get(66, 0).unwrap().0, ' ');
        assert_eq!(grid.get(67, 0).unwrap().1, CYAN);
        for w in [77u16, 78, 79, 80, 60, 20] {
            let mut a = anim(w, 30, 1, 60);
            let mut grid = Grid::new(w, 30);
            a.update(w, 30);
            a.draw(&mut grid);
            assert_eq!(grid.get(0, 0).unwrap().0, ' ', "ancho {w}");
            assert_eq!(grid.get(1, 0).unwrap().0, ':', "ancho {w}");
        }
        assert_eq!(margin(81), 2);
        // El bloque de descarga (texto y barra) lleva el mismo desplazamiento.
        let mut a = anim(211, 30, 1, 60);
        let mut grid = Grid::new(211, 30);
        for _ in 0..200 {
            a.update(211, 30);
        }
        a.draw(&mut grid);
        let y = a.history.len() as u16 + 2;
        assert_eq!(grid.get(67, y).unwrap().1, WHITE);
        assert_eq!(grid.get(66, y).unwrap().0, ' ');
        assert_eq!(grid.get(67 + 53, y).unwrap().0, '[');
        assert_eq!(grid.get(67 + 53 + 23, y).unwrap().0, ']');
    }

    #[test]
    fn nothing_is_clipped_on_a_grid_wide_enough() {
        let hist = run_full(1);
        let w = 100u16;
        let mut a = anim(w, 400, 1, 60);
        let mut grid = Grid::new(w, 400);
        while a.history.len() < hist.len() {
            a.update(w, 400);
        }
        a.draw(&mut grid);
        let m = margin(w);
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
                for i in 0..1_000 {
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

//! Panel de sistema: reloj ASCII, barras de RAM/DISK/CPU y un panel BROWSER.
//! Port de `dashboard` de cwork, alimentado con datos que reporta el navegador
//! (`feed` / `feed_text`) en lugar de `sysinfo`/`chrono`.
//!
//! Layout de `feed` (f32): `[hora, minuto, segundo, ram_usada_gb, ram_total_gb,
//! disco_usado_gb, disco_total_gb, n_nucleos, carga_0..carga_n]` (cargas en 0..1).

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};

const DIGITS: [[&str; 5]; 11] = [
    ["████", "█  █", "█  █", "█  █", "████"],
    ["  ██", "   █", "   █", "   █", "   █"],
    ["████", "   █", "████", "█   ", "████"],
    ["████", "   █", "████", "   █", "████"],
    ["█  █", "█  █", "████", "   █", "   █"],
    ["████", "█   ", "████", "   █", "████"],
    ["████", "█   ", "████", "█  █", "████"],
    ["████", "   █", "  ██", "  █ ", "  █ "],
    ["████", "█  █", "████", "█  █", "████"],
    ["████", "█  █", "████", "   █", "████"],
    ["    ", "  ■ ", "    ", "  ■ ", "    "],
];

const CYAN: u32 = rgb(0, 215, 255);
const GREEN: u32 = rgb(92, 180, 35);
const YELLOW: u32 = rgb(255, 220, 50);
const RED: u32 = rgb(255, 90, 90);
const GRAY: u32 = rgb(120, 120, 120);
const WHITE: u32 = rgb(235, 235, 235);
const BLUE: u32 = rgb(80, 160, 255);

const MAX_CORES: usize = 64;
const MAX_SHOWN_CORES: i32 = 16;
const MAX_LINES: usize = 12;
const MAX_LINE_CHARS: usize = 80;
/// Tope en GB para que `* 10` quepa de sobra en i32.
const MAX_GB: f32 = 1.0e8;
/// Ancho del encabezado del panel (igual al del original).
const HEADER_LEN: usize = 85;
const HEADER_TEXT: &str = "═ BROWSER (REPORTED BY THE BROWSER) ";

pub struct DashboardAnimation {
    time: (i32, i32, i32),
    used_ram: f32,
    total_ram: f32,
    used_disk: f32,
    total_disk: f32,
    loads: Vec<f32>,
    lines: Vec<String>,
}

/// NaN/inf/negativos → 0.
fn clean(x: f32) -> f32 {
    if x.is_finite() && x > 0.0 {
        x
    } else {
        0.0
    }
}

/// GB (f32 ≥ 0) → décimas enteras, redondeando al más cercano.
fn tenths(gb: f32) -> i32 {
    (gb.min(MAX_GB) * 10.0 + 0.5) as i32
}

fn fmt_gb(gb: f32) -> String {
    let t = tenths(gb);
    format!("{}.{}", t / 10, t % 10)
}

impl DashboardAnimation {
    pub fn new() -> Self {
        Self {
            time: (0, 0, 0),
            used_ram: 0.0,
            total_ram: 0.0,
            used_disk: 0.0,
            total_disk: 0.0,
            loads: Vec::new(),
            lines: Vec::new(),
        }
    }

    fn draw_digit(grid: &mut Grid, x: i32, y: i32, idx: usize) {
        let Some(sprite) = DIGITS.get(idx) else { return };
        for (dy, row) in sprite.iter().enumerate() {
            for (dx, ch) in row.chars().enumerate() {
                if ch != ' ' {
                    grid.set(x + dx as i32, y + dy as i32, ch, CYAN, NO_COLOR);
                }
            }
        }
    }

    /// Barra `LABEL [███---] ppp.d%  extra`. El porcentaje se da en décimas
    /// enteras (redondeo al más cercano), con el mismo ancho que `{:>5.1}`.
    fn draw_bar(grid: &mut Grid, x: i32, y: i32, label: &str, ratio: f32, extra: &str, bar_width: i32) {
        let ratio = if ratio.is_finite() { ratio.clamp(0.0, 1.0) } else { 0.0 };
        let fill = ((bar_width as f32 * ratio) as i32).clamp(0, bar_width);
        let head = format!("{label:<5} [");
        grid.set_str(x, y, &head, WHITE, NO_COLOR);
        let fx = x + head.chars().count() as i32;
        grid.set_str(fx, y, &"█".repeat(fill as usize), CYAN, NO_COLOR);
        let ex = fx + fill;
        grid.set_str(ex, y, &"-".repeat((bar_width - fill) as usize), GRAY, NO_COLOR);
        let color = if ratio > 0.85 {
            RED
        } else if ratio > 0.60 {
            YELLOW
        } else {
            GREEN
        };
        let t = (ratio * 1000.0 + 0.5) as i32;
        let close = format!("] {:>3}.{}%  {}", t / 10, t % 10, extra);
        grid.set_str(ex + bar_width - fill, y, &close, color, NO_COLOR);
    }

    fn draw_usage_bar(grid: &mut Grid, x: i32, y: i32, label: &str, used: f32, total: f32) {
        let extra = format!("{} GB / {} GB", fmt_gb(used), fmt_gb(total));
        Self::draw_bar(grid, x, y, label, used / total, &extra, 18);
    }

    fn draw_cores(&self, grid: &mut Grid, w: i32) {
        let shown = (self.loads.len() as i32).min(MAX_SHOWN_CORES);
        let two_cols = w > 70 && shown >= 4;
        let (base_x, rows_first) = if two_cols { ((w / 2 - 30).max(0), (shown + 1) / 2) } else { (((w - 30).max(0)) / 2, shown.min(6)) };
        for (i, load) in self.loads.iter().take(shown as usize).enumerate() {
            let i = i as i32;
            let (x, row) = if i < rows_first {
                (base_x, i)
            } else if two_cols {
                (w / 2 + 3, i - rows_first)
            } else {
                break;
            };
            Self::draw_bar(grid, x, 10 + row, &format!("CPU{i}"), *load, "", 12);
        }
    }

    fn draw_panel(&self, grid: &mut Grid, y0: i32, h: i32) {
        let tail = "═".repeat(HEADER_LEN.saturating_sub(HEADER_TEXT.chars().count()));
        grid.set_str(1, y0, &format!("{HEADER_TEXT}{tail}"), GRAY, NO_COLOR);
        for (i, line) in self.lines.iter().enumerate() {
            let y = y0 + 1 + i as i32;
            if y >= h {
                break;
            }
            match line.split_once(": ") {
                Some((label, value)) => {
                    grid.set_str(1, y, &format!("{label}:"), BLUE, NO_COLOR);
                    grid.set_str(2 + label.chars().count() as i32 + 1, y, value, WHITE, NO_COLOR);
                }
                None => grid.set_str(1, y, line, WHITE, NO_COLOR),
            }
        }
    }
}

impl Default for DashboardAnimation {
    fn default() -> Self {
        Self::new()
    }
}

impl Animation for DashboardAnimation {
    /// No muta nada: el reloj y las métricas llegan por `feed`.
    fn update(&mut self, _width: u16, _height: u16) {}

    fn draw(&mut self, grid: &mut Grid) {
        grid.clear();
        let (w, h) = (grid.width() as i32, grid.height() as i32);

        if h > 6 {
            let (hh, mm, ss) = self.time;
            let digits = [hh / 10, hh % 10, 10, mm / 10, mm % 10, 10, ss / 10, ss % 10];
            let mut x = ((w - 38).max(0)) / 2;
            for d in digits {
                Self::draw_digit(grid, x, 1, d as usize);
                x += if d == 10 { 3 } else { 5 };
            }
        }

        if h > 10 {
            let x = (((w - 60).max(0)) / 2).max(1);
            if self.total_ram > 0.0 {
                Self::draw_usage_bar(grid, x, 7, "RAM", self.used_ram, self.total_ram);
            }
            if self.total_disk > 0.0 && self.used_disk > 0.0 {
                Self::draw_usage_bar(grid, x, 8, "DISK", self.used_disk, self.total_disk);
            } else if self.total_disk > 0.0 {
                // Chrome de escritorio no informa el espacio libre: solo la capacidad, sin barra ni porcentaje.
                grid.set_str(x, 8, "DISK", WHITE, NO_COLOR);
                grid.set_str(x + 7, 8, &format!("{} GB total", fmt_gb(self.total_disk)), CYAN, NO_COLOR);
            }
        }

        let cores = self.loads.len() as i32;
        if h > 15 && cores > 0 {
            self.draw_cores(grid, w);
        }

        let shown = cores.min(MAX_SHOWN_CORES);
        let core_rows = if w > 70 { (shown + 1) / 2 } else { shown.min(6) };
        let y0 = 10 + core_rows + 1;
        if y0 + 3 < h {
            self.draw_panel(grid, y0, h);
        }
    }

    /// Ignora slices de menos de 8 valores; sanea todo lo demás.
    fn feed(&mut self, data: &[f32]) {
        if data.len() < 8 {
            return;
        }
        let at = |i: usize| clean(data.get(i).copied().unwrap_or(0.0));
        self.time = (at(0).min(23.0) as i32, at(1).min(59.0) as i32, at(2).min(59.0) as i32);
        self.used_ram = at(3).min(MAX_GB);
        self.total_ram = at(4).min(MAX_GB);
        self.used_disk = at(5).min(MAX_GB);
        self.total_disk = at(6).min(MAX_GB);
        let n = (at(7).min(MAX_CORES as f32) as usize).min(data.len() - 8);
        self.loads = (0..n).map(|i| at(8 + i).min(1.0)).collect();
    }

    fn feed_text(&mut self, text: &str) {
        self.lines = text
            .split('\n')
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.is_empty())
            .take(MAX_LINES)
            .map(|l| l.chars().take(MAX_LINE_CHARS).map(|c| if c.is_control() { ' ' } else { c }).collect())
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{rgb, NO_COLOR};

    const CYAN: u32 = rgb(0, 215, 255);
    const GREEN: u32 = rgb(92, 180, 35);
    const YELLOW: u32 = rgb(255, 220, 50);
    const RED: u32 = rgb(255, 90, 90);
    const GRAY: u32 = rgb(120, 120, 120);
    const WHITE: u32 = rgb(235, 235, 235);
    const BLUE: u32 = rgb(80, 160, 255);

    fn drawn(a: &mut DashboardAnimation, w: u16, h: u16) -> Grid {
        let mut g = Grid::new(w, h);
        a.update(w, h);
        a.draw(&mut g);
        g
    }

    fn row(g: &Grid, y: u16) -> String {
        (0..g.width()).filter_map(|x| g.get(x, y)).map(|c| c.0).collect()
    }

    fn text_at(g: &Grid, x: u16, y: u16, n: usize) -> String {
        (0..n as u16).filter_map(|i| g.get(x + i, y)).map(|c| c.0).collect()
    }

    fn data(h: f32, m: f32, s: f32, ram: (f32, f32), disk: (f32, f32), cores: &[f32]) -> Vec<f32> {
        let mut v = vec![h, m, s, ram.0, ram.1, disk.0, disk.1, cores.len() as f32];
        v.extend_from_slice(cores);
        v
    }

    fn fed(w: u16, h: u16, d: &[f32], text: &str) -> Grid {
        let mut a = DashboardAnimation::new();
        a.feed(d);
        a.feed_text(text);
        drawn(&mut a, w, h)
    }

    /// Comprueba los 8 sprites del reloj esperados (ancho 38) a partir de `x0`.
    fn clock_matches(g: &Grid, x0: i32, digits: [usize; 8]) -> bool {
        let sprites: [[&str; 5]; 11] = [
            ["████", "█  █", "█  █", "█  █", "████"],
            ["  ██", "   █", "   █", "   █", "   █"],
            ["████", "   █", "████", "█   ", "████"],
            ["████", "   █", "████", "   █", "████"],
            ["█  █", "█  █", "████", "   █", "   █"],
            ["████", "█   ", "████", "   █", "████"],
            ["████", "█   ", "████", "█  █", "████"],
            ["████", "   █", "  ██", "  █ ", "  █ "],
            ["████", "█  █", "████", "█  █", "████"],
            ["████", "█  █", "████", "   █", "████"],
            ["    ", "  ■ ", "    ", "  ■ ", "    "],
        ];
        let mut x = x0;
        for d in digits {
            for (dy, r) in sprites[d].iter().enumerate() {
                for (dx, ch) in r.chars().enumerate() {
                    let cell = g.get((x + dx as i32) as u16, 1 + dy as u16).unwrap();
                    // Los blancos no se dibujan (el ':' se solapa con el dígito siguiente).
                    if ch != ' ' && cell != (ch, CYAN, NO_COLOR) {
                        return false;
                    }
                }
            }
            x += if d == 10 { 3 } else { 5 };
        }
        true
    }

    #[test]
    fn initial_state_shows_clock_00_00_00_and_nothing_else() {
        let g = drawn(&mut DashboardAnimation::new(), 100, 40);
        assert!(clock_matches(&g, 31, [0, 0, 10, 0, 0, 10, 0, 0]));
        assert_eq!(row(&g, 7).trim(), "");
        assert_eq!(row(&g, 8).trim(), "");
        assert_eq!(row(&g, 10).trim(), "");
    }

    #[test]
    fn clock_draws_exact_digits_for_12_34_56_at_width_100() {
        let g = fed(100, 40, &data(12.0, 34.0, 56.0, (0.0, 0.0), (0.0, 0.0), &[]), "");
        assert!(clock_matches(&g, 31, [1, 2, 10, 3, 4, 10, 5, 6]));
    }

    #[test]
    fn clock_is_centered_and_clamped_at_narrow_widths() {
        let g = fed(38, 20, &data(8.0, 9.0, 7.0, (0.0, 0.0), (0.0, 0.0), &[]), "");
        assert!(clock_matches(&g, 0, [0, 8, 10, 0, 9, 10, 0, 7]));
        let g = fed(41, 20, &data(8.0, 9.0, 7.0, (0.0, 0.0), (0.0, 0.0), &[]), "");
        assert!(clock_matches(&g, 1, [0, 8, 10, 0, 9, 10, 0, 7]));
        // ancho < 38: x0 = 0 y se recorta por el borde
        let g = fed(20, 20, &data(8.0, 9.0, 7.0, (0.0, 0.0), (0.0, 0.0), &[]), "");
        assert_eq!(g.get(0, 1).unwrap(), ('█', CYAN, NO_COLOR));
    }

    #[test]
    fn invalid_time_is_clamped() {
        let g = fed(100, 40, &data(99.0, 75.0, 1e9, (0.0, 0.0), (0.0, 0.0), &[]), "");
        assert!(clock_matches(&g, 31, [2, 3, 10, 5, 9, 10, 5, 9]));
        let g = fed(100, 40, &data(-5.0, f32::NAN, f32::INFINITY, (0.0, 0.0), (0.0, 0.0), &[]), "");
        assert!(clock_matches(&g, 31, [0, 0, 10, 0, 0, 10, 0, 0]));
    }

    #[test]
    fn height_thresholds_for_each_section() {
        let d = data(1.0, 2.0, 3.0, (4.0, 8.0), (1.0, 2.0), &[0.5; 4]);
        let non_blank = |g: &Grid, y: u16| row(g, y).trim() != "";
        // reloj: alto > 6
        assert!(!non_blank(&fed(100, 6, &d, ""), 1));
        assert!(non_blank(&fed(100, 7, &d, ""), 1));
        // barras: alto > 10
        assert!(!non_blank(&fed(100, 10, &d, ""), 7));
        assert!(non_blank(&fed(100, 11, &d, ""), 7));
        assert!(non_blank(&fed(100, 11, &d, ""), 8));
        // CPU: alto > 15
        assert!(!non_blank(&fed(100, 15, &d, ""), 10));
        assert!(non_blank(&fed(100, 16, &d, ""), 10));
    }

    #[test]
    fn ram_bar_has_exact_layout_percentage_and_green_color() {
        // 4.0 / 16.0 GB = 25.0 %. start_x = (100-60)/2 = 20.
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (4.0, 16.0), (0.0, 0.0), &[]), "");
        let expected = format!("RAM   [{}{}]  25.0%  4.0 GB / 16.0 GB", "█".repeat(4), "-".repeat(14));
        assert_eq!(text_at(&g, 20, 7, expected.chars().count()), expected);
        assert_eq!(g.get(20, 7).unwrap(), ('R', WHITE, NO_COLOR));
        assert_eq!(g.get(27, 7).unwrap(), ('█', CYAN, NO_COLOR));
        assert_eq!(g.get(31, 7).unwrap(), ('-', GRAY, NO_COLOR));
        assert_eq!(g.get(45, 7).unwrap(), (']', GREEN, NO_COLOR));
        assert_eq!(g.get(60, 7).unwrap().1, GREEN);
    }

    #[test]
    fn bar_color_follows_thresholds() {
        let color_for = |used: f32| {
            let g = fed(100, 40, &data(0.0, 0.0, 0.0, (used, 100.0), (0.0, 0.0), &[]), "");
            g.get(45, 7).unwrap().1
        };
        assert_eq!(color_for(60.0), GREEN);
        assert_eq!(color_for(61.0), YELLOW);
        assert_eq!(color_for(85.0), YELLOW);
        assert_eq!(color_for(86.0), RED);
        assert_eq!(color_for(100.0), RED);
        assert_eq!(color_for(500.0), RED); // se acota a 100 %
    }

    #[test]
    fn disk_bar_is_hidden_when_total_is_zero_and_ram_when_its_total_is_zero() {
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (4.0, 8.0), (0.0, 0.0), &[]), "");
        assert_eq!(row(&g, 8).trim(), "");
        assert!(row(&g, 7).contains("RAM"));
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (0.0, 0.0), (100.0, 500.0), &[]), "");
        assert_eq!(row(&g, 7).trim(), "");
        let line = row(&g, 8);
        assert!(line.contains("DISK  ["), "{line}");
        assert!(line.contains("100.0 GB / 500.0 GB"), "{line}");
        assert!(line.contains("20.0%"), "{line}");
    }

    #[test]
    fn disk_without_used_space_shows_total_only_without_a_bar() {
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (4.0, 8.0), (0.0, 456.6), &[]), "");
        let line = row(&g, 8);
        assert!(line.contains("DISK"), "{line}");
        assert!(line.contains("456.6 GB total"), "{line}");
        assert!(!line.contains('[') && !line.contains('%') && !line.contains('█'), "{line}");
        // alineado con el inicio de la barra de RAM (x = 20 en 100 columnas) y con colores esperados
        assert_eq!(g.get(20, 8), Some(('D', WHITE, NO_COLOR)));
        assert_eq!(g.get(27, 8), Some(('4', CYAN, NO_COLOR)));
        // con espacio usado informado sigue habiendo barra
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (4.0, 8.0), (100.0, 500.0), &[]), "");
        assert!(row(&g, 8).contains("DISK  ["));
    }

    #[test]
    fn bars_start_at_x_1_on_narrow_grids() {
        let g = fed(30, 20, &data(0.0, 0.0, 0.0, (1.0, 2.0), (0.0, 0.0), &[]), "");
        assert_eq!(g.get(1, 7).unwrap().0, 'R');
        assert_eq!(g.get(0, 7).unwrap().0, ' ');
    }

    #[test]
    fn tenths_formatting_rounds_to_nearest() {
        // 0.05 GB -> 0.1 (al más cercano), 7.04 -> 7.0, 1023.96 -> 1024.0
        let g = fed(120, 40, &data(0.0, 0.0, 0.0, (0.05, 1023.96), (7.04, 100.0), &[]), "");
        let ram = row(&g, 7);
        assert!(ram.contains("0.1 GB / 1024.0 GB"), "{ram}");
        let disk = row(&g, 8);
        assert!(disk.contains("7.0 GB / 100.0 GB"), "{disk}");
        assert!(disk.contains("]   7.0%"), "{disk}");
        // porcentaje: mismo ancho que `{:>5.1}` (5 caracteres antes de '%')
        let g = fed(120, 40, &data(0.0, 0.0, 0.0, (1.0, 3.0), (0.0, 0.0), &[]), "");
        assert!(row(&g, 7).contains("]  33.3%  1.0 GB / 3.0 GB"), "{}", row(&g, 7));
        let g = fed(120, 40, &data(0.0, 0.0, 0.0, (2.0, 3.0), (0.0, 0.0), &[]), "");
        assert!(row(&g, 7).contains("]  66.7%"), "{}", row(&g, 7));
        let g = fed(120, 40, &data(0.0, 0.0, 0.0, (3.0, 3.0), (0.0, 0.0), &[]), "");
        assert!(row(&g, 7).contains("] 100.0%"), "{}", row(&g, 7));
        let g = fed(120, 40, &data(0.0, 0.0, 0.0, (0.0, 3.0), (0.0, 0.0), &[]), "");
        assert!(row(&g, 7).contains("]   0.0%"), "{}", row(&g, 7));
    }

    #[test]
    fn one_core_uses_single_column_with_cpu0_label() {
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (0.0, 0.0), (0.0, 0.0), &[0.5]), "");
        // x = (100-30)/2 = 35, y = 10
        let expected = format!("CPU0  [{}{}]  50.0%  ", "█".repeat(6), "-".repeat(6));
        assert_eq!(text_at(&g, 35, 10, expected.chars().count()), expected);
        assert_eq!(row(&g, 11).trim(), "");
    }

    #[test]
    fn four_cores_on_wide_grid_use_two_columns() {
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (0.0, 0.0), (0.0, 0.0), &[0.1, 0.2, 0.3, 0.4]), "");
        // left x = 50-30 = 20; right x = 50+3 = 53; half = 2
        assert_eq!(text_at(&g, 20, 10, 5), "CPU0 ");
        assert_eq!(text_at(&g, 20, 11, 5), "CPU1 ");
        assert_eq!(text_at(&g, 53, 10, 5), "CPU2 ");
        assert_eq!(text_at(&g, 53, 11, 5), "CPU3 ");
        assert_eq!(row(&g, 12).trim(), "");
    }

    #[test]
    fn twelve_cores_split_6_6_and_64_cores_are_capped_to_16() {
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (0.0, 0.0), (0.0, 0.0), &[0.9; 12]), "");
        assert_eq!(text_at(&g, 20, 15, 5), "CPU5 ");
        assert_eq!(text_at(&g, 53, 10, 5), "CPU6 ");
        assert_eq!(text_at(&g, 53, 15, 6), "CPU11 ");
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (0.0, 0.0), (0.0, 0.0), &[0.9; 64]), "");
        assert_eq!(text_at(&g, 20, 17, 5), "CPU7 ");
        assert_eq!(text_at(&g, 53, 17, 6), "CPU15 ");
        assert_eq!(text_at(&g, 20, 18, 3).trim(), "");
        assert!(!(0..40).any(|y| row(&g, y).contains("CPU16")));
    }

    #[test]
    fn narrow_grid_uses_one_column_of_at_most_six_cores() {
        let g = fed(70, 40, &data(0.0, 0.0, 0.0, (0.0, 0.0), (0.0, 0.0), &[0.2; 10]), "");
        // x = (70-30)/2 = 20
        for i in 0..6u16 {
            assert_eq!(text_at(&g, 20, 10 + i, 4), format!("CPU{i}"));
        }
        assert_eq!(text_at(&g, 20, 16, 3).trim(), "");
        // ancho 71 con 3 núcleos: una columna (menos de 4 núcleos)
        let g = fed(71, 40, &data(0.0, 0.0, 0.0, (0.0, 0.0), (0.0, 0.0), &[0.2; 3]), "");
        assert_eq!(text_at(&g, 20, 12, 4), "CPU2");
    }

    #[test]
    fn core_load_colors_the_percentage() {
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (0.0, 0.0), (0.0, 0.0), &[0.9, 0.7, 0.1]), "");
        // CPU0 en x=35: "CPU0  [" (7) + 12 -> ']' en 35+19
        assert_eq!(g.get(35 + 19, 10).unwrap().1, RED);
        assert_eq!(g.get(35 + 19, 11).unwrap().1, YELLOW);
        assert_eq!(g.get(35 + 19, 12).unwrap().1, GREEN);
    }

    #[test]
    fn loads_are_clamped_to_unit_range() {
        let g = fed(100, 40, &data(0.0, 0.0, 0.0, (0.0, 0.0), (0.0, 0.0), &[5.0, -1.0, f32::NAN]), "");
        assert!(text_at(&g, 35, 10, 30).contains("100.0%"));
        assert!(text_at(&g, 35, 11, 30).contains("  0.0%"));
        assert!(text_at(&g, 35, 12, 30).contains("  0.0%"));
    }

    #[test]
    fn missing_data_draws_zeros_without_panicking() {
        let mut a = DashboardAnimation::new();
        a.feed(&[]);
        a.feed(&[1.0, 2.0, 3.0]); // corto: se ignora
        let g = drawn(&mut a, 100, 40);
        assert!(clock_matches(&g, 31, [0, 0, 10, 0, 0, 10, 0, 0]));
    }

    #[test]
    fn corrupt_feed_never_panics_and_is_sanitized() {
        let mut a = DashboardAnimation::new();
        let bad = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, f32::NAN, 1e30, -5.0, 1e30, 0.5, f32::NAN];
        a.feed(&bad);
        let g = drawn(&mut a, 100, 40);
        assert!(clock_matches(&g, 31, [0, 0, 10, 0, 0, 10, 0, 0]));
        // N enorme pero solo 2 valores reales: se limita al slice
        a.feed(&[1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1e9, 0.5, 0.5]);
        let g = drawn(&mut a, 100, 40);
        assert_eq!(text_at(&g, 35, 10, 4), "CPU0");
        assert_eq!(text_at(&g, 35, 11, 4), "CPU1");
        assert!(!(0..40).any(|y| row(&g, y).contains("CPU2")));
        // N negativo -> sin núcleos
        a.feed(&[1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, -3.0, 0.5, 0.5]);
        assert_eq!(row(&drawn(&mut a, 100, 40), 10).trim(), "");
        // valores gigantes en GB no desbordan
        a.feed(&[0.0, 0.0, 0.0, 1e38, 1e38, f32::MAX, f32::MAX, 0.0]);
        let _ = drawn(&mut a, 100, 40);
    }

    #[test]
    fn short_feed_keeps_previous_state() {
        let mut a = DashboardAnimation::new();
        a.feed(&data(12.0, 34.0, 56.0, (0.0, 0.0), (0.0, 0.0), &[]));
        a.feed(&[1.0, 2.0]);
        assert!(clock_matches(&drawn(&mut a, 100, 40), 31, [1, 2, 10, 3, 4, 10, 5, 6]));
    }

    #[test]
    fn update_does_not_change_what_is_drawn() {
        let mut a = DashboardAnimation::new();
        a.feed(&data(1.0, 2.0, 3.0, (1.0, 2.0), (1.0, 2.0), &[0.3; 4]));
        let first = drawn(&mut a, 100, 40);
        for _ in 0..50 {
            a.update(100, 40);
        }
        let mut g = Grid::new(100, 40);
        a.draw(&mut g);
        assert_eq!(first.data(), g.data());
    }

    #[test]
    fn feed_text_caps_lines_and_chars() {
        let mut a = DashboardAnimation::new();
        let text: String = (0..20).map(|i| format!("L{i}: v")).collect::<Vec<_>>().join("\n");
        a.feed_text(&text);
        let g = drawn(&mut a, 100, 60);
        // sin núcleos: proc_y = 11; encabezado en 11, líneas 12..=23
        assert_eq!(text_at(&g, 1, 12, 5), "L0: v");
        assert_eq!(text_at(&g, 1, 23, 6), "L11: v");
        assert_eq!(row(&g, 24).trim(), "");
        // 80 caracteres por línea, por caracteres (no bytes)
        a.feed_text(&"é".repeat(200));
        let g = drawn(&mut a, 200, 60);
        assert_eq!(text_at(&g, 1, 12, 150).chars().filter(|&c| c == 'é').count(), 80);
        // vacío -> sin líneas
        a.feed_text("");
        assert_eq!(row(&drawn(&mut a, 100, 60), 12).trim(), "");
        a.feed_text("\n\n");
        assert_eq!(row(&drawn(&mut a, 100, 60), 12).trim(), "");
    }

    #[test]
    fn browser_panel_header_and_colored_lines() {
        let mut a = DashboardAnimation::new();
        a.feed_text("Chrome: 126.0\nCores: 8\nnolabel here");
        let g = drawn(&mut a, 100, 40);
        let head = "═ BROWSER (REPORTED BY THE BROWSER) ";
        let expected = format!("{head}{}", "═".repeat(85 - head.chars().count()));
        assert_eq!(text_at(&g, 1, 11, 85), expected);
        assert_eq!(g.get(1, 11).unwrap(), ('═', GRAY, NO_COLOR));
        assert_eq!(g.get(60, 11).unwrap().1, GRAY);
        assert_eq!(text_at(&g, 1, 12, 13), "Chrome: 126.0");
        assert_eq!(g.get(1, 12).unwrap().1, BLUE);
        assert_eq!(g.get(7, 12).unwrap().1, BLUE); // ':'
        assert_eq!(g.get(9, 12).unwrap().1, WHITE);
        assert_eq!(g.get(13, 12).unwrap().1, WHITE);
        assert_eq!(text_at(&g, 1, 13, 8), "Cores: 8");
        assert_eq!(text_at(&g, 1, 14, 12), "nolabel here");
        assert_eq!(g.get(1, 14).unwrap().1, WHITE);
        assert_eq!(g.get(12, 14).unwrap().1, WHITE);
    }

    #[test]
    fn browser_panel_moves_below_cpu_rows_and_needs_room() {
        let mut a = DashboardAnimation::new();
        a.feed(&data(0.0, 0.0, 0.0, (0.0, 0.0), (0.0, 0.0), &[0.1; 4]));
        a.feed_text("A: b");
        // ancho > 70: core_rows = 2, proc_y = 13; se dibuja si proc_y + 3 < alto
        let g = drawn(&mut a, 100, 17);
        assert!(row(&g, 13).contains("BROWSER"));
        assert_eq!(text_at(&g, 1, 14, 4), "A: b");
        let g = drawn(&mut a, 100, 16);
        assert!(!row(&g, 13).contains("BROWSER"));
        // una columna (ancho <= 70): 4 núcleos -> proc_y = 15
        let g = drawn(&mut a, 70, 19);
        assert!(row(&g, 15).contains("BROWSER"));
    }

    #[test]
    fn panel_lines_are_limited_to_the_available_rows() {
        let mut a = DashboardAnimation::new();
        a.feed_text("A: 1\nB: 2\nC: 3\nD: 4");
        // proc_y = 11, alto 15: filas 12..=14 -> A, B, C
        let g = drawn(&mut a, 100, 15);
        assert_eq!(text_at(&g, 1, 14, 4), "C: 3");
        assert!(!(0..15).any(|y| row(&g, y).contains("D: 4")));
    }

    #[test]
    fn extreme_and_tiny_sizes_never_panic() {
        let d = data(23.0, 59.0, 59.0, (8.0, 16.0), (100.0, 200.0), &[0.5; 64]);
        let text = "Chrome: 1\nOS: x\nCores: 64";
        for w in 0..=6u16 {
            for h in 0..=6u16 {
                let _ = fed(w, h, &d, text);
            }
        }
        for (w, h) in [(1024, 3), (3, 1024), (1, 1024), (1024, 1), (211, 58), (0, 0), (0, 40), (100, 0)] {
            let _ = fed(w, h, &d, text);
        }
    }

    #[test]
    fn drawing_is_deterministic() {
        let d = data(5.0, 6.0, 7.0, (3.0, 9.0), (1.0, 5.0), &[0.2, 0.4, 0.6, 0.8, 1.0]);
        let a = fed(120, 40, &d, "A: b\nC: d");
        let b = fed(120, 40, &d, "A: b\nC: d");
        assert_eq!(a.data(), b.data());
    }
}

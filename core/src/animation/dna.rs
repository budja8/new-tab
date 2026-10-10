//! Port de `refs/cwork/src/animation/dna.rs`.
//! Cambios: RNG propio, `Grid` en vez de `Buffer`, aritmética `i32` (sin
//! subdesbordes `u16` ni `gen_range` vacíos), fase de la hélice reducida a una
//! vuelta en `f64` (con `tick as f32` se perdía precisión tras horas), porcentaje
//! de compatibilidad en décimas enteras (sin formateo de flotantes en WASM),
//! cabecera sin la errata "DATABASE DATABASE" y composición hélice + log
//! centrada en rejillas anchas.

use super::Animation;
use crate::grid::{rgb, Grid, NO_COLOR};
use crate::rng::Rng;

const GREEN: u32 = rgb(92, 200, 35);
const YELLOW: u32 = rgb(255, 220, 50);
const RED: u32 = rgb(255, 85, 85);
const CYAN: u32 = rgb(0, 210, 230);
const MAGENTA: u32 = rgb(230, 90, 230);
const GRAY: u32 = rgb(120, 120, 120);
/// Verde y magenta atenuados: el lado lejano de cada hebra.
const DARK_GREEN: u32 = rgb(35, 85, 15);
const DARK_MAGENTA: u32 = rgb(90, 30, 90);

/// Rejilla "ancha" (con log): `width >= WIDE_MIN`, es decir `width > 55`.
const WIDE_MIN: i32 = 56;
const HELIX_CX: i32 = 20;
const HELIX_AMPLITUDE: f32 = 9.0;
const LOG_X: i32 = 42;
/// Columna izquierda de la hélice (`HELIX_CX - 9`) y ancho de la composición
/// hasta el borde derecho del log (`LOG_X + LOG_WIDTH - COMPOSITION_LEFT`).
const COMPOSITION_LEFT: i32 = 11;
const COMPOSITION_WIDTH: i32 = LOG_X + LOG_WIDTH - COMPOSITION_LEFT; // 101
const LOG_WIDTH: i32 = 70;
/// Exactamente `LOG_WIDTH` caracteres.
const HEADER: &str = "═ GENETIC SEQUENCING DATABASE ════════════════════════════════════════";

const LOG_EVERY_TICKS: u64 = 25;
const MUTATION_CHANCE: f32 = 0.05;
const PAIRS: [&str; 4] = ["A-T", "T-A", "C-G", "G-C"];

/// Fase de la hélice en `[0, TAU)`. Se reduce en `f64` antes de pasar a `f32`.
fn phase(tick: u64) -> f32 {
    ((tick as f64) * 0.05).rem_euclid(std::f64::consts::TAU) as f32
}

/// Máximo de líneas de log para una altura dada.
fn max_history(height: u16) -> usize {
    if height > 6 {
        height as usize - 6
    } else {
        5
    }
}

/// `(cx, amplitud, columna del log)` según el ancho; sin log en rejillas estrechas.
fn layout(width: i32) -> (i32, f32, Option<i32>) {
    if width >= WIDE_MIN {
        let shift = ((width - COMPOSITION_WIDTH) / 2 - COMPOSITION_LEFT).max(0);
        (HELIX_CX + shift, HELIX_AMPLITUDE, Some(LOG_X + shift))
    } else {
        (width / 2, (width as f32 / 2.0 - 3.0).clamp(3.0, 10.0), None)
    }
}

pub struct DnaAnimation {
    tick: u64,
    logs: Vec<(String, u32)>,
    width: u16,
    height: u16,
    rng: Rng,
}

impl DnaAnimation {
    pub fn new(width: u16, height: u16, seed: u32) -> Self {
        Self { tick: 0, logs: Vec::new(), width, height, rng: Rng::new(seed as u64) }
    }

    fn add_log(&mut self) {
        let seq_id = self.rng.range_incl_i32(1000, 9998);
        let pair = PAIRS[self.rng.range_i32(0, PAIRS.len() as i32) as usize];
        let entry = if self.rng.chance(MUTATION_CHANCE) {
            (format!("GENE #{seq_id}: [{pair}] -> *** MUTATION DETECTED: CLASS II (REPLACEMENT) ***"), RED)
        } else {
            // Décimas enteras: 96.2..=99.8 sin formatear flotantes.
            let t = self.rng.range_incl_i32(962, 998);
            let color = if t > 990 { CYAN } else { GREEN };
            (format!("GENE #{seq_id}: [{pair}] -> Match: {}.{}% compatibility", t / 10, t % 10), color)
        };
        self.logs.push(entry);
        let max = max_history(self.height);
        if self.logs.len() > max {
            let excess = self.logs.len() - max;
            self.logs.drain(..excess);
        }
    }
}

impl Animation for DnaAnimation {
    fn update(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.tick = self.tick.saturating_add(1);
        if self.tick % LOG_EVERY_TICKS == 0 {
            self.add_log();
        }
    }

    fn draw(&mut self, grid: &mut Grid) {
        grid.clear();
        let height = self.height as i32;
        let (cx, amplitude, log_x) = layout(self.width as i32);
        let phase = phase(self.tick);

        for y in 2..(height - 2) {
            let angle = y as f32 * 0.22 + phase;
            let x1_off = angle.sin() * amplitude;
            let x1 = (cx as f32 + x1_off).round() as i32;
            let x2 = (cx as f32 - x1_off).round() as i32;
            let z1 = angle.cos();

            if y % 3 == 0 {
                let color = if (y / 3) % 2 == 0 { CYAN } else { YELLOW };
                for bx in (x1.min(x2) + 1)..x1.max(x2) {
                    grid.set(bx, y, '─', color, NO_COLOR);
                }
            }
            if z1 > 0.0 {
                grid.set(x1, y, '●', GREEN, NO_COLOR);
                grid.set(x2, y, 'o', DARK_MAGENTA, NO_COLOR);
            } else {
                grid.set(x1, y, 'o', DARK_GREEN, NO_COLOR);
                grid.set(x2, y, '●', MAGENTA, NO_COLOR);
            }
        }

        if let Some(lx) = log_x {
            grid.set_str(lx, 1, HEADER, GRAY, NO_COLOR);
            for (i, (text, color)) in self.logs.iter().enumerate() {
                let row = 3 + i as i32;
                if row >= height {
                    break;
                }
                grid.set_str(lx, row, text, *color, NO_COLOR);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAIRS: [&str; 4] = ["A-T", "T-A", "C-G", "G-C"];

    /// Celdas de nodo de hélice en una fila: `(x, ch, fg)`. Los colores atenuados
    /// y `●` solo los usan los nodos, así que el texto del log no se confunde.
    fn nodes(grid: &Grid, y: u16) -> Vec<(i32, char, u32)> {
        let mut v = Vec::new();
        for x in 0..grid.width() {
            let (ch, fg, _) = grid.get(x, y).unwrap();
            if ch == '●' || (ch == 'o' && (fg == DARK_GREEN || fg == DARK_MAGENTA)) {
                v.push((x as i32, ch, fg));
            }
        }
        v
    }

    fn row_text(grid: &Grid, x: i32, y: u16, n: usize) -> String {
        (0..n).filter_map(|i| grid.get((x + i as i32) as u16, y)).map(|c| c.0).collect()
    }

    fn run(w: u16, h: u16, seed: u32, updates: usize) -> (DnaAnimation, Grid) {
        let mut d = DnaAnimation::new(w, h, seed);
        let mut grid = Grid::new(w, h);
        for _ in 0..updates {
            d.update(w, h);
        }
        d.draw(&mut grid);
        (d, grid)
    }

    #[test]
    fn helix_nodes_are_mirror_images_within_amplitude() {
        let (w, h) = (80u16, 24u16);
        for ticks in [1usize, 7, 40, 133, 600] {
            let (_, grid) = run(w, h, 3, ticks);
            for y in 2..(h - 2) {
                let n = nodes(&grid, y);
                assert!(!n.is_empty() && n.len() <= 2, "fila {y}: {n:?}");
                let cx = HELIX_CX;
                for &(x, _, _) in &n {
                    assert!((x - cx).abs() <= 9, "fuera de amplitud en fila {y}: {x}");
                }
                if n.len() == 2 {
                    let (a, b) = ((n[0].0 - cx).abs(), (n[1].0 - cx).abs());
                    assert!((a - b).abs() <= 1, "fila {y} no simétrica: {n:?}");
                } else {
                    assert!((n[0].0 - cx).abs() <= 1, "nodos solapados lejos del eje: {n:?}");
                }
            }
            for y in [0u16, 1, h - 2, h - 1] {
                assert!(nodes(&grid, y).is_empty(), "nodo fuera de 2..h-2 en fila {y}");
            }
        }
    }

    #[test]
    fn bridges_only_on_every_third_row_between_nodes_and_alternate_colors() {
        let (w, h) = (80u16, 24u16);
        let (_, grid) = run(w, h, 5, 77);
        for y in 0..h {
            let bridges: Vec<(i32, u32)> = (0..w)
                .filter_map(|x| {
                    let (ch, fg, _) = grid.get(x, y).unwrap();
                    (ch == '─').then_some((x as i32, fg))
                })
                .collect();
            let is_helix_row = y >= 2 && y < h - 2;
            if !is_helix_row || y % 3 != 0 {
                assert!(bridges.is_empty(), "puente en fila {y}");
                continue;
            }
            let n = nodes(&grid, y);
            let lo = n.iter().map(|c| c.0).min().unwrap();
            let hi = n.iter().map(|c| c.0).max().unwrap();
            assert_eq!(bridges.len() as i32, (hi - lo - 1).max(0), "fila {y}");
            let want = if (y / 3) % 2 == 0 { CYAN } else { YELLOW };
            for (x, fg) in bridges {
                assert!(x > lo && x < hi, "puente fuera de los nodos en fila {y}");
                assert_eq!(fg, want, "color de puente en fila {y}");
            }
        }
    }

    #[test]
    fn depth_decides_glyph_and_color_of_each_strand() {
        let (w, h) = (80u16, 24u16);
        for ticks in [3usize, 90, 250] {
            let (d, grid) = run(w, h, 9, ticks);
            let phase = phase(d.tick);
            let mut checked = 0;
            for y in 2..(h as i32 - 2) {
                let angle = y as f32 * 0.22 + phase;
                let off = angle.sin() * HELIX_AMPLITUDE;
                if off.abs() < 1.5 {
                    continue; // nodos pegados: uno tapa al otro
                }
                let x1 = (HELIX_CX as f32 + off).round() as u16;
                let x2 = (HELIX_CX as f32 - off).round() as u16;
                let c1 = grid.get(x1, y as u16).unwrap();
                let c2 = grid.get(x2, y as u16).unwrap();
                if angle.cos() > 0.0 {
                    assert_eq!((c1.0, c1.1), ('●', GREEN), "fila {y}");
                    assert_eq!((c2.0, c2.1), ('o', DARK_MAGENTA), "fila {y}");
                } else {
                    assert_eq!((c1.0, c1.1), ('o', DARK_GREEN), "fila {y}");
                    assert_eq!((c2.0, c2.1), ('●', MAGENTA), "fila {y}");
                }
                checked += 1;
            }
            assert!(checked > 8);
        }
    }

    #[test]
    fn logs_grow_every_25_ticks_and_cap_at_height_minus_six() {
        let (w, h) = (80u16, 24u16);
        let mut d = DnaAnimation::new(w, h, 1);
        let mut grid = Grid::new(w, h);
        for t in 1..=24 {
            d.update(w, h);
            assert!(d.logs.is_empty(), "log antes del tick 25 (tick {t})");
        }
        d.update(w, h);
        for k in 1..=25usize {
            if k > 1 {
                for _ in 0..25 {
                    d.update(w, h);
                }
            }
            assert_eq!(d.logs.len(), k.min(18), "k={k}");
            d.draw(&mut grid);
            for (i, (text, color)) in d.logs.iter().enumerate() {
                let y = 3 + i as u16;
                let n = (w as usize - 42).min(text.chars().count());
                assert_eq!(row_text(&grid, 42, y, n), text.chars().take(n).collect::<String>());
                assert_eq!(grid.get(42, y).unwrap().1, *color);
            }
            // La fila siguiente a la última línea no tiene texto de log.
            assert_eq!(grid.get(42, 3 + d.logs.len() as u16).unwrap().0, ' ');
        }
        // Altura <= 6: el tope es 5; con 7 es 1.
        for hh in [1u16, 5, 6] {
            let (d, _) = run(80, hh, 2, 25 * 12);
            assert_eq!(d.logs.len(), 5, "altura {hh}");
        }
        let (d, _) = run(80, 7, 2, 25 * 12);
        assert_eq!(d.logs.len(), 1);
    }

    #[test]
    fn log_lines_have_the_documented_format_and_mutations_happen() {
        let mut d = DnaAnimation::new(80, 24, 42);
        let mut seen = Vec::new();
        for _ in 0..25 * 400 {
            d.update(80, 24);
            if d.tick % 25 == 0 {
                seen.push(d.logs.last().unwrap().clone());
            }
        }
        let mut mutations = 0;
        for (text, color) in &seen {
            let rest = text.strip_prefix("GENE #").expect(text);
            let (id, rest) = rest.split_at(4);
            assert!(id.bytes().all(|b| b.is_ascii_digit()), "{text}");
            let id: i32 = id.parse().unwrap();
            assert!((1000..=9998).contains(&id), "{text}");
            let rest = rest.strip_prefix(": [").expect(text);
            let (pair, rest) = rest.split_at(3);
            assert!(PAIRS.contains(&pair), "{text}");
            let rest = rest.strip_prefix("] -> ").expect(text);
            if rest == "*** MUTATION DETECTED: CLASS II (REPLACEMENT) ***" {
                assert_eq!(*color, RED);
                mutations += 1;
                continue;
            }
            let rest = rest.strip_prefix("Match: ").expect(text);
            let pct = rest.strip_suffix("% compatibility").expect(text);
            let b = pct.as_bytes();
            assert!(
                b.len() == 4 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'.' && b[3].is_ascii_digit(),
                "{text}"
            );
            let tenths = (b[0] - b'0') as i32 * 100 + (b[1] - b'0') as i32 * 10 + (b[3] - b'0') as i32;
            assert!((962..=998).contains(&tenths), "{text}");
            assert_eq!(*color, if tenths > 990 { CYAN } else { GREEN }, "{text}");
        }
        assert!(mutations >= 1, "ninguna mutación en {} líneas", seen.len());
        assert!(mutations < seen.len() / 4, "demasiadas mutaciones: {mutations}");
    }

    #[test]
    fn layout_centers_the_composition_and_log_is_only_for_wide_grids() {
        // Ancho 211: shift = (211 - 101) / 2 - 11 = 44.
        let (_, grid) = run(211, 58, 4, 130);
        let shift = 44;
        let cx = HELIX_CX + shift;
        for y in 2..56u16 {
            let n = nodes(&grid, y);
            assert!(!n.is_empty());
            for &(x, _, _) in &n {
                assert!((x - cx).abs() <= 9);
            }
            if n.len() == 2 {
                assert!(((n[0].0 - cx).abs() - (n[1].0 - cx).abs()).abs() <= 1);
            }
        }
        assert_eq!(row_text(&grid, LOG_X + shift, 1, 70), HEADER);
        assert_eq!(grid.get((LOG_X + shift - 1) as u16, 1).unwrap().0, ' ');
        assert_eq!(grid.get((LOG_X + shift) as u16, 1).unwrap().1, GRAY);
        // Composición de 101 columnas centrada: 55 a la izquierda, 211 - 156 = 55 a la derecha.
        assert_eq!(HELIX_CX - 9 + shift, 55);
        assert_eq!(211 - (LOG_X + shift + LOG_WIDTH), 55);

        // Ancho 101: sin desplazamiento.
        let (_, grid) = run(101, 24, 4, 60);
        assert_eq!(row_text(&grid, LOG_X, 1, 101 - 42), HEADER.chars().take(101 - 42).collect::<String>()); // el log se recorta a 101 columnas
        assert_eq!(grid.get((LOG_X - 1) as u16, 1).unwrap().0, ' ');
        assert!(!nodes(&grid, 5).is_empty());

        // Estrecho (<= 55): sin log y hélice centrada en width/2.
        for w in [10u16, 40, 55] {
            let (_, grid) = run(w, 24, 4, 200);
            let (cx, amp) = (w as i32 / 2, (w as f32 / 2.0 - 3.0).clamp(3.0, 10.0));
            for y in 0..24u16 {
                for x in 0..w {
                    let ch = grid.get(x, y).unwrap().0;
                    assert!(matches!(ch, ' ' | '●' | 'o' | '─'), "texto de log en ancho {w}: {ch:?}");
                }
            }
            for y in 2..22u16 {
                let n = nodes(&grid, y);
                assert!(!n.is_empty());
                for &(x, _, _) in &n {
                    assert!(((x - cx).abs() as f32) <= amp + 1.0, "ancho {w} fila {y}");
                }
                if n.len() == 2 {
                    assert!(((n[0].0 - cx).abs() - (n[1].0 - cx).abs()).abs() <= 1);
                }
            }
        }
    }

    #[test]
    fn header_is_70_columns_with_a_single_database_word() {
        assert_eq!(LOG_WIDTH, 70);
        assert_eq!(HEADER.chars().count(), 70);
        assert_eq!(HEADER.matches("DATABASE").count(), 1);
        assert!(HEADER.starts_with("═ GENETIC SEQUENCING DATABASE ═"));
        let longest = "GENE #1234: [A-T] -> *** MUTATION DETECTED: CLASS II (REPLACEMENT) ***";
        assert_eq!(longest.chars().count(), LOG_WIDTH as usize);
    }

    #[test]
    fn degenerate_sizes_do_not_panic() {
        for (w, h) in [(0, 0), (0, 5), (5, 0), (1, 1), (4, 4), (3, 10), (10, 3), (55, 24), (56, 24), (101, 24)] {
            let mut d = DnaAnimation::new(w, h, 11);
            let mut grid = Grid::new(w, h);
            for _ in 0..2_000 {
                d.update(w, h);
                d.draw(&mut grid);
            }
        }
    }

    #[test]
    fn same_seed_same_frames_different_seed_differs() {
        let frame = |seed: u32| {
            let (d, grid) = run(80, 24, seed, 500);
            (grid.data().to_vec(), d.logs.clone())
        };
        assert_eq!(frame(4), frame(4));
        assert_ne!(frame(4).1, frame(5).1);
    }

    #[test]
    fn phase_stays_finite_and_in_one_turn_for_huge_ticks() {
        for tick in [0u64, 1, 125, 50_000_000_000, 1u64 << 60, u64::MAX] {
            let p = phase(tick);
            assert!(p.is_finite() && p >= 0.0 && (p as f64) <= std::f64::consts::TAU + 1e-6, "tick {tick}: {p}");
        }
        assert!(phase(50_000_000_000) < std::f32::consts::TAU);
    }

    #[test]
    fn update_picks_up_a_resize() {
        let mut d = DnaAnimation::new(80, 24, 1);
        d.update(40, 10);
        assert_eq!((d.width, d.height), (40, 10));
    }
}

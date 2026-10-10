//! Cuadrícula de celdas compartida con JS.
//! Cada celda ocupa `CELL_STRIDE` u32 consecutivos: `[carácter, fg, bg]`.
//! Un color `0` (`NO_COLOR`) equivale a `Color::Reset` de crossterm: sin color.

pub const CELL_STRIDE: usize = 3;
pub const NO_COLOR: u32 = 0;

/// Color opaco `0xFFRRGGBB`. El byte alto distingue cualquier color de `NO_COLOR`.
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub struct Grid {
    width: u16,
    height: u16,
    data: Vec<u32>,
}

impl Grid {
    pub fn new(width: u16, height: u16) -> Self {
        let mut grid = Self {
            width,
            height,
            data: vec![0; width as usize * height as usize * CELL_STRIDE],
        };
        grid.clear();
        grid
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.data.resize(width as usize * height as usize * CELL_STRIDE, 0);
        self.clear();
    }

    pub fn clear(&mut self) {
        self.fill_bg(NO_COLOR);
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn data(&self) -> &[u32] {
        &self.data
    }

    pub fn get(&self, x: u16, y: u16) -> Option<(char, u32, u32)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * CELL_STRIDE;
        Some((char::from_u32(self.data[i])?, self.data[i + 1], self.data[i + 2]))
    }

    /// Escribe una celda. Las coordenadas fuera de la cuadrícula se ignoran
    /// (cwork usaba `u16` y desbordaba con anchos pequeños).
    pub fn set(&mut self, x: i32, y: i32, ch: char, fg: u32, bg: u32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * CELL_STRIDE;
        self.data[i] = ch as u32;
        self.data[i + 1] = fg;
        self.data[i + 2] = bg;
    }

    pub fn set_str(&mut self, x: i32, y: i32, s: &str, fg: u32, bg: u32) {
        for (i, ch) in s.chars().enumerate() {
            self.set(x.saturating_add(i as i32), y, ch, fg, bg);
        }
    }

    /// Deja todas las celdas en blanco con el fondo dado.
    pub fn fill_bg(&mut self, bg: u32) {
        for cell in self.data.chunks_exact_mut(CELL_STRIDE) {
            cell[0] = ' ' as u32;
            cell[1] = NO_COLOR;
            cell[2] = bg;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_packs_with_opaque_alpha_marker() {
        assert_eq!(rgb(1, 2, 3), 0xFF01_0203);
        assert_ne!(rgb(0, 0, 0), NO_COLOR);
    }

    #[test]
    fn new_grid_is_all_blank_cells() {
        let g = Grid::new(4, 3);
        assert_eq!(g.data().len(), 4 * 3 * CELL_STRIDE);
        assert_eq!(g.get(0, 0), Some((' ', NO_COLOR, NO_COLOR)));
        assert_eq!(g.get(3, 2), Some((' ', NO_COLOR, NO_COLOR)));
        assert_eq!(g.get(4, 0), None);
    }

    #[test]
    fn set_writes_one_cell() {
        let mut g = Grid::new(4, 3);
        g.set(2, 1, 'x', rgb(9, 8, 7), rgb(1, 1, 1));
        assert_eq!(g.get(2, 1), Some(('x', rgb(9, 8, 7), rgb(1, 1, 1))));
        assert_eq!(g.get(1, 1), Some((' ', NO_COLOR, NO_COLOR)));
    }

    #[test]
    fn set_ignores_out_of_range_coordinates() {
        let mut g = Grid::new(2, 2);
        let before = g.data().to_vec();
        g.set(-1, 0, 'x', NO_COLOR, NO_COLOR);
        g.set(0, -1, 'x', NO_COLOR, NO_COLOR);
        g.set(2, 0, 'x', NO_COLOR, NO_COLOR);
        g.set(0, 2, 'x', NO_COLOR, NO_COLOR);
        g.set(i32::MAX, i32::MIN, 'x', NO_COLOR, NO_COLOR);
        assert_eq!(g.data(), &before[..]);
    }

    #[test]
    fn set_str_clips_on_both_sides() {
        let mut g = Grid::new(4, 1);
        g.set_str(2, 0, "abcd", NO_COLOR, NO_COLOR);
        assert_eq!(g.get(2, 0).unwrap().0, 'a');
        assert_eq!(g.get(3, 0).unwrap().0, 'b');
        let mut g = Grid::new(4, 1);
        g.set_str(-2, 0, "abcd", NO_COLOR, NO_COLOR);
        assert_eq!(g.get(0, 0).unwrap().0, 'c');
        assert_eq!(g.get(1, 0).unwrap().0, 'd');
    }

    #[test]
    fn fill_bg_paints_every_cell() {
        let mut g = Grid::new(3, 2);
        g.set(1, 1, 'x', rgb(1, 1, 1), NO_COLOR);
        g.fill_bg(rgb(5, 5, 5));
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(g.get(x, y), Some((' ', NO_COLOR, rgb(5, 5, 5))));
            }
        }
    }

    #[test]
    fn resize_changes_length_and_blanks() {
        let mut g = Grid::new(2, 2);
        g.set(0, 0, 'x', rgb(1, 1, 1), NO_COLOR);
        g.resize(5, 4);
        assert_eq!(g.data().len(), 5 * 4 * CELL_STRIDE);
        assert_eq!((g.width(), g.height()), (5, 4));
        assert_eq!(g.get(0, 0), Some((' ', NO_COLOR, NO_COLOR)));
    }

    #[test]
    fn zero_sized_grid_is_safe() {
        let mut g = Grid::new(0, 0);
        assert!(g.data().is_empty());
        g.set(0, 0, 'x', NO_COLOR, NO_COLOR);
        g.set_str(0, 0, "abc", NO_COLOR, NO_COLOR);
        g.fill_bg(rgb(1, 1, 1));
        g.clear();
        g.resize(0, 5);
        assert!(g.data().is_empty());
    }
}

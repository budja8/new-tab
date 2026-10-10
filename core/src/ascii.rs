//! Conversión imagen → ASCII (lógica de `refs/rust-ascii/src/bin/ascii.rs`).
//! JS reduce la imagen (`createImageBitmap` + `OffscreenCanvas`) y entrega RGBA ya
//! al tamaño de la cuadrícula; Rust solo mapea brillo → carácter. Así se evita la
//! crate `image`, que no cabría en el tope de 150 KB del `.wasm`.

use crate::grid::rgb;

/// De más oscuro a más brillante (el fondo de la página es oscuro).
pub const RAMP: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

/// Bytes por celda empaquetada: `[índice en RAMP, r, g, b]`.
pub const PACKED_STRIDE: usize = 4;

/// Corrección de aspecto: una celda de texto es ~1.8 veces más alta que ancha.
const CELL_ASPECT: f32 = 0.55;

/// Tamaño de la cuadrícula ASCII que conserva la proporción de la imagen sin
/// pasar de `max_w × max_h`. Siempre devuelve al menos `(1, 1)`.
pub fn target_dims(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || max_w == 0 || max_h == 0 {
        return (1, 1);
    }
    let ratio = src_h as f32 / src_w as f32 * CELL_ASPECT;
    let mut w = max_w;
    let mut h = (w as f32 * ratio).round() as u32;
    if h > max_h {
        h = max_h;
        w = (h as f32 / ratio).round() as u32;
    }
    (w.clamp(1, max_w), h.clamp(1, max_h))
}

/// RGBA (`width * height * 4` bytes) → celdas empaquetadas. Devuelve un vector
/// vacío si las dimensiones no cuadran con el buffer. La transparencia se
/// compone sobre negro. `invert` invierte la rampa de caracteres.
pub fn image_to_packed(rgba: &[u8], width: u32, height: u32, invert: bool) -> Vec<u8> {
    let cells = match (width as usize).checked_mul(height as usize) {
        Some(c) if c > 0 => c,
        _ => return Vec::new(),
    };
    if cells.checked_mul(4) != Some(rgba.len()) {
        return Vec::new();
    }

    let last = RAMP.len() - 1;
    let mut out = Vec::with_capacity(cells * PACKED_STRIDE);
    for px in rgba.chunks_exact(4) {
        let a = px[3] as u32;
        let r = (px[0] as u32 * a / 255) as u8;
        let g = (px[1] as u32 * a / 255) as u8;
        let b = (px[2] as u32 * a / 255) as u8;

        // Brillo percibido (pesos BT.709), igual que rust-ascii.
        let brightness = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
        let mut idx = ((brightness / 255.0) * last as f32).round() as usize;
        idx = idx.min(last);
        if invert {
            idx = last - idx;
        }
        out.extend_from_slice(&[idx as u8, r, g, b]);
    }
    out
}

/// Carácter y color (`rgb`) de la celda `index` de un buffer empaquetado.
pub fn packed_glyph(packed: &[u8], index: usize) -> Option<(char, u32)> {
    let base = index.checked_mul(PACKED_STRIDE)?;
    let end = base.checked_add(PACKED_STRIDE)?;
    let cell = packed.get(base..end)?;
    let ch = *RAMP.get(cell[0] as usize)?;
    Some((ch, rgb(cell[1], cell[2], cell[3])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::rgb;

    #[test]
    fn wide_image_fills_the_width() {
        // 1000×500 → ratio 0.5 * 0.55 = 0.275 → 200 × 55
        assert_eq!(target_dims(1000, 500, 200, 100), (200, 55));
    }

    #[test]
    fn tall_image_is_limited_by_height() {
        // 500×1000 → ratio 1.1 → alto 220 > 100 → 100 × 91
        assert_eq!(target_dims(500, 1000, 200, 100), (91, 100));
    }

    #[test]
    fn extreme_aspect_ratios_stay_valid() {
        assert_eq!(target_dims(10_000, 1, 200, 100), (200, 1));
        let (w, h) = target_dims(1, 10_000, 200, 100);
        assert_eq!(h, 100);
        assert!(w >= 1 && w <= 200);
    }

    #[test]
    fn zero_or_tiny_inputs_give_one_cell() {
        assert_eq!(target_dims(0, 100, 200, 100), (1, 1));
        assert_eq!(target_dims(100, 0, 200, 100), (1, 1));
        assert_eq!(target_dims(100, 100, 0, 100), (1, 1));
        // Cuadrada: al ancho máximo le tocarían 110 filas (> 100), así que manda el alto.
        assert_eq!(target_dims(1, 1, 200, 100), (182, 100));
    }

    #[test]
    fn white_maps_to_densest_and_black_to_blank() {
        let rgba = [255, 255, 255, 255, 0, 0, 0, 255];
        let packed = image_to_packed(&rgba, 2, 1, false);
        assert_eq!(packed, vec![9, 255, 255, 255, 0, 0, 0, 0]);
        assert_eq!(packed_glyph(&packed, 0), Some(('@', rgb(255, 255, 255))));
        assert_eq!(packed_glyph(&packed, 1), Some((' ', rgb(0, 0, 0))));
    }

    #[test]
    fn invert_flips_the_ramp_but_keeps_the_color() {
        let rgba = [255, 255, 255, 255];
        let packed = image_to_packed(&rgba, 1, 1, true);
        assert_eq!(packed, vec![0, 255, 255, 255]);
    }

    #[test]
    fn fully_transparent_pixels_become_blank_black() {
        let rgba = [255, 255, 255, 0];
        let packed = image_to_packed(&rgba, 1, 1, false);
        assert_eq!(packed, vec![0, 0, 0, 0]);
    }

    #[test]
    fn semi_transparent_pixels_are_composited_over_black() {
        let rgba = [200, 100, 50, 128];
        let packed = image_to_packed(&rgba, 1, 1, false);
        assert_eq!(&packed[1..], &[100, 50, 25]);
    }

    #[test]
    fn mismatched_buffer_length_returns_empty() {
        assert!(image_to_packed(&[0, 0, 0, 255], 2, 1, false).is_empty());
        assert!(image_to_packed(&[], 0, 0, false).is_empty());
        assert!(image_to_packed(&[0; 8], 1, 1, false).is_empty());
    }

    #[test]
    fn packed_glyph_rejects_bad_indices_and_corrupt_ramp_values() {
        let packed = vec![9, 1, 2, 3];
        assert!(packed_glyph(&packed, 1).is_none());
        assert!(packed_glyph(&packed, usize::MAX).is_none());
        let corrupt = vec![200, 1, 2, 3];
        assert!(packed_glyph(&corrupt, 0).is_none());
    }

    #[test]
    fn max_grid_fits_in_the_storage_budget() {
        // Spec: imagen guardada ≤ 200 KB con cuadrícula máxima 200×100.
        assert!(200 * 100 * PACKED_STRIDE <= 200 * 1024);
    }
}

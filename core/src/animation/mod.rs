pub mod compile_sim;
pub mod dashboard;
pub mod dna;
pub mod fire;
pub mod pacman;
pub mod pacman_install;
pub mod pipes;
pub mod rain;
pub mod starfield;
pub mod synthwave;

use crate::grid::Grid;

pub trait Animation {
    /// Avanza el estado un frame. Recibe el tamaño actual y se adapta si cambió.
    fn update(&mut self, width: u16, height: u16);

    /// Dibuja el estado actual en la cuadrícula.
    fn draw(&mut self, grid: &mut Grid);

    /// Informa del FPS objetivo (para animaciones que miden tiempo en frames).
    fn set_fps(&mut self, _fps: u32) {}

    /// Datos numéricos reportados por el navegador (la animación decide su layout).
    fn feed(&mut self, _data: &[f32]) {}

    /// Texto reportado por el navegador (líneas "Etiqueta: valor").
    fn feed_text(&mut self, _text: &str) {}
}

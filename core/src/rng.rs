//! Generador pseudoaleatorio xorshift64* sembrable.
//! Sustituye a `rand::thread_rng()` de cwork: no añade dependencias a WASM y
//! con semilla fija las animaciones son reproducibles en pruebas.

pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed ^ 0x9E37_79B9_7F4A_7C15 }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        if x == 0 {
            x = 0x9E37_79B9_7F4A_7C15;
        }
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// Valor en `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    /// Entero en `[lo, hi)`. Si el rango está vacío devuelve `lo`.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u32() % (hi - lo) as u32) as i32
    }

    /// Entero en `[lo, hi]`.
    pub fn range_incl_i32(&mut self, lo: i32, hi: i32) -> i32 {
        self.range_i32(lo, hi.saturating_add(1))
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..32).filter(|_| a.next_u32() == b.next_u32()).count();
        assert!(same < 4);
    }

    #[test]
    fn seed_zero_still_produces_values() {
        let mut r = Rng::new(0);
        assert!((0..16).any(|_| r.next_u32() != 0));
    }

    #[test]
    fn unit_is_in_half_open_interval() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
    }

    #[test]
    fn range_f32_stays_inside_bounds() {
        let mut r = Rng::new(9);
        for _ in 0..10_000 {
            let v = r.range_f32(-500.0, 500.0);
            assert!((-500.0..500.0).contains(&v));
        }
    }

    #[test]
    fn range_i32_is_half_open_and_empty_range_returns_lo() {
        let mut r = Rng::new(3);
        for _ in 0..1_000 {
            let v = r.range_i32(2, 5);
            assert!((2..5).contains(&v));
        }
        // Rangos vacíos o invertidos no deben hacer panic (rand::gen_range sí lo hace).
        assert_eq!(r.range_i32(5, 5), 5);
        assert_eq!(r.range_i32(5, -3), 5);
    }

    #[test]
    fn range_incl_i32_reaches_both_ends() {
        let mut r = Rng::new(11);
        let mut seen = [false; 3];
        for _ in 0..1_000 {
            seen[r.range_incl_i32(0, 2) as usize] = true;
        }
        assert_eq!(seen, [true, true, true]);
    }

    #[test]
    fn chance_extremes() {
        let mut r = Rng::new(5);
        assert!((0..100).all(|_| !r.chance(0.0)));
        assert!((0..100).all(|_| r.chance(1.0)));
    }
}

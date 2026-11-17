//! El mismo generador de números aleatorios que `java.util.Random`, que es el que usa RARS
//! en las syscalls 40-42. Así, con la misma semilla salen los mismos números que en RARS.

const MULT: u64 = 0x5_DEEC_E66D;
const MASK: u64 = (1 << 48) - 1;

#[derive(Clone, Debug)]
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        JavaRandom { seed: (seed as u64 ^ MULT) & MASK }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(MULT).wrapping_add(0xB)) & MASK;
        (self.seed >> (48 - bits)) as u32 as i32
    }

    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// Número en `[0, bound)`. `bound` debe ser positivo.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
            return ((bound as i64 * r as i64) >> 31) as i32;
        }
        let mut u = r;
        loop {
            r = u % bound;
            if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                return r;
            }
            u = self.next(31);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn igual_que_java() {
        // new java.util.Random(42).nextInt() == -1170105035
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_int(), -1170105035);
        // new java.util.Random(42).nextInt(10) == 0, y luego 3
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_int_bound(10), 0);
        assert_eq!(r.next_int_bound(10), 3);
        assert_eq!(r.next_int_bound(7), 6);
        assert_eq!(r.next_int_bound(64), 3);
        let mut r = JavaRandom::new(-5);
        assert_eq!(r.next_int_bound(1000), 411);
        assert_eq!(r.next_int(), 1560933443);
    }
}

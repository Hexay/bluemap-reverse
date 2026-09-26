//! `java.util.Random` (Minecraft's `LegacyRandomSource`): a 48-bit LCG.

pub const MULT: u64 = 0x5_DEEC_E66D;
pub const ADD: u64 = 0xB;
pub const MASK: u64 = (1 << 48) - 1;
/// MULT⁻¹ mod 2^48
pub const MULT_INV: u64 = 0xDFE0_5BCB_1365;

#[derive(Clone, Copy, Debug)]
pub struct JavaRandom {
    state: u64,
}

impl JavaRandom {
    /// `new Random(seed)`: scrambles the seed.
    pub fn new(seed: i64) -> Self {
        Self { state: (seed as u64 ^ MULT) & MASK }
    }

    pub fn next(&mut self, bits: u32) -> i32 {
        self.state = step(self.state);
        (self.state >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if (bound as u32).is_power_of_two() {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            // Java's rejection of the biased tail: the sum overflows i32
            if bits.wrapping_sub(val).wrapping_add(bound - 1) >= 0 {
                return val;
            }
        }
    }

    pub fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32).wrapping_add(self.next(32) as i64)
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }
}

pub fn step(state: u64) -> u64 {
    state.wrapping_mul(MULT).wrapping_add(ADD) & MASK
}

pub fn step_back(state: u64) -> u64 {
    state.wrapping_sub(ADD).wrapping_mul(MULT_INV) & MASK
}

/// Java `String.hashCode`, which is what a non-numeric text seed becomes.
pub fn string_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java() {
        // values from OpenJDK: new Random(0).nextInt(), new Random(0).nextLong(), new Random(42).nextInt(10)
        assert_eq!(JavaRandom::new(0).next(32), -1155484576);
        assert_eq!(JavaRandom::new(0).next_long(), -4962768465676381896);
        assert_eq!(JavaRandom::new(42).next_int(10), 0);
        assert_eq!(string_hash("hello"), 99162322);
    }

    #[test]
    fn inverse_multiplier() {
        assert_eq!(MULT.wrapping_mul(MULT_INV) & MASK, 1);
        assert_eq!(step_back(step(123456789)), 123456789);
    }
}

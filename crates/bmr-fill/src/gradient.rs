//! Vanilla's seed-derived `vertical_gradient` surface rules (bedrock floor and roof, deepslate transition),
//! computed natively: Xoroshiro128++ positional randoms keyed by the rule's id, as `RandomState` builds them.

use crate::profile::{Kind, Profile};

#[derive(Debug, Clone, Copy)]
struct Xoroshiro(u64, u64);

impl Xoroshiro {
    fn from_seed(seed: i64) -> Self {
        let lo = seed as u64 ^ 0x6a09_e667_f3bc_c909;
        let hi = lo.wrapping_add(0x9e37_79b9_7f4a_7c15);
        Self::new(mix_stafford13(lo), mix_stafford13(hi))
    }

    fn new(lo: u64, hi: u64) -> Self {
        if lo == 0 && hi == 0 { Self(0x9e37_79b9_7f4a_7c15, 0x6a09_e667_f3bc_c909) } else { Self(lo, hi) }
    }

    fn next_long(&mut self) -> u64 {
        let (l, mut m) = (self.0, self.1);
        let n = l.wrapping_add(m).rotate_left(17).wrapping_add(l);
        m ^= l;
        self.0 = l.rotate_left(49) ^ m ^ (m << 21);
        self.1 = m.rotate_left(28);
        n
    }

    fn next_float(&mut self) -> f32 {
        (self.next_long() >> 40) as f32 * 5.960_464_5e-8
    }

    fn fork_positional(&mut self) -> Positional {
        Positional::Xoroshiro(self.next_long(), self.next_long())
    }
}

/// `LegacyRandomSource` (java.util.Random), what the nether's noise settings use (`legacy_random_source`).
#[derive(Debug, Clone, Copy)]
struct Legacy(i64);

impl Legacy {
    const MASK: i64 = (1 << 48) - 1;

    fn new(seed: i64) -> Self {
        Self((seed ^ 0x5_DEEC_E66D) & Self::MASK)
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5_DEEC_E66D).wrapping_add(0xB)) & Self::MASK;
        (self.0 >> (48 - bits)) as i32
    }

    fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32).wrapping_add(self.next(32) as i64)
    }

    fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5e-8
    }
}

/// Java `String.hashCode`.
fn java_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

fn mix_stafford13(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A `PositionalRandomFactory`.
#[derive(Debug, Clone, Copy)]
enum Positional {
    Xoroshiro(u64, u64),
    Legacy(i64),
}

impl Positional {
    fn world(seed: i64, legacy: bool) -> Self {
        if legacy { Self::Legacy(Legacy::new(seed).next_long()) } else { Xoroshiro::from_seed(seed).fork_positional() }
    }

    /// `fromHashOf(id).forkPositional()`: Xoroshiro keys by the id's MD5, legacy by its `String.hashCode`.
    fn for_id(self, id: &str, md5: (u64, u64)) -> Self {
        match self {
            Self::Xoroshiro(lo, hi) => Xoroshiro::new(md5.0 ^ lo, md5.1 ^ hi).fork_positional(),
            Self::Legacy(s) => Self::Legacy(Legacy::new(i64::from(java_hash(id)) ^ s).next_long()),
        }
    }

    fn float_at(self, x: i32, y: i32, z: i32) -> f32 {
        let p = position_seed(x, y, z);
        match self {
            Self::Xoroshiro(lo, hi) => Xoroshiro::new(p as u64 ^ lo, hi).next_float(),
            Self::Legacy(s) => Legacy::new(p ^ s).next_float(),
        }
    }
}

/// `Mth.getSeed`: note `x * 3129871` overflows as an int before widening.
fn position_seed(x: i32, y: i32, z: i32) -> i64 {
    let mut l = (x.wrapping_mul(3_129_871) as i64) ^ (z as i64).wrapping_mul(116_129_781) ^ y as i64;
    l = l.wrapping_mul(l).wrapping_mul(42_317_861).wrapping_add(l.wrapping_mul(11));
    l >> 16
}

/// Rule ids with their MD5 (two big-endian longs).
const BEDROCK_FLOOR: (&str, (u64, u64)) = ("minecraft:bedrock_floor", (0xbbf7_928b_7bf1_d285, 0xc4dc_7cf9_0e1b_3b94));
const BEDROCK_ROOF: (&str, (u64, u64)) = ("minecraft:bedrock_roof", (0x8ebd_4a1d_131d_71cc, 0xc984_cfbb_684a_26c4));
const DEEPSLATE: (&str, (u64, u64)) = ("minecraft:deepslate", (0x907d_dbc8_bae9_5dcf, 0xcfad_a31a_09f2_6a63));

/// One `vertical_gradient` condition: true at and below `lo`, false at and above `hi`, linear between.
#[derive(Debug, Clone, Copy)]
struct Gradient {
    random: Positional,
    lo: i32,
    hi: i32,
}

impl Gradient {
    fn new(world: Positional, (id, md5): (&str, (u64, u64)), lo: i32, hi: i32) -> Self {
        Self { random: world.for_id(id, md5), lo, hi }
    }

    fn test(&self, x: i32, y: i32, z: i32) -> bool {
        if y <= self.lo {
            return true;
        }
        if y >= self.hi {
            return false;
        }
        let p = 1.0 - f64::from(y - self.lo) / f64::from(self.hi - self.lo);
        f64::from(self.random.float_at(x, y, z)) < p
    }
}

/// The seed's exact bedrock and deepslate per cell, where the profile's majority-by-height would guess.
#[derive(Debug, Clone, Copy)]
pub struct Gradients {
    kind: Kind,
    floor: Gradient,
    roof: Gradient,
    deepslate: Gradient,
}

impl Gradients {
    pub fn new(seed: i64, profile: &Profile) -> Self {
        let world = Positional::world(seed, profile.kind == Kind::Nether);
        let b = profile.min_y;
        Self {
            kind: profile.kind,
            floor: Gradient::new(world, BEDROCK_FLOOR, b, b + 5),
            // the nether's generation height is 128: belowTop(5)..belowTop(0)
            roof: Gradient::new(world, BEDROCK_ROOF, 122, 127),
            deepslate: Gradient::new(world, DEEPSLATE, 0, 8),
        }
    }

    /// The terrain block vanilla puts at an unseen cell with no other evidence (no caves, ores or blobs).
    pub fn block(&self, x: i32, y: i32, z: i32) -> &'static str {
        match self.kind {
            Kind::Overworld if self.floor.test(x, y, z) => "minecraft:bedrock",
            Kind::Overworld if self.deepslate.test(x, y, z) => "minecraft:deepslate",
            Kind::Overworld => "minecraft:stone",
            Kind::Nether if self.floor.test(x, y, z) || (y >= 122 && !self.roof.test(x, y, z)) => "minecraft:bedrock",
            Kind::Nether => "minecraft:netherrack",
            Kind::End => "minecraft:end_stone",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // exact on the fixture worlds: examples/check_gradients.rs
    #[test]
    fn java_primitives_match() {
        assert_eq!(java_hash("bluemap_reverse"), 1_539_995_237);
        assert_eq!(Legacy::new(0).next_long(), -4_962_768_465_676_381_896);
        assert_eq!(position_seed(0, 0, 0), 0);
    }
}

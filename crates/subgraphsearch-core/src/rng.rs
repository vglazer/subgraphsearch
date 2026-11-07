//! Random number generators.
//!
//! The search and the graph generators only ever need "a uniform integer below `bound`", which
//! is what [`SearchRng`] provides. Two implementations are available:
//!
//! - [`Randu`] reproduces the `RAND` macro shared by all the original USRA C programs bit for
//!   bit, so that `subgraphsearch` can regenerate their graphs and search runs exactly. It is a
//!   variant of RANDU (`x <- 65539 x mod 2^31`), which is fast but statistically weak.
//! - [`Xoshiro`] (xoshiro256++, seeded through SplitMix64) is the default for new work.

/// A source of uniformly distributed integers.
pub trait SearchRng {
    /// A uniformly distributed integer in `0..bound`. `bound` must be positive.
    fn below(&mut self, bound: u32) -> u32;
}

/// Which generator to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RngKind {
    /// The original programs' RANDU variant; reproduces their output exactly.
    Legacy,
    /// xoshiro256++.
    Xoshiro,
}

impl std::str::FromStr for RngKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "legacy" | "randu" => Ok(RngKind::Legacy),
            "xoshiro" | "xoshiro256++" | "default" => Ok(RngKind::Xoshiro),
            _ => Err(format!(
                "unknown random number generator '{s}' (expected 'xoshiro' or 'legacy')"
            )),
        }
    }
}

/// The USRA programs' generator: `((seed = (seed * 65539) & 0x7fffffff) / 7) % bound`.
///
/// The original computes this in a 64-bit `long`; the multiplication wraps here exactly as it
/// does in practice there.
#[derive(Clone, Debug)]
pub struct Randu {
    state: i64,
}

impl Randu {
    /// Starts from `seed`, as the C programs do. A zero seed yields zero forever.
    pub fn new(seed: i64) -> Self {
        Randu { state: seed }
    }
}

impl SearchRng for Randu {
    #[inline]
    fn below(&mut self, bound: u32) -> u32 {
        self.state = self.state.wrapping_mul(65539) & 0x7fff_ffff;
        ((self.state / 7) % i64::from(bound)) as u32
    }
}

/// xoshiro256++ (Blackman and Vigna), seeded with SplitMix64.
#[derive(Clone, Debug)]
pub struct Xoshiro {
    s: [u64; 4],
}

#[inline]
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl Xoshiro {
    /// Seeds the generator from a single 64-bit value.
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        Xoshiro {
            s: [
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
            ],
        }
    }

    /// An independent generator for stream `stream` of `seed`, e.g. one per experiment, so that
    /// results do not depend on how experiments are scheduled across threads.
    pub fn for_stream(seed: u64, stream: u64) -> Self {
        let mut sm = stream ^ 0x5851_f42d_4c95_7f2d;
        Xoshiro::new(seed ^ splitmix64(&mut sm))
    }

    /// The next 64 random bits.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// A uniform float in `[0, 1)`.
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

impl SearchRng for Xoshiro {
    /// Lemire's nearly divisionless method: unbiased, and usually a single multiplication.
    #[inline]
    fn below(&mut self, bound: u32) -> u32 {
        let bound = u64::from(bound);
        let mut m = (self.next_u64() >> 32) * bound;
        let mut low = m & 0xffff_ffff;
        if low < bound {
            let threshold = (u32::MAX as u64 + 1 - bound) % bound;
            while low < threshold {
                m = (self.next_u64() >> 32) * bound;
                low = m & 0xffff_ffff;
            }
        }
        (m >> 32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn randu_matches_the_c_macro() {
        // values computed by hand from the C definition with seed 1
        let mut r = Randu::new(1);
        let mut state: i64 = 1;
        for bound in [10u32, 7, 1000, 3, 65536, 2] {
            state = (state * 65539) & 0x7fff_ffff;
            assert_eq!(r.below(bound) as i64, (state / 7) % bound as i64);
        }
        let mut zero = Randu::new(0);
        assert!((0..10).all(|_| zero.below(5) == 0));
    }

    #[test]
    fn xoshiro_below_is_in_range_and_roughly_uniform() {
        let mut r = Xoshiro::new(42);
        let mut counts = [0usize; 6];
        for _ in 0..60_000 {
            counts[r.below(6) as usize] += 1;
        }
        assert!(
            counts.iter().all(|&c| (9_000..11_000).contains(&c)),
            "{counts:?}"
        );
        assert_ne!(
            Xoshiro::for_stream(1, 0).next_u64(),
            Xoshiro::for_stream(1, 1).next_u64()
        );
    }
}

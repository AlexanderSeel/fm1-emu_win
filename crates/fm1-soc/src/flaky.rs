//! A flaky core 1, as on some FM-1 units: now and then it loads a word that is not in RAM.
//!
//! Measured by Jangada (github.com/zednaked/jangada; fm1-x0x#10) and described in Felucca's and
//! X0X's `fm1_cpu1.h`: on affected units core 1 reads 0x00200000 where core 0 reads 0, idle now
//! and then and more often under load. Firmwares that use core 1 defend against it (a check word
//! with each job) and fall back to one core; this makes that fallback testable on any machine.
//!
//! The model: each load core 1 makes from RAM (0x01C00000.., not flash, not peripherals, not an
//! instruction fetch) returns its value with bit 21 flipped (0x00200000, the bit Jangada saw) with
//! probability `rate`, drawn from a seeded generator so a run can be reproduced. A byte or
//! halfword load sees the flip only when the lane it returns holds that bit (the byte at offset 2
//! of the word; the halfword at offsets 2 and 3, as `Soc::read16` returns the half at `addr & 2`),
//! and only then counts as a misread. Core 0 is untouched, and a write stores what core 1 gives it:
//! a read-modify-write on core 1 (a bit set or clear in memory, `testset`) that misread writes the
//! wrong value back, where core 0 then sees it, as a corrupted read written back plausibly is on the
//! hardware too. "More often under load" is not modelled separately: the rate is per load, so a
//! core 1 that reads more misreads more.
//!
//! Off (the default, `Soc::flaky_core1` None) the output is the same as without this; core 1's
//! interpreted RAM loads only pay a check that the option is off.

/// The bit a misread flips: Jangada's 0x00200000 where core 0 read 0.
pub const MISREAD_BIT: u32 = 0x0020_0000;

#[derive(Debug, Clone)]
pub struct FlakyCore1 {
    /// A load misreads when the next random draw is below this (rate × 2^64).
    threshold: u64,
    /// splitmix64's state
    state: u64,
    /// RAM loads by core 1 so far
    pub loads: u64,
    /// of those, misread (a flip the load could see)
    pub misreads: u64,
}

impl FlakyCore1 {
    /// `rate`: the chance that one RAM load by core 1 misreads (0..=1); `seed`: the generator's.
    pub fn new(rate: f64, seed: u64) -> Self {
        let rate = rate.clamp(0.0, 1.0);
        let threshold = if rate >= 1.0 { u64::MAX } else { (rate * 18_446_744_073_709_551_616.0) as u64 };
        Self { threshold, state: seed, loads: 0, misreads: 0 }
    }

    /// splitmix64 (Steele, Lea, Flood 2014): small, fast, and every seed gives a good stream.
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// The value a `size`-byte load at `addr` (in RAM) returns, given what RAM holds.
    pub fn load(&mut self, addr: u32, size: u32, value: u32) -> u32 {
        self.loads += 1;
        let draw = self.next();
        if draw >= self.threshold && self.threshold != u64::MAX {
            return value;
        }
        // the flipped bit, as this load sees it: the lane of the word it returns (a byte at
        // `addr & 3`, a halfword at `addr & 2`: Soc::read8 / read16), little-endian
        let mask = match size {
            1 => (MISREAD_BIT >> ((addr & 3) * 8)) & 0xff,
            2 => (MISREAD_BIT >> ((addr & 2) * 8)) & 0xffff,
            _ => MISREAD_BIT,
        };
        if mask != 0 {
            self.misreads += 1;
        }
        value ^ mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_zero_never_misreads() {
        let mut f = FlakyCore1::new(0.0, 1);
        for i in 0..100_000u32 {
            assert_eq!(f.load(0x01c0_0000 + 4 * (i & 0xff), 4, i), i);
        }
        assert_eq!((f.loads, f.misreads), (100_000, 0));
    }

    #[test]
    fn rate_one_always_flips_bit_21() {
        let mut f = FlakyCore1::new(1.0, 7);
        assert_eq!(f.load(0x01c0_0100, 4, 0), 0x0020_0000);
        assert_eq!(f.load(0x01c0_0100, 4, 0x0020_0001), 1);
        assert_eq!((f.loads, f.misreads), (2, 2));
    }

    /// Every byte and halfword offset, against what the SoC returns for it: the flip lands on
    /// bit 21 of the word, and counts, only when the lane returned holds it.
    #[test]
    fn byte_and_halfword_lanes() {
        use fm1_core::Bus;
        let mut soc = crate::Soc::new(1 << 16, vec![0; 16]);
        let base = crate::RAM_BASE + 0x100;
        for (size, off, flips) in [(1, 0, false), (1, 1, false), (1, 2, true), (1, 3, false),
                                   (2, 0, false), (2, 1, false), (2, 2, true), (2, 3, true)] {
            let addr = base + off;
            let mut f = FlakyCore1::new(1.0, 7);
            // what the load returns with bit 21 of the word set: the lane's own view of the bit
            soc.write32(base, MISREAD_BIT);
            let lane_bit = if size == 1 { soc.read8(addr) as u32 } else { soc.read16(addr) as u32 };
            soc.write32(base, 0);
            let got = if size == 1 { soc.read8(addr) as u32 } else { soc.read16(addr) as u32 };
            assert_eq!(got, 0);
            let v = f.load(addr, size, got);
            assert_eq!(v, lane_bit, "size {size} offset {off}");
            assert_eq!(v != 0, flips, "size {size} offset {off}");
            assert_eq!(f.misreads, flips as u64, "size {size} offset {off}: counted only when it flips");
            assert_eq!(f.loads, 1);
        }
    }

    #[test]
    fn rate_is_about_right_and_seeded() {
        let run = |seed| {
            let mut f = FlakyCore1::new(0.01, seed);
            (0..200_000).map(|_| f.load(0x01c0_0000, 4, 0)).collect::<Vec<_>>()
        };
        let a = run(42);
        let hits = a.iter().filter(|&&v| v != 0).count();
        assert!((1600..2400).contains(&hits), "{hits} misreads in 200000 at 1%");
        assert_eq!(a, run(42), "same seed, same misreads");
        assert_ne!(a, run(43), "another seed, others");
    }
}

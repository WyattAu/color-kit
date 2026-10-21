//! Deterministic perceptual quantisation: a seeded palette → N colours, in
//! Oklab, with a hard iteration bound (a theme build can never hang).
//!
//! The terminal path (`theme-kit`'s ANSI-16 emitter) and every snapshot
//! golden in the estate depend on this being byte-stable for a seed
//! (REQ-COL-006/007).

use crate::color::{Oklab, Rgba};
use alloc::vec;
use alloc::vec::Vec;

/// xorshift64* — a tiny deterministic generator; the estate forbids `rand`
/// in L0 and reproducibility is a gate, not a nicety.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn unit(&mut self) -> f64 {
        // 53-bit mantissa from 64 bits: uniform in [0, 1).
        ((self.next() >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
    }
}

fn to_lab(c: Rgba) -> Oklab {
    // Non-finite inputs are sanitised by the Rgba constructors' documented
    // saturation rules; clamp here so distance maths never sees NaN.
    let clean = Rgba::linear_rgba(finite(c.r), finite(c.g), finite(c.b), 1.0);
    clean.oklab().unwrap_or(Oklab {
        l: 0.0,
        a: 0.0,
        b: 0.0,
    })
}

fn finite(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn dist2(a: Oklab, b: Oklab) -> f64 {
    let (dl, da, db) = (a.l - b.l, a.a - b.a, a.b - b.b);
    dl * dl + da * da + db * db
}

/// Quantise `palette` to `n` colours: seeded k-means++ in Oklab, ≤ 32
/// Lloyd iterations.
///
/// Total by construction: an empty palette, `n == 0`, `n` larger than the
/// palette, all-identical colours, and non-finite channels all terminate
/// with a sane answer (the empty case returns empty).
#[must_use]
pub fn quantize(palette: &[Rgba], n: usize, seed: u64) -> Vec<Rgba> {
    if palette.is_empty() || n == 0 {
        return Vec::new();
    }
    let points: Vec<Oklab> = palette.iter().copied().map(to_lab).collect();
    let mut centroids: Vec<Oklab> = Vec::with_capacity(n);

    // Seeding: the first centroid is seed-chosen, the rest by farthest-point
    // (deterministic, and better than random when `n` is small).
    let mut rng = Lcg(if seed == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        seed
    });
    let first = (rng.unit() * points.len() as f64) as usize % points.len().max(1);
    centroids.push(points.get(first).copied().unwrap_or(Oklab {
        l: 0.0,
        a: 0.0,
        b: 0.0,
    }));
    while centroids.len() < n {
        let mut best = 0usize;
        let mut best_d = -1.0f64;
        for (i, p) in points.iter().enumerate() {
            let d = centroids
                .iter()
                .map(|c| dist2(*p, *c))
                .fold(f64::INFINITY, |acc, v| if v < acc { v } else { acc });
            if d > best_d {
                best_d = d;
                best = i;
            }
        }
        // No point is meaningfully farther than the current centroids (a
        // uniform or tiny palette): adding more would duplicate a colour.
        if best_d <= 1e-12 {
            break;
        }
        centroids.push(points.get(best).copied().unwrap_or(Oklab {
            l: 0.0,
            a: 0.0,
            b: 0.0,
        }));
    }

    // Lloyd iterations with a hard bound.
    let mut assignment = vec![0usize; points.len()];
    for _ in 0..32 {
        let mut moved = false;
        for (i, p) in points.iter().enumerate() {
            let mut best = 0usize;
            let mut best_d = f64::INFINITY;
            for (ci, c) in centroids.iter().enumerate() {
                let d = dist2(*p, *c);
                if d < best_d {
                    best_d = d;
                    best = ci;
                }
            }
            if assignment.get(i) != Some(&best) {
                moved = true;
            }
            if let Some(slot) = assignment.get_mut(i) {
                *slot = best;
            }
        }
        // Recompute centroids as means; empty clusters keep their position.
        let mut sums = vec![(0.0f64, 0.0f64, 0.0f64, 0usize); n];
        for (i, p) in points.iter().enumerate() {
            if let Some(&ci) = assignment.get(i) {
                if let Some(slot) = sums.get_mut(ci) {
                    slot.0 += p.l;
                    slot.1 += p.a;
                    slot.2 += p.b;
                    slot.3 += 1;
                }
            }
        }
        for (ci, (sl, sa, sb, count)) in sums.iter().enumerate() {
            if *count > 0 {
                if let Some(c) = centroids.get_mut(ci) {
                    c.l = sl / f64::from(*count as u32);
                    c.a = sa / f64::from(*count as u32);
                    c.b = sb / f64::from(*count as u32);
                }
            }
        }
        if !moved {
            break;
        }
    }

    centroids
        .iter()
        .filter_map(|c| Rgba::from_oklab(*c).ok())
        .map(|c| Rgba {
            r: finite(c.r),
            g: finite(c.g),
            b: finite(c.b),
            a: 1.0,
        })
        .collect()
}

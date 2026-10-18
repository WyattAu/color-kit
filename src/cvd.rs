//! Dichromat (CVD) simulation — Brettel & Viénot 1997, the two-plane
//! severity-1.0 matrices, applied in linear RGB as color.js does.
//!
//! Used twice: to *derive* CVD-adapted palettes, and to *test* them — a
//! pair that separates under simulation does not separate for a dichromat,
//! and the property that survives is rank, not identity.

use crate::color::Rgba;

/// The three dichromacies the theme variants adapt for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CvdKind {
    /// Red-blind (protanopia).
    Protanopia,
    /// Green-blind (deuteranopia) — the worst case the shroom themes treat
    /// as the conservative collision detector.
    Deuteranopia,
    /// Blue-blind (tritanopia).
    Tritanopia,
}

/// Severity-1.0 projection through linear RGB (no indexing: the crate
/// forbids it outright).
fn apply(kind: CvdKind, r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    match kind {
        CvdKind::Protanopia => (
            0.145_10 * r + 1.201_65 * g - 0.346_75 * b,
            0.104_47 * r + 0.853_16 * g + 0.042_37 * b,
            0.004_29 * r - 0.006_03 * g + 1.001_74 * b,
        ),
        CvdKind::Deuteranopia => (
            0.361_98 * r + 0.867_55 * g - 0.229_53 * b,
            0.260_99 * r + 0.645_12 * g + 0.093_89 * b,
            -0.019_75 * r + 0.026_85 * g + 0.992_90 * b,
        ),
        CvdKind::Tritanopia => (
            1.013_54 * r + 0.142_68 * g - 0.156_22 * b,
            0.104_81 * r + 0.875_32 * g + 0.019_89 * b,
            -0.003_81 * r - 0.008_41 * g + 1.012_22 * b,
        ),
    }
}

/// Project a colour through a dichromat's vision. Input channels are
/// clamped to `0..=1` first (the matrices assume in-gamut linear values);
/// output is clamped too. Non-finite input yields opaque black rather than
/// a NaN cascade — total, like every function in this crate.
#[must_use]
pub fn simulate_cvd(c: Rgba, kind: CvdKind) -> Rgba {
    let clamp = |v: f64| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    let (r, g, b) = (clamp(c.r), clamp(c.g), clamp(c.b));
    let (rr, rg, rb) = apply(kind, r, g, b);
    Rgba {
        r: clamp(rr),
        g: clamp(rg),
        b: clamp(rb),
        a: clamp(c.a),
    }
}

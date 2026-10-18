//! Legibility computations: the WCAG 2.1 gate threshold, and the perceptual
//! lightness contrast the theme rank checks use.
//!
//! Naming note: [`lightness_contrast`] implements the APCA-style algorithm.
//! The upstream reference implementation's licence restricts conformance
//! claims and marks; the math is portable, the name is not. This crate
//! computes it neutrally and claims no conformance.

use crate::color::Rgba;

/// WCAG 2.1 AA for normal text (4.5:1).
pub const WCAG_AA_NORMAL: f64 = 4.5;
/// WCAG 2.1 AA for large text (3:1).
pub const WCAG_AA_LARGE: f64 = 3.0;
/// WCAG 2.1 AAA for normal text (7:1).
pub const WCAG_AAA_NORMAL: f64 = 7.0;

/// Which WCAG conformance level a pair reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WcagLevel {
    /// Below AA for large text.
    None,
    /// AA or better for large text only (≥ 3:1).
    AaLarge,
    /// AA or better for normal text (≥ 4.5:1).
    Aa,
    /// AAA for normal text (≥ 7:1).
    Aaa,
}

/// WCAG 2.1 contrast ratio of two opaque colours: `(L1 + 0.05) / (L2 + 0.05)`
/// on relative luminance, `1.0` for identical luminance.
///
/// `None` when either colour has a non-finite channel — the gate must see
/// a failed comparison, never a silent `1.0`.
#[must_use]
pub fn contrast_ratio(fg: Rgba, bg: Rgba) -> Option<f64> {
    if !fg.is_finite() || !bg.is_finite() {
        return None;
    }
    let l1 = fg.luminance_wcag();
    let l2 = bg.luminance_wcag();
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    let delta = hi - lo;
    if delta < 1e-12 {
        return Some(1.0);
    }
    Some((hi + 0.05) / (lo + 0.05))
}

/// The conformance level a measured ratio reaches.
#[must_use]
pub fn wcag_level(ratio: f64) -> WcagLevel {
    if ratio >= WCAG_AAA_NORMAL {
        WcagLevel::Aaa
    } else if ratio >= WCAG_AA_NORMAL {
        WcagLevel::Aa
    } else if ratio >= WCAG_AA_LARGE {
        WcagLevel::AaLarge
    } else {
        WcagLevel::None
    }
}

/// Convenience for the gate: does `ratio` meet AA for this text size?
#[must_use]
pub fn meets_wcag(ratio: f64, normal_text: bool) -> bool {
    if normal_text {
        ratio >= WCAG_AA_NORMAL
    } else {
        ratio >= WCAG_AA_LARGE
    }
}

/// Perceptual lightness contrast (Lc-style): positive for dark text on light
/// backgrounds, negative for the reverse, `0.0` below the clip threshold or
/// for imperceptible differences.
///
/// This is a *rank* signal, not a compliance signal — use
/// [`contrast_ratio`] for compliance. The soft black-level clamp and the
/// polarity-dependent exponents are the algorithm's published constants.
#[must_use]
pub fn lightness_contrast(fg: Rgba, bg: Rgba) -> Option<f64> {
    if !fg.is_finite() || !bg.is_finite() {
        return None;
    }
    const DELTA_Y_MIN: f64 = 0.000_5;
    const LOW_CLIP: f64 = 0.1;
    const BLACK_THRESHOLD: f64 = 0.022;
    const SCALE: f64 = 1.14;
    const OFFSET: f64 = 0.027;

    fn soft_clamp(y: f64) -> f64 {
        if y < BLACK_THRESHOLD {
            y + libm::pow(BLACK_THRESHOLD - y, 1.414)
        } else {
            y
        }
    }

    let y_txt = soft_clamp(fg.luminance_lightness());
    let y_bg = soft_clamp(bg.luminance_lightness());
    if libm::fabs(y_bg - y_txt) < DELTA_Y_MIN {
        return Some(0.0);
    }
    if y_bg > y_txt {
        // Dark text on a light background (normal polarity).
        let sapc = (libm::pow(y_bg, 0.56) - libm::pow(y_txt, 0.57)) * SCALE;
        Some(if sapc < LOW_CLIP {
            0.0
        } else {
            // ×100 puts the result on the published Lc scale (black on
            // white ≈ 106).
            (sapc - OFFSET) * 100.0
        })
    } else {
        // Light text on a dark background (reverse polarity).
        let sapc = (libm::pow(y_bg, 0.65) - libm::pow(y_txt, 0.62)) * SCALE;
        Some(if sapc > -LOW_CLIP {
            0.0
        } else {
            (sapc + OFFSET) * 100.0
        })
    }
}

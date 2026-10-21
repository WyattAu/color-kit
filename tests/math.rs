//! REQ-COL-001…007: the maths is goldens plus invariants. Test targets lift
//! the lib's deny-level lints at file scope, per the estate's decoder
//! discipline (the same pattern font-parse's tests use).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::float_cmp
)]

use color_kit::{
    contrast_ratio, lightness_contrast, meets_wcag, meets_wcag as _wcag_alias, parse_hex, quantize,
    simulate_cvd, CvdKind, Rgba, Rgba8, WcagLevel,
};
use proptest::prelude::*;

fn hex(s: &str) -> Rgba {
    let v = parse_hex(s).unwrap_or_else(|| panic!("{s} is a hex literal"));
    v.to_linear()
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

// -- REQ-COL-003: WCAG golden values (the spec's own published examples) --

#[test]
fn wcag_golden_values() {
    let ratio = |a: &str, b: &str| contrast_ratio(hex(a), hex(b)).unwrap_or(1.0);
    // Black on white and white on black are exactly 21:1.
    assert!(close(ratio("#000", "#fff"), 21.0, 1e-9));
    assert!(close(ratio("#fff", "#000"), 21.0, 1e-9));
    // The classic AA-boundary greys.
    assert!(close(ratio("#767676", "#ffffff"), 4.54, 0.01));
    assert!(close(ratio("#595959", "#ffffff"), 7.0, 0.01));
    // Identical colours are 1:1.
    assert!(close(ratio("#123456", "#123456"), 1.0, 1e-9));
}

#[test]
fn wcag_level_mapping() {
    assert_eq!(color_kit::wcag_level(4.49), WcagLevel::AaLarge);
    assert_eq!(color_kit::wcag_level(4.5), WcagLevel::Aa);
    assert_eq!(color_kit::wcag_level(7.0), WcagLevel::Aaa);
    assert!(meets_wcag(4.5, true));
    assert!(!meets_wcag(4.4, true));
    assert!(meets_wcag(3.0, false));
    let _ = _wcag_alias; // keep the alias import honest under -D warnings
}

#[test]
fn contrast_ratio_rejects_non_finite() {
    let bad = Rgba::linear(f64::NAN, 0.0, 0.0);
    assert_eq!(contrast_ratio(bad, hex("#fff")), None);
    assert_eq!(
        lightness_contrast(hex("#000"), Rgba::linear(0.0, f64::INFINITY, 0.0)),
        None
    );
}

// -- REQ-COL-004: perceptual lightness contrast ---------------------------------

#[test]
fn lightness_contrast_polarity_and_ordering() {
    let black_on_white = lightness_contrast(hex("#000"), hex("#fff")).unwrap_or(0.0);
    let white_on_black = lightness_contrast(hex("#fff"), hex("#000")).unwrap_or(0.0);
    assert!(
        black_on_white > 90.0,
        "black on white is far above AA: {black_on_white}"
    );
    assert!(
        white_on_black < -90.0,
        "white on black is strongly negative: {white_on_black}"
    );
    // Identical colours produce no signal.
    assert_eq!(lightness_contrast(hex("#abc"), hex("#abc")), Some(0.0));
    // Grey ramps on white: darker text carries MORE lightness contrast
    // (#222222 ≈ 100, #555555 ≈ 82, …), so the ramp descends as it lightens.
    let mut prev = f64::INFINITY;
    for hexcode in ["#222222", "#555555", "#888888", "#bbbbbb", "#eeeeee"] {
        let lc = lightness_contrast(hex(hexcode), hex("#fff")).unwrap_or(0.0);
        assert!(lc <= prev, "{hexcode} ({lc}) should carry less lightness contrast than the previous ramp step ({prev})");
        prev = lc;
    }
}

// -- REQ-COL-001/002: conversions -------------------------------------------------

#[test]
fn srgb8_round_trip_is_exact() {
    for v in [0u32, 1, 127, 128, 254, 255, 0x00FF_FFFF, 0xFFFF_FFFF] {
        let c = Rgba8 {
            r: ((v >> 16) & 0xFF) as u8,
            g: ((v >> 8) & 0xFF) as u8,
            b: (v & 0xFF) as u8,
            a: ((v >> 24) & 0xFF) as u8,
        };
        let back = c.to_linear().to_srgb8().unwrap_or(c);
        assert_eq!(back, c, "u32 colour {v:08x} must round-trip exactly");
    }
}

#[test]
fn oklab_round_trip_is_stable() {
    for hexcode in [
        "#000000", "#ffffff", "#ff0000", "#00ff00", "#0000ff", "#7f3fbf", "#123456",
    ] {
        let c = hex(hexcode);
        let lab = c.oklab().unwrap();
        let back = Rgba::from_oklab(lab).unwrap();
        assert!(
            c.delta_e_ok(back).unwrap_or(9.0) < 1e-6,
            "{hexcode} must survive the Oklab round-trip"
        );
    }
}

#[test]
fn hex_parsing_is_total() {
    assert_eq!(parse_hex("#fff"), parse_hex("fff"));
    assert_eq!(parse_hex("#11223344").map(|c| c.a), Some(0x44));
    assert_eq!(parse_hex("#zzzzzz"), None);
    assert_eq!(parse_hex("#12345"), None);
    assert_eq!(parse_hex(""), None);
    // and it round-trips through the text form
    let c = Rgba8 {
        r: 18,
        g: 52,
        b: 86,
        a: 255,
    };
    assert_eq!(parse_hex(&c.to_hex()), Some(c));
}

// -- REQ-COL-002: gamut mapping ----------------------------------------------------

#[test]
fn absurd_chroma_lands_in_gamut() {
    // Wild chroma at a mid lightness: mapped, never Err, never NaN.
    let loud = color_kit::Oklch {
        l: 0.7,
        c: 0.9,
        h: 1.2,
    };
    let mapped = Rgba::from_oklch(loud).unwrap();
    let eight = mapped
        .to_srgb8()
        .expect("mapped colour must be displayable");
    assert!(eight.to_linear().is_finite());
    // Out-of-range lightness is clamped, not rejected.
    let dark = Rgba::from_oklch(color_kit::Oklch {
        l: -0.5,
        c: 0.1,
        h: 0.0,
    })
    .unwrap();
    assert!(dark.to_srgb8().is_ok());
}

// -- REQ-COL-005: CVD simulation -----------------------------------------------------

#[test]
fn cvd_simulation_collapses_red_green_pairs() {
    // The deuteranopia fixture from the theme work: two colours that differ
    // for trichromats must come closer for a dichromat.
    let red = hex("#e74c3c");
    let green = hex("#2ecc71");
    let sim = |c: Rgba| simulate_cvd(c, CvdKind::Deuteranopia);
    let before = red.delta_e_ok(green).unwrap_or(f64::INFINITY);
    let after = sim(red).delta_e_ok(sim(green)).unwrap_or(f64::INFINITY);
    assert!(
        after < before,
        "deuteranopia must shrink the red/green separation: {before} → {after}"
    );
}

#[test]
fn cvd_simulation_stays_in_range() {
    for hexcode in ["#000000", "#ffffff", "#7f3fbf", "#00ff00"] {
        for kind in [
            CvdKind::Protanopia,
            CvdKind::Deuteranopia,
            CvdKind::Tritanopia,
        ] {
            let sim = simulate_cvd(hex(hexcode), kind);
            assert!(sim.is_finite());
            let _ = sim.to_srgb8().expect("simulated colour is displayable");
        }
    }
    // Non-finite input is sanitised, not propagated.
    let sim = simulate_cvd(
        Rgba::linear(f64::NAN, f64::NAN, f64::NAN),
        CvdKind::Deuteranopia,
    );
    assert!(sim.is_finite());
}

// -- REQ-COL-006/007: quantisation ---------------------------------------------------

#[test]
fn quantize_is_deterministic_and_total() {
    let palette: Vec<Rgba> = [
        "#000000", "#ff0000", "#00ff00", "#0000ff", "#888888", "#ffffff",
    ]
    .iter()
    .map(|s| hex(s))
    .collect();
    let a = quantize(&palette, 4, 42);
    let b = quantize(&palette, 4, 42);
    assert_eq!(
        a, b,
        "same seed, same bytes — the attestation surface depends on it"
    );
    assert!(a.len() <= 4 && !a.is_empty());

    // Degenerate inputs terminate with sane answers.
    assert!(quantize(&[], 4, 1).is_empty());
    assert!(quantize(&palette, 0, 1).is_empty());
    let single = quantize(&palette[..1], 16, 1);
    assert!(!single.is_empty());
    let uniform = vec![hex("#777777"); 10];
    assert_eq!(quantize(&uniform, 3, 7).len(), 1);
    let with_nan = vec![Rgba::linear(f64::NAN, 1.0, 0.0), hex("#123456")];
    let out = quantize(&with_nan, 2, 3);
    assert!(out.iter().all(|c| c.is_finite()));
}

// -- property tests --------------------------------------------------------------------

proptest! {
    #[test]
    fn any_u32_colour_round_trips(v in any::<u32>()) {
        let c = Rgba8 {
            r: ((v >> 16) & 0xFF) as u8,
            g: ((v >> 8) & 0xFF) as u8,
            b: (v & 0xFF) as u8,
            a: ((v >> 24) & 0xFF) as u8,
        };
        prop_assert_eq!(c.to_linear().to_srgb8().unwrap(), c);
    }

    #[test]
    fn oklab_round_trips_for_all_opaque_colours(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
        let c = Rgba::from_srgb8(r, g, b, 255);
        let lab = c.oklab().unwrap();
        let back = Rgba::from_oklab(lab).unwrap();
        prop_assert!(c.delta_e_ok(back).unwrap() < 1e-6);
    }

    #[test]
    fn contrast_is_symmetric(a in 0u8..=255, b in 0u8..=255) {
        let x = Rgba::from_srgb8(a, 0, a / 2, 255);
        let y = Rgba::from_srgb8(b, 0, b / 2, 255);
        prop_assert!((contrast_ratio(x, y).unwrap() - contrast_ratio(y, x).unwrap()).abs() < 1e-12);
    }
}

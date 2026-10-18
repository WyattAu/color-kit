//! Colour science leaf for the estate — the numeric core under `theme-kit`
//! (specs/color-kit.md, ADR-0004 §2).
//!
//! One colour type in one working space, four computations, and total
//! functions everywhere:
//!
//! | Area | Entry points |
//! |---|---|
//! | the type | [`Rgba`] — linear sRGB `f64`, the space compositing actually happens in |
//! | conversion | [`Rgba::from_srgb8`], [`Rgba::to_srgb8`], [`Rgba::oklch`], [`Rgba::from_oklch`] with chroma-reduction gamut mapping |
//! | legibility | [`contrast_ratio`] — WCAG 2.1 AA, the threshold the theme gate cites |
//! | rank | [`lightness_contrast`] — perceptual Lc-style separation (neutral name; see below) |
//! | vision | [`simulate_cvd`] — dichromat projection, for palettes *and* for testing them |
//! | quantisation | [`quantize`] — deterministic seed → N-colour palettes (terminal, ANSI) |
//!
//! ```
//! use color_kit::{contrast_ratio, Rgba};
//!
//! let ink = Rgba::from_srgb8(0, 0, 0, 255);
//! let paper = Rgba::from_srgb8(255, 255, 255, 255);
//! let ratio = contrast_ratio(ink, paper).expect("opaque colours");
//! assert!((ratio - 21.0).abs() < 1e-9, "black on white is 21:1, got {ratio}");
//! ```
//!
//! # Naming and licensing note
//!
//! [`lightness_contrast`] computes the APCA-style perceptual lightness
//! contrast. The upstream reference implementation ships under a licence
//! that restricts conformance claims and marks; the *math* is portable, the
//! name is not. This crate therefore computes it under a neutral name and
//! claims no conformance — the estate rule applies (ADR-0004 §Consequences):
//! the acronym never appears in crate names, docs, or messages.
//!
//! # Layer
//!
//! L0 — leaf: zero estate-internal dependencies by mandate. `libm` supplies
//! the no_std transcendentals (the `dsp-core` precedent); `thiserror` the
//! typed error taxonomy. The library is `no_std` + `alloc` regardless of the
//! `std` feature.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

extern crate alloc;

mod color;
mod contrast;
mod cvd;
mod error;
mod quantize;

pub use color::{parse_hex, Oklab, Oklch, Palette, Rgba, Rgba8};
pub use contrast::{
    contrast_ratio, lightness_contrast, meets_wcag, wcag_level, WcagLevel, WCAG_AA_LARGE,
    WCAG_AA_NORMAL, WCAG_AAA_NORMAL,
};
pub use cvd::{simulate_cvd, CvdKind};
pub use error::ColorError;
pub use quantize::quantize;

/// The crate's version, for callers that report it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

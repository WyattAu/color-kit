//! Typed errors for colour operations (specs/color-kit.md REQ-COL-007).

/// Why a colour operation could not produce its exact answer.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum ColorError {
    /// An input channel was NaN or infinite — colour maths cannot assign it a
    /// position in any space.
    #[error("non-finite channel in {component}")]
    NonFinite {
        /// Which colour space produced the value ("srgb", "oklch", …).
        component: &'static str,
    },
    /// The requested representation is outside sRGB and gamut mapping was not
    /// requested. Call [`crate::Rgba::to_srgb8`] for the mapping path.
    #[error("colour out of sRGB gamut: {reason}")]
    OutOfGamut {
        /// Human-readable detail (e.g. "oklch chroma 0.4 at lightness -0.1").
        reason: &'static str,
    },
    /// Quantisation needs at least one input colour.
    #[error("quantisation needs a non-empty palette")]
    EmptyPalette,
}

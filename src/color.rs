//! The colour type and its conversions. Linear sRGB `f64` is the working
//! space on purpose: alpha compositing is only correct there, and theme
//! tokens composite (hover/disabled overlays are alpha over role colours).

use crate::error::ColorError;
use alloc::vec::Vec;

/// Tolerance for "inside sRGB" tests during gamut mapping.
const GAMUT_EPS: f64 = 1e-6;

/// A colour in **linear** sRGB, channels nominally in `0..=1`.
///
/// Out-of-range channels are representable (gamut mapping needs them) but
/// every consumer documented as in-gamut clamps first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    /// Linear red.
    pub r: f64,
    /// Linear green.
    pub g: f64,
    /// Linear blue.
    pub b: f64,
    /// Alpha, straight (not premultiplied), `0..=1`.
    pub a: f64,
}

/// An 8-bit-per-channel sRGB colour, alpha included — a *view* for storage
/// and wire formats, never the working type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba8 {
    /// sRGB red.
    pub r: u8,
    /// sRGB green.
    pub g: u8,
    /// sRGB blue.
    pub b: u8,
    /// Alpha.
    pub a: u8,
}

/// Oklab: perceptually uniform, the interpolation space for palettes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklab {
    /// Lightness.
    pub l: f64,
    /// Green–red axis.
    pub a: f64,
    /// Blue–yellow axis.
    pub b: f64,
}

/// OkLCH: Oklab in cylindrical coordinates; hue in radians, `0..2π`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Lightness.
    pub l: f64,
    /// Chroma.
    pub c: f64,
    /// Hue in radians.
    pub h: f64,
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        libm::pow((c + 0.055) / 1.055, 2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    let clamped = c.clamp(0.0, 1.0);
    if clamped <= 0.003_130_8 {
        12.92 * clamped
    } else {
        1.055 * libm::pow(clamped, 1.0 / 2.4) - 0.055
    }
}

fn clamp01(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

fn cbrt(v: f64) -> f64 {
    if v < 0.0 {
        -libm::cbrt(-v)
    } else {
        libm::cbrt(v)
    }
}

impl Rgba {
    /// A colour from linear channels directly (alpha `1.0`).
    #[must_use]
    pub const fn linear(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// A colour from linear channels with explicit alpha.
    #[must_use]
    pub const fn linear_rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    /// A colour from sRGB bytes — the door every hex literal comes through.
    #[must_use]
    pub fn from_srgb8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: srgb_to_linear(f64::from(r) / 255.0),
            g: srgb_to_linear(f64::from(g) / 255.0),
            b: srgb_to_linear(f64::from(b) / 255.0),
            a: f64::from(a) / 255.0,
        }
    }

    /// Encode to sRGB bytes: gamut-mapped and clamped.
    ///
    /// # Errors
    /// [`ColorError::NonFinite`] when any linear channel is NaN or infinite —
    /// the one case where no documented saturation rule applies.
    pub fn to_srgb8(self) -> Result<Rgba8, ColorError> {
        if !self.is_finite() {
            return Err(ColorError::NonFinite { component: "srgb" });
        }
        Ok(Rgba8 {
            r: encode(clamp01(self.r)),
            g: encode(clamp01(self.g)),
            b: encode(clamp01(self.b)),
            // Alpha is linear already — it must NOT take the sRGB transfer
            // curve (the estate's first caught bug in this crate: alpha 13
            // round-tripped to 1).
            a: encode_linear(clamp01(self.a)),
        })
    }

    /// True when every channel is finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.r.is_finite() && self.g.is_finite() && self.b.is_finite() && self.a.is_finite()
    }

    /// Perceptual lightness coordinates.
    ///
    /// # Errors
    /// [`ColorError::NonFinite`] on NaN/inf input.
    pub fn oklab(self) -> Result<Oklab, ColorError> {
        if !self.is_finite() {
            return Err(ColorError::NonFinite { component: "oklab" });
        }
        let (r, g, b) = (self.r, self.g, self.b);
        let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
        let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
        let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
        let (l_, m_, s_) = (cbrt(l), cbrt(m), cbrt(s));
        Ok(Oklab {
            l: 0.210_454_255_3 * l_ + 0.793_617_785_0 * m_ - 0.004_072_046_8 * s_,
            a: 1.977_998_495_1 * l_ - 2.428_592_205_0 * m_ + 0.450_593_709_9 * s_,
            b: 0.025_904_037_1 * l_ + 0.782_771_766_2 * m_ - 0.808_675_766_0 * s_,
        })
    }

    /// Inverse of [`Rgba::oklab`]. Result may be out of gamut; use
    /// [`Rgba::from_oklch`] for the mapped path.
    ///
    /// # Errors
    /// [`ColorError::NonFinite`] on NaN/inf input.
    pub fn from_oklab(lab: Oklab) -> Result<Self, ColorError> {
        if !(lab.l.is_finite() && lab.a.is_finite() && lab.b.is_finite()) {
            return Err(ColorError::NonFinite { component: "oklab" });
        }
        let l_ = lab.l + 0.396_337_777_4 * lab.a + 0.215_803_757_3 * lab.b;
        let m_ = lab.l - 0.105_561_345_8 * lab.a - 0.063_854_172_8 * lab.b;
        let s_ = lab.l - 0.089_484_177_5 * lab.a - 1.291_485_548_0 * lab.b;
        let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
        Ok(Self::linear(
            4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
            -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
            -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s,
        ))
    }

    /// Cylindrical form of [`Rgba::oklab`].
    ///
    /// # Errors
    /// [`ColorError::NonFinite`] propagated from [`Rgba::oklab`].
    pub fn oklch(self) -> Result<Oklch, ColorError> {
        let lab = self.oklab()?;
        Ok(Oklch {
            l: lab.l,
            c: libm::hypot(lab.a, lab.b),
            h: libm::atan2(lab.b, lab.a),
        })
    }

    /// From OkLCH **with gamut mapping**: chroma is binary-searched down to
    /// the largest value that lands inside sRGB (CSS Color 4's approach),
    /// then linear channels are clamped. A lightness outside `0..=1` is
    /// clamped. The result is always displayable, never `Err` for
    /// in-gamut-ness reasons.
    ///
    /// # Errors
    /// [`ColorError::NonFinite`] when `l`, `c`, or `h` is NaN/inf.
    pub fn from_oklch(c: Oklch) -> Result<Self, ColorError> {
        if !(c.l.is_finite() && c.c.is_finite() && c.h.is_finite()) {
            return Err(ColorError::NonFinite { component: "oklch" });
        }
        let (sin, cos) = (libm::sin(c.h), libm::cos(c.h));
        let l = c.l.clamp(0.0, 1.0);
        let direct = Self::from_oklab(Oklab {
            l,
            a: c.c * cos,
            b: c.c * sin,
        })?;
        if in_gamut(direct) {
            return Ok(Self {
                r: clamp01(direct.r),
                g: clamp01(direct.g),
                b: clamp01(direct.b),
                a: clamp01(direct.a),
            });
        }
        // Chroma reduction: find the largest in-gamut chroma at (l, h).
        let mut lo = 0.0f64;
        let mut hi = c.c.abs();
        for _ in 0..16 {
            let mid = 0.5 * (lo + hi);
            let trial = Self::from_oklab(Oklab {
                l,
                a: mid * cos,
                b: mid * sin,
            })?;
            if in_gamut(trial) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let mapped = Self::from_oklab(Oklab {
            l,
            a: lo * cos,
            b: lo * sin,
        })?;
        Ok(Self {
            r: clamp01(mapped.r),
            g: clamp01(mapped.g),
            b: clamp01(mapped.b),
            a: 1.0,
        })
    }

    /// Alpha-composite `self` over `bg` **in linear space** (the correct
    /// place for it; sRGB compositing is the classic theme-token bug).
    #[must_use]
    pub fn composite_over(self, bg: Rgba) -> Rgba {
        let a = clamp01(self.a);
        Rgba {
            r: self.r * a + bg.r * (1.0 - a),
            g: self.g * a + bg.g * (1.0 - a),
            b: self.b * a + bg.b * (1.0 - a),
            a: 1.0,
        }
    }

    /// WCAG 2.1 relative luminance (channels clamped to `0..=1` first).
    #[must_use]
    pub fn luminance_wcag(self) -> f64 {
        let (r, g, b) = (clamp01(self.r), clamp01(self.g), clamp01(self.b));
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// The luminance coefficients the APCA-style computation uses (they
    /// differ from WCAG's in the third decimal — that is not a typo).
    #[must_use]
    pub fn luminance_lightness(self) -> f64 {
        let (r, g, b) = (clamp01(self.r), clamp01(self.g), clamp01(self.b));
        0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b
    }

    /// Perceptual distance in Oklab (used by quantisation and by CVD
    /// collision checks).
    ///
    /// # Errors
    /// [`ColorError::NonFinite`] propagated from [`Rgba::oklab`].
    pub fn delta_e_ok(self, other: Rgba) -> Result<f64, ColorError> {
        let (a, b) = (self.oklab()?, other.oklab()?);
        Ok(libm::sqrt(
            (a.l - b.l) * (a.l - b.l) + (a.a - b.a) * (a.a - b.a) + (a.b - b.b) * (a.b - b.b),
        ))
    }
}

fn in_gamut(c: Rgba) -> bool {
    c.r >= -GAMUT_EPS
        && c.r <= 1.0 + GAMUT_EPS
        && c.g >= -GAMUT_EPS
        && c.g <= 1.0 + GAMUT_EPS
        && c.b >= -GAMUT_EPS
        && c.b <= 1.0 + GAMUT_EPS
}

fn encode(linear: f64) -> u8 {
    encode_linear(linear_to_srgb(linear))
}

/// Round a `0..=1` linear value to 8 bits (no transfer curve).
fn encode_linear(v: f64) -> u8 {
    // `+ 0.5` floor is the standard round-half-up; the input is already
    // clamped to 0..=255 so the cast is total.
    let v = libm::floor(v * 255.0 + 0.5);
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v as u8
    }
}

impl Oklab {
    /// Cylindrical form.
    #[must_use]
    pub fn to_lch(self) -> Oklch {
        Oklch {
            l: self.l,
            c: libm::hypot(self.a, self.b),
            h: libm::atan2(self.b, self.a),
        }
    }
}

impl Oklch {
    /// Cartesian form.
    #[must_use]
    pub fn to_lab(self) -> Oklab {
        Oklab {
            l: self.l,
            a: self.c * libm::cos(self.h),
            b: self.c * libm::sin(self.h),
        }
    }
}

impl Rgba8 {
    /// Back to the linear working type.
    #[must_use]
    pub fn to_linear(self) -> Rgba {
        Rgba::from_srgb8(self.r, self.g, self.b, self.a)
    }

    /// The classic `#rrggbb` / `#rrggbbaa` text form (round-trips through
    /// [`Rgba::to_srgb8`]).
    #[must_use]
    pub fn to_hex(self) -> alloc::string::String {
        use core::fmt::Write as _;
        let mut s = alloc::format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b);
        if self.a != 255 {
            let _ = write!(s, "{:02x}", self.a);
        }
        s
    }
}

/// Parse `#rgb`, `#rrggbb`, `#rrggbbaa` (the leading `#` optional).
///
/// Total: malformed input is `None`, never a panic — theme files are
/// user-authored input, and the crate forbids indexing outright.
#[must_use]
pub fn parse_hex(src: &str) -> Option<Rgba8> {
    fn hex_val(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    }
    let s = src.strip_prefix('#').unwrap_or(src);
    let digits: alloc::vec::Vec<u8> = s.bytes().map(hex_val).collect::<Option<_>>()?;
    match digits.len() {
        3 => Some(Rgba8 {
            r: digits.first()? * 17,
            g: digits.get(1)? * 17,
            b: digits.get(2)? * 17,
            a: 255,
        }),
        6 | 8 => {
            let mut out = [0u8; 4];
            out[3] = 255;
            for (i, pair) in digits.chunks_exact(2).enumerate() {
                let v = pair.first()? * 16 + pair.get(1)?;
                *out.get_mut(i)? = v;
            }
            Some(Rgba8 {
                r: *out.first()?,
                g: *out.get(1)?,
                b: *out.get(2)?,
                a: *out.get(3)?,
            })
        }
        _ => None,
    }
}

/// A palette is just a `Vec` — the alias exists so signatures read as the
/// domain does.
pub type Palette = Vec<Rgba>;

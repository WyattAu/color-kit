# color-kit

Colour science leaf for the WyattAu estate: the numeric core under
[`theme-kit`](../theme-kit) (spec: `engineering-standards/specs/color-kit.md`,
ADR-0004 §2).

| Area | What |
|---|---|
| The type | `Rgba`: **linear** sRGB `f64`; alpha composites happen there, not in sRGB |
| Conversion | sRGB ↔ linear, Oklab/OkLCH, chroma-reduction gamut mapping (CSS Color 4 style) |
| Legibility | `contrast_ratio`: WCAG 2.1 AA, the threshold the theme gate cites |
| Rank | `lightness_contrast`: perceptual Lc-style separation, neutral name (licence/brand: ADR-0004 §Consequences) |
| Vision | `simulate_cvd`: Brettel–Viénot dichromat projection, for palettes *and* for testing them |
| Quantisation | `quantize`: deterministic seeded k-means in Oklab (terminal palettes, snapshot goldens) |

```rust
use color_kit::{contrast_ratio, Rgba};

let ink = Rgba::from_srgb8(0, 0, 0, 255);
let paper = Rgba::from_srgb8(255, 255, 255, 255);
let ratio = contrast_ratio(ink, paper).expect("opaque colours");
assert!((ratio - 21.0).abs() < 1e-9);
```

## Properties

- **L0 leaf**: zero estate-internal dependencies; `libm` only (the `dsp-core`
  precedent). `no_std` + `alloc` on `--no-default-features`; clippy denies
  `unwrap`/`expect`/`panic`/indexing, and every public function is total:
  NaN/inf/empty inputs are typed errors or documented saturation rules.
- **Deterministic**: `quantize` is byte-stable for a seed (the attestation
  surface REQ-COL-006/007 depend on).
- **Tested**: WCAG golden values, Oklab round-trip properties over all
  colours, CVD collision property, quantiser determinism/degeneracy.

Licensed under MIT OR Apache-2.0.

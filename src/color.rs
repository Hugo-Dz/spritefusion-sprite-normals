//! Convert sRGB colors to Oklab lightness.

use std::sync::LazyLock;

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Every 8-bit channel value, linearized once instead of a `powf` per pixel.
static LINEAR: LazyLock<[f32; 256]> =
    LazyLock::new(|| std::array::from_fn(|v| srgb_to_linear(v as f32 / 255.0)));

#[allow(clippy::many_single_char_names)] // Conventional RGB and LMS matrix coordinates.
pub(crate) fn lightness(rgb: [u8; 3]) -> f32 {
    let [r, g, b] = rgb.map(|v| LINEAR[usize::from(v)]);
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s
}

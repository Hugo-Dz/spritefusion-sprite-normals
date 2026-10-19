use image::{Rgba, RgbaImage};

use crate::{NormalSettings, color, distance, field, field::Field, neutral_map, slopes_to_normal};

pub(crate) fn generate(rgba: &RgbaImage, settings: &NormalSettings) -> RgbaImage {
    let pad = settings.padding();
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    let (width, height) = (w + 2 * pad, h + 2 * pad);
    let mut opacity = Field::zero(width, height);
    let mut lightness = opacity.clone();
    let mut mask = vec![false; width * height];
    let (mut low, mut high) = (1.0_f32, 0.0_f32);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let i = (y as usize + pad) * width + x as usize + pad;
        let alpha = f32::from(pixel[3]) / 255.0;
        if alpha <= settings.alpha_threshold {
            continue;
        }
        mask[i] = true;
        opacity.data[i] = alpha;
        let value = color::lightness([pixel[0], pixel[1], pixel[2]]);
        lightness.data[i] = value;
        low = low.min(value);
        high = high.max(value);
    }
    // A constant height offset has no normal. Centering avoids cancellation noise on flat art.
    for (i, value) in lightness.data.iter_mut().enumerate() {
        if mask[i] {
            *value -= (low + high) * 0.5;
        }
    }
    let mut shape = distance::edt(&mask, width, height);
    let reach = settings.shape_radius;
    for value in &mut shape.data {
        let inset = (*value - 0.5).max(0.0);
        *value = settings.shape_strength * reach * (inset / reach).ln_1p();
    }
    let mut combined = field::gaussian(&shape, settings.shape_softness);
    let fine = field::masked_gaussian(&lightness, &opacity, settings.detail_scale);
    let broad = field::masked_gaussian(&lightness, &opacity, settings.detail_scale * 2.0);
    for (i, value) in combined.data.iter_mut().enumerate() {
        *value += settings.detail_strength * (0.7 * fine.data[i] + 0.3 * broad.data[i]);
    }
    let mut result = neutral_map(rgba);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let i = (y as usize + pad) * width + x as usize + pad;
        if mask[i] {
            // Padding keeps every opaque pixel's 3x3 neighborhood inside the field.
            let (dx, dy) = field::sobel(&combined, i);
            let normal = slopes_to_normal(dx, dy, settings);
            let [red, green, blue] = normal.map(|v| ((v * 0.5 + 0.5) * 255.0).round() as u8);
            result.put_pixel(x, y, Rgba([red, green, blue, pixel[3]]));
        }
    }
    result
}

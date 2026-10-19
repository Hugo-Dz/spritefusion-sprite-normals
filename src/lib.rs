//! Normal maps for pixel art sprites, sprite sheets, and animations.

mod color;
mod components;
mod distance;
mod field;
pub mod media;
mod relief;
mod settings;
#[cfg(target_arch = "wasm32")]
mod wasm;

use image::{Rgba, RgbaImage};
use serde::Serialize;
use thiserror::Error;

pub use settings::{NormalFormat, NormalSettings};
#[cfg(target_arch = "wasm32")]
pub use wasm::{NormalMapResult, generate_normal_map};

/// Maximum pixels per image, including convolution padding.
pub const MAX_PIXELS: usize = 4_194_304;

#[derive(Debug, Error)]
pub enum GeneratorError {
    #[error("invalid setting: {0}")]
    InvalidSetting(String),
    #[error("image is empty or exceeds the {MAX_PIXELS}-pixel processing limit")]
    ImageDimensions,
    #[error("image dimensions must be exact multiples of frame-size")]
    FrameDimensions,
    #[error(transparent)]
    Media(#[from] media::MediaError),
}

#[derive(Debug, Serialize)]
pub struct Metadata {
    pub width: u32,
    pub height: u32,
    pub animation: Option<media::Animation>,
}

pub struct ProcessedSprite {
    pub normal: RgbaImage,
    pub color: RgbaImage,
    pub metadata: Metadata,
}

/// Decode PNG/GIF/APNG and generate matching color and normal atlases.
///
/// # Errors
/// Returns an error for invalid images, settings, frame dimensions, or resource limits.
pub fn process_image(
    bytes: &[u8],
    settings: &NormalSettings,
) -> Result<ProcessedSprite, GeneratorError> {
    settings.validate()?;
    let mut asset = media::decode(bytes)?;
    let mut settings = settings.clone();
    if let Some(animation) = &asset.animation {
        let frame_size = (animation.frame_width, animation.frame_height);
        if settings.frame_size.is_some_and(|size| size != frame_size) {
            return Err(GeneratorError::InvalidSetting(
                "animation frame dimensions come from the file".into(),
            ));
        }
        settings.frame_size = Some(frame_size);
    }
    let normal = generate_normal(&asset.rgba, &settings)?;
    if asset.animation.is_none() {
        if let Some((width, height)) = settings.frame_size {
            let columns = asset.rgba.width() / width;
            let count = (columns * (asset.rgba.height() / height)) as usize;
            asset.animation = Some(media::Animation {
                frame_width: width,
                frame_height: height,
                columns,
                frame_count: count,
                delays_ms: vec![[100, 1]; count],
            });
        }
    }
    Ok(ProcessedSprite {
        metadata: Metadata {
            width: normal.width(),
            height: normal.height(),
            animation: asset.animation,
        },
        normal,
        color: asset.rgba,
    })
}

/// Generate a normal map with the source dimensions and alpha.
/// Disconnected silhouettes and explicit frame cells are filtered independently.
///
/// # Errors
/// Returns an error for invalid settings, excessive dimensions, or an invalid frame grid.
pub fn generate_normal(
    rgba: &RgbaImage,
    settings: &NormalSettings,
) -> Result<RgbaImage, GeneratorError> {
    settings.validate()?;
    let (width, height) = (rgba.width(), rgba.height());
    let pad = settings.padding();
    let padded_pixels = (width as usize + 2 * pad).checked_mul(height as usize + 2 * pad);
    if width == 0 || height == 0 || padded_pixels.is_none_or(|n| n > MAX_PIXELS) {
        return Err(GeneratorError::ImageDimensions);
    }
    let (fw, fh) = settings.frame_size.unwrap_or((width, height));
    if width % fw != 0 || height % fh != 0 {
        return Err(GeneratorError::FrameDimensions);
    }
    let mut result = neutral_map(rgba);
    for y in (0..height).step_by(fh as usize) {
        for x in (0..width).step_by(fw as usize) {
            let frame = image::imageops::crop_imm(rgba, x, y, fw, fh).to_image();
            let mask: Vec<bool> = frame
                .pixels()
                .map(|p| f32::from(p[3]) / 255.0 > settings.alpha_threshold)
                .collect();
            for part in components::components(&mask, fw as usize, fh as usize) {
                let mut island = RgbaImage::new(
                    (part.max_x - part.min_x + 1) as u32,
                    (part.max_y - part.min_y + 1) as u32,
                );
                for &index in &part.pixels {
                    let (sx, sy) = (index % fw as usize, index / fw as usize);
                    island.put_pixel(
                        (sx - part.min_x) as u32,
                        (sy - part.min_y) as u32,
                        *frame.get_pixel(sx as u32, sy as u32),
                    );
                }
                let generated = relief::generate(&island, settings);
                for index in part.pixels {
                    let (sx, sy) = (index % fw as usize, index / fw as usize);
                    result.put_pixel(
                        x + sx as u32,
                        y + sy as u32,
                        *generated.get_pixel((sx - part.min_x) as u32, (sy - part.min_y) as u32),
                    );
                }
            }
        }
    }
    Ok(result)
}

fn neutral_map(rgba: &RgbaImage) -> RgbaImage {
    RgbaImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        Rgba([128, 128, 255, rgba.get_pixel(x, y)[3]])
    })
}

fn slopes_to_normal(dx: f32, dy_image: f32, settings: &NormalSettings) -> [f32; 3] {
    let mut x = -dx * settings.normal_strength;
    let mut y = dy_image * settings.normal_strength;
    if settings.normal_format == NormalFormat::Directx {
        y = -y;
    }
    if settings.invert_x {
        x = -x;
    }
    if settings.invert_y {
        y = -y;
    }
    let length = x.mul_add(x, y.mul_add(y, 1.0)).sqrt();
    [x / length, y / length, 1.0 / length]
}

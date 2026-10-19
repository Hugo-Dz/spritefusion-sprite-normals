//! Shared PNG/GIF/APNG decoding. Animated sources become a lossless, frame-isolated PNG atlas.

use std::io::Cursor;

use image::{
    AnimationDecoder, ColorType, ImageDecoder, ImageEncoder, ImageFormat, RgbaImage,
    codecs::{
        gif::GifDecoder,
        png::{PngDecoder, PngEncoder},
    },
};
use serde::Serialize;
use thiserror::Error;

use crate::MAX_PIXELS;

pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_FRAMES: usize = 256;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("image decode/encode failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("use PNG, a PNG sprite sheet, GIF, or APNG")]
    Format,
    #[error("image exceeds the input, frame-count, or decoded-pixel limit")]
    Limit,
}

#[derive(Clone, Debug, Serialize)]
pub struct Animation {
    pub frame_width: u32,
    pub frame_height: u32,
    pub columns: u32,
    pub frame_count: usize,
    /// Exact numerator/denominator pairs in milliseconds, in row-major frame order.
    pub delays_ms: Vec<[u32; 2]>,
}

pub struct DecodedAsset {
    pub rgba: RgbaImage,
    pub animation: Option<Animation>,
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), MediaError> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || pixels > MAX_PIXELS as u64 {
        return Err(MediaError::Limit);
    }
    Ok(())
}

/// Inspect animation headers without decoding frames. Used to reserve CLI output paths.
///
/// # Errors
/// Rejects unsupported formats, malformed headers, and excessive input bytes.
pub fn is_animation(bytes: &[u8]) -> Result<bool, MediaError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(MediaError::Limit);
    }
    match image::guess_format(bytes)? {
        ImageFormat::Gif => Ok(true),
        ImageFormat::Png => Ok(PngDecoder::new(Cursor::new(bytes))?.is_apng()),
        _ => Err(MediaError::Format),
    }
}

/// Decode with frame compositing/disposal handled by the image codecs.
///
/// # Errors
/// Rejects malformed/unsupported images and excessive dimensions, frames, or allocations.
pub fn decode(bytes: &[u8]) -> Result<DecodedAsset, MediaError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(MediaError::Limit);
    }
    let cursor = Cursor::new(bytes);
    match image::guess_format(bytes)? {
        ImageFormat::Png => {
            let mut decoder = PngDecoder::new(cursor)?;
            let (w, h) = decoder.dimensions();
            validate_dimensions(w, h)?;
            decoder.set_limits(limits())?;
            if decoder.is_apng() {
                pack_frames(decoder.apng().into_frames(), w, h)
            } else {
                Ok(DecodedAsset {
                    rgba: image::DynamicImage::from_decoder(decoder)?.to_rgba8(),
                    animation: None,
                })
            }
        }
        ImageFormat::Gif => {
            let mut decoder = GifDecoder::new(cursor)?;
            let (w, h) = decoder.dimensions();
            validate_dimensions(w, h)?;
            decoder.set_limits(limits())?;
            pack_frames(decoder.into_frames(), w, h)
        }
        _ => Err(MediaError::Format),
    }
}

fn limits() -> image::io::Limits {
    let mut limits = image::io::Limits::default();
    limits.max_alloc = Some((MAX_PIXELS * 16) as u64);
    limits
}

fn pack_frames(
    frames: image::Frames<'_>,
    width: u32,
    height: u32,
) -> Result<DecodedAsset, MediaError> {
    let mut images = Vec::new();
    let mut delays = Vec::new();
    for frame in frames {
        if images.len() >= MAX_FRAMES
            || (images.len() + 1) as u64 * u64::from(width) * u64::from(height) > MAX_PIXELS as u64
        {
            return Err(MediaError::Limit);
        }
        let frame = frame?;
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        delays.push([numerator, denominator]);
        let image = frame.into_buffer();
        if image.dimensions() != (width, height) {
            return Err(MediaError::Format);
        }
        images.push(image);
    }
    if images.is_empty() {
        return Err(MediaError::Format);
    }
    let count = images.len();
    let columns = (count as f64).sqrt().ceil() as u32;
    let rows = (count as u32).div_ceil(columns);
    let atlas_width = width.checked_mul(columns).ok_or(MediaError::Limit)?;
    let atlas_height = height.checked_mul(rows).ok_or(MediaError::Limit)?;
    validate_dimensions(atlas_width, atlas_height)?;
    let mut rgba = RgbaImage::new(atlas_width, atlas_height);
    for (i, image) in images.into_iter().enumerate() {
        image::imageops::replace(
            &mut rgba,
            &image,
            i64::from(i as u32 % columns * width),
            i64::from(i as u32 / columns * height),
        );
    }
    Ok(DecodedAsset {
        rgba,
        animation: Some(Animation {
            frame_width: width,
            frame_height: height,
            columns,
            frame_count: count,
            delays_ms: delays,
        }),
    })
}

/// Encode normals without palette quantization or a color-space transformation.
///
/// # Errors
/// Returns PNG encoder errors.
pub fn png(rgba: &RgbaImage) -> Result<Vec<u8>, MediaError> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
        ColorType::Rgba8,
    )?;
    Ok(bytes)
}

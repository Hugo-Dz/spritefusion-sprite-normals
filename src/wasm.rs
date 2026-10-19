use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::{NormalFormat, NormalSettings, media, process_image};

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    strength: Option<f32>,
    detail: Option<f32>,
    smoothing: Option<f32>,
    volume: Option<f32>,
    frame_width: Option<u32>,
    frame_height: Option<u32>,
    directx: bool,
}

#[wasm_bindgen]
pub struct NormalMapResult {
    normal_png: Vec<u8>,
    color_png: Vec<u8>,
    metadata: String,
}

#[wasm_bindgen]
impl NormalMapResult {
    #[wasm_bindgen(getter)]
    pub fn normal_png(&self) -> Vec<u8> {
        self.normal_png.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn color_png(&self) -> Vec<u8> {
        self.color_png.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> String {
        self.metadata.clone()
    }
}

fn process(bytes: &[u8], options: &str) -> Result<NormalMapResult, String> {
    let options: Options =
        serde_json::from_str(options).map_err(|e| format!("Invalid options: {e}"))?;
    let defaults = NormalSettings::default();
    let settings = NormalSettings {
        normal_strength: options.strength.unwrap_or(defaults.normal_strength),
        detail_strength: options.detail.unwrap_or(defaults.detail_strength),
        detail_scale: options.smoothing.unwrap_or(defaults.detail_scale),
        shape_strength: options.volume.unwrap_or(defaults.shape_strength),
        normal_format: if options.directx {
            NormalFormat::Directx
        } else {
            NormalFormat::Opengl
        },
        frame_size: match (options.frame_width, options.frame_height) {
            (None, None) => None,
            (Some(w), Some(h)) => Some((w, h)),
            _ => return Err("Supply both frame_width and frame_height".into()),
        },
        ..defaults
    };
    let result = process_image(bytes, &settings).map_err(|e| e.to_string())?;
    Ok(NormalMapResult {
        normal_png: media::png(&result.normal).map_err(|e| e.to_string())?,
        color_png: media::png(&result.color).map_err(|e| e.to_string())?,
        metadata: serde_json::to_string(&result.metadata).map_err(|e| e.to_string())?,
    })
}

/// Generate PNG normal maps from PNG, GIF, or APNG bytes. Pass "{}" for default options.
/// Call `free()` on the result after copying its arrays and metadata.
///
/// # Errors
/// Throws for invalid options, malformed images, and resource limits.
#[wasm_bindgen]
pub fn generate_normal_map(bytes: &[u8], options_json: &str) -> Result<NormalMapResult, JsValue> {
    process(bytes, options_json).map_err(|e| JsValue::from_str(&e))
}

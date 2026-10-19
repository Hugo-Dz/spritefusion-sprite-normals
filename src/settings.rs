use serde::Serialize;

use crate::GeneratorError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "kebab-case")]
pub enum NormalFormat {
    #[default]
    Opengl,
    Directx,
}

/// Heights and smoothing radii are measured in native sprite pixels.
#[derive(Clone, Debug)]
pub struct NormalSettings {
    /// Pixels at or below this opacity keep neutral normals.
    pub alpha_threshold: f32,
    /// Distance over which silhouette slopes fall off.
    pub shape_radius: f32,
    pub shape_strength: f32,
    pub shape_softness: f32,
    pub detail_strength: f32,
    pub detail_scale: f32,
    pub normal_strength: f32,
    pub normal_format: NormalFormat,
    pub invert_x: bool,
    pub invert_y: bool,
    /// Uniform sprite-sheet cells, processed independently.
    pub frame_size: Option<(u32, u32)>,
}

impl Default for NormalSettings {
    fn default() -> Self {
        Self {
            alpha_threshold: 0.03,
            shape_radius: 4.0,
            shape_strength: 1.15,
            shape_softness: 1.15,
            detail_strength: 6.0,
            detail_scale: 0.6,
            normal_strength: 1.5,
            normal_format: NormalFormat::Opengl,
            invert_x: false,
            invert_y: false,
            frame_size: None,
        }
    }
}

impl NormalSettings {
    /// # Errors
    /// Rejects non-finite values, settings outside their ranges, and empty frame cells.
    pub fn validate(&self) -> Result<(), GeneratorError> {
        for (name, value, min, max) in [
            ("alpha-threshold", self.alpha_threshold, 0.0, 0.99),
            ("shape-radius", self.shape_radius, 0.5, 512.0),
            ("shape-strength", self.shape_strength, 0.0, 8.0),
            ("shape-softness", self.shape_softness, 0.0, 8.0),
            ("detail-strength", self.detail_strength, 0.0, 8.0),
            ("detail-scale", self.detail_scale, 0.6, 8.0),
            ("normal-strength", self.normal_strength, 0.0, 8.0),
        ] {
            if !value.is_finite() || value < min || value > max {
                return Err(GeneratorError::InvalidSetting(format!(
                    "{name} must be finite and between {min} and {max}"
                )));
            }
        }
        if self.frame_size.is_some_and(|(w, h)| w == 0 || h == 0) {
            return Err(GeneratorError::InvalidSetting(
                "frame-size dimensions must be positive".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn padding(&self) -> usize {
        (3.0 * self.shape_softness.max(self.detail_scale * 2.0)).ceil() as usize + 3
    }
}

//! Scalar image operations. All working images stay at the input pixel resolution.

/// A row-major scalar image, in native sprite pixel coordinates.
#[derive(Clone)]
pub(crate) struct Field {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl Field {
    pub(crate) fn zero(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; width * height],
        }
    }
}

/// Separable Gaussian with zero extension. Shape height is allowed to extend beyond alpha.
/// Taps accumulate in kernel order a whole row at a time, so the inner loops vectorize.
pub(crate) fn gaussian(input: &Field, sigma: f32) -> Field {
    if sigma < 0.05 {
        return input.clone();
    }
    let radius = (sigma * 3.0).ceil() as isize;
    let mut kernel: Vec<f32> = (-radius..=radius)
        .map(|i| (-0.5 * (i as f32 / sigma).powi(2)).exp())
        .collect();
    let total: f32 = kernel.iter().sum();
    for value in &mut kernel {
        *value /= total;
    }
    let width = input.width;
    let mut horizontal = Field::zero(width, input.height);
    let rows = input.data.chunks_exact(width);
    for (output, source) in horizontal.data.chunks_exact_mut(width).zip(rows) {
        for (offset, &weight) in (-radius..=radius).zip(&kernel) {
            // Output x reads source x + offset; taps outside the row are zero.
            let skip = offset.unsigned_abs().min(width);
            let (output, source) = if offset < 0 {
                (&mut output[skip..], &source[..width - skip])
            } else {
                (&mut output[..width - skip], &source[skip..])
            };
            add_scaled(output, source, weight);
        }
    }
    let mut result = Field::zero(width, input.height);
    for (y, output) in result.data.chunks_exact_mut(width).enumerate() {
        for (offset, &weight) in (-radius..=radius).zip(&kernel) {
            if let Some(row) = y
                .checked_add_signed(offset)
                .filter(|&row| row < input.height)
            {
                add_scaled(output, &horizontal.data[row * width..][..width], weight);
            }
        }
    }
    result
}

fn add_scaled(output: &mut [f32], source: &[f32], weight: f32) {
    for (value, sample) in output.iter_mut().zip(source) {
        *value += sample * weight;
    }
}

/// Opacity-weighted normalized convolution; hidden RGB never contributes.
pub(crate) fn masked_gaussian(input: &Field, weight: &Field, sigma: f32) -> Field {
    let mut weighted = input.clone();
    for (value, alpha) in weighted.data.iter_mut().zip(&weight.data) {
        *value *= alpha;
    }
    let numerator = gaussian(&weighted, sigma);
    let denominator = gaussian(weight, sigma);
    let mut result = numerator;
    for (value, weight) in result.data.iter_mut().zip(denominator.data) {
        *value /= weight.max(1e-8);
    }
    result
}

/// Normalized Sobel *correlation* at a pixel away from the border:
/// positive X slopes increase to the right, Y downward.
pub(crate) fn sobel(input: &Field, index: usize) -> (f32, f32) {
    let d = &input.data;
    let (up, down) = (index - input.width, index + input.width);
    let dx = (d[up + 1] + 2.0 * d[index + 1] + d[down + 1]
        - d[up - 1]
        - 2.0 * d[index - 1]
        - d[down - 1])
        * 0.125;
    let dy =
        (d[down - 1] + 2.0 * d[down] + d[down + 1] - d[up - 1] - 2.0 * d[up] - d[up + 1]) * 0.125;
    (dx, dy)
}

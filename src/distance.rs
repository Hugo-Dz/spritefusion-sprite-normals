//! Exact Euclidean distance transform using separable lower envelopes of parabolas.

use crate::field::Field;

fn squared_line(source: &[f32], destination: &mut [f32]) {
    let mut sites: Vec<usize> = Vec::with_capacity(source.len());
    let mut starts: Vec<f64> = Vec::with_capacity(source.len());
    for (q, &height) in source.iter().enumerate() {
        if !height.is_finite() {
            continue;
        }
        let mut intersection = f64::NEG_INFINITY;
        while let Some(&p) = sites.last() {
            intersection = (f64::from(height) + (q as f64).powi(2)
                - f64::from(source[p])
                - (p as f64).powi(2))
                / (2.0 * (q - p) as f64);
            if intersection > *starts.last().expect("site has a start") {
                break;
            }
            sites.pop();
            starts.pop();
        }
        if sites.is_empty() {
            intersection = f64::NEG_INFINITY;
        }
        sites.push(q);
        starts.push(intersection);
    }
    if sites.is_empty() {
        destination.fill(f32::INFINITY);
        return;
    }
    let mut active = 0;
    for (q, output) in destination.iter_mut().enumerate() {
        while active + 1 < starts.len() && starts[active + 1] < q as f64 {
            active += 1;
        }
        let delta = q as f32 - sites[active] as f32;
        *output = delta.mul_add(delta, source[sites[active]]);
    }
}

pub(crate) fn edt(mask: &[bool], width: usize, height: usize) -> Field {
    let mut result = Field::zero(width, height);
    let mut source = vec![0.0; width.max(height)];
    let mut destination = source.clone();
    for y in 0..height {
        for x in 0..width {
            source[x] = if mask[y * width + x] {
                f32::INFINITY
            } else {
                0.0
            };
        }
        squared_line(&source[..width], &mut destination[..width]);
        result.data[y * width..(y + 1) * width].copy_from_slice(&destination[..width]);
    }
    for x in 0..width {
        for (y, value) in source.iter_mut().take(height).enumerate() {
            *value = result.data[y * width + x];
        }
        squared_line(&source[..height], &mut destination[..height]);
        for (y, value) in destination.iter().take(height).enumerate() {
            result.data[y * width + x] = value.sqrt();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_distance_matches_brute_force_on_small_random_masks() {
        let mut state = 75_u32;
        for height in 1..13 {
            for width in 1..17 {
                let mask: Vec<bool> = (0..width * height)
                    .map(|_| {
                        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        state % 4 != 0
                    })
                    .collect();
                let distances = edt(&mask, width, height);
                for (index, &actual) in distances.data.iter().enumerate() {
                    let expected = mask
                        .iter()
                        .enumerate()
                        .filter(|(_, inside)| !**inside)
                        .map(|(other, _)| {
                            let dx = (index % width) as f32 - (other % width) as f32;
                            let dy = (index / width) as f32 - (other / width) as f32;
                            dx.hypot(dy)
                        })
                        .fold(f32::INFINITY, f32::min);
                    assert!(
                        (actual.is_infinite() && expected.is_infinite())
                            || (actual - expected).abs() < 1e-5
                    );
                }
            }
        }
    }
}

pub(crate) struct Component {
    pub pixels: Vec<usize>,
    pub min_x: usize,
    pub min_y: usize,
    pub max_x: usize,
    pub max_y: usize,
}

/// Eight-connected islands retain diagonal pixel-art strokes.
pub(crate) fn components(mask: &[bool], width: usize, height: usize) -> Vec<Component> {
    let mut visited = vec![false; mask.len()];
    let mut result = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || visited[start] {
            continue;
        }
        let mut part = Component {
            pixels: vec![start],
            min_x: start % width,
            min_y: start / width,
            max_x: start % width,
            max_y: start / width,
        };
        visited[start] = true;
        let mut cursor = 0;
        while cursor < part.pixels.len() {
            let index = part.pixels[cursor];
            let (x, y) = (index % width, index / width);
            part.min_x = part.min_x.min(x);
            part.max_x = part.max_x.max(x);
            part.min_y = part.min_y.min(y);
            part.max_y = part.max_y.max(y);
            for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let neighbor = ny * width + nx;
                    if mask[neighbor] && !visited[neighbor] {
                        visited[neighbor] = true;
                        part.pixels.push(neighbor);
                    }
                }
            }
            cursor += 1;
        }
        result.push(part);
    }
    result
}

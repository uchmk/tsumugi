//! The app's icon (the design's Logo, A): two threads, cyan and gold, on a
//! dark tile, drawn here at any size so no image file is carried. The band
//! draws the same mark (`chrome::top_band`).

/// The two threads in the logo's own 16-unit box: each two cubic Béziers,
/// as the design's SVG writes them (`M2 4c3 0 3 8 6 8s3-8 6-8` and
/// `M2 8c3 0 3 4 6 4s3-4 6-4`).
const THREADS: [([[f32; 2]; 7], [u8; 3]); 2] = [
    ([[2.0, 4.0], [5.0, 4.0], [5.0, 12.0], [8.0, 12.0], [11.0, 12.0], [11.0, 4.0], [14.0, 4.0]], [0x6f, 0xd0, 0xd0]),
    ([[2.0, 8.0], [5.0, 8.0], [5.0, 12.0], [8.0, 12.0], [11.0, 12.0], [11.0, 8.0], [14.0, 8.0]], [0xe8, 0xc8, 0x7a]),
];

/// The tile's colour: the design's window ground.
const TILE: [u8; 3] = [0x16, 0x18, 0x1d];

/// The icon as RGBA, `size` × `size`.
pub fn pixels(size: u32) -> Vec<u8> {
    let n = size as usize;
    let unit = size as f32 / 16.0;
    // Thicker at small sizes, as the design's 32 and 16 px versions are.
    let width = if size <= 16 { 2.0 } else if size <= 32 { 1.8 } else { 1.5 } * unit;
    let lines: Vec<(Vec<[f32; 2]>, [u8; 3])> = THREADS.iter().map(|(p, c)| (curve(p, unit), *c)).collect();
    let mut out = vec![0u8; n * n * 4];
    let radius = 3.5 * unit;
    for y in 0..n {
        for x in 0..n {
            let at = [x as f32 + 0.5, y as f32 + 0.5];
            let tile = rounded(at, size as f32, radius);
            if tile <= 0.0 {
                continue;
            }
            let mut rgb = TILE.map(f32::from);
            for (line, color) in &lines {
                let d = distance(at, line);
                let cover = (width / 2.0 - d + 0.5).clamp(0.0, 1.0);
                for k in 0..3 {
                    rgb[k] += (f32::from(color[k]) - rgb[k]) * cover;
                }
            }
            let i = (y * n + x) * 4;
            out[i..i + 3].copy_from_slice(&rgb.map(|v| v.round() as u8));
            out[i + 3] = (tile * 255.0).round() as u8;
        }
    }
    out
}

/// The two Béziers as a run of points, in pixels.
fn curve(p: &[[f32; 2]; 7], unit: f32) -> Vec<[f32; 2]> {
    let mut out = Vec::new();
    for seg in [&p[0..4], &p[3..7]] {
        for k in 0..=24 {
            let t = k as f32 / 24.0;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            let x: f32 = (0..4).map(|j| w[j] * seg[j][0]).sum();
            let y: f32 = (0..4).map(|j| w[j] * seg[j][1]).sum();
            out.push([x * unit, y * unit]);
        }
    }
    out
}

/// How far `p` is from the run of points (its segments).
fn distance(p: [f32; 2], line: &[[f32; 2]]) -> f32 {
    line.windows(2)
        .map(|s| {
            let (a, b) = (s[0], s[1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = dx * dx + dy * dy;
            let t = if len == 0.0 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len).clamp(0.0, 1.0) };
            let (cx, cy) = (a[0] + t * dx, a[1] + t * dy);
            ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt()
        })
        .fold(f32::MAX, f32::min)
}

/// How much of the pixel at `p` is inside the rounded tile, 0 to 1.
fn rounded(p: [f32; 2], size: f32, r: f32) -> f32 {
    let q = |v: f32| (v - size / 2.0).abs() - (size / 2.0 - r);
    let (qx, qy) = (q(p[0]), q(p[1]));
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
    (0.5 - outside).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_has_both_threads_on_its_tile() {
        let px = pixels(64);
        assert_eq!(px.len(), 64 * 64 * 4);
        let at = |x: usize, y: usize| &px[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(at(0, 0)[3], 0, "the corner is cut round");
        assert_eq!(at(32, 2), &[0x16, 0x18, 0x1d, 255], "the tile");
        // The cyan thread passes (5, 8) units, the gold one starts at (2, 8);
        // they meet at (8, 12), the gold one over.
        let cyan = at(20, 32);
        assert!(cyan[2] > 0xb0 && cyan[0] < 0x90, "cyan where the first thread dips: {cyan:?}");
        let gold = at(8, 32);
        assert!(gold[0] > 0xc0 && gold[2] < 0x90, "gold where the second starts: {gold:?}");
        assert_eq!(pixels(16).len(), 16 * 16 * 4);
    }
}

// Hex pixel renderer — renders HexRect[] data to canvas bitmap
// BNRi's 96×96 hex pixel art, rendered client-side (frontend law)
// SPDX-License-Identifier: AGPL-3.0-only
//
// Z-1: correct pointy-top hexagon fill (half-plane inequalities)
// Z-2: HexRect 8-byte packed codec (writer + reader, shared vectors)

use image::{DynamicImage, Rgba, RgbaImage};

// ──────────────────────────────────────────────────────────
// HexRect struct (matches on-chain SSTORE2 data)
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexRect {
    pub col: u8,
    pub row: u8,
    pub width: u8,
    pub height: u8,
    pub color: u32, // 0xRRGGBB
}

// ──────────────────────────────────────────────────────────
// Z-2: HexRect 8-byte packed codec
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexCodecError {
    NotMultipleOf8,
    ReservedByteNonZero { index: usize, value: u8 },
    OutOfGrid { col: u8, row: u8 },
    ZeroDimension { col: u8, row: u8 },
}

/// Encode a slice of HexRects into the packed 8-byte-per-rect format.
///
/// Layout (big-endian, no padding):
///   byte 0      col    (u8, 0..=95)
///   byte 1      row    (u8, 0..=95)
///   byte 2      width  (u8, >= 1)
///   byte 3      height (u8, >= 1)
///   bytes 4..6  color  (uint24, 0xRRGGBB, big-endian)
///   byte 7      reserved, MUST be 0x00
pub fn encode_hexrects(rects: &[HexRect]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rects.len() * 8);
    for r in rects {
        out.push(r.col);
        out.push(r.row);
        out.push(r.width);
        out.push(r.height);
        // color big-endian uint24: RR GG BB
        out.push(((r.color >> 16) & 0xFF) as u8);
        out.push(((r.color >> 8) & 0xFF) as u8);
        out.push((r.color & 0xFF) as u8);
        out.push(0x00); // reserved
    }
    out
}

/// Parse a packed blob into a Vec<HexRect>.
///
/// Returns an error on any malformed data — never clamps, never silently
/// drops bytes. The blob length MUST be a multiple of 8.
pub fn parse_hexrects(data: &[u8]) -> Result<Vec<HexRect>, HexCodecError> {
    if !data.len().is_multiple_of(8) {
        return Err(HexCodecError::NotMultipleOf8);
    }

    let mut rects = Vec::with_capacity(data.len() / 8);
    for (i, chunk) in data.chunks_exact(8).enumerate() {
        let col = chunk[0];
        let row = chunk[1];
        let width = chunk[2];
        let height = chunk[3];
        let color =
            ((chunk[4] as u32) << 16) | ((chunk[5] as u32) << 8) | (chunk[6] as u32);
        let reserved = chunk[7];

        if reserved != 0x00 {
            return Err(HexCodecError::ReservedByteNonZero {
                index: i,
                value: reserved,
            });
        }
        if col > 95 || row > 95 {
            return Err(HexCodecError::OutOfGrid { col, row });
        }
        if width == 0 || height == 0 {
            return Err(HexCodecError::ZeroDimension { col, row });
        }

        rects.push(HexRect {
            col,
            row,
            width,
            height,
            color,
        });
    }
    Ok(rects)
}

// ──────────────────────────────────────────────────────────
// Z-1: Hex grid math (pointy-top, odd-row offset)
// ──────────────────────────────────────────────────────────

/// Compute the pixel-space center of a hex at grid position (col, row)
/// with circumradius `r` (scale).
///
/// Pointy-top grid with odd-row offset:
///   cx = sqrt(3) * r * (col + 0.5 * (row & 1)) + r
///   cy = 1.5 * r * row + r
pub fn hex_center(col: u32, row: u32, r: f64) -> (f64, f64) {
    let sqrt3 = 3f64.sqrt();
    let cx = sqrt3 * r * (col as f64 + 0.5 * (row & 1) as f64) + r;
    let cy = 1.5 * r * row as f64 + r;
    (cx, cy)
}

/// Compute the 6 vertices of a pointy-top hexagon with circumradius `r`
/// centred at (cx, cy).
///
/// Vertices are returned clockwise starting from the top vertex:
///   V0 (top), V1 (top-right), V2 (bottom-right),
///   V3 (bottom), V4 (bottom-left), V5 (top-left)
pub fn hex_vertices(cx: f64, cy: f64, r: f64) -> [(f64, f64); 6] {
    let sqrt3_2 = 3f64.sqrt() / 2.0;
    let r_half = r / 2.0;
    [
        (cx, cy - r),           // V0 top
        (cx + r * sqrt3_2, cy - r_half), // V1 top-right
        (cx + r * sqrt3_2, cy + r_half), // V2 bottom-right
        (cx, cy + r),           // V3 bottom
        (cx - r * sqrt3_2, cy + r_half), // V4 bottom-left
        (cx - r * sqrt3_2, cy - r_half), // V5 top-left
    ]
}

/// Point-in-regular-hexagon test for a pointy-top hex of circumradius `r`
/// centred at (cx, cy).
///
/// Uses three half-plane inequalities (reduced from six by symmetry):
///   1. |dx| <= r * sqrt(3) / 2          (vertical edges, left/right)
///   2. |dy| <= r                         (top/bottom vertices)
///   3. |dy| + |dx| / sqrt(3) <= r        (diagonal edges)
///
/// Inequality 2 is implied by 3 for all dx >= 0 (since dy + dx/sqrt3 >= dy
/// when dx >= 0, so if 3 holds then dy <= r). Kept as a fast reject.
///
/// The apothem is computed as `r * sqrt3_2` (same expression as hex_vertices)
/// to ensure floating-point consistency between vertex generation and
/// point-in-hex testing. Boundary is inclusive (<=).
pub fn point_in_hex(px: f64, py: f64, cx: f64, cy: f64, r: f64) -> bool {
    let dx = (px - cx).abs();
    let dy = (py - cy).abs();
    let sqrt3 = 3f64.sqrt();
    let sqrt3_2 = sqrt3 / 2.0;
    let apothem = r * sqrt3_2; // Must match hex_vertices computation

    dx <= apothem && dy <= r && dy + dx / sqrt3 <= r
}

/// Test whether a point is on the 1px border of the hex (for XL bold stroke).
/// Returns true if the point is inside the hex but within 1px of any edge.
fn is_border_pixel(px: f64, py: f64, cx: f64, cy: f64, r: f64) -> bool {
    let dx = (px - cx).abs();
    let dy = (py - cy).abs();
    let sqrt3 = 3f64.sqrt();
    let apothem = r * sqrt3 / 2.0;

    // Check if within 1px of any of the three edge constraints
    dx >= apothem - 1.0 || dy >= r - 1.0 || dy + dx / sqrt3 >= r - 1.0
}

// ──────────────────────────────────────────────────────────
// Z-1: Rendering
// ──────────────────────────────────────────────────────────

/// Render a list of HexRects to a bitmap at the given scale.
/// scale = pixels per hex circumradius (30 = XL bold stroke standard).
pub fn render_hexrects(rects: &[HexRect], scale: u32) -> DynamicImage {
    let grid_size = 96u32;
    let sqrt3 = 3f64.sqrt();
    let s = scale as f64;

    let img_w = (grid_size as f64 * sqrt3 * s + s * 2.0) as u32;
    let img_h = (grid_size as f64 * 1.5 * s + s * 2.0) as u32;

    let mut img = RgbaImage::new(img_w, img_h);

    // Fill background #0A0A0F
    for pixel in img.pixels_mut() {
        *pixel = Rgba([10, 10, 15, 255]);
    }

    // Expand rects to individual hex positions and render each
    for rect in rects {
        let color = uint24_to_rgba(rect.color);

        for dy in 0..rect.height {
            for dx in 0..rect.width {
                let col = rect.col as u32 + dx as u32;
                let row = rect.row as u32 + dy as u32;
                let (cx, cy) = hex_center(col, row, s);
                draw_hex(&mut img, cx, cy, s, color, scale);
            }
        }
    }

    DynamicImage::ImageRgba8(img)
}

fn uint24_to_rgba(color: u32) -> Rgba<u8> {
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    Rgba([r, g, b, 255])
}

fn draw_hex(img: &mut RgbaImage, cx: f64, cy: f64, r: f64, color: Rgba<u8>, scale: u32) {
    let (width, height) = img.dimensions();

    let x0 = (cx - r).max(0.0) as u32;
    let x1 = ((cx + r).min(width as f64) + 1.0) as u32;
    let y0 = (cy - r).max(0.0) as u32;
    let y1 = ((cy + r).min(height as f64) + 1.0) as u32;

    for y in y0..y1.min(height) {
        for x in x0..x1.min(width) {
            let px = x as f64;
            let py = y as f64;

            if point_in_hex(px, py, cx, cy, r) {
                if scale >= 10 && is_border_pixel(px, py, cx, cy, r) {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255])); // Black border
                } else {
                    img.put_pixel(x, y, color);
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────
// Unit tests (inline — for module-level correctness checks)
// ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_in_hex_center() {
        let (cx, cy) = hex_center(0, 0, 30.0);
        assert!(point_in_hex(cx, cy, cx, cy, 30.0));
    }

    #[test]
    fn test_point_outside_hex() {
        let (cx, cy) = hex_center(0, 0, 30.0);
        // Point well outside (100px to the right)
        assert!(!point_in_hex(cx + 100.0, cy, cx, cy, 30.0));
    }

    #[test]
    fn test_point_on_top_vertex() {
        let (cx, cy) = hex_center(0, 0, 30.0);
        // Top vertex: (cx, cy - r) = exactly on boundary
        assert!(point_in_hex(cx, cy - 30.0, cx, cy, 30.0));
    }

    #[test]
    fn test_point_just_outside_top() {
        let (cx, cy) = hex_center(0, 0, 30.0);
        // 1px beyond top vertex
        assert!(!point_in_hex(cx, cy - 31.0, cx, cy, 30.0));
    }

    #[test]
    fn test_hex_vertices_circumradius() {
        let (cx, cy) = hex_center(0, 0, 30.0);
        let verts = hex_vertices(cx, cy, 30.0);
        // Every vertex must be exactly circumradius (30) from center
        for (vx, vy) in &verts {
            let dist = ((vx - cx).powi(2) + (vy - cy).powi(2)).sqrt();
            assert!(
                (dist - 30.0).abs() < 0.001,
                "Vertex distance {} != circumradius 30",
                dist
            );
        }
    }

    #[test]
    fn test_no_colored_pixels_outside_hull() {
        // Render a single hex at scale 30 and verify no colored pixels
        // exist outside the convex hull of its vertices.
        let rect = HexRect {
            col: 0,
            row: 0,
            width: 1,
            height: 1,
            color: 0xFF0000, // red — easy to detect
        };
        let img = render_hexrects(&[rect], 30);
        let rgba = img.to_rgba8();

        let (cx, cy) = hex_center(0, 0, 30.0);

        for y in 0..rgba.height() {
            for x in 0..rgba.width() {
                let pixel = rgba.get_pixel(x, y);
                let is_colored = pixel[0] > 10 || pixel[1] > 10 || pixel[2] > 15;
                // Skip background and black border
                let is_background = pixel[0] == 10 && pixel[1] == 10 && pixel[2] == 15;
                let is_border = pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0;

                if is_colored && !is_background && !is_border {
                    // This is a red (fill) pixel — it must be inside the hex
                    let px = x as f64;
                    let py = y as f64;
                    assert!(
                        point_in_hex(px, py, cx, cy, 30.0),
                        "Colored pixel at ({},{}) is outside the hex hull",
                        x, y
                    );
                }
            }
        }
    }
}

// Z-1: Hex fixture tests — validates hex math against shared JSON vectors
// These same vectors must pass in the Solidity HexLib and JS HexLib.
// SPDX-License-Identifier: AGPL-3.0-only

use bnri_cosmic::hex_renderer::{hex_center, hex_vertices, point_in_hex};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct HexVectors {
    cases: Vec<HexCase>,
}

#[derive(Debug, Deserialize)]
struct HexCase {
    id: String,
    col: u32,
    row: u32,
    scale: u32,
    center: [f64; 2],
    vertices: Vec<[f64; 2]>,
    point_tests: Vec<PointTest>,
}

#[derive(Debug, Deserialize)]
struct PointTest {
    point: [f64; 2],
    expected: String,
}

fn load_vectors() -> HexVectors {
    let json = std::fs::read_to_string("tests/fixtures/hex_vectors.json")
        .expect("Failed to read hex_vectors.json");
    serde_json::from_str(&json).expect("Failed to parse hex_vectors.json")
}

#[test]
fn hex_centers_match_vectors() {
    let vectors = load_vectors();
    for case in &vectors.cases {
        let r = case.scale as f64;
        let (cx, cy) = hex_center(case.col, case.row, r);
        assert!(
            (cx - case.center[0]).abs() < 0.01,
            "case {} center x: got {}, expected {}",
            case.id,
            cx,
            case.center[0]
        );
        assert!(
            (cy - case.center[1]).abs() < 0.01,
            "case {} center y: got {}, expected {}",
            case.id,
            cy,
            case.center[1]
        );
    }
}

#[test]
fn hex_vertices_match_vectors() {
    let vectors = load_vectors();
    for case in &vectors.cases {
        let r = case.scale as f64;
        let (cx, cy) = hex_center(case.col, case.row, r);
        let verts = hex_vertices(cx, cy, r);
        assert_eq!(
            verts.len(),
            case.vertices.len(),
            "case {} vertex count mismatch",
            case.id
        );
        for (i, (got, expected)) in verts.iter().zip(case.vertices.iter()).enumerate() {
            assert!(
                (got.0 - expected[0]).abs() < 0.01,
                "case {} vertex {} x: got {}, expected {}",
                case.id,
                i,
                got.0,
                expected[0]
            );
            assert!(
                (got.1 - expected[1]).abs() < 0.01,
                "case {} vertex {} y: got {}, expected {}",
                case.id,
                i,
                got.1,
                expected[1]
            );
        }
    }
}

#[test]
fn hex_vertex_circumradius_correct() {
    let vectors = load_vectors();
    for case in &vectors.cases {
        let r = case.scale as f64;
        let (cx, cy) = hex_center(case.col, case.row, r);
        let verts = hex_vertices(cx, cy, r);
        for (i, (vx, vy)) in verts.iter().enumerate() {
            let dist = ((vx - cx).powi(2) + (vy - cy).powi(2)).sqrt();
            assert!(
                (dist - r).abs() < 0.001,
                "case {} vertex {} distance {} != circumradius {}",
                case.id,
                i,
                dist,
                r
            );
        }
    }
}

#[test]
fn point_in_hex_tests_match_vectors() {
    let vectors = load_vectors();
    for case in &vectors.cases {
        let r = case.scale as f64;
        let (cx, cy) = hex_center(case.col, case.row, r);
        for pt in &case.point_tests {
            let result = point_in_hex(pt.point[0], pt.point[1], cx, cy, r);
            match pt.expected.as_str() {
                "inside" => {
                    assert!(
                        result,
                        "case {} point ({},{}) should be inside",
                        case.id,
                        pt.point[0],
                        pt.point[1]
                    );
                }
                "outside" => {
                    assert!(
                        !result,
                        "case {} point ({},{}) should be outside",
                        case.id,
                        pt.point[0],
                        pt.point[1]
                    );
                }
                "boundary" => {
                    // Boundary points must be inside (inclusive test)
                    assert!(
                        result,
                        "case {} point ({},{}) should be on boundary (inclusive)",
                        case.id,
                        pt.point[0],
                        pt.point[1]
                    );
                }
                _ => panic!("Unknown expected value: {}", pt.expected),
            }
        }
    }
}

#[test]
fn no_edge_bleed_single_hex() {
    use bnri_cosmic::hex_renderer::{render_hexrects, HexRect};

    let rect = HexRect {
        col: 5,
        row: 5,
        width: 1,
        height: 1,
        color: 0xFF0000,
    };
    let img = render_hexrects(&[rect], 30);
    let rgba = img.to_rgba8();
    let r = 30.0f64;
    let (cx, cy) = hex_center(5, 5, r);

    for y in 0..rgba.height() {
        for x in 0..rgba.width() {
            let pixel = rgba.get_pixel(x, y);
            let is_fill = pixel[0] == 255 && pixel[1] == 0 && pixel[2] == 0;
            if is_fill {
                let px = x as f64;
                let py = y as f64;
                assert!(
                    point_in_hex(px, py, cx, cy, r),
                    "Fill pixel at ({},{}) outside hex hull — edge bleed",
                    x, y
                );
            }
        }
    }
}

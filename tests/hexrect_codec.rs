// Z-2: HexRect codec tests — round-trip property test + golden vectors + error cases
// SPDX-License-Identifier: AGPL-3.0-only

use bnri_cosmic::hex_renderer::{encode_hexrects, parse_hexrects, HexCodecError, HexRect};
use serde::Deserialize;

// ── Golden vector test ───────────────────────────────────

#[derive(Debug, Deserialize)]
struct GoldenFile {
    rects: Vec<GoldenRect>,
}

#[derive(Debug, Deserialize)]
struct GoldenRect {
    col: u8,
    row: u8,
    width: u8,
    height: u8,
    color: u32,
    hex: String,
}

#[test]
fn golden_vectors_encode_correctly() {
    let json =
        std::fs::read_to_string("tests/fixtures/hexrect_golden.json").expect("read golden file");
    let golden: GoldenFile = serde_json::from_str(&json).expect("parse golden file");

    for gr in &golden.rects {
        let rect = HexRect {
            col: gr.col,
            row: gr.row,
            width: gr.width,
            height: gr.height,
            color: gr.color,
        };
        let encoded = encode_hexrects(std::slice::from_ref(&rect));
        let expected_bytes = hex::decode(&gr.hex).expect("decode golden hex");
        assert_eq!(
            encoded, expected_bytes,
            "encode mismatch for rect col={} row={}",
            gr.col, gr.row
        );
    }
}

#[test]
fn golden_vectors_parse_correctly() {
    let json =
        std::fs::read_to_string("tests/fixtures/hexrect_golden.json").expect("read golden file");
    let golden: GoldenFile = serde_json::from_str(&json).expect("parse golden file");

    for gr in &golden.rects {
        let bytes = hex::decode(&gr.hex).expect("decode golden hex");
        let parsed = parse_hexrects(&bytes).expect("parse should succeed");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].col, gr.col);
        assert_eq!(parsed[0].row, gr.row);
        assert_eq!(parsed[0].width, gr.width);
        assert_eq!(parsed[0].height, gr.height);
        assert_eq!(parsed[0].color, gr.color);
    }
}

#[test]
fn golden_vectors_round_trip() {
    let json =
        std::fs::read_to_string("tests/fixtures/hexrect_golden.json").expect("read golden file");
    let golden: GoldenFile = serde_json::from_str(&json).expect("parse golden file");

    let rects: Vec<HexRect> = golden
        .rects
        .iter()
        .map(|gr| HexRect {
            col: gr.col,
            row: gr.row,
            width: gr.width,
            height: gr.height,
            color: gr.color,
        })
        .collect();

    let encoded = encode_hexrects(&rects);
    let parsed = parse_hexrects(&encoded).expect("round-trip parse");
    let re_encoded = encode_hexrects(&parsed);

    assert_eq!(encoded, re_encoded, "round-trip: encode → parse → encode must be byte-identical");
    assert_eq!(parsed.len(), rects.len());
    for (a, b) in parsed.iter().zip(rects.iter()) {
        assert_eq!(a, b);
    }
}

// ── Round-trip property test (1000+ rects) ───────────────

#[test]
fn property_round_trip_1000_rects() {
    // Deterministic pseudo-random generation (no external rand dep)
    let mut seed: u64 = 0xDEADBEEFCAFE1234;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed
    };

    let mut rects = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let col = (next() % 96) as u8;
        let row = (next() % 96) as u8;
        let width = ((next() % 20) + 1) as u8; // 1..=20
        let height = ((next() % 20) + 1) as u8;
        let color = (next() & 0xFFFFFF) as u32;
        rects.push(HexRect {
            col,
            row,
            width,
            height,
            color,
        });
    }

    let encoded = encode_hexrects(&rects);
    let parsed = parse_hexrects(&encoded).expect("round-trip parse must succeed");
    let re_encoded = encode_hexrects(&parsed);

    assert_eq!(encoded, re_encoded, "encode → parse → encode must be byte-identical");
    assert_eq!(parsed.len(), 1000);
    for (a, b) in parsed.iter().zip(rects.iter()) {
        assert_eq!(a, b, "round-trip content mismatch");
    }
}

// ── Error case tests ─────────────────────────────────────

#[test]
fn error_not_multiple_of_8() {
    let bad = [0x00; 7]; // 7 bytes — not a multiple of 8
    assert_eq!(parse_hexrects(&bad), Err(HexCodecError::NotMultipleOf8));
}

#[test]
fn error_reserved_byte_non_zero() {
    // Valid rect but reserved byte = 0xFF
    let bad = [0x00, 0x00, 0x01, 0x01, 0xFF, 0x00, 0x00, 0xFF];
    assert_eq!(
        parse_hexrects(&bad),
        Err(HexCodecError::ReservedByteNonZero {
            index: 0,
            value: 0xFF
        })
    );
}

#[test]
fn error_out_of_grid() {
    // col = 96 (> 95)
    let bad = [0x60, 0x00, 0x01, 0x01, 0xFF, 0x00, 0x00, 0x00];
    assert_eq!(
        parse_hexrects(&bad),
        Err(HexCodecError::OutOfGrid { col: 96, row: 0 })
    );
    // row = 96 (> 95)
    let bad2 = [0x00, 0x60, 0x01, 0x01, 0xFF, 0x00, 0x00, 0x00];
    assert_eq!(
        parse_hexrects(&bad2),
        Err(HexCodecError::OutOfGrid { col: 0, row: 96 })
    );
}

#[test]
fn error_zero_dimension() {
    // width = 0
    let bad = [0x00, 0x00, 0x00, 0x01, 0xFF, 0x00, 0x00, 0x00];
    assert_eq!(
        parse_hexrects(&bad),
        Err(HexCodecError::ZeroDimension { col: 0, row: 0 })
    );
    // height = 0
    let bad2 = [0x00, 0x00, 0x01, 0x00, 0xFF, 0x00, 0x00, 0x00];
    assert_eq!(
        parse_hexrects(&bad2),
        Err(HexCodecError::ZeroDimension { col: 0, row: 0 })
    );
}

#[test]
fn empty_blob_parses_to_empty_vec() {
    let empty: [u8; 0] = [];
    let result = parse_hexrects(&empty).expect("empty blob should parse");
    assert!(result.is_empty());
}

#[test]
fn multiple_rects_in_one_blob() {
    let rects = vec![
        HexRect {
            col: 0,
            row: 0,
            width: 1,
            height: 1,
            color: 0xFF0000,
        },
        HexRect {
            col: 95,
            row: 95,
            width: 1,
            height: 1,
            color: 0xFFFFFF,
        },
    ];
    let encoded = encode_hexrects(&rects);
    assert_eq!(encoded.len(), 16); // 2 rects × 8 bytes
    let parsed = parse_hexrects(&encoded).expect("parse multiple");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0], rects[0]);
    assert_eq!(parsed[1], rects[1]);
}

// hex decode helper (inline to avoid adding hex dependency)
mod hex {
    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if !s.len().is_multiple_of(2) {
            return Err("odd-length hex string".to_string());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string())
            })
            .collect()
    }
}

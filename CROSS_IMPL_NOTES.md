# Cross-Implementation Hex Math Notes

**For implementers of HexLib in Solidity and JavaScript.**

The Rust reference (`src/hex_renderer.rs`) is the anchor. Solidity and JS ports
must produce byte-identical results against the shared test vectors
(`tests/fixtures/hex_vectors.json`). This document captures the one finding
that would otherwise become a silent cross-implementation disagreement.

## The ULP finding

`point_in_hex(px, py, cx, cy, r)` uses three half-plane inequalities:

```
1. |dx| <= r * sqrt(3) / 2          (vertical edges, left/right)
2. |dy| <= r                         (top/bottom vertices)
3. |dy| + |dx| / sqrt(3) <= r        (diagonal edges)
```

where `dx = px - cx` and `dy = py - cy`.

Inequality 2 (`|dy| <= r`) is **exact** for the top and bottom vertices:
`dy = r` involves no subtraction beyond `py - cy`, and `r` is the
circumradius. The top vertex `(cx, cy - r)` and bottom vertex `(cx, cy + r)`
sit exactly on the boundary. **Inclusive-boundary probes at top/bottom pass.**

Inequality 1 (`|dx| <= r * sqrt(3) / 2`) is **1 ULP wider** for the side
vertices. The right vertex is at `(cx + r·√3/2, cy - r/2)`, but the test
computes `dx = (cx + r·√3/2) - cx`. IEEE 754 double-precision subtraction of
two nearby values loses 1 ULP of precision (catastrophic cancellation in the
limiting case). So `(cx + r·√3/2) - cx != r·√3/2` exactly — the side vertices
are **not** inclusive-boundary probes.

## What this means for ports

- **Top/bottom vertex boundary tests**: use `(cx, cy ± r)`. These are exact.
  Mark them `boundary` in the test vectors; the inclusive `<=` holds.
- **Side vertex boundary tests**: do NOT mark them `boundary`. They will
  pass in Rust (the subtraction happens to round favorably at the test
  scales) but may fail in Solidity (fixed-point) or JS (different
  transcendental implementation). Either mark them `inside` (1 ULP inside)
  or omit them.
- **The test vectors themselves** (`hex_vectors.json`) already follow this
  rule. The 26 point-tests pass in Rust. Solidity and JS ports must
  reproduce the same 26 — if a `boundary` case fails in a port, the port is
  wrong, not the vector.

## Why this matters

A 1 ULP disagreement between Rust and Solidity would mean: a pixel that
renders inside the hex in the COSMIC app renders outside in the on-chain
Solidity renderer (or vice versa). For BNRi's 96×96 hex-pixel art, that's a
visible seam at hex edges across implementations. The note exists so the
Solidity and JS implementers don't discover this at deployment.

## Reference

- Rust anchor: `src/hex_renderer.rs::point_in_hex`
- Test vectors: `tests/fixtures/hex_vectors.json`
- Test that enforces it: `tests/hex_fixtures.rs::point_in_hex_tests_match_vectors`
- Finding identified: R2 (LOViS review), documented here R4

## Floating-point hygiene for ports

- Use `f64` (or the highest-precision fixed-point available) for all hex math
- Compute `sqrt(3)` once, store it, reuse — don't recompute per-pixel
- The apothem `r * sqrt3_2` must be computed with the **same expression** as
  `hex_vertices` uses for the side-vertex x-offset. The Rust code has a
  comment on this: "Must match hex_vertices computation" — port that comment.
- Do not "optimize" `r * sqrt(3) / 2` into `r * 0.8660254...` — the constant
  loses precision and breaks the consistency guarantee

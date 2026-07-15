# Cross-Implementation Hex Math Notes

**For implementers of HexLib in Solidity and JavaScript.**

The Rust reference (`src/hex_renderer.rs`) is the anchor. This document captures
the findings that would otherwise become silent cross-implementation
disagreements.

## The contract is layered — read this before porting

An earlier revision of this document said Solidity "must produce byte-identical
results against the shared test vectors," including the 26 `point_in_hex`
point-tests. **That was over-broad, and acting on it would cost a fixed-point
rewrite of the Rust renderer for no visible gain.** Amended per Fable's S-3
ruling (contract lane, 2026-07-15):

| Layer | Solidity | Rust | JS | Contract |
|---|---|---|---|---|
| **HexRect 8-byte codec** | **yes** | yes | yes | **byte-identical, 3-way** |
| `hex_center`, `hex_vertices` | **yes** (SVG coords) | yes | yes | **±0.01 px at scale 30, 3-way** |
| `point_in_hex` | **no — not a chain concern** | yes | yes | Rust/JS only, 2-way |

**Why `point_in_hex` is not ported.** It is a *rasteriser* predicate: it answers
"is this pixel inside this hex?" The chain does not rasterise — `tokenURI` emits
**vector** SVG (`<polygon points="…">`) and the client rasterises it. There is no
on-chain caller for the predicate, so there is nothing to keep in agreement. The
ULP finding below is a rasteriser concern and stays where it lives.

**Why geometry is a tolerance, not an equality.** Solidity has no floats. A
fixed-point vertex will differ from Rust's `f64` in the last places, necessarily.
At scale 30 that difference is sub-pixel and invisible. Demanding byte-identity
would force the Rust renderer into fixed-point to meet a bar that buys nothing.
**±0.01 px at scale 30** is the honest bar and it is testable against
`tests/fixtures/hex_vectors.json`.

**Why the codec stays exact.** It is integer data with no arithmetic — packing
and unpacking bytes. There is no representational excuse for disagreement, the
goldens already pin it, and it is the layer where a disagreement would corrupt
art rather than nudge a pixel. **This is the hard contract.**

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
  rule. The 26 point-tests pass in Rust. **The JS port** must reproduce the same
  26 — if a `boundary` case fails there, the port is wrong, not the vector.
  **Solidity does not** — see the layered contract above; `point_in_hex` has no
  on-chain caller. Solidity uses the same file for the *centres and vertices*,
  against the ±0.01 px bar.

## Why this matters

A 1 ULP disagreement between **Rust and JS** would mean: a pixel that renders
inside the hex in the COSMIC app renders outside in the web explorer's canvas
(or vice versa). For BNRi's 96×96 hex-pixel art, that's a visible seam at hex
edges between the two rasterisers. The note exists so the JS implementer doesn't
discover it at launch.

It does **not** apply to Solidity, and the earlier revision saying it did was
wrong: the chain emits vector polygons and never evaluates the predicate. The
Solidity-side risk is different in kind — a fixed-point *vertex* off by enough to
be seen — and that is what the ±0.01 px bar exists to catch.

## Reference

- Rust anchor: `src/hex_renderer.rs::point_in_hex`
- Test vectors: `tests/fixtures/hex_vectors.json`
- Test that enforces it: `tests/hex_fixtures.rs::point_in_hex_tests_match_vectors`
- Finding identified: R2 (LOViS review), documented here R4
- Layered contract ruled: Fable, S-3 (contract lane), 2026-07-15 — supersedes
  this document's original "Solidity must reproduce the same 26" claim

## Floating-point hygiene for ports

- Use `f64` (or the highest-precision fixed-point available) for all hex math
- Compute `sqrt(3)` once, store it, reuse — don't recompute per-pixel
- The apothem `r * sqrt3_2` must be computed with the **same expression** as
  `hex_vertices` uses for the side-vertex x-offset. The Rust code has a
  comment on this: "Must match hex_vertices computation" — port that comment.
- Do not "optimize" `r * sqrt(3) / 2` into `r * 0.8660254...` — the constant
  loses precision and breaks the consistency guarantee

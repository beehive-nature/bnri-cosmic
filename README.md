# bnri-cosmic

The BNRi inscription explorer and bLOVErAi interface — a local COSMIC desktop
app, in Rust.

**Status: pre-release.** The library builds and is tested. It is not finished, it
has never run against a live chain, and parts of it are stubs the compiler is
deliberately left complaining about. Read [Honest state](#honest-state) before
judging anything here.

---

## What this is, and what it isn't

BNRi is an ERC-20i hex-pixel-art inscription token on exSat EVM. It is the front
door to the [Beehive Nature Reserve kernel](https://github.com/beehive-nature/beehive-nature)
— a sovereign AI operating system in Rust. The kernel is the product; BNRi is how
people arrive.

This repo is **one** of BNRi's faces — the *local* one:

| Repo | Role |
|---|---|
| [`beehive-nature`](https://github.com/beehive-nature/beehive-nature) | the kernel — the actual OS |
| **`bnri-cosmic`** | **this: the local COSMIC desktop app** |
| `bnri-design` | the spec the implementations consume |
| `bnri-contracts` | the Solidity side |
| `bnri-explorer` | the public web explorer — the hook, reachable without installing anything |

Two GUIs, two audiences. That's the funnel, not duplication: the web explorer
opens without a download; this app is what you run once you're in.

## Layout

```
src/
  lib.rs           the library — everything testable lives here
  agent.rs         LlmSidecar: framed IPC to a local LLM, id-correlated, fail-closed
  hex_renderer.rs  hex math + the HexRect 8-byte packed codec
  quote.rs         TransactionQuote — the quote a human reads before they sign
  views.rs         view helpers
  wallet.rs        exSat EVM wallet — a STUB, behind the `evm` feature
  main.rs          the COSMIC view layer — a thin binary, behind the `gui` feature
tests/
  hex_fixtures.rs   hex math vs shared JSON vectors
  hexrect_codec.rs  codec round-trip, golden vectors, reject cases
  sidecar_ipc.rs    real infer() calls against real fake sidecars
  fixtures/         hex_vectors.json, hexrect_golden.json — shared with the
                    Solidity and JS ports; see CROSS_IMPL_NOTES.md
```

It depends on **no kernel crate**. The b-balance seam is a Unix socket — a wire
protocol, not linkage.

## Build and test

```sh
cargo test                      # 21 passing: lib 6, hex_fixtures 5, hexrect_codec 10
cargo clippy --lib --tests      # exactly one warning, and it must stay — see below
cargo build --features gui      # the COSMIC binary (Linux)
```

`default = []` — the library and its tests, nothing else. Both optional features
carry known RUSTSEC advisories today, and **no vulnerable dependency may sit in
default**:

- **`gui`** — the COSMIC view layer, via `libcosmic`
- **`evm`** — the exSat EVM client, via `ethers`

An unused vulnerability shouldn't be in the shipped surface at all. Every
advisory is named, traced and owned in [`docs/AUDIT.md`](docs/AUDIT.md). None are
suppressed — a suppressed advisory is one the next person doesn't get to see.

## Honest state

Things this repo does **not** claim, stated here rather than left to be
discovered:

- **`cargo clippy` reports one warning — `field context_path is never read` — and
  it must remain.** It marks an unfinished stub. Silencing it with `#[allow]` or
  an underscore would make the toolchain lie about how finished this is. It goes
  to zero when the code is written, not before.
- **`src/wallet.rs` is 8 TODOs.** Signing is not implemented. Nothing here has
  ever signed a transaction or spoken to exSat.
- **`tests/sidecar_ipc.rs` needs a real `python3`.** On a host without one, 3 of
  its 4 tests fail — *correctly*. They are built so a dead interpreter trips a
  liveness probe loudly instead of passing vacuously. Don't weaken them; fix the
  host.
- **The GUI has not been built on hardware this project currently has.**
  `libcosmic` is pinned to a commit, and that pin is verified to *resolve*, not
  to *build*.
- **No commit here carries a DCO `Signed-off-by:`.** A known, deliberate
  deviation with a reason — see [CONTRIBUTING.md](CONTRIBUTING.md).

## A note on how this is built

This code was written largely by machine seats under human direction, and the
project's working rule is that **a document asserting a property its mechanism
doesn't have is a defect in itself** — not documentation of one. A comment that
overclaims, a README that flatters, an audit row nobody can reproduce: the same
bug wearing different clothes. Several have been found and fixed on exactly that
basis, and the honest-state list above exists because of it.

If you find a claim here the code doesn't honour, that's a real finding and we
want it — even where nothing is exploitable.

Receipts over assertions: no ✅ without the command and its real, unedited output.

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) — DCO sign-off, branch policy, the receipt
rule, the test and clippy bars.

## Security

[SECURITY.md](SECURITY.md) — report to **beehivenature@protonmail.com**, never a
public issue.

## License

**AGPL-3.0-only** for the code — see [LICENSE](LICENSE). **CC0 1.0** for the
sprite pixel data. CC0 applies to the data; AGPL applies to the code that renders
it.

// BNRi COSMIC — library crate.
// SPDX-License-Identifier: AGPL-3.0-only
//
// Everything testable lives here; `src/main.rs` is a thin binary that owns
// only the COSMIC view layer. That split is what lets `cargo test` run the
// hex, codec, and sidecar suites without building a GUI toolkit.
//
// This crate depends on no kernel crate and references no kernel type. The
// b-balance seam is a Unix socket — a wire protocol, not linkage.

pub mod agent;
pub mod hex_renderer;
pub mod quote;
pub mod views;

/// The exSat EVM wallet. Gated behind `evm` (default-off) because it is the
/// only thing that pulls `ethers`, whose transitive pins carry four RUSTSEC
/// advisories that no `cargo update` can move. It is a stub today — 8 TODOs,
/// four never-read fields — so nothing is lost by not compiling it, and the
/// default tree audits clean. C-3 enables `evm` when it wires real signing.
#[cfg(feature = "evm")]
pub mod wallet;

pub use quote::TransactionQuote;

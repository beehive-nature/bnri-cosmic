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
pub mod wallet;

pub use quote::TransactionQuote;

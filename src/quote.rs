// Canonical TransactionQuote — single definition for the crate.
// SPDX-License-Identifier: AGPL-3.0-only
//
// Z-4: unified from the split-brain in wallet.rs and agent.rs.
// A later dispatch may move this to the kernel crate — do not move it now.

/// Quote for a simulated transaction, shown to the human before signing.
///
/// bLOVErAi generates this via eth_estimateGas + eth_call (local simulation).
/// The human confirms after reading the quote — bLOVErAi never signs.
/// On anomaly: `anomaly` carries the reason string; bLOVErAi declines to
/// sponsor (the paymaster refuses its own money), but the human can always
/// pay their own gas and proceed (permissionless path, C1.7).
pub struct TransactionQuote {
    /// Human-readable description of the action (e.g. "Swap 10 BNRi → XBTC").
    pub action: String,
    /// Estimated gas cost in BTC (exSat gas token).
    pub gas_btc: String,
    /// Equivalent cost in b-token (kernel accounting, SPIRIT-1).
    pub b_cost: String,
    /// User's current b-token balance (for display).
    pub b_balance: String,
    /// Whether the user has sufficient b-token balance.
    pub sufficient: bool,
    /// Anomaly reason, if detected (e.g. "unusual gas cost", "failed simulation").
    /// None = no anomaly. Some(reason) = bLOVErAi declines to sponsor.
    pub anomaly: Option<String>,
}

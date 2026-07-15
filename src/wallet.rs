// Wallet module — exSat EVM + kernel b-token accounting
// SPDX-License-Identifier: AGPL-3.0-only
//
// Z-4: TransactionQuote moved to crate::quote — import, don't redefine.

use crate::quote::TransactionQuote;
use ethers::providers::{Provider, Http};
use ethers::contract::Contract;
use ethers::types::{Address, U256};

pub struct Wallet {
    evm_provider: Option<Provider<Http>>,
    bnri_contract: Option<Contract<Provider<Http>>>,
    user_address: Address,
    // Kernel-side b-token balance (not on any chain)
    b_balance: U256,
}

impl Wallet {
    pub async fn connect(rpc_url: &str, bnri_address: Address, user_address: Address) -> Result<Self, String> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .map_err(|e| format!("RPC connection failed: {}", e))?;

        // TODO: Initialize BNRi ERC-20i contract instance
        // TODO: Query balanceOf(user_address) for BNRi balance
        // TODO: Query lockedBalanceOf(user_address) for locked BNRi

        Ok(Wallet {
            evm_provider: Some(provider),
            bnri_contract: None,  // TODO: initialize contract
            user_address,
            b_balance: U256::zero(),
        })
    }

    pub async fn refresh_balances(&mut self) -> Result<(), String> {
        // TODO: Query exSat EVM for BNRi balance
        // TODO: Query kernel for b-token balance (Unix socket IPC)
        Ok(())
    }

    pub fn bnri_balance(&self) -> u64 {
        // TODO: Return BNRi balance in raw units
        0
    }

    pub fn b_balance(&self) -> U256 {
        self.b_balance
    }

    pub async fn simulate_transaction(&self, action: &str) -> Result<TransactionQuote, String> {
        // TODO: bLOVErAi simulates via eth_estimateGas + eth_call
        // Returns quote in b-token (kernel accounting)
        // This is the CONSENT-1 disclose-and-confirm pattern

        Err("Not yet implemented".to_string())
    }
}

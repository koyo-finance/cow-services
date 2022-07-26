use crate::sources::uniswap_v2::pair_provider::PairProvider;
use anyhow::Result;
use model::TokenPair;
use primitive_types::H160;

pub mod blockscout;

/// To detect bad tokens we need to find some address on the network that owns the token so that we
/// can use it in our simulations.
#[async_trait::async_trait]
pub trait TokenOwnerFinding: Send + Sync {
    /// Find candidate addresses that might own the token.
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>>;
}

pub struct UniswapLikePairProviderFinder {
    pub inner: PairProvider,
    pub base_tokens: Vec<H160>,
}

#[async_trait::async_trait]
impl TokenOwnerFinding for UniswapLikePairProviderFinder {
    async fn find_candidate_owners(&self, token: H160) -> Result<Vec<H160>> {
        Ok(self
            .base_tokens
            .iter()
            .filter_map(|&base_token| TokenPair::new(base_token, token))
            .map(|pair| self.inner.pair_address(&pair))
            .collect())
    }
}

/// The balancer vault contract contains all the balances of all pools.
pub struct BalancerVaultFinder(pub contracts::BalancerV2Vault);

#[async_trait::async_trait]
impl TokenOwnerFinding for BalancerVaultFinder {
    async fn find_candidate_owners(&self, _: H160) -> Result<Vec<H160>> {
        Ok(vec![self.0.address()])
    }
}

#[derive(Debug, Clone, Copy, clap::ArgEnum)]
pub enum FeeValues {
    /// Use hardcoded list
    Static,
    /// Fetch on creation based on events queried from node.
    /// Some nodes struggle with the request and take a long time to respond leading to timeouts.
    Dynamic,
}

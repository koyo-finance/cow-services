//! Top-level module organizing all baseline liquidity sources.

pub mod balancer_v2;
pub mod koyo_v2;
pub mod oolongswap;
pub mod gin_finance;
pub mod uniswap_v2;

use self::uniswap_v2::{
    pair_provider::PairProvider,
    pool_fetching::{Pool, PoolFetching},
};
use crate::{recent_block_cache::Block, Web3};
use anyhow::{bail, Result};
use model::TokenPair;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, clap::ArgEnum)]
#[clap(rename_all = "verbatim")]
pub enum BaselineSource {
    UniswapV2,
    OolongSwap,
    GinFinance,
    KoyoV2,
    BalancerV2,
}

pub fn defaults_for_chain(chain_id: u64) -> Result<Vec<BaselineSource>> {
    Ok(match chain_id {
        288 => vec![BaselineSource::KoyoV2, BaselineSource::OolongSwap, BaselineSource::GinFinance],
        _ => bail!("unsupported chain {:#x}", chain_id),
    })
}

/// Returns a mapping of UniswapV2-like baseline sources to their respective
/// pair providers and pool fetchers.
pub async fn uniswap_like_liquidity_sources(
    web3: &Web3,
    sources: &[BaselineSource],
) -> Result<HashMap<BaselineSource, (PairProvider, Arc<dyn PoolFetching>)>> {
    let mut liquidity_sources = HashMap::new();
    for source in sources {
        let liquidity_source = match source {
            BaselineSource::UniswapV2 => uniswap_v2::get_liquidity_source(web3).await?,
            BaselineSource::OolongSwap => oolongswap::get_liquidity_source(web3).await?,
            BaselineSource::GinFinance => gin_finance::get_liquidity_source(web3).await?,
            BaselineSource::KoyoV2 => continue,
            BaselineSource::BalancerV2 => continue,
        };

        liquidity_sources.insert(*source, liquidity_source);
    }
    Ok(liquidity_sources)
}

pub struct PoolAggregator {
    pub pool_fetchers: Vec<Arc<dyn PoolFetching>>,
}

#[async_trait::async_trait]
impl PoolFetching for PoolAggregator {
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>> {
        // vk: Using try join means if any pool fetcher fails we fail too. Alternatively we could
        // return the succeeding ones but I feel it is cleaner to forward the error.
        let results = futures::future::try_join_all(
            self.pool_fetchers
                .iter()
                .map(|pool_fetcher| pool_fetcher.fetch(token_pairs.clone(), at_block)),
        )
        .await?;
        Ok(results.into_iter().flatten().collect())
    }
}

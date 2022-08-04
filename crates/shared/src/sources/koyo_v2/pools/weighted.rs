//! Module implementing weighted pool specific indexing logic.

use super::{common, FactoryIndexing, PoolIndexing};
use crate::{
    sources::balancer_v2::{pools::weighted::TokenState, swap::fixed_point::Bfp},
    sources::koyo_v2::graph_api::{PoolData, PoolType},
    Web3CallBatch,
};
use anyhow::{anyhow, Result};
use contracts::{KoyoV2WeightedPool, KoyoV2WeightedPoolFactory};
use ethcontract::{BlockId, H160};
use futures::{future::BoxFuture, FutureExt as _};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
    pub weights: Vec<Bfp>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub tokens: BTreeMap<H160, TokenState>,
    pub swap_fee: Bfp,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
        Ok(PoolInfo {
            common: common::PoolInfo::for_type(PoolType::Weighted, pool, block_created)?,
            weights: pool
                .tokens
                .iter()
                .map(|token| {
                    token
                        .weight
                        .ok_or_else(|| anyhow!("missing weights for pool {:?}", pool.id))
                })
                .collect::<Result<_>>()?,
        })
    }

    fn common(&self) -> &common::PoolInfo {
        &self.common
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for KoyoV2WeightedPoolFactory {
    type PoolInfo = PoolInfo;
    type PoolState = PoolState;

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        let pool_contract = KoyoV2WeightedPool::at(&self.raw_instance().web3(), pool.address);
        let weights = pool_contract
            .methods()
            .get_normalized_weights()
            .call()
            .await?
            .into_iter()
            .map(Bfp::from_wei)
            .collect();

        Ok(PoolInfo {
            common: pool,
            weights,
        })
    }

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        _: &mut Web3CallBatch,
        _: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        let pool_info = pool_info.clone();
        async move {
            let common = common_pool_state.await;
            let tokens = common
                .tokens
                .into_iter()
                .zip(&pool_info.weights)
                .map(|((address, common), &weight)| (address, TokenState { common, weight }))
                .collect();
            let swap_fee = common.swap_fee;

            Ok(Some(PoolState { tokens, swap_fee }))
        }
        .boxed()
    }
}

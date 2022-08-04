//! Module implementing two-token weighted pool specific indexing logic.

pub use super::weighted::{PoolInfo, PoolState};
use super::{common, FactoryIndexing};
use crate::Web3CallBatch;
use anyhow::Result;
use contracts::{KoyoV2OracleWeightedPoolFactory, KoyoV2WeightedPoolFactory};
use ethcontract::BlockId;
use futures::future::BoxFuture;

#[async_trait::async_trait]
impl FactoryIndexing for KoyoV2OracleWeightedPoolFactory {
    type PoolInfo = PoolInfo;
    type PoolState = PoolState;

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        as_weighted_factory(self).specialize_pool_info(pool).await
    }

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        as_weighted_factory(self).fetch_pool_state(pool_info, common_pool_state, batch, block)
    }
}

fn as_weighted_factory(factory: &KoyoV2OracleWeightedPoolFactory) -> KoyoV2WeightedPoolFactory {
    KoyoV2WeightedPoolFactory::at(&factory.raw_instance().web3(), factory.address())
}

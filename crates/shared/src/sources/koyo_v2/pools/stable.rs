//! Module implementing stable pool specific indexing logic.

use super::{common, FactoryIndexing, PoolIndexing};
use crate::{
    sources::{
        balancer_v2::swap::fixed_point::Bfp,
        koyo_v2::graph_api::{PoolData, PoolType},
    },
    Web3CallBatch,
};
use anyhow::Result;
use contracts::{KoyoV2StablePool, KoyoV2StablePoolFactory};
use ethcontract::{BlockId, H160};
use futures::{future::BoxFuture, FutureExt as _};
use std::collections::BTreeMap;

pub use crate::sources::balancer_v2::pools::stable::AmplificationParameter;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
        Ok(PoolInfo {
            common: common::PoolInfo::for_type(PoolType::Stable, pool, block_created)?,
        })
    }

    fn common(&self) -> &common::PoolInfo {
        &self.common
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub tokens: BTreeMap<H160, common::TokenState>,
    pub swap_fee: Bfp,
    pub amplification_parameter: AmplificationParameter,
}

#[async_trait::async_trait]
impl FactoryIndexing for KoyoV2StablePoolFactory {
    type PoolInfo = PoolInfo;
    type PoolState = PoolState;

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        Ok(PoolInfo { common: pool })
    }

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        let pool_contract =
            KoyoV2StablePool::at(&self.raw_instance().web3(), pool_info.common.address);

        let amplification_parameter = pool_contract
            .get_amplification_parameter()
            .block(block)
            .batch_call(batch);

        async move {
            let common = common_pool_state.await;
            let amplification_parameter = {
                let (factor, _, precision) = amplification_parameter.await?;
                AmplificationParameter::new(factor, precision)?
            };

            Ok(Some(PoolState {
                tokens: common.tokens,
                swap_fee: common.swap_fee,
                amplification_parameter,
            }))
        }
        .boxed()
    }
}

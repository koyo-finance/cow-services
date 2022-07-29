//! Module with data types and logic common to multiple Koyo pool types

use super::{FactoryIndexing, Pool, PoolIndexing as _, PoolStatus};
use crate::{
    sources::{
        balancer_v2::swap::fixed_point::Bfp,
        koyo_v2::graph_api::{PoolData, PoolType},
    },
    token_info::TokenInfoFetching,
    Web3CallBatch,
};
use anyhow::{anyhow, ensure, Result};
use contracts::{BalancerV2BasePool, KoyoV2Vault};
use ethcontract::{BlockId, Bytes, H160, H256, U256};
use futures::{future::BoxFuture, FutureExt as _};
use std::{collections::BTreeMap, future::Future, sync::Arc};
use tokio::sync::oneshot;

pub use crate::sources::balancer_v2::pools::common::TokenState;

/// Trait for fetching pool data that is generic on a factory type.
#[mockall::automock]
#[async_trait::async_trait]
pub trait PoolInfoFetching<Factory>: Send + Sync
where
    Factory: FactoryIndexing,
{
    async fn fetch_pool_info(
        &self,
        pool_address: H160,
        block_created: u64,
    ) -> Result<Factory::PoolInfo>;

    fn fetch_pool(
        &self,
        pool: &Factory::PoolInfo,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<PoolStatus>>;
}

/// Generic pool info fetcher for fetching pool info and state that is generic
/// on a pool factory type and its inner pool type.
pub struct PoolInfoFetcher<Factory> {
    vault: KoyoV2Vault,
    factory: Factory,
    token_infos: Arc<dyn TokenInfoFetching>,
}

impl<Factory> PoolInfoFetcher<Factory> {
    pub fn new(
        vault: KoyoV2Vault,
        factory: Factory,
        token_infos: Arc<dyn TokenInfoFetching>,
    ) -> Self {
        Self {
            vault,
            factory,
            token_infos,
        }
    }

    /// Returns a Koyo base pool contract instance at the specified address.
    fn base_pool_at(&self, pool_address: H160) -> BalancerV2BasePool {
        let web3 = self.vault.raw_instance().web3();
        BalancerV2BasePool::at(&web3, pool_address)
    }

    /// Retrieves the scaling exponents for the specified tokens.
    async fn scaling_exponents(&self, tokens: &[H160]) -> Result<Vec<u8>> {
        let token_infos = self.token_infos.get_token_infos(tokens).await;
        tokens
            .iter()
            .map(|token| {
                let decimals = token_infos
                    .get(token)
                    .ok_or_else(|| anyhow!("missing token info for {:?}", token))?
                    .decimals
                    .ok_or_else(|| anyhow!("missing decimals for token {:?}", token))?;
                scaling_exponent_from_decimals(decimals)
            })
            .collect()
    }

    async fn fetch_common_pool_info(
        &self,
        pool_address: H160,
        block_created: u64,
    ) -> Result<PoolInfo> {
        let pool = self.base_pool_at(pool_address);

        let pool_id = H256(pool.methods().get_pool_id().call().await?.0);
        let (tokens, _, _) = self
            .vault
            .methods()
            .get_pool_tokens(Bytes(pool_id.0))
            .call()
            .await?;
        let scaling_exponents = self.scaling_exponents(&tokens).await?;

        Ok(PoolInfo {
            id: pool_id,
            address: pool_address,
            tokens,
            scaling_exponents,
            block_created,
        })
    }

    fn fetch_common_pool_state(
        &self,
        pool: &PoolInfo,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<PoolState>> {
        let pool_contract = self.base_pool_at(pool.address);
        let paused = pool_contract
            .get_paused_state()
            .block(block)
            .batch_call(batch);
        let swap_fee = pool_contract
            .get_swap_fee_percentage()
            .block(block)
            .batch_call(batch);
        let balances = self
            .vault
            .get_pool_tokens(Bytes(pool.id.0))
            .block(block)
            .batch_call(batch);

        // Because of a `mockall` limitation, we **need** the future returned
        // here to be `'static`. This requires us to clone and move `pool` into
        // the async closure - otherwise it would only live for as long as
        // `pool`, i.e. `'_`.
        let pool = pool.clone();
        async move {
            let (paused, _, _) = paused.await?;
            let swap_fee = Bfp::from_wei(swap_fee.await?);

            let (token_addresses, balances, _) = balances.await?;
            ensure!(pool.tokens == token_addresses, "pool token mismatch");
            let tokens = itertools::izip!(&pool.tokens, balances, &pool.scaling_exponents)
                .map(|(&address, balance, &scaling_exponent)| {
                    (
                        address,
                        TokenState {
                            balance,
                            scaling_exponent,
                        },
                    )
                })
                .collect();

            Ok(PoolState {
                paused,
                swap_fee,
                tokens,
            })
        }
        .boxed()
    }
}

#[async_trait::async_trait]
impl<Factory> PoolInfoFetching<Factory> for PoolInfoFetcher<Factory>
where
    Factory: FactoryIndexing,
{
    async fn fetch_pool_info(
        &self,
        pool_address: H160,
        block_created: u64,
    ) -> Result<Factory::PoolInfo> {
        let common_pool_info = self
            .fetch_common_pool_info(pool_address, block_created)
            .await?;
        self.factory.specialize_pool_info(common_pool_info).await
    }

    fn fetch_pool(
        &self,
        pool_info: &Factory::PoolInfo,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<PoolStatus>> {
        let pool_id = pool_info.common().id;
        let (common_pool_state, common_pool_state_ok) =
            share_common_pool_state(self.fetch_common_pool_state(pool_info.common(), batch, block));
        let pool_state =
            self.factory
                .fetch_pool_state(pool_info, common_pool_state_ok.boxed(), batch, block);

        async move {
            let common_pool_state = common_pool_state.await?;
            if common_pool_state.paused {
                return Ok(PoolStatus::Paused);
            }
            let pool_state = match pool_state.await? {
                Some(state) => state,
                None => return Ok(PoolStatus::Disabled),
            };

            Ok(PoolStatus::Active(Pool {
                id: pool_id,
                kind: pool_state.into(),
            }))
        }
        .boxed()
    }
}

/// Common pool data shared across all Koyo pools.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub id: H256,
    pub address: H160,
    pub tokens: Vec<H160>,
    pub scaling_exponents: Vec<u8>,
    pub block_created: u64,
}

impl PoolInfo {
    /// Loads a pool info from Graph pool data.
    pub fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
        ensure!(pool.tokens.len() > 1, "insufficient tokens in pool");

        Ok(PoolInfo {
            id: pool.id,
            address: pool.address,
            tokens: pool.tokens.iter().map(|token| token.address).collect(),
            scaling_exponents: pool
                .tokens
                .iter()
                .map(|token| scaling_exponent_from_decimals(token.decimals))
                .collect::<Result<_>>()?,
            block_created,
        })
    }

    /// Loads a common pool info from Graph pool data, requiring the pool type
    /// to be the specified value.
    pub fn for_type(pool_type: PoolType, pool: &PoolData, block_created: u64) -> Result<Self> {
        ensure!(
            pool.pool_type == pool_type,
            "cannot convert {:?} pool to {:?} pool",
            pool.pool_type,
            pool_type,
        );
        Self::from_graph_data(pool, block_created)
    }
}

/// Common pool state information shared across all pool types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub paused: bool,
    pub swap_fee: Bfp,
    pub tokens: BTreeMap<H160, TokenState>,
}

impl Default for PoolState {
    fn default() -> Self {
        Self {
            paused: false,
            swap_fee: 0.into(),
            tokens: BTreeMap::new(),
        }
    }
}

/// Compute the scaling rate from a Koyo pool's scaling exponent.
///
/// This method returns an error on any arithmetic underflow when computing the
/// token decimals. Note that in theory, this should be impossible to happen.
/// However, we are extra careful and return an `Error` in case it does to avoid
/// panicking. Additionally, wrapped math could have been used here, but that
/// would create invalid settlements.
pub fn compute_scaling_rate(scaling_exponent: u8) -> Result<U256> {
    // Koyo `scaling_exponent`s are `18 - decimals`, we want the rate which
    // is `10 ** decimals`.
    let decimals = 18_u8
        .checked_sub(scaling_exponent)
        .ok_or_else(|| anyhow!("underflow computing decimals from Koyo pool scaling exponent"))?;

    debug_assert!(decimals <= 18);
    // `decimals` is guaranteed to be between 0 and 18, and 10**18 cannot
    // cannot overflow a `U256`, so we do not need to use `checked_pow`.
    Ok(U256::from(10).pow(decimals.into()))
}

/// Converts a token decimal count to its corresponding scaling exponent.
fn scaling_exponent_from_decimals(decimals: u8) -> Result<u8> {
    // Technically this should never fail for Koyo Pools since tokens
    // with more than 18 decimals (not supported by balancer contracts)
    // https://github.com/balancer-labs/balancer-v2-monorepo/blob/deployments-latest/pkg/pool-utils/contracts/BasePool.sol#L476-L487
    18u8.checked_sub(decimals)
        .ok_or_else(|| anyhow!("unsupported token with more than 18 decimals"))
}

/// An internal utility method for sharing the success value for an
/// `anyhow::Result`.
///
/// Typically, this is pretty trivial using `FutureExt::shared`. However, since
/// `anyhow::Error: !Clone` we need to use a different approach.
///
/// # Panics
///
/// Polling the future with the shared success value will panic if the result
/// future has not already resolved to a `Ok` value. This method is only ever
/// meant to be used internally, so we don't have to worry that these
/// assumptions leak out of this module.
fn share_common_pool_state(
    fut: impl Future<Output = Result<PoolState>>,
) -> (
    impl Future<Output = Result<PoolState>>,
    impl Future<Output = PoolState>,
) {
    let (pool_sender, mut pool_receiver) = oneshot::channel();

    let result = fut.inspect(|pool_result| {
        // We can't clone `anyhow::Error` so just clone the pool data and use
        // an empty `()` error.
        let pool_result = pool_result.as_ref().map(Clone::clone).map_err(|_| ());
        // Ignore error if the shared future was dropped.
        let _ = pool_sender.send(pool_result);
    });
    let shared = async move {
        pool_receiver
            .try_recv()
            .expect("result future is still pending or has been dropped")
            .expect("result future resolved to an error")
    };

    (result, shared)
}

//! Pool Fetching is primarily concerned with retrieving relevant pools from the `KoyoPoolRegistry`
//! when given a collection of `TokenPair`. Each of these pools are then queried for
//! their `token_balances` and the `PoolFetcher` returns all up-to-date `Weighted` and `Stable`
//! pools to be consumed by external users (e.g. Price Estimators and Solvers).

mod aggregate;
mod cache;
mod internal;
mod pool_storage;
mod registry;

pub use self::cache::{KoyoPoolCacheMetrics, NoopKoyoPoolCacheMetrics};
use self::{
    aggregate::Aggregate, cache::Cache, internal::InternalPoolFetching, registry::Registry,
};
use super::{
    graph_api::{KoyoSubgraphClient, RegisteredPools},
    pool_init::PoolInitializing,
    pools::{
        common::{self, PoolInfoFetcher},
        stable, weighted, FactoryIndexing, Pool, PoolIndexing, PoolKind,
    },
};
use crate::{
    current_block::CurrentBlockStream,
    maintenance::Maintaining,
    recent_block_cache::{Block, CacheConfig},
    sources::balancer_v2::swap::fixed_point::Bfp,
    token_info::TokenInfoFetching,
    Web3, Web3Transport,
};
use anyhow::Result;
use clap::ArgEnum;
use contracts::{
    KoyoV2OracleWeightedPoolFactory, KoyoV2StablePoolFactory, KoyoV2Vault,
    KoyoV2WeightedPoolFactory,
};
use ethcontract::{dyns::DynInstance, Instance, H160, H256};
use model::TokenPair;
use reqwest::Client;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub use crate::sources::balancer_v2::pools::weighted::TokenState as WeightedTokenState;
pub use common::TokenState;
pub use stable::AmplificationParameter;

pub trait KoyoPoolEvaluating {
    fn properties(&self) -> CommonPoolState;
}

#[derive(Clone, Debug)]
pub struct CommonPoolState {
    pub id: H256,
    pub address: H160,
    pub swap_fee: Bfp,
    pub paused: bool,
}

#[derive(Clone, Debug)]
pub struct WeightedPool {
    pub common: CommonPoolState,
    pub reserves: HashMap<H160, WeightedTokenState>,
}

impl WeightedPool {
    pub fn new_unpaused(pool_id: H256, weighted_state: weighted::PoolState) -> Self {
        WeightedPool {
            common: CommonPoolState {
                id: pool_id,
                address: pool_address_from_id(pool_id),
                swap_fee: weighted_state.swap_fee,
                paused: false,
            },
            reserves: weighted_state.tokens.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StablePool {
    pub common: CommonPoolState,
    pub reserves: HashMap<H160, TokenState>,
    pub amplification_parameter: AmplificationParameter,
}

impl StablePool {
    pub fn new_unpaused(pool_id: H256, stable_state: stable::PoolState) -> Self {
        StablePool {
            common: CommonPoolState {
                id: pool_id,
                address: pool_address_from_id(pool_id),
                swap_fee: stable_state.swap_fee,
                paused: false,
            },
            reserves: stable_state.tokens.into_iter().collect(),
            amplification_parameter: stable_state.amplification_parameter,
        }
    }
}

#[derive(Default)]
pub struct FetchedKoyoPools {
    pub stable_pools: Vec<StablePool>,
    pub weighted_pools: Vec<WeightedPool>,
}

impl FetchedKoyoPools {
    pub fn relevant_tokens(&self) -> HashSet<H160> {
        let mut tokens = HashSet::new();
        tokens.extend(
            self.stable_pools
                .iter()
                .flat_map(|pool| pool.reserves.keys().copied()),
        );
        tokens.extend(
            self.weighted_pools
                .iter()
                .flat_map(|pool| pool.reserves.keys().copied()),
        );
        tokens
    }
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait KoyoPoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<FetchedKoyoPools>;
}

pub struct KoyoPoolFetcher {
    fetcher: Arc<dyn InternalPoolFetching>,
    // We observed some balancer pools like https://app.balancer.fi/#/pool/0x072f14b85add63488ddad88f855fda4a99d6ac9b000200000000000000000027
    // being problematic because their token balance becomes out of sync leading to simulation
    // failures.
    // https://forum.balancer.fi/t/medium-severity-bug-found/3161
    pool_id_deny_list: Vec<H256>,
}

/// An enum containing all supported Koyo factory types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ArgEnum)]
#[clap(rename_all = "verbatim")]
pub enum KoyoFactoryKind {
    Weighted,
    Oracle,
    Stable,
}

impl KoyoFactoryKind {
    /// Returns a vector with supported factories for the specified chain ID.
    pub fn for_chain(chain_id: u64) -> Vec<Self> {
        match chain_id {
            288 => Self::value_variants().to_owned(),
            _ => Default::default(),
        }
    }
}

/// All koyo related contracts that we expect to exist.
pub struct KoyoContracts {
    pub vault: KoyoV2Vault,
    pub factories: HashMap<KoyoFactoryKind, DynInstance>,
}

impl KoyoContracts {
    pub async fn new(web3: &Web3, factory_kinds: Vec<KoyoFactoryKind>) -> Result<Self> {
        let vault = KoyoV2Vault::deployed(web3).await?;

        macro_rules! instance {
            ($factory:ident) => {{
                $factory::deployed(web3).await?.raw_instance().clone()
            }};
        }

        let mut factories = HashMap::new();
        for kind in factory_kinds {
            let instance = match &kind {
                KoyoFactoryKind::Weighted => instance!(KoyoV2WeightedPoolFactory),
                KoyoFactoryKind::Oracle => {
                    instance!(KoyoV2OracleWeightedPoolFactory)
                }
                KoyoFactoryKind::Stable => instance!(KoyoV2StablePoolFactory),
            };

            factories.insert(kind, instance);
        }

        Ok(Self { vault, factories })
    }
}

impl KoyoPoolFetcher {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        chain_id: u64,
        token_infos: Arc<dyn TokenInfoFetching>,
        config: CacheConfig,
        block_stream: CurrentBlockStream,
        metrics: Arc<dyn KoyoPoolCacheMetrics>,
        client: Client,
        contracts: &KoyoContracts,
        deny_listed_pool_ids: Vec<H256>,
    ) -> Result<Self> {
        let pool_initializer = KoyoSubgraphClient::for_chain(chain_id, client)?;
        let fetcher = Arc::new(Cache::new(
            create_aggregate_pool_fetcher(pool_initializer, token_infos, contracts).await?,
            config,
            block_stream,
            metrics,
        )?);

        Ok(Self {
            fetcher,
            pool_id_deny_list: deny_listed_pool_ids,
        })
    }

    async fn fetch_pools(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Pool>> {
        let mut pool_ids = self.fetcher.pool_ids_for_token_pairs(token_pairs).await;
        for id in &self.pool_id_deny_list {
            pool_ids.remove(id);
        }
        let pools = self.fetcher.pools_by_id(pool_ids, at_block).await?;

        Ok(pools)
    }
}

#[async_trait::async_trait]
impl KoyoPoolFetching for KoyoPoolFetcher {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<FetchedKoyoPools> {
        let pools = self.fetch_pools(token_pairs, at_block).await?;

        // For now, split the `Vec<Pool>` into a `FetchedKoyoPools` to keep
        // compatibility with the rest of the project. This should eventually
        // be removed and we should use `koyo_v2::pools::Pool` everywhere
        // instead.
        let fetched_pools =
            pools
                .into_iter()
                .fold(FetchedKoyoPools::default(), |mut fetched_pools, pool| {
                    match pool.kind {
                        PoolKind::Weighted(state) => fetched_pools
                            .weighted_pools
                            .push(WeightedPool::new_unpaused(pool.id, state)),
                        PoolKind::Stable(state) => fetched_pools
                            .stable_pools
                            .push(StablePool::new_unpaused(pool.id, state)),
                    }
                    fetched_pools
                });

        Ok(fetched_pools)
    }
}

#[async_trait::async_trait]
impl Maintaining for KoyoPoolFetcher {
    async fn run_maintenance(&self) -> Result<()> {
        self.fetcher.run_maintenance().await
    }
}

/// Creates an aggregate fetcher for all supported pool factories.
async fn create_aggregate_pool_fetcher(
    pool_initializer: impl PoolInitializing,
    token_infos: Arc<dyn TokenInfoFetching>,
    contracts: &KoyoContracts,
) -> Result<Aggregate> {
    let registered_pools = pool_initializer.initialize_pools().await?;
    let fetched_block_number = registered_pools.fetched_block_number;
    let mut registered_pools_by_factory = registered_pools.group_by_factory();

    macro_rules! registry {
        ($factory:ident, $instance:expr) => {{
            create_internal_pool_fetcher(
                contracts.vault.clone(),
                $factory::with_deployment_info(
                    &$instance.web3(),
                    $instance.address(),
                    $instance.deployment_information(),
                ),
                token_infos.clone(),
                $instance,
                registered_pools_by_factory
                    .remove(&$instance.address())
                    .unwrap_or_else(|| RegisteredPools::empty(fetched_block_number)),
            )?
        }};
    }

    let mut fetchers = Vec::new();
    for (kind, instance) in &contracts.factories {
        let registry = match kind {
            KoyoFactoryKind::Weighted => registry!(KoyoV2WeightedPoolFactory, instance),
            KoyoFactoryKind::Oracle => {
                registry!(KoyoV2OracleWeightedPoolFactory, instance)
            }
            KoyoFactoryKind::Stable => registry!(KoyoV2StablePoolFactory, instance),
        };
        fetchers.push(registry);
    }

    // Just to catch cases where new Koyo factories get added for a pool
    // kind, but we don't index it, log a warning for unused pools.
    if !registered_pools_by_factory.is_empty() {
        let total_count = registered_pools_by_factory
            .values()
            .map(|registered| registered.pools.len())
            .sum::<usize>();
        tracing::warn!(
            %total_count, unused_pools = ?registered_pools_by_factory,
            "found pools that don't correspond to any known Koyo pool factory",
        );
    }

    Ok(Aggregate::new(fetchers))
}

/// Helper method for creating a boxed `InternalPoolFetching` instance for the
/// specified factory and parameters.
fn create_internal_pool_fetcher<Factory>(
    vault: KoyoV2Vault,
    factory: Factory,
    token_infos: Arc<dyn TokenInfoFetching>,
    factory_instance: &Instance<Web3Transport>,
    registered_pools: RegisteredPools,
) -> Result<Box<dyn InternalPoolFetching>>
where
    Factory: FactoryIndexing,
{
    let initial_pools = registered_pools
        .pools
        .iter()
        .map(|pool| Factory::PoolInfo::from_graph_data(pool, registered_pools.fetched_block_number))
        .collect::<Result<_>>()?;
    let start_sync_at_block = Some(registered_pools.fetched_block_number);

    Ok(Box::new(Registry::new(
        Arc::new(PoolInfoFetcher::new(vault, factory, token_infos)),
        factory_instance,
        initial_pools,
        start_sync_at_block,
    )))
}

/// Extract the pool address from an ID.
///
/// This takes advantage that the first 20 bytes of the ID is the address of
/// the pool. For example the GNO-BAL pool with ID
/// `0x36128d5436d2d70cab39c9af9cce146c38554ff0000200000000000000000009`:
/// <https://etherscan.io/address/0x36128D5436d2d70cab39C9AF9CcE146C38554ff0>
fn pool_address_from_id(pool_id: H256) -> H160 {
    let mut address = H160::default();
    address.0.copy_from_slice(&pool_id.0[..20]);
    address
}

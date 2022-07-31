use crate::{
    baseline_solver::BaseTokens,
    http_solver::{
        gas_model::GasModel,
        model::{
            AmmModel, AmmParameters, BatchAuctionModel, ConstantProductPoolParameters,
            MetadataModel, OrderModel, SettledBatchAuctionModel, StablePoolParameters, TokenAmount,
            TokenInfoModel, WeightedPoolTokenData, WeightedProductPoolParameters,
        },
        HttpSolverApi,
    },
    price_estimation::{
        gas::{ERC20_TRANSFER, GAS_PER_ORDER, INITIALIZATION_COST, SETTLEMENT},
        rate_limited, Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
    },
    rate_limiter::RateLimiter,
    recent_block_cache::Block,
    request_sharing::RequestSharing,
    sources::{
        balancer_v2::{
            pools::common::compute_scaling_rate, BalancerPoolFetcher, BalancerPoolFetching,
        },
        koyo_v2::{KoyoPoolFetcher, KoyoPoolFetching},
        uniswap_v2::{pool_cache::PoolCache, pool_fetching::PoolFetching},
    },
    token_info::TokenInfoFetching,
};
use anyhow::{Context, Result};
use ethcontract::{H160, U256};
use futures::{future::BoxFuture, FutureExt, StreamExt};
use gas_estimation::GasPriceEstimating;
use model::{order::OrderKind, TokenPair};
use num::{BigInt, BigRational};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::Duration,
};

pub struct HttpPriceEstimator {
    api: Arc<dyn HttpSolverApi>,
    sharing: RequestSharing<
        Query,
        BoxFuture<'static, Result<SettledBatchAuctionModel, PriceEstimationError>>,
    >,
    pools: Arc<PoolCache>,
    balancer_pools: Option<Arc<BalancerPoolFetcher>>,
    koyo_pools: Option<Arc<KoyoPoolFetcher>>,
    token_info: Arc<dyn TokenInfoFetching>,
    gas_info: Arc<dyn GasPriceEstimating>,
    native_token: H160,
    base_tokens: Arc<BaseTokens>,
    network_name: String,
    rate_limiter: Arc<RateLimiter>,
}

impl HttpPriceEstimator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api: Arc<dyn HttpSolverApi>,
        pools: Arc<PoolCache>,
        balancer_pools: Option<Arc<BalancerPoolFetcher>>,
        koyo_pools: Option<Arc<KoyoPoolFetcher>>,
        token_info: Arc<dyn TokenInfoFetching>,
        gas_info: Arc<dyn GasPriceEstimating>,
        native_token: H160,
        base_tokens: Arc<BaseTokens>,
        network_name: String,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self {
            api,
            sharing: Default::default(),
            pools,
            balancer_pools,
            koyo_pools,
            token_info,
            gas_info,
            native_token,
            base_tokens,
            network_name,
            rate_limiter,
        }
    }

    async fn estimate(&self, query: &Query) -> Result<Estimate, PriceEstimationError> {
        let gas_price = U256::from_f64_lossy(self.gas_info.estimate().await?.effective_gas_price());

        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Buy => (U256::max_value(), query.in_amount),
            OrderKind::Sell => (query.in_amount, U256::one()),
        };

        let orders = maplit::btreemap! {
            0 => OrderModel {
                sell_token: query.sell_token,
                buy_token: query.buy_token,
                sell_amount,
                buy_amount,
                allow_partial_fill: false,
                is_sell_order: query.kind == OrderKind::Sell,
                fee: TokenAmount {
                    amount: U256::from(GAS_PER_ORDER) * gas_price,
                    token: self.native_token,
                },
                cost: TokenAmount {
                    amount: U256::from(GAS_PER_ORDER) * gas_price,
                    token: self.native_token,
                },
                is_liquidity_order: false,
                mandatory: true,
                has_atomic_execution: false,
            },
        };

        let token_pair = TokenPair::new(query.sell_token, query.buy_token).unwrap();
        let pairs = self.base_tokens.relevant_pairs([token_pair].into_iter());
        let gas_model = GasModel {
            native_token: self.native_token,
            gas_price: gas_price.to_f64_lossy(),
        };

        let (uniswap_pools, balancer_pools, koyo_pools) = futures::try_join!(
            self.uniswap_pools(pairs.clone(), &gas_model),
            self.balancer_pools(pairs.clone(), &gas_model),
            self.koyo_pools(pairs.clone(), &gas_model)
        )?;
        let amms: BTreeMap<usize, AmmModel> = uniswap_pools
            .into_iter()
            .chain(balancer_pools)
            .chain(koyo_pools)
            .enumerate()
            .collect();

        let mut tokens: HashSet<H160> = Default::default();
        tokens.insert(query.sell_token);
        tokens.insert(query.buy_token);
        tokens.insert(self.native_token);
        for amm in amms.values() {
            match &amm.parameters {
                AmmParameters::ConstantProduct(params) => tokens.extend(params.reserves.keys()),
                AmmParameters::WeightedProduct(params) => tokens.extend(params.reserves.keys()),
                AmmParameters::Stable(params) => tokens.extend(params.reserves.keys()),
            }
        }
        let tokens: Vec<_> = tokens.drain().collect();
        let token_infos = self.token_info.get_token_infos(&tokens).await;
        let tokens = tokens
            .iter()
            .map(|token| {
                let info = token_infos.get(token).cloned().unwrap_or_default();
                (
                    *token,
                    TokenInfoModel {
                        decimals: info.decimals,
                        alias: info.symbol,
                        normalize_priority: Some(if *token == self.native_token { 1 } else { 0 }),
                        ..Default::default()
                    },
                )
            })
            .collect();

        let model = BatchAuctionModel {
            tokens,
            orders,
            amms,
            metadata: Some(MetadataModel {
                environment: Some(self.network_name.clone()),
                gas_price: Some(gas_price.to_f64_lossy()),
                native_token: Some(self.native_token),
                ..Default::default()
            }),
        };

        let api = self.api.clone();
        let settlement_future = async move {
            api.solve(
                &model,
                // We need at least three seconds of timeout. Quasimodo
                // reserves one second of timeout for shutdown, plus one
                // more second is reserved for network interactions.
                Duration::from_secs(3),
            )
            .await
            .map_err(PriceEstimationError::Other)
        };
        let settlement_future = rate_limited(self.rate_limiter.clone(), settlement_future);
        let settlement = self
            .sharing
            .shared(*query, settlement_future.boxed())
            .await?;

        if !settlement.orders.contains_key(&0) {
            return Err(PriceEstimationError::NoLiquidity);
        }

        let mut cost = self.extract_cost(&settlement.orders[&0].cost)?;
        for amm in settlement.amms.values() {
            cost += self.extract_cost(&amm.cost)? * amm.execution.len();
        }
        let gas = (cost / gas_price).as_u64()
            + INITIALIZATION_COST // Call into contract
            + SETTLEMENT // overhead for entering the `settle()` function
            + ERC20_TRANSFER * 2; // transfer in and transfer out

        Ok(Estimate {
            out_amount: match query.kind {
                OrderKind::Buy => settlement.orders[&0].exec_sell_amount,
                OrderKind::Sell => settlement.orders[&0].exec_buy_amount,
            },
            gas,
        })
    }

    async fn uniswap_pools(
        &self,
        pairs: HashSet<TokenPair>,
        gas_model: &GasModel,
    ) -> Result<Vec<AmmModel>> {
        let pools = self
            .pools
            .fetch(pairs, Block::Recent)
            .await
            .context("pools")?;
        Ok(pools
            .into_iter()
            .map(|pool| AmmModel {
                parameters: AmmParameters::ConstantProduct(ConstantProductPoolParameters {
                    reserves: BTreeMap::from([
                        (pool.tokens.get().0, pool.reserves.0.into()),
                        (pool.tokens.get().1, pool.reserves.1.into()),
                    ]),
                }),
                fee: BigRational::from((
                    BigInt::from(*pool.fee.numer()),
                    BigInt::from(*pool.fee.denom()),
                )),
                cost: gas_model.uniswap_cost(),
                mandatory: false,
            })
            .collect())
    }

    async fn balancer_pools(
        &self,
        pairs: HashSet<TokenPair>,
        gas_model: &GasModel,
    ) -> Result<Vec<AmmModel>> {
        let pools = match &self.balancer_pools {
            Some(balancer) => balancer
                .fetch(pairs, Block::Recent)
                .await
                .context("balancer_pools")?,
            None => return Ok(Vec::new()),
        };
        // There is some code duplication between here and crates/solver/src/solver/http_solver.rs  fn amm_models .
        // To avoid that we would need to make both components work on the same input balancer
        // types. Currently solver uses a liquidity type that is specific to the solver crate.
        let weighted = pools.weighted_pools.into_iter().map(|pool| AmmModel {
            parameters: AmmParameters::WeightedProduct(WeightedProductPoolParameters {
                reserves: pool
                    .reserves
                    .into_iter()
                    .map(|(token, state)| {
                        (
                            token,
                            WeightedPoolTokenData {
                                balance: state.common.balance,
                                weight: BigRational::from(state.weight),
                            },
                        )
                    })
                    .collect(),
            }),
            fee: pool.common.swap_fee.into(),
            cost: gas_model.balancer_cost(),
            mandatory: false,
        });
        let stable = pools
            .stable_pools
            .into_iter()
            .map(|pool| -> Result<AmmModel> {
                Ok(AmmModel {
                    parameters: AmmParameters::Stable(StablePoolParameters {
                        reserves: pool
                            .reserves
                            .iter()
                            .map(|(token, state)| (*token, state.balance))
                            .collect(),
                        scaling_rates: pool
                            .reserves
                            .into_iter()
                            .map(|(token, state)| {
                                Ok((token, compute_scaling_rate(state.scaling_exponent)?))
                            })
                            .collect::<Result<_>>()
                            .with_context(|| "convert stable pool to solver model".to_string())?,
                        amplification_parameter: pool.amplification_parameter.as_big_rational(),
                    }),
                    fee: pool.common.swap_fee.into(),
                    cost: gas_model.balancer_cost(),
                    mandatory: false,
                })
            });
        let mut models = Vec::from_iter(weighted);
        for stable in stable {
            models.push(stable?);
        }
        Ok(models)
    }

    async fn koyo_pools(
        &self,
        pairs: HashSet<TokenPair>,
        gas_model: &GasModel,
    ) -> Result<Vec<AmmModel>> {
        let pools = match &self.koyo_pools {
            Some(koyo) => koyo
                .fetch(pairs, Block::Recent)
                .await
                .context("koyo_pools")?,
            None => return Ok(Vec::new()),
        };
        let weighted = pools.weighted_pools.into_iter().map(|pool| AmmModel {
            parameters: AmmParameters::WeightedProduct(WeightedProductPoolParameters {
                reserves: pool
                    .reserves
                    .into_iter()
                    .map(|(token, state)| {
                        (
                            token,
                            WeightedPoolTokenData {
                                balance: state.common.balance,
                                weight: BigRational::from(state.weight),
                            },
                        )
                    })
                    .collect(),
            }),
            fee: pool.common.swap_fee.into(),
            cost: gas_model.koyo_cost(),
            mandatory: false,
        });
        let stable = pools
            .stable_pools
            .into_iter()
            .map(|pool| -> Result<AmmModel> {
                Ok(AmmModel {
                    parameters: AmmParameters::Stable(StablePoolParameters {
                        reserves: pool
                            .reserves
                            .iter()
                            .map(|(token, state)| (*token, state.balance))
                            .collect(),
                        scaling_rates: pool
                            .reserves
                            .into_iter()
                            .map(|(token, state)| {
                                Ok((token, compute_scaling_rate(state.scaling_exponent)?))
                            })
                            .collect::<Result<_>>()
                            .with_context(|| "convert stable pool to solver model".to_string())?,
                        amplification_parameter: pool.amplification_parameter.as_big_rational(),
                    }),
                    fee: pool.common.swap_fee.into(),
                    cost: gas_model.koyo_cost(),
                    mandatory: false,
                })
            });
        let mut models = Vec::from_iter(weighted);
        for stable in stable {
            models.push(stable?);
        }
        Ok(models)
    }

    fn extract_cost(&self, cost: &Option<TokenAmount>) -> Result<U256, PriceEstimationError> {
        if let Some(cost) = cost {
            if cost.token != self.native_token {
                Err(anyhow::anyhow!("cost specified as an unknown token {}", cost.token).into())
            } else {
                Ok(cost.amount)
            }
        } else {
            Ok(U256::zero())
        }
    }
}

impl PriceEstimating for HttpPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        futures::stream::iter(queries)
            .then(|query| self.estimate(query))
            .enumerate()
            .boxed()
    }
}

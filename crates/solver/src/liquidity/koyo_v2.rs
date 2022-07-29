//! Module for providing Koyo V2 pool liquidity to the solvers.

use crate::{
    interactions::{
        allowances::{AllowanceManager, AllowanceManaging, Allowances},
        KoyoSwapGivenOutInteraction,
    },
    liquidity::{
        slippage, AmmOrderExecution, LimitOrder, SettlementHandling, StablePoolOrder,
        WeightedProductOrder,
    },
    settlement::SettlementEncoder,
};
use anyhow::Result;
use contracts::{GPv2Settlement, KoyoV2Vault};
use ethcontract::H256;
use model::TokenPair;
use shared::{
    baseline_solver::BaseTokens, recent_block_cache::Block,
    sources::koyo_v2::pool_fetching::KoyoPoolFetching, Web3,
};
use std::sync::Arc;

/// A liquidity provider for Koyo V2 weighted pools.
pub struct KoyoV2Liquidity {
    settlement: GPv2Settlement,
    vault: KoyoV2Vault,
    pool_fetcher: Arc<dyn KoyoPoolFetching>,
    allowance_manager: Box<dyn AllowanceManaging>,
    base_tokens: Arc<BaseTokens>,
}

impl KoyoV2Liquidity {
    pub fn new(
        web3: Web3,
        pool_fetcher: Arc<dyn KoyoPoolFetching>,
        base_tokens: Arc<BaseTokens>,
        settlement: GPv2Settlement,
        vault: KoyoV2Vault,
    ) -> Self {
        let allowance_manager = AllowanceManager::new(web3, settlement.address());
        Self {
            settlement,
            vault,
            pool_fetcher,
            allowance_manager: Box::new(allowance_manager),
            base_tokens,
        }
    }

    /// Returns relevant Koyo V2 weighted pools given a list of off-chain
    /// orders.
    pub async fn get_liquidity(
        &self,
        orders: &[LimitOrder],
        block: Block,
    ) -> Result<(Vec<StablePoolOrder>, Vec<WeightedProductOrder>)> {
        let pairs = self.base_tokens.relevant_pairs(
            &mut orders
                .iter()
                .flat_map(|order| TokenPair::new(order.buy_token, order.sell_token)),
        );
        let pools = self.pool_fetcher.fetch(pairs, block).await?;

        let tokens = pools.relevant_tokens();
        let allowances = Arc::new(
            self.allowance_manager
                .get_allowances(tokens, self.vault.address())
                .await?,
        );

        let weighted_product_orders = pools
            .weighted_pools
            .into_iter()
            .map(|pool| WeightedProductOrder {
                reserves: pool.reserves,
                fee: pool.common.swap_fee,
                settlement_handling: Arc::new(SettlementHandler {
                    pool_id: pool.common.id,
                    settlement: self.settlement.clone(),
                    vault: self.vault.clone(),
                    allowances: allowances.clone(),
                }),
            })
            .collect();
        let stable_pool_orders = pools
            .stable_pools
            .into_iter()
            .map(|pool| StablePoolOrder {
                reserves: pool.reserves,
                fee: pool.common.swap_fee.into(),
                amplification_parameter: pool.amplification_parameter,
                settlement_handling: Arc::new(SettlementHandler {
                    pool_id: pool.common.id,
                    settlement: self.settlement.clone(),
                    vault: self.vault.clone(),
                    allowances: allowances.clone(),
                }),
            })
            .collect();

        Ok((stable_pool_orders, weighted_product_orders))
    }
}

pub struct SettlementHandler {
    pool_id: H256,
    settlement: GPv2Settlement,
    vault: KoyoV2Vault,
    allowances: Arc<Allowances>,
}

#[cfg(test)]
impl SettlementHandler {
    pub fn new(
        pool_id: H256,
        settlement: GPv2Settlement,
        vault: KoyoV2Vault,
        allowances: Arc<Allowances>,
    ) -> Self {
        SettlementHandler {
            pool_id,
            settlement,
            vault,
            allowances,
        }
    }
}

impl SettlementHandling<WeightedProductOrder> for SettlementHandler {
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        self.inner_encode(execution, encoder)
    }
}

impl SettlementHandling<StablePoolOrder> for SettlementHandler {
    fn encode(&self, execution: AmmOrderExecution, encoder: &mut SettlementEncoder) -> Result<()> {
        self.inner_encode(execution, encoder)
    }
}

impl SettlementHandler {
    fn inner_encode(
        &self,
        execution: AmmOrderExecution,
        encoder: &mut SettlementEncoder,
    ) -> Result<()> {
        let (asset_in, amount_in) = execution.input;
        let (asset_out, amount_out) = execution.output;

        encoder.append_to_execution_plan(self.allowances.approve_token(asset_in, amount_in)?);
        encoder.append_to_execution_plan(KoyoSwapGivenOutInteraction {
            settlement: self.settlement.clone(),
            vault: self.vault.clone(),
            pool_id: self.pool_id,
            asset_in,
            asset_out,
            amount_out,
            amount_in_max: slippage::amount_plus_max_slippage(amount_in),
            user_data: Default::default(),
        });

        Ok(())
    }
}

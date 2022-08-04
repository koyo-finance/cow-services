//! Solver using the Koyo SOR.

use super::{
    single_order_solver::{execution_respects_order, SettlementError, SingleOrderSolving},
    Auction,
};
use crate::{
    encoding::EncodedInteraction,
    interactions::{allowances::ApprovalRequest, balancer_v2::SwapKind},
    liquidity::LimitOrder,
    settlement::{Interaction, Settlement},
};
use crate::{
    interactions::{allowances::AllowanceManaging, balancer_v2},
    liquidity::slippage,
};
use anyhow::Result;
use contracts::{GPv2Settlement, KoyoV2Vault};
use ethcontract::{Account, Bytes, I256, U256};
use maplit::hashmap;
use model::order::OrderKind;
use shared::balancer_sor_api::{Query, Quote};
use shared::koyo_sor_api::KoyoSorApi;
use std::sync::Arc;

/// A GPv2 solver that matches GP orders to the Koyo SOR API.
pub struct KoyoSorSolver {
    account: Account,
    vault: KoyoV2Vault,
    settlement: GPv2Settlement,
    api: Arc<dyn KoyoSorApi>,
    allowance_fetcher: Arc<dyn AllowanceManaging>,
}

impl KoyoSorSolver {
    pub fn new(
        account: Account,
        vault: KoyoV2Vault,
        settlement: GPv2Settlement,
        api: Arc<dyn KoyoSorApi>,
        allowance_fetcher: Arc<dyn AllowanceManaging>,
    ) -> Self {
        Self {
            account,
            vault,
            settlement,
            api,
            allowance_fetcher,
        }
    }
}

#[async_trait::async_trait]
impl SingleOrderSolving for KoyoSorSolver {
    async fn try_settle_order(
        &self,
        order: LimitOrder,
        auction: &Auction,
    ) -> Result<Option<Settlement>, SettlementError> {
        let amount = match order.kind {
            OrderKind::Sell => order.sell_amount,
            OrderKind::Buy => order.buy_amount,
        };
        let query = Query {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            order_kind: order.kind,
            amount,
            gas_price: U256::from_f64_lossy(auction.gas_price),
        };

        let quote = match self.api.quote(query).await? {
            Some(quote) => quote,
            None => {
                tracing::debug!("No route found");
                return Ok(None);
            }
        };

        let (quoted_sell_amount, quoted_buy_amount) = match order.kind {
            OrderKind::Sell => (quote.swap_amount, quote.return_amount),
            OrderKind::Buy => (quote.return_amount, quote.swap_amount),
        };

        if !execution_respects_order(&order, quoted_sell_amount, quoted_buy_amount) {
            tracing::debug!("execution does not respect order");
            return Ok(None);
        }

        let (quoted_sell_amount_with_slippage, quoted_buy_amount_with_slippage) = match order.kind {
            OrderKind::Sell => (
                quoted_sell_amount,
                slippage::amount_minus_max_slippage(quoted_buy_amount),
            ),
            OrderKind::Buy => (
                slippage::amount_plus_max_slippage(quoted_sell_amount),
                quoted_buy_amount,
            ),
        };

        let prices = hashmap! {
            order.sell_token => quoted_buy_amount,
            order.buy_token => quoted_sell_amount,
        };
        let approval = self
            .allowance_fetcher
            .get_approval(&ApprovalRequest {
                token: order.sell_token,
                spender: self.vault.address(),
                amount: quoted_sell_amount_with_slippage,
            })
            .await?;
        let limits = compute_swap_limits(
            &quote,
            quoted_sell_amount_with_slippage,
            quoted_buy_amount_with_slippage,
        )?;
        let batch_swap = BatchSwap {
            vault: self.vault.clone(),
            settlement: self.settlement.clone(),
            kind: order.kind,
            quote,
            limits,
        };

        let mut settlement = Settlement::new(prices);
        settlement.with_liquidity(&order, order.full_execution_amount())?;
        settlement.encoder.append_to_execution_plan(approval);
        settlement.encoder.append_to_execution_plan(batch_swap);

        Ok(Some(settlement))
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        "KoyoSOR"
    }
}

fn compute_swap_limits(
    quote: &Quote,
    quoted_sell_amount_with_slippage: U256,
    quoted_buy_amount_with_slippage: U256,
) -> Result<Vec<I256>> {
    quote
        .token_addresses
        .iter()
        .map(|&token| -> Result<I256> {
            let limit = if token == quote.token_in {
                // Use positive swap limit for sell amounts (that is, maximum
                // amount that can be transferred in)
                quoted_sell_amount_with_slippage.try_into()?
            } else if token == quote.token_out {
                // Use negative swap limit for buy amounts (that is, minimum
                // amount that must be transferred out)
                I256::try_from(quoted_buy_amount_with_slippage)?
                    .checked_neg()
                    .expect("positive integer can't overflow negation")
            } else {
                // For other tokens we don't want any net transfer in or out.
                I256::zero()
            };

            Ok(limit)
        })
        .collect()
}

#[derive(Debug)]
struct BatchSwap {
    vault: KoyoV2Vault,
    settlement: GPv2Settlement,
    kind: OrderKind,
    quote: Quote,
    limits: Vec<I256>,
}

impl Interaction for BatchSwap {
    fn encode(&self) -> Vec<EncodedInteraction> {
        let kind = match self.kind {
            OrderKind::Sell => SwapKind::GivenIn,
            OrderKind::Buy => SwapKind::GivenOut,
        } as _;
        let swaps = self
            .quote
            .swaps
            .iter()
            .map(|swap| {
                (
                    Bytes(swap.pool_id.0),
                    swap.asset_in_index.into(),
                    swap.asset_out_index.into(),
                    swap.amount,
                    Bytes(swap.user_data.clone()),
                )
            })
            .collect();
        let assets = self.quote.token_addresses.clone();
        let funds = (
            self.settlement.address(), // sender
            false,                     // fromInternalBalance
            self.settlement.address(), // recipient
            false,                     // toInternalBalance
        );
        let limits = self.limits.clone();

        let calldata = self
            .vault
            .methods()
            .batch_swap(kind, swaps, assets, funds, limits, *balancer_v2::NEVER)
            .tx
            .data
            .expect("no calldata")
            .0;

        vec![(self.vault.address(), 0.into(), Bytes(calldata))]
    }
}

use super::{
    gas::{GAS_PER_KOYO_SWAP, SETTLEMENT_SINGLE_TRADE},
    Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
};
use crate::{
    balancer_sor_api, koyo_sor_api::KoyoSorApi, rate_limiter::RateLimiter,
    request_sharing::RequestSharing,
};
use anyhow::Result;
use futures::{future::BoxFuture, stream::BoxStream, FutureExt, StreamExt};
use gas_estimation::GasPriceEstimating;
use primitive_types::U256;
use std::sync::Arc;

pub struct KoyoSor {
    api: Arc<dyn KoyoSorApi>,
    sharing: RequestSharing<
        Query,
        BoxFuture<'static, Result<balancer_sor_api::Quote, PriceEstimationError>>,
    >,
    rate_limiter: Arc<RateLimiter>,
    gas: Arc<dyn GasPriceEstimating>,
}

impl KoyoSor {
    pub fn new(
        api: Arc<dyn KoyoSorApi>,
        rate_limiter: Arc<RateLimiter>,
        gas: Arc<dyn GasPriceEstimating>,
    ) -> Self {
        Self {
            api,
            sharing: Default::default(),
            rate_limiter,
            gas,
        }
    }

    async fn estimate(&self, query: &Query) -> PriceEstimateResult {
        let gas_price = self.gas.estimate().await?;
        let query_ = balancer_sor_api::Query {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            order_kind: query.kind,
            amount: query.in_amount,
            gas_price: U256::from_f64_lossy(gas_price.effective_gas_price()),
        };
        let api = self.api.clone();
        let future = async move {
            match api.quote(query_).await {
                Ok(Some(quote)) => Ok(quote),
                Ok(None) => Err(PriceEstimationError::NoLiquidity),
                Err(err) => Err(PriceEstimationError::from(err)),
            }
        };
        let future = super::rate_limited(self.rate_limiter.clone(), future);
        let future = self.sharing.shared(*query, future.boxed());
        let quote = future.await?;
        Ok(Estimate {
            out_amount: quote.return_amount,
            gas: SETTLEMENT_SINGLE_TRADE + (quote.swaps.len() as u64) * GAS_PER_KOYO_SWAP,
        })
    }
}

impl PriceEstimating for KoyoSor {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> BoxStream<'_, (usize, PriceEstimateResult)> {
        futures::stream::iter(queries)
            .then(|query| self.estimate(query))
            .enumerate()
            .boxed()
    }
}

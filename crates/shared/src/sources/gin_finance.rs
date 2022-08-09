//! Gin Finance baseline liquidity source implementation.

use super::uniswap_v2::macros::impl_uniswap_like_liquidity;

impl_uniswap_like_liquidity! {
    factory: contracts::GinFinanceFactory,
    init_code_digest: "600f62a2bbcebd19e25e035e25f77c256245572c21588866b6188af3eaab8ea3",
}

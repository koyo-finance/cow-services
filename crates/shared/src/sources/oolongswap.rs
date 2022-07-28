//! Oolongswap baseline liquidity source implementation.

use super::uniswap_v2::macros::impl_uniswap_like_liquidity;

impl_uniswap_like_liquidity! {
    factory: contracts::OolongSwapFactory,
    init_code_digest: "1db9efb13a1398e31bb71895c392fa1217130f78dc65080174491adcec5da9b9",
}

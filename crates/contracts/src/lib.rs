#![allow(clippy::let_unit_value)]
#[cfg(feature = "bin")]
pub mod paths;
pub mod vault;

include!(concat!(env!("OUT_DIR"), "/ERC20.rs"));
include!(concat!(env!("OUT_DIR"), "/ERC20Mintable.rs"));
include!(concat!(env!("OUT_DIR"), "/ERC1271SignatureValidator.rs"));
include!(concat!(env!("OUT_DIR"), "/WETH9.rs"));

include!(concat!(env!("OUT_DIR"), "/GPv2AllowListAuthentication.rs"));
include!(concat!(env!("OUT_DIR"), "/GPv2Settlement.rs"));

include!(concat!(env!("OUT_DIR"), "/GnosisSafe.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/GnosisSafeCompatibilityFallbackHandler.rs"
));
include!(concat!(env!("OUT_DIR"), "/GnosisSafeProxy.rs"));

include!(concat!(env!("OUT_DIR"), "/BalancerV2Authorizer.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2Vault.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2BasePool.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2BasePoolFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2StablePool.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2StablePoolFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/BalancerV2StablePoolV2.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2StablePoolFactoryV2.rs"
));
include!(concat!(env!("OUT_DIR"), "/BalancerV2WeightedPool.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2WeightedPoolFactory.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/BalancerV2WeightedPool2TokensFactory.rs"
));

include!(concat!(env!("OUT_DIR"), "/Koyo.rs"));
include!(concat!(env!("OUT_DIR"), "/VotingEscrow.rs"));

include!(concat!(env!("OUT_DIR"), "/KoyoV2Authorizer.rs"));
include!(concat!(env!("OUT_DIR"), "/KoyoV2Vault.rs"));
include!(concat!(env!("OUT_DIR"), "/KoyoV2BasePool.rs"));
include!(concat!(env!("OUT_DIR"), "/KoyoV2BasePoolFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/KoyoV2WeightedPool.rs"));
include!(concat!(env!("OUT_DIR"), "/KoyoV2WeightedPoolFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/KoyoV2OracleWeightedPool.rs"));
include!(concat!(env!("OUT_DIR"), "/KoyoV2OracleWeightedPoolFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/KoyoV2StablePool.rs"));
include!(concat!(env!("OUT_DIR"), "/KoyoV2StablePoolFactory.rs"));

include!(concat!(env!("OUT_DIR"), "/IUniswapLikePair.rs"));
include!(concat!(env!("OUT_DIR"), "/IUniswapLikeRouter.rs"));
include!(concat!(env!("OUT_DIR"), "/UniswapV2Factory.rs"));
include!(concat!(env!("OUT_DIR"), "/UniswapV2Router02.rs"));

include!(concat!(env!("OUT_DIR"), "/OolongSwapFactory.rs"));
include!(concat!(env!("OUT_DIR"), "/OolongSwapRouter02.rs"));

#[cfg(test)]
mod tests {
    use ethcontract::{
        futures::future::{self, Ready},
        json::json,
        jsonrpc::{Call, Id, MethodCall, Params, Value},
        web3::{error::Result as Web3Result, BatchTransport, RequestId, Transport},
    };

    #[derive(Debug, Clone)]
    struct ChainIdTransport(u64);

    impl Transport for ChainIdTransport {
        type Out = Ready<Web3Result<Value>>;

        fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
            assert_eq!(method, "net_version");
            assert_eq!(params.len(), 0);
            (
                0,
                MethodCall {
                    jsonrpc: None,
                    method: method.to_string(),
                    params: Params::Array(params),
                    id: Id::Num(0),
                }
                .into(),
            )
        }

        fn send(&self, _id: RequestId, _request: Call) -> Self::Out {
            future::ready(Ok(json!(format!("{}", self.0))))
        }
    }

    impl BatchTransport for ChainIdTransport {
        type Batch = Ready<Web3Result<Vec<Web3Result<Value>>>>;

        fn send_batch<T>(&self, requests: T) -> Self::Batch
        where
            T: IntoIterator<Item = (RequestId, Call)>,
        {
            future::ready(Ok(requests
                .into_iter()
                .map(|_| Ok(json!(format!("{}", self.0))))
                .collect()))
        }
    }
}

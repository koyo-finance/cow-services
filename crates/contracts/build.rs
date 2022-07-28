use ethcontract::{
    common::{contract::Network, DeploymentInformation},
    Address,
};
use ethcontract_generate::{loaders::TruffleLoader, ContractBuilder};
use std::{env, path::Path};

#[path = "src/paths.rs"]
mod paths;

fn main() {
    // NOTE: This is a workaround for `rerun-if-changed` directives for
    // non-existent files cause the crate's build unit to get flagged for a
    // rebuild if any files in the workspace change.
    //
    // See:
    // - https://github.com/rust-lang/cargo/issues/6003
    // - https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorerun-if-changedpath
    println!("cargo:rerun-if-changed=build.rs");

    generate_contract_with_config("BalancerV2Authorizer", |builder| {
        builder.contract_mod_override("balancer_v2_authorizer")
    });
    generate_contract_with_config("BalancerV2BasePool", |builder| {
        builder.contract_mod_override("balancer_v2_base_pool")
    });
    generate_contract_with_config("BalancerV2BasePoolFactory", |builder| {
        builder.contract_mod_override("balancer_v2_base_pool_factory")
    });
    generate_contract_with_config("BalancerV2Vault", |builder| {
        builder
            .contract_mod_override("balancer_v2_vault")
            .add_network(
                "288",
                Network {
                    address: addr("0x2A4409Cc7d2AE7ca1E3D915337D1B6Ba2350D6a3"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(668337)),
                },
            )
    });
    generate_contract_with_config("BalancerV2WeightedPoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_weighted_pool_factory")
            .add_network(
                "288",
                Network {
                    address: addr("0xEa34bb7F24F3BB120DAF64Cd1BC9e958FFF9ED0c"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(673848)),
                },
            )
    });
    generate_contract_with_config("BalancerV2WeightedPool2TokensFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_weighted_pool_2_tokens_factory")
            .add_network(
                "288",
                Network {
                    address: addr("0x06f607EC266BB98bcb9Bae402D61Ab5E008ab018"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(673576)),
                },
            )
    });
    generate_contract_with_config("BalancerV2StablePoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_stable_pool_factory")
            .add_network(
                "1",
                Network {
                    address: addr("0xc66ba2b6595d3613ccab350c886ace23866ede24"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(12703127)),
                },
            )
    });
    generate_contract_with_config("BalancerV2StablePoolFactoryV2", |builder| {
        builder
            .contract_mod_override("balancer_v2_stable_pool_factory_v2")
            .add_network(
                "288",
                Network {
                    address: addr("0xb4455B572b4dBF39d76a10de530988803C13d854"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(684091)),
                },
            )
    });
    generate_contract("BalancerV2WeightedPool");
    generate_contract_with_config("BalancerV2StablePool", |builder| {
        builder.add_method_alias(
            "onSwap((uint8,address,address,uint256,bytes32,uint256,address,address,bytes),uint256[],uint256,uint256)",
            "on_swap_with_balances"
        )
    });
    generate_contract_with_config("BalancerV2StablePoolV2", |builder| {
        builder.add_method_alias(
            "onSwap((uint8,address,address,uint256,bytes32,uint256,address,address,bytes),uint256[],uint256,uint256)",
            "on_swap_with_balances"
        )
    });
    generate_contract("ERC20");
    generate_contract("ERC20Mintable");
    generate_contract("GPv2AllowListAuthentication");
    generate_contract_with_config("GPv2Settlement", |builder| {
        builder
            .contract_mod_override("gpv2_settlement")
            .add_network(
                "288",
                Network {
                    address: addr("0xc3E6AEC4300c78b2D12966457f113f8C2B30949b"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(745834)),
                },
            )
    });
    generate_contract("GnosisSafe");
    generate_contract_with_config("GnosisSafeCompatibilityFallbackHandler", |builder| {
        builder.add_method_alias("isValidSignature(bytes,bytes)", "is_valid_signature_legacy")
    });
    generate_contract("GnosisSafeProxy");
    generate_contract("IUniswapLikeRouter");
    generate_contract("IUniswapLikePair");
    // EIP-1271 contract - SignatureValidator
    generate_contract("ERC1271SignatureValidator");
    generate_contract_with_config("UniswapV2Factory", |builder| {
        builder
            .add_network_str("137", "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .add_network_str("42220", "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
    });
    generate_contract_with_config("UniswapV2Router02", |builder| {
        builder
            .add_network_str("137", "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
            .add_network_str("42220", "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
    });
    generate_contract_with_config("WETH9", |builder| {
        builder
            .add_network_str("288", "0xDeadDeAddeAddEAddeadDEaDDEAdDeaDDeAD0000")
    });
    generate_contract_with_config("Koyo", |builder| {
        builder.add_network_str("288", "0x618CC6549ddf12de637d46CDDadaFC0C2951131C")
    });
    generate_contract_with_config("VotingEscrow", |builder| {
        builder
            .add_network_str("288", "0xD3535a7797F921cbCD275d746A4EFb1fBba0989F")
            .add_method_alias("totalSupply(uint256)", "total_supply_at_timestamp")
            .add_method_alias("balanceOf(address,uint256)", "balance_of_at_timestamp")
    });
}

fn generate_contract(name: &str) {
    generate_contract_with_config(name, |builder| builder)
}

fn generate_contract_with_config(
    name: &str,
    config: impl FnOnce(ContractBuilder) -> ContractBuilder,
) {
    let path = paths::contract_artifacts_dir()
        .join(name)
        .with_extension("json");
    let contract = TruffleLoader::new()
        .name(name)
        .load_contract_from_file(&path)
        .unwrap_or_else(|_| panic!("contract file {:?} not found", name));
    let dest = env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed={}", path.display());

    config(ContractBuilder::new().visibility_modifier("pub"))
        .generate(&contract)
        .unwrap()
        .write_to_file(Path::new(&dest).join(format!("{}.rs", name)))
        .unwrap();
}

fn addr(s: &str) -> Address {
    s.parse().unwrap()
}

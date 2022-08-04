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

    generate_contract("ERC20");
    generate_contract("ERC20Mintable");
    // EIP-1271 contract - SignatureValidator
    generate_contract("ERC1271SignatureValidator");
    generate_contract_with_config("WETH9", |builder| {
        builder.add_network_str("288", "0xDeadDeAddeAddEAddeadDEaDDEAdDeaDDeAD0000")
    });

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
                "1",
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://etherscan.io/tx/0x28c44bb10d469cbd42accf97bd00b73eabbace138e9d44593e851231fbed1cb7>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12272146)),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://rinkeby.etherscan.io/tx/0x5fe65a242760f7f32b582dc402a081791d57ea561474617fcd0e763c995cfec7>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8441702)),
                },
            )
            .add_network(
                "5",
                Network {
                    address: addr("0xBA12222222228d8Ba445958a75a0704d566BF2C8"),
                    // <https://goerli.etherscan.io/tx/0x116a2c379d6e496f7848d5704ed3fe0c6e1caa841dd1cac10f631b7bc71b0ec5>
                    deployment_information: Some(DeploymentInformation::BlockNumber(4648099)),
                },
            )
    });
    generate_contract("BalancerV2WeightedPool");
    generate_contract_with_config("BalancerV2WeightedPoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_weighted_pool_factory")
            .add_network(
                "1",
                Network {
                    address: addr("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"),
                    // <https://etherscan.io/tx/0x0f9bb3624c185b4e107eaf9176170d2dc9cb1c48d0f070ed18416864b3202792>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12272147)),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"),
                    // <https://rinkeby.etherscan.io/tx/0xae8c45c1d40756d0eb312723a2993341e379ea6d8bef4adfae2709345939f8eb>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8441703)),
                },
            )
            .add_network(
                "5",
                Network {
                    address: addr("0x8E9aa87E45e92bad84D5F8DD1bff34Fb92637dE9"),
                    // <https://goerli.etherscan.io/tx/0x0ce1710e896fb090a2387e94a31e1ac40f3005de30388a63c44381f2c900d732>
                    deployment_information: Some(DeploymentInformation::BlockNumber(4648101)),
                },
            )
    });
    generate_contract_with_config("BalancerV2WeightedPool2TokensFactory", |builder| {
        builder
            .add_network(
                "1",
                Network {
                    address: addr("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0"),
                    // <https://etherscan.io/tx/0xf40c05058422d730b7035c254f8b765722935a5d3003ac37b13a61860adbaf08>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12349891)),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0"),
                    // <https://rinkeby.etherscan.io/tx/0xbe28062b575c2743b3b4525c3a175b9acad36695c15dba1c69af5f3fc3ceca37>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8510540)),
                },
            )
            .add_network(
                "5",
                Network {
                    address: addr("0xa5bf2ddf098bb0ef6d120c98217dd6b141c74ee0"),
                    // <https://goerli.etherscan.io/tx/0x5d5aa13cce6f81c36c69ad5aae6f5cb9cc6f8605a5eb1dc99815b5d74ae0796a>
                    deployment_information: Some(DeploymentInformation::BlockNumber(4716924)),
                },
            )
    });
    generate_contract_with_config("BalancerV2StablePool", |builder| {
        builder.add_method_alias(
            "onSwap((uint8,address,address,uint256,bytes32,uint256,address,address,bytes),uint256[],uint256,uint256)",
            "on_swap_with_balances"
        )
    });
    generate_contract_with_config("BalancerV2StablePoolFactory", |builder| {
        builder
            .contract_mod_override("balancer_v2_stable_pool_factory")
            .add_network(
                "1",
                Network {
                    address: addr("0xc66ba2b6595d3613ccab350c886ace23866ede24"),
                    // <https://etherscan.io/tx/0xfd417511f3902a304cca51023e8e771de22ffa7f30b9c8650ec5757328ab89a6>
                    deployment_information: Some(DeploymentInformation::BlockNumber(12703127)),
                },
            )
            .add_network(
                "4",
                Network {
                    address: addr("0xc66ba2b6595d3613ccab350c886ace23866ede24"),
                    // <https://rinkeby.etherscan.io/tx/0x26ccac4bd7af78607107489fa05868a68291b5e6f217f6829fc3767d8926264a>
                    deployment_information: Some(DeploymentInformation::BlockNumber(8822477)),
                },
            )
    });
    generate_contract_with_config("BalancerV2StablePoolFactoryV2", |builder| {
        builder
            .contract_mod_override("balancer_v2_stable_pool_factory_v2")
            .add_network(
                "1",
                Network {
                    address: addr("0x8df6efec5547e31b0eb7d1291b511ff8a2bf987c"),
                    // <https://etherscan.io/tx/0xef36451947ebd97b72278face57a53806e90071f4c902259db2db41d0c9a143d>
                    deployment_information: Some(DeploymentInformation::BlockNumber(14934936)),
                },
            )
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

    generate_contract_with_config("KoyoV2Authorizer", |builder| {
        builder
            .contract_mod_override("koyo_v2_authorizer")
            .add_network_str("288", "0xeC9c70b34C4CF4b91cC057D726b114Ef3C7A1749")
    });
    generate_contract_with_config("KoyoV2Vault", |builder| {
        builder //
            .contract_mod_override("koyo_v2_vault")
            .add_network(
                "288",
                Network {
                    address: addr("0x2A4409Cc7d2AE7ca1E3D915337D1B6Ba2350D6a3"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(668337)),
                },
            )
    });
    generate_contract_with_config("KoyoV2BasePool", |builder| {
        builder.contract_mod_override("koyo_v2_base_pool")
    });
    generate_contract_with_config("KoyoV2BasePoolFactory", |builder| {
        builder.contract_mod_override("koyo_v2_base_pool_factory")
    });
    generate_contract_with_config("KoyoV2WeightedPool", |builder| {
        builder.contract_mod_override("koyo_v2_weighted_pool")
    });
    generate_contract_with_config("KoyoV2WeightedPoolFactory", |builder| {
        builder
            .contract_mod_override("koyo_v2_weighted_pool_factory")
            .add_network(
                "288",
                Network {
                    address: addr("0xEa34bb7F24F3BB120DAF64Cd1BC9e958FFF9ED0c"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(673848)),
                },
            )
    });
    generate_contract_with_config("KoyoV2OracleWeightedPool", |builder| {
        builder.contract_mod_override("koyo_v2_oracle_weighted_pool")
    });
    generate_contract_with_config("KoyoV2OracleWeightedPoolFactory", |builder| {
        builder
            .contract_mod_override("koyo_v2_oracle_weighted_pool_factory")
            .add_network(
                "288",
                Network {
                    address: addr("0x06f607EC266BB98bcb9Bae402D61Ab5E008ab018"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(673576)),
                },
            )
    });
    generate_contract_with_config("KoyoV2StablePool", |builder| {
        builder
            .add_method_alias(
                "onSwap((uint8,address,address,uint256,bytes32,uint256,address,address,bytes),uint256[],uint256,uint256)",
                "on_swap_with_balances"
            )
    });
    generate_contract_with_config("KoyoV2StablePoolFactory", |builder| {
        builder
            .contract_mod_override("koyo_v2_stable_pool_factory")
            .add_network(
                "288",
                Network {
                    address: addr("0xb4455B572b4dBF39d76a10de530988803C13d854"),
                    deployment_information: Some(DeploymentInformation::BlockNumber(684091)),
                },
            )
    });

    generate_contract("IUniswapLikeRouter");
    generate_contract("IUniswapLikePair");
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

    // Addresses obtained from https://github.com/OolongSwap/oolongswap-deployments
    generate_contract_with_config("OolongSwapFactory", |builder| {
        builder.add_network_str("288", "0x7DDaF116889D655D1c486bEB95017a8211265d29")
    });
    generate_contract_with_config("OolongSwapRouter02", |builder| {
        builder.add_network_str("288", "0x17C83E2B96ACfb5190d63F5E46d93c107eC0b514")
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

//! This script is used to vendor Truffle JSON artifacts to be used for code
//! generation with `ethcontract`. This is done instead of fetching contracts
//! at build time to reduce the risk of failure.

use anyhow::Result;
use contracts::paths;
use env_logger::Env;
use ethcontract_generate::Source;
use serde_json::{Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn main() {
    env_logger::init_from_env(Env::default().default_filter_or("warn,vendor=info"));

    if let Err(err) = run() {
        log::error!("Error vendoring contracts: {:?}", err);
        std::process::exit(-1);
    }
}

fn run() -> Result<()> {
    let vendor = Vendor::new()?;

    vendor
        .full()
        .npm(
            "ERC20Mintable",
            "@openzeppelin/contracts@2.5.0/build/contracts/ERC20Mintable.json",
        )?
        .npm("WETH9", "canonical-weth@1.4.0/build/contracts/WETH9.json")?
        .npm(
            "GnosisSafe",
            "@gnosis.pm/safe-contracts@1.3.0/build/artifacts/contracts/\
             GnosisSafe.sol/GnosisSafe.json",
        )?
        .npm(
            "GnosisSafeCompatibilityFallbackHandler",
            "@gnosis.pm/safe-contracts@1.3.0/build/artifacts/contracts/\
             handler/CompatibilityFallbackHandler.sol/CompatibilityFallbackHandler.json",
        )?
        .npm(
            "GnosisSafeProxy",
            "@gnosis.pm/safe-contracts@1.3.0/build/artifacts/contracts/\
             proxies/GnosisSafeProxy.sol/GnosisSafeProxy.json",
        )?
        .npm(
            "GPv2AllowListAuthentication",
            // We use `_Implementation` because the use of a proxy contract makes
            // deploying for the e2e tests more cumbersome.
            "@cowprotocol/contracts@1.1.2/\
             deployments/mainnet/GPv2AllowListAuthentication_Implementation.json",
        )?
        .npm(
            "GPv2Settlement",
            "@cowprotocol/contracts@1.1.2/deployments/mainnet/GPv2Settlement.json",
        )?
        .github(
            "BalancerV2Authorizer",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/deployed/mainnet/Authorizer.json",
        )?
        .github(
            "BalancerV2Vault",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/deployed/mainnet/Vault.json",
        )?
        .github(
            "BalancerV2WeightedPoolFactory",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/deployed/mainnet/WeightedPoolFactory.json",
        )?
        .github(
            "BalancerV2WeightedPool2TokensFactory",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/deployed/mainnet/WeightedPool2TokensFactory.json",
        )?
        .github(
            "BalancerV2StablePoolFactory",
            "balancer-labs/balancer-v2-monorepo/ad1442113b26ec22081c2047e2ec95355a7f12ba/\
             pkg/deployments/tasks/20210624-stable-pool/abi/StablePoolFactory.json",
        )?
        .npm(
            "UniswapV2Factory",
            "@uniswap/v2-core@1.0.1/build/UniswapV2Factory.json",
        )?
        .npm(
            "UniswapV2Router02",
            "@uniswap/v2-periphery@1.1.0-beta.0/build/UniswapV2Router02.json",
        )?;

    vendor
        .abi_only()
        .npm(
            "ERC20",
            "@openzeppelin/contracts@3.3.0/build/contracts/ERC20.json",
        )?
        .github(
            "ERC1271SignatureValidator",
            "koyo-finance/external-abis/e5c1410f4ac7c501396abac9681c1856eaaefd33/network/_/ERC1271SignatureValidator.json",
        )?
        .github(
            "BalancerV2BasePool",
            "koyo-finance/exchange-vault-monorepo/770722fc4332dfbbc7598451bb7ff2e62f2322d8/pkg/pool-utils/abis/BasePool.json"
        )?
        .github(
            "BalancerV2BasePoolFactory",
            "koyo-finance/exchange-vault-monorepo/770722fc4332dfbbc7598451bb7ff2e62f2322d8/pkg/pool-utils/abis/BasePoolSplitCodeFactory.json"
        )?
        .github(
            "BalancerV2WeightedPool",
            "balancer-labs/balancer-v2-monorepo/a3b570a2aa655d4c4941a67e3db6a06fbd72ef09/\
             pkg/deployments/extra-abis/WeightedPool.json",
        )?
        .github(
            "BalancerV2StablePool",
            "balancer-labs/balancer-subgraph-v2/2b97edd5e65aed06718ce64a69111ccdabccf048/\
             abis/StablePool.json",
        )?
        .github(
            "BalancerV2StablePoolFactoryV2",
            "balancer-labs/balancer-v2-monorepo/903d34e491a5e9c5d59dabf512c7addf1ccf9bbd/\
            pkg/deployments/tasks/20220609-stable-pool-v2/abi/StablePoolFactory.json",
        )?
        .github(
            "KoyoV2Authorizer",
            "koyo-finance/exchange-vault-monorepo/42103a3f81e0b63c0b5f994e9bf4d3a66cffe9ec/pkg/vault/abis/Authorizer.json",
        )?
        .github(
            "KoyoV2Vault",
            "koyo-finance/exchange-vault-monorepo/477b369ac6a7d13ffc666b8e5cf10ebc99a72b2e/pkg/vault/abis/Vault.json",
        )?
        .github(
            "KoyoV2BasePool",
            "koyo-finance/exchange-vault-monorepo/770722fc4332dfbbc7598451bb7ff2e62f2322d8/pkg/pool-utils/abis/BasePool.json"
        )?
        .github(
            "KoyoV2BasePoolFactory",
            "koyo-finance/exchange-vault-monorepo/770722fc4332dfbbc7598451bb7ff2e62f2322d8/pkg/pool-utils/abis/BasePoolSplitCodeFactory.json"
        )?
        .github(
            "KoyoV2OracleWeightedPool",
            "koyo-finance/exchange-vault-monorepo/477b369ac6a7d13ffc666b8e5cf10ebc99a72b2e/pkg/pools/oracle/abis/OracleWeightedPool.json"
        )?
        .github(
            "KoyoV2OracleWeightedPoolFactory",
            "koyo-finance/exchange-vault-monorepo/477b369ac6a7d13ffc666b8e5cf10ebc99a72b2e/pkg/pools/oracle/abis/OracleWeightedPoolFactory.json"
        )?
        .github(
            "KoyoV2WeightedPool",
            "koyo-finance/exchange-vault-monorepo/477b369ac6a7d13ffc666b8e5cf10ebc99a72b2e/pkg/pools/weighted/abis/WeightedPool.json"
        )?
        .github(
            "KoyoV2WeightedPoolFactory",
            "koyo-finance/exchange-vault-monorepo/477b369ac6a7d13ffc666b8e5cf10ebc99a72b2e/pkg/pools/weighted/abis/WeightedPoolFactory.json"
        )?
        .github(
            "KoyoV2WeightedPoolNoAMFactory",
            "koyo-finance/exchange-vault-monorepo/477b369ac6a7d13ffc666b8e5cf10ebc99a72b2e/pkg/pools/weighted/abis/WeightedPoolNoAMFactory.json"
        )?
        .github(
            "KoyoV2StablePool",
            "koyo-finance/exchange-vault-monorepo/42103a3f81e0b63c0b5f994e9bf4d3a66cffe9ec/pkg/pools/stable/abis/StablePool.json"
        )?
        .github(
            "KoyoV2StablePoolFactory",
            "koyo-finance/exchange-vault-monorepo/42103a3f81e0b63c0b5f994e9bf4d3a66cffe9ec/pkg/pools/stable/abis/StablePoolFactory.json"
        )?
        .github(
            "Koyo",
            "koyo-finance/koyo/23f50fab5ac84d80a2b9916070ae0903c3418d6b/abis/Koyo.json"
        )?
        .github(
            "VotingEscrow",
            "koyo-finance/koyo/23f50fab5ac84d80a2b9916070ae0903c3418d6b/abis/VotingEscrow.json"
        )?
        .npm(
            "IUniswapLikeFactory",
            "@uniswap/v2-periphery@1.1.0-beta.0/build/IUniswapV2Factory.json",
        )?
        .npm(
            "IUniswapLikePair",
            "@uniswap/v2-periphery@1.1.0-beta.0/build/IUniswapV2Pair.json",
        )?
        .npm(
            "IUniswapLikeRouter",
            "@uniswap/v2-periphery@1.1.0-beta.0/build/IUniswapV2Router02.json",
        )?
        .github(
            "OolongSwapFactory",
            "koyo-finance/external-abis/c3a794c13a3919b26e7b570f661edaf33eede93a/network/boba/oolongswap/OolongSwapFactory.json"
        )?
        .github(
            "OolongSwapRouter02",
            "koyo-finance/external-abis/c3a794c13a3919b26e7b570f661edaf33eede93a/network/boba/oolongswap/OolongSwapRouter02.json"
        )?;

    Ok(())
}

struct Vendor {
    artifacts: PathBuf,
}

impl Vendor {
    fn new() -> Result<Self> {
        let artifacts = paths::contract_artifacts_dir();
        log::info!("vendoring contract artifacts to '{}'", artifacts.display());
        fs::create_dir_all(&artifacts)?;
        Ok(Self { artifacts })
    }

    /// Creates a context for vendoring "full" contract data, including bytecode
    /// used for deploying the contract for end-to-end test.
    fn full(&self) -> VendorContext {
        VendorContext {
            artifacts: &self.artifacts,
            properties: &[
                ("abi", "abi,compilerOutput.abi"),
                ("devdoc", "devdoc,compilerOutput.devdoc"),
                ("userdoc", "userdoc"),
                ("bytecode", "bytecode"),
            ],
        }
    }

    /// Creates a context for vendoring only the contract ABI for generating
    /// bindings. This is preferred over [`Vendor::full`] for contracts that do
    /// not need to be deployed for tests, or get created by alternative means
    /// (e.g. `UniswapV2Pair` contracts don't require bytecode as they get
    /// created by `UniswapV2Factory` instances on-chain).
    fn abi_only(&self) -> VendorContext {
        VendorContext {
            artifacts: &self.artifacts,
            properties: &[
                ("abi", "abi,compilerOutput.abi"),
                ("devdoc", "devdoc,compilerOutput.devdoc"),
                ("userdoc", "userdoc"),
            ],
        }
    }
}

struct VendorContext<'a> {
    artifacts: &'a Path,
    properties: &'a [(&'a str, &'a str)],
}

impl VendorContext<'_> {
    fn npm(&self, name: &str, path: &str) -> Result<&Self> {
        self.vendor_source(name, Source::npm(path))
    }

    fn github(&self, name: &str, path: &str) -> Result<&Self> {
        self.vendor_source(
            name,
            Source::http(&format!("https://raw.githubusercontent.com/{}", path))?,
        )
    }

    fn manual(&self, name: &str, reason: &str) -> &Self {
        // We just keep these here to document that they are manually generated
        // and not pulled from some source.
        log::info!("skipping {}: {}", name, reason);
        self
    }

    fn retrieve_value_from_path<'a>(source: &'a Value, path: &'a str) -> Value {
        let mut current_value: &Value = source;
        for property in path.split('.') {
            current_value = &current_value[property];
        }
        current_value.clone()
    }

    fn vendor_source(&self, name: &str, source: Source) -> Result<&Self> {
        log::info!("retrieving {:?}", source);
        let artifact_json = source.artifact_json()?;

        log::debug!("pruning artifact JSON");
        let pruned_artifact_json = {
            let json = serde_json::from_str::<Value>(&artifact_json)?;
            let mut pruned = Map::new();
            for (property, paths) in self.properties {
                if let Some(value) = paths
                    .split(',')
                    .map(|path| Self::retrieve_value_from_path(&json, path))
                    .find(|value| !value.is_null())
                {
                    pruned.insert(property.to_string(), value);
                }
            }
            serde_json::to_string(&pruned)?
        };

        let path = self.artifacts.join(name).with_extension("json");
        log::debug!("saving artifact to {}", path.display());
        fs::write(path, pruned_artifact_json)?;

        Ok(self)
    }
}

use crate::{encoding::EncodedSettlement, settlement::Settlement};
use anyhow::{anyhow, Context, Error, Result};
use contracts::GPv2Settlement;
use ethcontract::{
    batch::CallBatch,
    contract::MethodBuilder,
    dyns::{DynMethodBuilder, DynTransport},
    errors::ExecutionError,
    transaction::TransactionBuilder,
    Account, Address,
};
use futures::FutureExt;
use gas_estimation::GasPrice1559;
use primitive_types::{H160, H256, U256};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, IntoUrl, Url,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use shared::Web3;
use web3::types::{AccessList, BlockId};

const SIMULATE_BATCH_SIZE: usize = 10;

/// The maximum amount the base gas fee can increase from one block to the other.
///
/// This is derived from [EIP-1559](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-1559.md):
/// ```text
/// BASE_FEE_MAX_CHANGE_DENOMINATOR = 8
/// base_fee_per_gas_delta = max(parent_base_fee_per_gas * gas_used_delta // parent_gas_target // BASE_FEE_MAX_CHANGE_DENOMINATOR, 1)
/// ```
///
/// Because the elasticity factor is 2, this means that the highes possible `gas_used_delta == parent_gas_target`.
/// Therefore, the highest possible `base_fee_per_gas_delta` is `parent_base_fee_per_gas / 8`.
///
/// Example of this in action:
/// [Block 12998225](https://etherscan.io/block/12998225) with base fee of `43.353224173` and ~100% over the gas target.
/// Next [block 12998226](https://etherscan.io/block/12998226) has base fee of `48.771904644` which is an increase of ~12.5%.
const MAX_BASE_GAS_FEE_INCREASE: f64 = 1.125;

pub async fn simulate_and_estimate_gas_at_current_block(
    settlements: impl Iterator<Item = (Account, Settlement, Option<AccessList>)>,
    contract: &GPv2Settlement,
    web3: &Web3,
    gas_price: GasPrice1559,
) -> Result<Vec<Result<U256, ExecutionError>>> {
    // Collect into Vec to not rely on Itertools::chunk which would make this future !Send.
    let settlements: Vec<_> = settlements.collect();

    // Needed because sending an empty batch request gets an empty response which doesn't
    // deserialize correctly.
    if settlements.is_empty() {
        return Ok(Vec::new());
    }

    let web3 = web3::Web3::new(shared::transport::buffered::Buffered::new(
        web3.transport().clone(),
    ));
    let contract_with_buffered_transport = GPv2Settlement::at(&web3, contract.address());
    let mut results = Vec::new();
    for chunk in settlements.chunks(SIMULATE_BATCH_SIZE) {
        let calls = chunk
            .iter()
            .map(|(account, settlement, access_list)| {
                let tx = settle_method(
                    gas_price,
                    &contract_with_buffered_transport,
                    settlement.clone(),
                    account.clone(),
                )
                .tx;
                let tx = match access_list {
                    Some(access_list) => tx.access_list(access_list.clone()),
                    None => tx,
                };
                tx.estimate_gas()
            })
            .collect::<Vec<_>>();
        let chuck_results = futures::future::join_all(calls).await;
        results.extend(chuck_results);
    }
    Ok(results)
}

#[allow(clippy::needless_collect)]
pub async fn simulate_and_error_with_tenderly_link(
    settlements: impl Iterator<Item = (Account, Settlement, Option<AccessList>)>,
    contract: &GPv2Settlement,
    web3: &Web3,
    gas_price: GasPrice1559,
    network_id: &str,
    block: u64,
    simulation_gas_limit: u128,
) -> Vec<Result<()>> {
    let mut batch = CallBatch::new(web3.transport());
    let futures = settlements
        .map(|(account, settlement, access_list)| {
            let method = settle_method(gas_price, contract, settlement, account);
            let method = match access_list {
                Some(access_list) => method.access_list(access_list),
                None => method,
            };
            let transaction_builder = method.tx.clone();
            let view = method
                .view()
                .block(BlockId::Number(block.into()))
                // Since we now supply the gas price for the simulation, make sure to also
                // set a gas limit so we don't get failed simulations because of insufficient
                // solver balance. The limit should be below the current block gas
                // limit of 30M gas
                .gas(simulation_gas_limit.into());
            (view.batch_call(&mut batch), transaction_builder)
        })
        .collect::<Vec<_>>();
    batch.execute_all(SIMULATE_BATCH_SIZE).await;

    futures
        .into_iter()
        .map(|(future, transaction_builder)| {
            future.now_or_never().unwrap().map(|_| ()).map_err(|err| {
                Error::new(err).context(tenderly_link(block, network_id, transaction_builder))
            })
        })
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
struct TenderlyResponse {
    transaction: TenderlyTransaction,
}

#[derive(Debug, Clone, Deserialize)]
struct TenderlyTransaction {
    gas_used: u64,
}

pub async fn simulate_before_after_access_list(
    web3: &Web3,
    tenderly: &TenderlyApi,
    network_id: String,
    transaction_hash: H256,
) -> Result<f64> {
    let transaction = web3
        .eth()
        .transaction(transaction_hash.into())
        .await?
        .context("no transaction found")?;

    if transaction.access_list.is_none() {
        return Err(anyhow!(
            "no need to analyze access list since no access list was found in mined transaction"
        ));
    }

    let (block_number, from, to, transaction_index) = (
        transaction
            .block_number
            .context("no block number field exist")?
            .as_u64(),
        transaction.from.context("no from field exist")?,
        transaction.to.context("no to field exist")?,
        transaction
            .transaction_index
            .context("no transaction_index field exist")?
            .as_u64(),
    );

    let request = TenderlyRequest {
        network_id,
        block_number,
        from,
        input: transaction.input.0,
        to,
        gas: Some(transaction.gas.as_u64()),
        generate_access_list: false,
        transaction_index: Some(transaction_index),
    };

    let gas_used_without_access_list = tenderly
        .send::<TenderlyResponse>(request)
        .await?
        .transaction
        .gas_used;
    let gas_used_with_access_list = web3
        .eth()
        .transaction_receipt(transaction_hash)
        .await?
        .ok_or_else(|| anyhow!("no transaction receipt"))?
        .gas_used
        .ok_or_else(|| anyhow!("no gas used field"))?;

    Ok(gas_used_without_access_list as f64 - gas_used_with_access_list.to_f64_lossy())
}

pub fn settle_method(
    gas_price: GasPrice1559,
    contract: &GPv2Settlement,
    settlement: Settlement,
    account: Account,
) -> MethodBuilder<DynTransport, ()> {
    // Increase the gas price by the highest possible base gas fee increase. This
    // is done because the between retrieving the gas price and executing the simulation,
    // a block may have been mined that increases the base gas fee and causes the
    // `eth_call` simulation to fail with `max fee per gas less than block base fee`.
    let gas_price = gas_price.bump(MAX_BASE_GAS_FEE_INCREASE);
    settle_method_builder(contract, settlement.into(), account)
        .gas_price(crate::into_gas_price(&gas_price))
}

pub fn settle_method_builder(
    contract: &GPv2Settlement,
    settlement: EncodedSettlement,
    from: Account,
) -> DynMethodBuilder<()> {
    contract
        .settle(
            settlement.tokens,
            settlement.clearing_prices,
            settlement.trades,
            settlement.interactions,
        )
        .from(from)
}

/// The call data of a settle call with this settlement.
pub fn call_data(settlement: EncodedSettlement) -> Vec<u8> {
    let contract = GPv2Settlement::at(&shared::transport::dummy::web3(), H160::default());
    let method = contract.settle(
        settlement.tokens,
        settlement.clearing_prices,
        settlement.trades,
        settlement.interactions,
    );
    // Unwrap because there should always be calldata.
    method.tx.data.unwrap().0
}

// Creates a simulation link in the gp-v2 tenderly workspace
pub fn tenderly_link(
    current_block: u64,
    network_id: &str,
    tx: TransactionBuilder<DynTransport>,
) -> String {
    // Tenderly simulates transactions for block N at transaction index 0, while
    // `eth_call` simulates transactions "on top" of the block (i.e. after the
    // last transaction index). Therefore, in order for the Tenderly simulation
    // to be as close as possible to the `eth_call`, we want to create it on the
    // next block (since `block_N{tx_last} ~= block_(N+1){tx_0}`).
    let next_block = current_block + 1;
    format!(
        "https://dashboard.tenderly.co/gp-v2/staging/simulator/new?block={}&blockIndex=0&from={:#x}&gas=8000000&gasPrice=0&value=0&contractAddress={:#x}&network={}&rawFunctionInput=0x{}",
        next_block,
        tx.from.unwrap().address(),
        tx.to.unwrap(),
        network_id,
        hex::encode(tx.data.unwrap().0)
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenderlyRequest {
    pub network_id: String,
    pub block_number: u64,
    pub from: Address,
    #[serde(with = "model::bytes_hex")]
    pub input: Vec<u8>,
    pub to: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_index: Option<u64>,
    pub generate_access_list: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockNumber {
    pub block_number: u64,
}

#[derive(Debug)]
pub struct TenderlyApi {
    url: Url,
    client: Client,
    header: HeaderMap,
}

impl TenderlyApi {
    pub fn new(url: impl IntoUrl, client: Client, api_key: &str) -> Result<Self> {
        Ok(Self {
            url: url.into_url()?,
            client,
            header: {
                let mut header = HeaderMap::new();
                header.insert("x-access-key", HeaderValue::from_str(api_key)?);
                header
            },
        })
    }

    pub async fn send<T>(&self, body: TenderlyRequest) -> reqwest::Result<T>
    where
        T: DeserializeOwned,
    {
        self.client
            .post(self.url.clone())
            .headers(self.header.clone())
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    pub async fn block_number(&self, network_id: &str) -> reqwest::Result<BlockNumber> {
        self.client
            .get(format!(
                "https://api.tenderly.co/api/v1/network/{}/block-number",
                network_id
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::{Account, PrivateKey};
    use shared::transport::create_env_test_transport;
    use std::str::FromStr;

    // cargo test -p solver settlement_simulation::tests::mainnet -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        // Create some bogus settlements to see that the simulation returns an error.
        shared::tracing::initialize("solver=debug,shared=debug", tracing::Level::ERROR.into());
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let block = web3.eth().block_number().await.unwrap().as_u64();
        let network_id = web3.net().version().await.unwrap();
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let account = Account::Offline(PrivateKey::from_raw([1; 32]).unwrap(), None);

        let settlements = vec![
            (
                account.clone(),
                Settlement::with_trades(Default::default(), vec![Default::default()], vec![]),
                None,
            ),
            (account.clone(), Settlement::new(Default::default()), None),
        ];
        let result = simulate_and_error_with_tenderly_link(
            settlements.iter().cloned(),
            &contract,
            &web3,
            Default::default(),
            network_id.as_str(),
            block,
            15000000u128,
        )
        .await;
        let _ = dbg!(result);

        let result = simulate_and_estimate_gas_at_current_block(
            settlements.iter().cloned(),
            &contract,
            &web3,
            Default::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);

        let result = simulate_and_estimate_gas_at_current_block(
            std::iter::empty(),
            &contract,
            &web3,
            Default::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);
    }

    // cargo test -p solver settlement_simulation::tests::mainnet_chunked -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn mainnet_chunked() {
        shared::tracing::initialize("solver=debug,shared=debug", tracing::Level::ERROR.into());
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let account = Account::Offline(PrivateKey::from_raw([1; 32]).unwrap(), None);

        // 12 so that we hit more than one chunk.
        let settlements = vec![
            (account.clone(), Settlement::new(Default::default()), None);
            SIMULATE_BATCH_SIZE + 2
        ];
        let result = simulate_and_estimate_gas_at_current_block(
            settlements.iter().cloned(),
            &contract,
            &web3,
            GasPrice1559::default(),
        )
        .await
        .unwrap();
        let _ = dbg!(result);
    }

    #[tokio::test]
    #[ignore]
    async fn simulate_before_after_access_list_test() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let transaction_hash =
            H256::from_str("e337fcd52afd6b98847baab279cda6c3980fcb185da9e959fd489ffd210eac60")
                .unwrap();
        let tenderly_api = TenderlyApi::new(
            // http://api.tenderly.co/api/v1/account/<USER_NAME>/project/<PROJECT_NAME>/simulate
            Url::parse(&std::env::var("TENDERLY_URL").unwrap()).unwrap(),
            Client::new(),
            &std::env::var("TENDERLY_API_KEY").unwrap(),
        )
        .unwrap();
        let gas_saved = simulate_before_after_access_list(
            &web3,
            &tenderly_api,
            "1".to_string(),
            transaction_hash,
        )
        .await
        .unwrap();

        dbg!(gas_saved);
    }

    #[test]
    fn calldata_works() {
        let settlement = EncodedSettlement::default();
        let data = call_data(settlement);
        assert!(!data.is_empty());
    }
}

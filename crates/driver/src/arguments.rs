use reqwest::Url;
use shared::{
    arguments::{display_list, display_option, duration_from_seconds},
    gas_price_estimation::GasEstimatorType,
};
use solver::{
    arguments::TransactionStrategyArg, settlement_access_list::AccessListEstimatorType,
    solver::ExternalSolverArg,
};
use std::{net::SocketAddr, time::Duration};
use tracing::level_filters::LevelFilter;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_address: SocketAddr,

    #[clap(
        long,
        env,
        default_value = "warn,driver=debug,shared=debug,shared::transport::http=info"
    )]
    pub log_filter: String,

    #[clap(long, env, default_value = "error")]
    pub log_stderr_threshold: LevelFilter,

    /// List of solvers in the form of `name|url|account`.
    #[clap(long, env, use_value_delimiter = true)]
    pub solvers: Vec<ExternalSolverArg>,

    /// The Ethereum node URL to connect to.
    #[clap(long, env, default_value = "http://localhost:8545")]
    pub node_url: Url,

    /// Timeout in seconds for all http requests.
    #[clap(
        long,
        default_value = "10",
        parse(try_from_str = duration_from_seconds),
    )]
    pub http_timeout: Duration,

    /// If solvers should use internal buffers to improve solution quality.
    #[clap(long, env)]
    pub use_internal_buffers: bool,

    /// The RPC endpoints to use for submitting transaction to a custom set of nodes.
    #[clap(long, env, use_value_delimiter = true)]
    pub transaction_submission_nodes: Vec<Url>,

    /// How to to submit settlement transactions.
    /// Expected to contain either:
    /// 1. One value equal to TransactionStrategyArg::DryRun or
    /// 2. One or more values equal to any combination of enum variants except TransactionStrategyArg::DryRun
    #[clap(
        long,
        env,
        default_value = "PublicMempool",
        arg_enum,
        ignore_case = true,
        use_value_delimiter = true
    )]
    pub transaction_strategy: Vec<TransactionStrategyArg>,

    /// Additional tip in percentage of max_fee_per_gas we are willing to give to miners above regular gas price estimation
    #[clap(
        long,
        env,
        default_value = "0.05",
        parse(try_from_str = shared::arguments::parse_percentage_factor)
    )]
    pub additional_tip_percentage: f64,

    /// Which access list estimators to use. Multiple estimators are used in sequence if a previous one
    /// fails. Individual estimators might support different networks.
    /// `Tenderly`: supports every network.
    /// `Web3`: supports every network.
    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub access_list_estimators: Vec<AccessListEstimatorType>,

    /// The URL for tenderly transaction simulation.
    #[clap(long, env)]
    pub tenderly_url: Option<Url>,

    /// Tenderly requires api key to work. Optional since Tenderly could be skipped in access lists estimators.
    #[clap(long, env)]
    pub tenderly_api_key: Option<String>,

    /// The target confirmation time in seconds for settlement transactions used to estimate gas price.
    #[clap(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub target_confirm_time: Duration,

    /// The maximum time in seconds we spend trying to settle a transaction through the ethereum
    /// network before going to back to solving.
    #[clap(
        long,
        default_value = "120",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub max_submission_seconds: Duration,

    /// Amount of time to wait before retrying to submit the tx to the ethereum network
    #[clap(
        long,
        default_value = "2",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub submission_retry_interval_seconds: Duration,

    /// The maximum gas price in Gwei the solver is willing to pay in a settlement.
    #[clap(
        long,
        env,
        default_value = "1500",
        parse(try_from_str = shared::arguments::wei_from_gwei)
    )]
    pub gas_price_cap: f64,

    /// Which gas estimators to use. Multiple estimators are used in sequence if a previous one
    /// fails. Individual estimators support different networks.
    /// `EthGasStation`: supports mainnet.
    /// `GasNow`: supports mainnet.
    /// `GnosisSafe`: supports mainnet, rinkeby and goerli.
    /// `Web3`: supports every network.
    /// `Native`: supports every network.
    #[clap(
        long,
        env,
        default_value = "Web3",
        arg_enum,
        ignore_case = true,
        use_value_delimiter = true
    )]
    pub gas_estimators: Vec<GasEstimatorType>,

    /// BlockNative requires api key to work. Optional since BlockNative could be skipped in gas estimators.
    #[clap(long, env)]
    pub blocknative_api_key: Option<String>,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "bind_address: {}", self.bind_address)?;
        writeln!(f, "log_filter: {}", self.log_filter)?;
        writeln!(f, "log_stderr_threshold: {}", self.log_stderr_threshold)?;
        writeln!(f, "solvers: {:?}", self.solvers)?;
        writeln!(f, "node_url: {}", self.node_url)?;
        writeln!(f, "http_timeout: {:?}", self.http_timeout)?;
        writeln!(f, "use_internal_buffers: {}", self.use_internal_buffers)?;
        write!(f, "transaction_submission_nodes: ",)?;
        display_list(self.transaction_submission_nodes.iter(), f)?;
        writeln!(f)?;
        writeln!(f, "transaction_strategy: {:?}", self.transaction_strategy)?;
        writeln!(
            f,
            "additional_tip_percentage: {}",
            self.additional_tip_percentage
        )?;
        writeln!(
            f,
            "access_list_estimators: {:?}",
            self.access_list_estimators
        )?;
        write!(f, "tenderly_url: ")?;
        display_option(&self.tenderly_url, f)?;
        writeln!(f)?;
        writeln!(
            f,
            "tenderly_api_key: {}",
            self.tenderly_api_key
                .as_deref()
                .map(|_| "SECRET")
                .unwrap_or("None")
        )?;
        writeln!(f, "target_confirm_time: {:?}", self.target_confirm_time)?;
        writeln!(
            f,
            "max_submission_seconds: {:?}",
            self.max_submission_seconds
        )?;
        writeln!(
            f,
            "submission_retry_interval_seconds: {:?}",
            self.submission_retry_interval_seconds
        )?;
        writeln!(f, "gas_price_cap: {}", self.gas_price_cap)?;
        writeln!(f, "gas_estimators: {:?}", self.gas_estimators)?;
        writeln!(
            f,
            "blocknative_api_key: {}",
            self.blocknative_api_key
                .as_ref()
                .map(|_| "SECRET")
                .unwrap_or("None")
        )?;
        Ok(())
    }
}

//! Contains command line arguments and related helpers that are shared between the binaries.
use crate::{
    gas_price_estimation::GasEstimatorType,
    rate_limiter::RateLimitingStrategy,
    sources::{balancer_v2::BalancerFactoryKind, koyo_v2::KoyoFactoryKind, BaselineSource},
};
use anyhow::{ensure, Context, Result};
use ethcontract::{H160, H256, U256};
use std::{
    fmt::{Display, Formatter},
    num::{NonZeroU64, ParseFloatError},
    str::FromStr,
    time::Duration,
};
use tracing::level_filters::LevelFilter;
use url::Url;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(
        long,
        env,
        default_value = "warn,orderbook=debug,solver=debug,shared=debug,shared::transport::http=info"
    )]
    pub log_filter: String,

    #[clap(long, env, default_value = "error", parse(try_from_str))]
    pub log_stderr_threshold: LevelFilter,

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

    /// Base tokens used for finding multi-hop paths between multiple AMMs
    /// Should be the most liquid tokens of the given network.
    #[clap(long, env, use_value_delimiter = true)]
    pub base_tokens: Vec<H160>,

    /// Which Liquidity sources to be used by Price Estimator.
    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub baseline_sources: Option<Vec<BaselineSource>>,

    /// The number of blocks kept in the pool cache.
    #[clap(long, env, default_value = "10")]
    pub pool_cache_blocks: NonZeroU64,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "4")]
    pub pool_cache_maximum_recent_block_age: u64,

    /// How often to retry requests in the pool cache.
    #[clap(long, env, default_value = "5")]
    pub pool_cache_maximum_retries: u32,

    /// How long to sleep in seconds between retries in the pool cache.
    #[clap(long, env, default_value = "1", parse(try_from_str = duration_from_seconds))]
    pub pool_cache_delay_between_retries_seconds: Duration,

    /// How often in seconds we poll the node to check if the current block has changed.
    #[clap(
        long,
        env,
        default_value = "5",
        parse(try_from_str = duration_from_seconds),
    )]
    pub block_stream_poll_interval_seconds: Duration,

    /// The Balancer V2 factories to consider for indexing liquidity. Allows
    /// specific pool kinds to be disabled via configuration. Will use all
    /// supported Balancer V2 factory kinds if not specified.
    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub balancer_factories: Option<Vec<BalancerFactoryKind>>,

    /// Deny list of balancer pool ids.
    #[clap(long, env, use_value_delimiter = true)]
    pub balancer_pool_deny_list: Vec<H256>,

    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub koyo_factories: Option<Vec<KoyoFactoryKind>>,

    #[clap(long, env, use_value_delimiter = true)]
    pub koyo_pool_deny_list: Vec<H256>,

    #[clap(long, env, use_value_delimiter = true, default_value = "288")]
    pub koyo_sor_supported_chains: Vec<u64>,

    /// Value of the authorization header for the solver competition post api.
    #[clap(long, env)]
    pub solver_competition_auth: Option<String>,
}

pub fn display_option(option: &Option<impl Display>, f: &mut Formatter<'_>) -> std::fmt::Result {
    match option {
        Some(display) => write!(f, "{}", display),
        None => write!(f, "None"),
    }
}

pub fn display_list<T>(iter: impl Iterator<Item = T>, f: &mut Formatter<'_>) -> std::fmt::Result
where
    T: Display,
{
    write!(f, "[")?;
    for t in iter {
        write!(f, "{}, ", t)?;
    }
    write!(f, "]")?;
    Ok(())
}

// We have a custom Display implementation so that we can log the arguments on start up without
// leaking any potentially secret values.
impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "log_filter: {}", self.log_filter)?;
        writeln!(f, "log_stderr_threshold: {}", self.log_stderr_threshold)?;
        writeln!(f, "node_url: {}", self.node_url)?;
        writeln!(f, "http_timeout: {:?}", self.http_timeout)?;
        writeln!(f, "gas_estimators: {:?}", self.gas_estimators)?;
        writeln!(
            f,
            "blocknative_api_key: {}",
            self.blocknative_api_key
                .as_ref()
                .map(|_| "SECRET")
                .unwrap_or("None")
        )?;
        writeln!(f, "base_tokens: {:?}", self.base_tokens)?;
        writeln!(f, "baseline_sources: {:?}", self.baseline_sources)?;
        writeln!(f, "pool_cache_blocks: {}", self.pool_cache_blocks)?;
        writeln!(
            f,
            "pool_cache_maximum_recent_block_age: {}",
            self.pool_cache_maximum_recent_block_age
        )?;
        writeln!(
            f,
            "pool_cache_maximum_retries: {}",
            self.pool_cache_maximum_retries
        )?;
        writeln!(
            f,
            "pool_cache_delay_between_retries_seconds: {:?}",
            self.pool_cache_delay_between_retries_seconds
        )?;
        writeln!(
            f,
            "block_stream_poll_interval_seconds: {:?}",
            self.block_stream_poll_interval_seconds
        )?;
        writeln!(f)?;
        writeln!(f, "balancer_factories: {:?}", self.balancer_factories)?;
        writeln!(
            f,
            "balancer_pool_deny_list: {:?}",
            self.balancer_pool_deny_list
        )?;
        writeln!(f, "koyo_factories: {:?}", self.koyo_factories)?;
        writeln!(f, "koyo_pool_deny_list: {:?}", self.koyo_pool_deny_list)?;
        writeln!(
            f,
            "koyo_sor_supported_chains: {:?}",
            self.koyo_sor_supported_chains
        )?;
        writeln!(
            f,
            "solver_competition_auth: {}",
            self.solver_competition_auth
                .as_ref()
                .map(|_| "SECRET")
                .unwrap_or("None")
        )?;
        Ok(())
    }
}

pub fn parse_unbounded_factor(s: &str) -> Result<f64> {
    let factor = f64::from_str(s)?;
    ensure!(factor.is_finite() && factor >= 0.);
    Ok(factor)
}

pub fn parse_percentage_factor(s: &str) -> Result<f64> {
    let percentage_factor = f64::from_str(s)?;
    ensure!(percentage_factor.is_finite() && percentage_factor >= 0. && percentage_factor <= 1.0);
    Ok(percentage_factor)
}

pub fn duration_from_seconds(s: &str) -> Result<Duration, ParseFloatError> {
    Ok(Duration::from_secs_f32(s.parse()?))
}

pub fn wei_from_base_unit(s: &str) -> anyhow::Result<U256> {
    Ok(U256::from_dec_str(s)? * U256::exp10(18))
}

pub fn wei_from_gwei(s: &str) -> anyhow::Result<f64> {
    let in_gwei: f64 = s.parse()?;
    Ok(in_gwei * 1e9)
}

impl FromStr for RateLimitingStrategy {
    type Err = anyhow::Error;

    fn from_str(config: &str) -> Result<Self> {
        let mut parts = config.split(',');
        let back_off_growth_factor = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing back_off_growth_factor"))?;
        let min_back_off = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing min_back_off"))?;
        let max_back_off = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing max_back_off"))?;
        ensure!(
            parts.next().is_none(),
            "extraneous rate limiting parameters"
        );
        let back_off_growth_factor: f64 = back_off_growth_factor
            .parse()
            .context("parsing back_off_growth_factor")?;
        let min_back_off = duration_from_seconds(min_back_off).context("parsing min_back_off")?;
        let max_back_off = duration_from_seconds(max_back_off).context("parsing max_back_off")?;
        Self::try_new(back_off_growth_factor, min_back_off, max_back_off)
    }
}

use crate::chain_spec;
use crate::rpc::EthApiCmd;
use clap::Parser;
use sc_cli::{
  KeySubcommand, SignCmd, VanityCmd, VerifyCmd, CliConfiguration, DefaultConfigurationValues,
  ChainSpec, ImportParams, KeystoreParams, NetworkParams, Result, RuntimeVersion, SharedParams, SubstrateCli,
};
use std::path::PathBuf;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use regex::Regex;
use sc_telemetry::TelemetryEndpoints;
use sc_service::{
  config::{BasePath, PrometheusConfig, TransactionPoolOptions},
	Role,
};

/// Possible subcommands of the main binary.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
  /// Key management cli utilities
  #[clap(subcommand)]
  Key(KeySubcommand),

  /// Verify a signature for a message, provided on STDIN, with a given
  /// (public or secret) key.
  Verify(VerifyCmd),

  /// Generate a seed that provides a vanity address.
  Vanity(VanityCmd),

  /// Sign a message, with a given (secret) key.
  Sign(SignCmd),

  /// Build a chain specification.
  BuildSpec(sc_cli::BuildSpecCmd),

  /// Validate blocks.
  CheckBlock(sc_cli::CheckBlockCmd),

  /// Export blocks.
  ExportBlocks(sc_cli::ExportBlocksCmd),

  /// Export the state of a given block into a chain spec.
  ExportState(sc_cli::ExportStateCmd),

  /// Export the genesis state of the parachain.
  //#[clap(name = "export-genesis-state")]
  //ExportGenesisState(ExportGenesisStateCommand),

  /// Export the genesis wasm of the parachain.
  #[clap(name = "export-genesis-wasm")]
  ExportGenesisWasm(ExportGenesisWasmCommand),

  /// Import blocks.
  ImportBlocks(sc_cli::ImportBlocksCmd),

  /// Remove the whole chain.
  //PurgeChain(cumulus_client_cli::PurgeChainCmd),

  /// Revert the chain to a previous state.
  Revert(sc_cli::RevertCmd),
  // Try some testing command against a specified runtime state.
  // TryRuntime(try_runtime_cli::TryRuntimeCmd),
}

#[allow(missing_docs)]
#[derive(Debug, clap::Parser)]
pub struct RunCmd {
  #[clap(subcommand)]
  pub subcommand: Option<Subcommand>,

  #[allow(missing_docs)]
  #[clap(flatten)]
  pub base: sc_cli::RunCmd,

  ///// Id of the parachain this collator collates for.
  //#[clap(long)]
  //pub parachain_id: Option<u32>,

  /// Enable EVM tracing module on a non-authority node.
  #[clap(long, conflicts_with = "validator", require_delimiter = true)]
  pub ethapi: Vec<EthApiCmd>,

  /// Number of concurrent tracing tasks. Meant to be shared by both "debug" and "trace" modules.
  #[clap(long, default_value = "10")]
  pub ethapi_max_permits: u32,
  /// Maximum number of trace entries a single request of `trace_filter` is allowed to return.
  /// A request asking for more or an unbounded one going over this limit will both return an
  /// error.
  #[clap(long, default_value = "500")]
  pub ethapi_trace_max_count: u32,

  /// Duration (in seconds) after which the cache of `trace_filter` for a given block will be
  /// discarded.
  #[clap(long, default_value = "300")]
  pub ethapi_trace_cache_duration: u64,

  /// Size of the LRU cache for block data and their transaction statuses.
  #[clap(long, default_value = "3000")]
  pub eth_log_block_cache: usize,

  /// Maximum number of logs in a query.
  #[clap(long, default_value = "10000")]
  pub max_past_logs: u32,

  /// Maximum fee history cache size.
  #[clap(long, default_value = "2048")]
  pub fee_history_limit: u64,
}

impl DefaultConfigurationValues for RunCmd {
  fn p2p_listen_port() -> u16 {
    30334
  }

  fn rpc_ws_listen_port() -> u16 {
    9945
  }

  fn rpc_http_listen_port() -> u16 {
    9934
  }

  fn prometheus_listen_port() -> u16 {
    9616
  }
}

impl CliConfiguration<Self> for RunCmd {
  fn shared_params(&self) -> &SharedParams {
    self.base.shared_params()
  }

  fn import_params(&self) -> Option<&ImportParams> {
    self.base.import_params()
  }

  fn network_params(&self) -> Option<&NetworkParams> {
    self.base.network_params()
  }

  fn keystore_params(&self) -> Option<&KeystoreParams> {
    self.base.keystore_params()
  }

  fn base_path(&self) -> Result<Option<BasePath>> {
    self.base.base_path()
  }

  fn rpc_http(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
    self.base.rpc_http(default_listen_port)
  }

  fn rpc_ipc(&self) -> Result<Option<String>> {
    self.base.rpc_ipc()
  }

  fn rpc_ws(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
    self.base.rpc_ws(default_listen_port)
  }

  fn prometheus_config(
    &self,
    default_listen_port: u16,
    chain_spec: &Box<dyn ChainSpec>,
  ) -> Result<Option<PrometheusConfig>> {
    self
      .base
      .prometheus_config(default_listen_port, chain_spec)
  }

  fn init<F>(
    &self,
    support_url: &String,
    impl_version: &String,
    logger_hook: F,
    config: &sc_service::Configuration,
  ) -> Result<()>
  where
    F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
  {
    self.base.init(support_url, impl_version, logger_hook, config)
  }

  fn chain_id(&self, is_dev: bool) -> Result<String> {
    self.base.chain_id(is_dev)
  }

  fn role(&self, is_dev: bool) -> Result<sc_service::Role> {
    self.base.role(is_dev)
  }

  fn transaction_pool(&self) -> Result<sc_service::config::TransactionPoolOptions> {
    self.base.transaction_pool()
  }

  fn state_cache_child_ratio(&self) -> Result<Option<usize>> {
    self.base.state_cache_child_ratio()
  }

  fn rpc_methods(&self) -> Result<sc_service::config::RpcMethods> {
    self.base.rpc_methods()
  }

  fn rpc_ws_max_connections(&self) -> Result<Option<usize>> {
    self.base.rpc_ws_max_connections()
  }

  fn rpc_cors(&self, is_dev: bool) -> Result<Option<Vec<String>>> {
    self.base.rpc_cors(is_dev)
  }

  // fn telemetry_external_transport(&self) -> Result<Option<sc_service::config::ExtTransport>> {
  //   self.base.telemetry_external_transport()
  // }

  fn default_heap_pages(&self) -> Result<Option<u64>> {
    self.base.default_heap_pages()
  }

  fn force_authoring(&self) -> Result<bool> {
    self.base.force_authoring()
  }

  fn disable_grandpa(&self) -> Result<bool> {
    self.base.disable_grandpa()
  }

  fn max_runtime_instances(&self) -> Result<Option<usize>> {
    self.base.max_runtime_instances()
  }

  fn announce_block(&self) -> Result<bool> {
    self.base.announce_block()
  }

  fn telemetry_endpoints(
    &self,
    chain_spec: &Box<dyn ChainSpec>,
  ) -> Result<Option<sc_telemetry::TelemetryEndpoints>> {
    self.base.telemetry_endpoints(chain_spec)
  }
}

/// Command for exporting the genesis state of the parachain
#[derive(Debug, Parser)]
pub struct ExportGenesisStateCommand {
  /// Output file name or stdout if unspecified.
  #[clap(parse(from_os_str))]
  pub output: Option<PathBuf>,

  /// Write output in binary. Default is to write in hex.
  #[clap(short, long)]
  pub raw: bool,

  /// The name of the chain for that the genesis state should be exported.
  #[clap(long)]
  pub chain: Option<String>,

  /// Id of the parachain this state is for.
  #[clap(long, default_value = "2002")]
  pub parachain_id: u32,
}

/// Command for exporting the genesis wasm file.
#[derive(Debug, Parser)]
pub struct ExportGenesisWasmCommand {
  /// Output file name or stdout if unspecified.
  #[clap(parse(from_os_str))]
  pub output: Option<PathBuf>,

  /// Write output in binary. Default is to write in hex.
  #[clap(short, long)]
  pub raw: bool,

  /// The name of the chain for that the genesis wasm file should be exported.
  #[clap(long)]
  pub chain: Option<String>,
}

#[derive(Debug, clap::Parser)]
#[clap(
  propagate_version = true,
  args_conflicts_with_subcommands = true,
  subcommand_negates_reqs = true
)]
pub struct Cli {
  #[clap(subcommand)]
  pub subcommand: Option<Subcommand>,

  #[clap(flatten)]
  pub run: RunCmd,

  #[clap(long)]
  pub no_hardware_benchmarks: bool,

  /// Relaychain arguments
  #[clap(raw = true)]
  pub relaychain_args: Vec<String>,
}

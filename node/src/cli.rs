use crate::chain_spec;
use crate::rpc::EthApiCmd;
use clap::Parser;
use sc_cli::{KeySubcommand, SignCmd, VanityCmd, VerifyCmd};
use std::path::PathBuf;

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
  #[clap(name = "export-genesis-state")]
  ExportGenesisState(ExportGenesisStateCommand),

  /// Export the genesis wasm of the parachain.
  #[clap(name = "export-genesis-wasm")]
  ExportGenesisWasm(ExportGenesisWasmCommand),

  /// Import blocks.
  ImportBlocks(sc_cli::ImportBlocksCmd),

  /// Remove the whole chain.
  PurgeChain(cumulus_client_cli::PurgeChainCmd),

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
  pub base: cumulus_client_cli::RunCmd,

  /// Id of the parachain this collator collates for.
  #[clap(long)]
  pub parachain_id: Option<u32>,

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

#[derive(Debug)]
pub struct RelayChainCli {
  /// The actual relay chain cli object.
  pub base: polkadot_cli::RunCmd,

  /// Optional chain id that should be passed to the relay chain.
  pub chain_id: Option<String>,

  /// The base path that should be used by the relay chain.
  pub base_path: Option<PathBuf>,
}

impl RelayChainCli {
  /// Create a new instance of `Self`.
  pub fn new<'a>(
    para_config: &sc_service::Configuration,
    relay_chain_args: impl Iterator<Item = &'a String>,
  ) -> Self {
    let extension = chain_spec::Extensions::try_get(&*para_config.chain_spec);
    let chain_id = extension.map(|e| e.relay_chain.clone());
    let base_path = para_config
      .base_path
      .as_ref()
      .map(|x| x.path().join("polkadot"));
    Self {
      base_path,
      chain_id,
      base: polkadot_cli::RunCmd::from_iter(relay_chain_args),
    }
  }
}

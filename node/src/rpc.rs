//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]
use std::sync::Arc;

use fc_rpc::{CacheRequester as TraceFilterCacheRequester, DebugRequester};
use fc_rpc::{
  Debug, DebugApiServer, EthBlockDataCache, OverrideHandle, RuntimeApiStorageOverride,
  SchemaV1Override, SchemaV2Override, SchemaV3Override, StorageOverride, Trace, TraceApiServer,
};
use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
use jsonrpsee::RpcModule;
use pallet_ethereum::EthereumStorageSchema;
use primitives::{AccountId, Balance, Block, BlockNumber, Hash, Index};
use sc_consensus_babe::{Config, Epoch};
use sc_client_api::backend::{AuxStore, Backend, StateBackend, StorageProvider};
use sc_network::NetworkService;
pub use sc_rpc::SubscriptionTaskExecutor;
pub use sc_rpc_api::DenyUnsafe;
use sc_service::TransactionPool;
use sc_transaction_pool::{ChainApi, Pool};
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_runtime::traits::BlakeTwo256;
use std::collections::BTreeMap;
use sp_consensus_babe::BabeApi;

/// Extra dependencies for BABE.
pub struct BabeDeps {
  /// BABE protocol config.
  pub babe_config: Config,
  /// BABE pending epoch changes.
  pub shared_epoch_changes: sc_consensus_epochs::SharedEpochChanges<Block, Epoch>,
  /// The keystore that manages the keys of the node.
  pub keystore: sp_keystore::SyncCryptoStorePtr,
}

/// Extra dependencies for GRANDPA
pub struct GrandpaDeps<B> {
  /// Voting round info.
  pub shared_voter_state: sc_finality_grandpa::SharedVoterState,
  /// Authority set info.
  pub shared_authority_set: sc_finality_grandpa::SharedAuthoritySet<Hash, BlockNumber>,
  /// Receives notifications about justification events from Grandpa.
  pub justification_stream: sc_finality_grandpa::GrandpaJustificationStream<Block>,
  /// Subscription manager to keep track of pubsub subscribers.
  pub subscription_executor: SubscriptionTaskExecutor,
  /// Finality proof provider.
  pub finality_provider: Arc<sc_finality_grandpa::FinalityProofProvider<B, Block>>,
}

/// Full client dependencies.
pub struct FullDeps<C, P, A: ChainApi, SC, B> {
  /// The client instance to use.
  pub client: Arc<C>,
  /// Transaction pool instance.
  pub pool: Arc<P>,
  /// The SelectChain Strategy
  pub select_chain: SC,
  /// Graph pool instance.
  pub graph: Arc<Pool<A>>,
  // pub chain_spec: Box<dyn sc_chain_spec::ChainSpec>,
  /// Whether to deny unsafe calls
  pub deny_unsafe: DenyUnsafe,
  /// Ethereum pending transactions.
  /// BABE specific dependencies.
  pub babe: BabeDeps,
  /// GRANDPA specific dependencies.
  pub grandpa: GrandpaDeps<B>,
  // pub pending_transactions: PendingTransactions,
  /// EthFilterApi pool.
  pub filter_pool: Option<FilterPool>,
  /// Backend.
  pub backend: Arc<fc_db::Backend<Block>>,
  /// Maximum number of logs in a query.
  pub max_past_logs: u32,
  /// Maximum fee history cache size.
  pub fee_history_cache_limit: FeeHistoryCacheLimit,
  /// Fee history cache.
  pub fee_history_cache: FeeHistoryCache,
  /// Ethereum data access overrides.
  pub overrides: Arc<OverrideHandle<Block>>,
  /// Cache for Ethereum block data.
  pub block_data_cache: Arc<EthBlockDataCache<Block>>,
  /// Rpc requester for evm trace
  pub tracing_requesters: RpcRequesters,
  /// Rpc Config
  pub rpc_config: RpcConfig,
  /// The Node authority flag
  pub is_authority: bool,
  /// Network service
  pub network: Arc<NetworkService<Block, Hash>>,
}

#[allow(missing_docs)]
#[derive(Clone)]
pub struct RpcRequesters {
  pub debug: Option<DebugRequester>,
  pub trace: Option<TraceFilterCacheRequester>,
}

#[allow(missing_docs)]
#[derive(Debug, PartialEq, Clone)]
pub struct RpcConfig {
  pub ethapi: Vec<EthApiCmd>,
  pub ethapi_max_permits: u32,
  pub ethapi_trace_max_count: u32,
  pub ethapi_trace_cache_duration: u64,
  pub eth_log_block_cache: usize,
  pub max_past_logs: u32,
}

use std::str::FromStr;
impl FromStr for EthApiCmd {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(match s {
      "debug" => Self::Debug,
      "trace" => Self::Trace,
      _ => {
        return Err(format!(
          "`{}` is not recognized as a supported Ethereum Api",
          s
        ))
      }
    })
  }
}

#[allow(missing_docs)]
#[derive(Debug, PartialEq, Clone)]
pub enum EthApiCmd {
  Debug,
  Trace,
}

#[allow(missing_docs)]
pub fn overrides_handle<C, BE>(client: Arc<C>) -> Arc<OverrideHandle<Block>>
where
  C: ProvideRuntimeApi<Block> + StorageProvider<Block, BE> + AuxStore,
  C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
  C: Send + Sync + 'static,
  C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
  BE: Backend<Block> + 'static,
  BE::State: StateBackend<BlakeTwo256>,
{
  let mut overrides_map = BTreeMap::new();
  overrides_map.insert(
    EthereumStorageSchema::V1,
    Box::new(SchemaV1Override::new(client.clone())) as Box<dyn StorageOverride<_> + Send + Sync>,
  );
  overrides_map.insert(
    EthereumStorageSchema::V2,
    Box::new(SchemaV2Override::new(client.clone())) as Box<dyn StorageOverride<_> + Send + Sync>,
  );
  overrides_map.insert(
    EthereumStorageSchema::V3,
    Box::new(SchemaV3Override::new(client.clone())) as Box<dyn StorageOverride<_> + Send + Sync>,
  );

  Arc::new(OverrideHandle {
    schemas: overrides_map,
    fallback: Box::new(RuntimeApiStorageOverride::new(client.clone())),
  })
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, B, A, SC>(
  deps: FullDeps<C, P, A, SC, B>,
  subscription_task_executor: SubscriptionTaskExecutor,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
  C: ProvideRuntimeApi<Block>
    + sc_client_api::backend::StorageProvider<Block, B>
    + sc_client_api::BlockBackend<Block>
    + HeaderBackend<Block>
    + AuxStore
    + HeaderMetadata<Block, Error = BlockChainError>
    + Sync
    + Send
    + 'static,
  C: sc_client_api::client::BlockchainEvents<Block>,
  C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + 'static,
  C: Send + Sync + 'static,
  C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
  C::Api: pallet_contracts_rpc::ContractsRuntimeApi<Block, AccountId, Balance, BlockNumber, Hash>,
  C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
  C::Api: fp_rpc::ConvertTransactionRuntimeApi<Block>,
  C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
  C::Api: BlockBuilder<Block>,
  C::Api: BabeApi<Block>,
  P: TransactionPool<Block = Block> + 'static,
  B: Backend<Block> + 'static,
  B::State: StateBackend<BlakeTwo256>,
  A: ChainApi<Block = Block> + 'static,
  SC: sp_consensus::SelectChain<Block> + 'static,
  B: sc_client_api::Backend<Block> + Send + Sync + 'static,
  B::State: sc_client_api::backend::StateBackend<sp_runtime::traits::HashFor<Block>>,
{
  use fc_rpc::{
    Eth, EthApiServer, EthDevSigner, EthFilter, EthFilterApiServer, EthPubSub, EthPubSubApiServer,
    EthSigner, Net, NetApiServer, Web3, Web3ApiServer,
  };
  use pallet_contracts_rpc::{Contracts, ContractsApiServer};
  use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
  use substrate_frame_rpc_system::{System, SystemApiServer};
  use sc_consensus_babe_rpc::{Babe, BabeApiServer};

  let mut io = RpcModule::new(());

  let FullDeps {
    client,
    pool,
    select_chain,
    graph,
    // chain_spec: _,
    deny_unsafe,
    babe,
    grandpa,
    network,
    // pending_transactions,
    filter_pool,
    fee_history_cache_limit,
    fee_history_cache,
    backend,
    max_past_logs,
    is_authority,
    overrides: _,
    block_data_cache,
    tracing_requesters,
    rpc_config,
  } = deps;

  let BabeDeps {
    keystore,
    babe_config,
    shared_epoch_changes,
  } = babe;
  let GrandpaDeps {
    shared_voter_state,
    shared_authority_set,
    justification_stream,
    subscription_executor,
    finality_provider,
  } = grandpa;

  io.merge(System::new(client.clone(), pool.clone(), deny_unsafe).into_rpc())?;
  io.merge(TransactionPayment::new(client.clone()).into_rpc())?;
  io.merge(Contracts::new(client.clone()).into_rpc())?;
  io.merge(Babe::new(
    client.clone(),
    shared_epoch_changes.clone(),
    keystore,
    babe_config,
    select_chain,
    deny_unsafe,
  ).into_rpc());

  //   io.extend_with(
  // 		sc_sync_state_rpc::SyncStateRpcApi::to_delegate(
  // 			sc_sync_state_rpc::SyncStateRpcHandler::new(
  // 				chain_spec,
  // 				client.clone(),
  // 				shared_authority_set,
  // 				shared_epoch_changes,
  // 				deny_unsafe,
  // 			)
  // 		)
  // 	);

  let mut signers = Vec::new();
  signers.push(Box::new(EthDevSigner::new()) as Box<dyn EthSigner>);

  let mut overrides_map = BTreeMap::new();
  overrides_map.insert(
    EthereumStorageSchema::V1,
    Box::new(SchemaV1Override::new(client.clone())) as Box<dyn StorageOverride<_> + Send + Sync>,
  );

  let overrides = Arc::new(OverrideHandle {
    schemas: overrides_map,
    fallback: Box::new(RuntimeApiStorageOverride::new(client.clone())),
  });

  io.merge(
    Eth::new(
      client.clone(),
      pool.clone(),
      graph,
      Some(glitch_runtime::TransactionConverter),
      network.clone(),
      // pending_transactions.clone(),
      signers,
      overrides.clone(),
      backend.clone(),
      is_authority,
      max_past_logs,
      block_data_cache.clone(),
      fee_history_cache,
      fee_history_cache_limit,
    )
    .into_rpc(),
  )?;

  if let Some(filter_pool) = filter_pool {
    io.merge(
      EthFilter::new(
        client.clone(),
        backend,
        filter_pool.clone(),
        500 as usize, // max stored filters
        max_past_logs,
        block_data_cache.clone(),
      )
      .into_rpc(),
    )?;
  }

  io.merge(Net::new(client.clone(), network.clone(), true).into_rpc())?;

  io.merge(Web3::new(client.clone()).into_rpc())?;
  if let Some(trace_filter_requester) = tracing_requesters.trace {
    io.merge(
      Trace::new(
        client.clone(),
        trace_filter_requester,
        rpc_config.ethapi_trace_max_count,
      )
      .into_rpc(),
    )?;
  }

  if let Some(debug_requester) = tracing_requesters.debug {
    io.merge(Debug::new(debug_requester).into_rpc())?;
  }

  io.merge(
    EthPubSub::new(
      pool,
      client.clone(),
      network.clone(),
      subscription_task_executor,
      overrides,
    )
    .into_rpc(),
  )?;

  Ok(io)
}

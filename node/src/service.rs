//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::{
  collections::{BTreeMap},
  sync::{Arc, Mutex},
  time::Duration,
};

use crate::rpc::{EthApiCmd, RpcConfig, RpcRequesters};

use jsonrpsee::RpcModule;

use sc_network::{Event};

use glitch_runtime::{self, opaque::Block, RuntimeApi};
use fc_rpc::{CacheTask, DebugTask, OverrideHandle};
use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
use sc_cli::SubstrateCli;
use sc_client_api::{BlockchainEvents, ExecutorProvider, BlockBackend};
pub use sc_executor::NativeElseWasmExecutor;
use sc_service::{
  error::Error as ServiceError, BasePath, Configuration, Role, TaskManager,
};
use sp_runtime::traits::Block as BlockT;
// pub use sc_executor::NativeExecutor;
use fc_consensus::FrontierBlockImport;
use fc_mapping_sync::{MappingSyncWorker, SyncStrategy};
use futures::StreamExt;
use sc_telemetry::{Telemetry, TelemetryWorker, TelemetryWorkerHandle};
use tokio::sync::Semaphore;

use crate::cli::Cli;

// Our native executor instance.
pub struct ExecutorDispatch;

impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
  type ExtendHostFunctions = fp_trace_ext::evm_ext::HostFunctions;

  fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
    glitch_runtime::api::dispatch(method, data)
  }

  fn native_version() -> sc_executor::NativeVersion {
    glitch_runtime::native_version()
  }
}

type FullClient =
  sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FrontierBackend = fc_db::Backend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

pub fn open_frontier_backend(config: &Configuration) -> Result<Arc<fc_db::Backend<Block>>, String> {
  let config_dir = config
    .base_path
    .as_ref()
    .map(|base_path| base_path.config_dir(config.chain_spec.id()))
    .unwrap_or_else(|| {
      BasePath::from_project("", "", &crate::cli::Cli::executable_name())
        .config_dir(config.chain_spec.id())
    });
  let database_dir = config_dir.join("frontier").join("db");

  Ok(Arc::new(fc_db::Backend::<Block>::new(
    &fc_db::DatabaseSettings {
      source: fc_db::DatabaseSettingsSrc::RocksDb {
        path: database_dir,
        cache_size: 0,
      },
    },
  )?))
}

fn create_rpc_requesters(
  rpc_config: &RpcConfig,
  client: Arc<FullClient>,
  substrate_backend: Arc<FullBackend>,
  frontier_backend: Arc<FrontierBackend>,
  task_manager: &TaskManager,
  overrides: Arc<OverrideHandle<Block>>,
) -> RpcRequesters {
  let cmd = rpc_config.ethapi.clone();
  let (trace_requester, debug_requester) =
    if cmd.contains(&EthApiCmd::Debug) || cmd.contains(&EthApiCmd::Trace) {
      let permit_pool = Arc::new(Semaphore::new(rpc_config.ethapi_max_permits as usize));
      let trace_filter_requester = if rpc_config.ethapi.contains(&EthApiCmd::Trace) {
        let (trace_filter_task, trace_filter_requester) = CacheTask::create(
          Arc::clone(&client),
          Arc::clone(&substrate_backend),
          Duration::from_secs(rpc_config.ethapi_trace_cache_duration),
          Arc::clone(&permit_pool),
          overrides.clone(),
        );
        task_manager
          .spawn_essential_handle()
          .spawn("trace-filter-cache", None, trace_filter_task);
        Some(trace_filter_requester)
      } else {
        None
      };

      let debug_requester = if rpc_config.ethapi.contains(&EthApiCmd::Debug) {
        let (debug_task, debug_requester) = DebugTask::task(
          Arc::clone(&client),
          Arc::clone(&substrate_backend),
          Arc::clone(&frontier_backend),
          Arc::clone(&permit_pool),
        );
        task_manager
          .spawn_essential_handle()
          .spawn("ethapi-debug", None, debug_task);
        Some(debug_requester)
      } else {
        None
      };
      (trace_filter_requester, debug_requester)
    } else {
      (None, None)
    };
  RpcRequesters {
    debug: debug_requester,
    trace: trace_requester,
  }
}

type FullGrandpaBlockImport = sc_finality_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;
type FullBabeBlockImport = sc_consensus_babe::BabeBlockImport<Block, FullClient, FrontierBlockImport<Block, FullGrandpaBlockImport, FullClient>>;

#[allow(clippy::type_complexity)]
pub fn new_partial(
  config: &Configuration,
  cli: &Cli,
) -> Result<
  sc_service::PartialComponents<
    FullClient,
    FullBackend,
    FullSelectChain,
    sc_consensus::DefaultImportQueue<Block, FullClient>,
    sc_transaction_pool::FullPool<Block, FullClient>,
    (
      impl Fn(
        crate::rpc::DenyUnsafe,
        crate::rpc::SubscriptionTaskExecutor,
        Arc<sc_network::NetworkService<Block, primitives::Hash>>,
      ) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>,
      Option<FilterPool>,
      Arc<fc_db::Backend<Block>>,
      Option<Telemetry>,
      Option<TelemetryWorkerHandle>,
      (FeeHistoryCache, FeeHistoryCacheLimit),
      FullGrandpaBlockImport,
      FullBabeBlockImport, 
      sc_finality_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
      sc_consensus_babe::BabeLink<Block>,
      sc_finality_grandpa::SharedVoterState,
    ),
  >,
  ServiceError,
> {
  let telemetry = config
    .telemetry_endpoints
    .clone()
    .filter(|x| !x.is_empty())
    .map(|endpoints| -> Result<_, sc_telemetry::Error> {
      let worker = TelemetryWorker::new(16)?;
      let telemetry = worker.handle().new_telemetry(endpoints);
      Ok((worker, telemetry))
    })
    .transpose()?;

  let _registry = config.prometheus_registry();

  let executor = NativeElseWasmExecutor::<ExecutorDispatch>::new(
    config.wasm_method,
    config.default_heap_pages,
    config.max_runtime_instances,
    config.runtime_cache_size,
  );

  let rpc_config = RpcConfig {
    ethapi: cli.run.ethapi.clone(),
    ethapi_max_permits: cli.run.ethapi_max_permits,
    ethapi_trace_max_count: cli.run.ethapi_trace_max_count,
    ethapi_trace_cache_duration: cli.run.ethapi_trace_cache_duration,
    eth_log_block_cache: cli.run.eth_log_block_cache,
    max_past_logs: cli.run.max_past_logs,
  };

  let (client, backend, keystore_container, task_manager) =
    sc_service::new_full_parts::<Block, RuntimeApi, _>(
      &config,
      telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
      executor,
    )?;

  let client = Arc::new(client);

  let select_chain = sc_consensus::LongestChain::new(backend.clone());

  let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

  let telemetry = telemetry.map(|(worker, telemetry)| {
    task_manager
      .spawn_handle()
      .spawn("telemetry", None, worker.run());
    telemetry
  });

  let transaction_pool = sc_transaction_pool::BasicPool::new_full(
    config.transaction_pool.clone(),
    config.role.is_authority().into(),
    config.prometheus_registry(),
    task_manager.spawn_essential_handle(),
    client.clone(),
  );

  //  let pending_transactions: PendingTransactions
  // = Some(Arc::new(Mutex::new(HashMap::new())));

  let filter_pool: Option<FilterPool> = Some(Arc::new(Mutex::new(BTreeMap::new())));
  let fee_history_cache: FeeHistoryCache = Arc::new(Mutex::new(BTreeMap::new()));
  let fee_history_cache_limit: FeeHistoryCacheLimit = cli.run.fee_history_limit;

  let frontier_backend = open_frontier_backend(config)?;

  let (grandpa_block_import, grandpa_link) = sc_finality_grandpa::block_import(
    client.clone(),
    &(client.clone() as Arc<_>),
    select_chain.clone(),
    telemetry.as_ref().map(|x| x.handle()),
  )?;

  let justification_import = grandpa_block_import.clone();
  let frontier_block_import = FrontierBlockImport::new(
    grandpa_block_import.clone(),
    client.clone(),
    frontier_backend.clone(),
  );

  let (block_import, babe_link) = sc_consensus_babe::block_import(
    sc_consensus_babe::Config::get(&*client)?,
    frontier_block_import,
    client.clone(),
  )?;

  let slot_duration = babe_link.config().slot_duration();
  let import_queue = sc_consensus_babe::import_queue(
    babe_link.clone(),
    block_import.clone(),
    Some(Box::new(justification_import)),
    client.clone(),
    select_chain.clone(),
    move |_, ()| async move {
      let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
      
      let slot =
        sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
          *timestamp,
          slot_duration,
        );
      
      let uncles =
        sp_authorship::InherentDataProvider::<<Block as BlockT>::Header>::check_inherents();
      
      Ok((timestamp, slot, uncles))
    }, 
    &task_manager.spawn_essential_handle(),
    config.prometheus_registry(),
    sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
    telemetry.as_ref().map(|x| x.handle()),
  )?;


  let shared_voter_state = sc_finality_grandpa::SharedVoterState::empty();
  let rpc_extensions_builder = {
    let justification_stream = grandpa_link.justification_stream();
    let shared_authority_set = grandpa_link.shared_authority_set().clone();
    let shared_voter_state = shared_voter_state.clone();

    let finality_proof_provider = sc_finality_grandpa::FinalityProofProvider::new_for_service(
      backend.clone(),
      Some(shared_authority_set.clone()),
    );

    let babe_config = babe_link.config().clone();
    let shared_epoch_changes = babe_link.epoch_changes().clone();

    let client = client.clone();
    let pool = transaction_pool.clone();
    let select_chain = select_chain.clone();
    let chain_spec = config.chain_spec.cloned_box();
    let keystore = keystore_container.sync_keystore();
    let is_authority = config.role.is_authority();
    // let subscription_task_executor =
    // sc_rpc::SubscriptionTaskExecutor::new(task_manager.spawn_handle());

    // let pending = pending_transactions.clone();
    let filter_pool_clone = filter_pool.clone();
    let fee_history_cache_clone = fee_history_cache.clone();
    let _backend_clone = backend.clone();
    let frontier_backend_clone = frontier_backend.clone();
    let max_past_logs = cli.run.max_past_logs;

    let overrides = crate::rpc::overrides_handle(client.clone());
    let fee_history_cache_limit = cli.run.fee_history_limit;

    let block_data_cache = Arc::new(fc_rpc::EthBlockDataCache::new(
      task_manager.spawn_handle(),
      overrides.clone(),
      50,
      50,
    ));

    let tracing_requesters = create_rpc_requesters(
      &rpc_config,
      client.clone(),
      backend.clone(),
      frontier_backend.clone(),
      &task_manager,
      overrides.clone(),
    );

    let rpc_extensions_builder =
      move |deny_unsafe,
            subscription_task_executor: sc_rpc::SubscriptionTaskExecutor,
            network: Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>| {
        let overrides = overrides.clone();
        let deps = crate::rpc::FullDeps {
          client: client.clone(),
          pool: pool.clone(),
          select_chain: select_chain.clone(),
          graph: pool.pool().clone(),
          chain_spec: chain_spec.cloned_box(),
          deny_unsafe,
          babe: crate::rpc::BabeDeps {
            babe_config: babe_config.clone(),
            shared_epoch_changes: shared_epoch_changes.clone(),
            keystore: keystore.clone(),
          },
          grandpa: crate::rpc::GrandpaDeps {
            shared_voter_state: shared_voter_state.clone(),
            shared_authority_set: shared_authority_set.clone(),
            justification_stream: justification_stream.clone(),
            subscription_executor: subscription_task_executor.clone(),
            finality_provider: finality_proof_provider.clone(),
          },
          // pending_transactions: pending.clone(),
          filter_pool: filter_pool_clone.clone(),
          backend: frontier_backend_clone.clone(),
          is_authority,
          max_past_logs,
          fee_history_cache_limit,
          fee_history_cache: fee_history_cache_clone.clone(),
          network: network,
          overrides: overrides.clone(),
          block_data_cache: block_data_cache.clone(),
          tracing_requesters: tracing_requesters.clone(),
          rpc_config: rpc_config.clone(),
        };

        crate::rpc::create_full(deps, subscription_task_executor.clone()).map_err(Into::into)
      };

    rpc_extensions_builder
  };

  Ok(sc_service::PartialComponents {
    client,
    backend,
    task_manager,
    keystore_container,
    select_chain,
    import_queue,
    transaction_pool,
    other: (
      rpc_extensions_builder,
      filter_pool,
      frontier_backend,
      telemetry,
      telemetry_worker_handle,
      (fee_history_cache, fee_history_cache_limit),
      grandpa_block_import,
      block_import,
      grandpa_link,
      babe_link,
      shared_voter_state,
    ),
  })
}

/// Builds a new service for a full client.
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl(
  mut parachain_config: Configuration,
  cli: &Cli,
  _hwbench: Option<sc_sysinfo::HwBench>,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient>)> {
  if matches!(parachain_config.role, Role::Light) {
    return Err("Light client not supported!".into());
  }

  //let parachain_config = prepare_node_config(parachain_config);

  let sc_service::PartialComponents {
    client,
    backend,
    mut task_manager,
    import_queue,
    keystore_container,
    select_chain,
    transaction_pool,
    other:
      (
        partial_rpc_extensions_builder,
        filter_pool,
        frontier_backend,
        mut telemetry,
        _telemetry_worker_handle,
        (_fee_history_cache, _fee_history_cache_limit),
        _grandpa_block_import,
        block_import,
        grandpa_link,
        babe_link,
        shared_voter_state,
      ),
  } = new_partial(&parachain_config, cli)?;

  let auth_disc_publish_non_global_ips = parachain_config.network.allow_non_globals_in_dht;
  let grandpa_protocol_name = sc_finality_grandpa::protocol_standard_name(
    &client.block_hash(0).ok().flatten().expect("Genesis block exists; qed"),
    &parachain_config.chain_spec,
  );

  parachain_config
    .network
    .extra_sets
    .push(sc_finality_grandpa::grandpa_peers_set_config(grandpa_protocol_name.clone()));
  let warp_sync = Arc::new(sc_finality_grandpa::warp_proof::NetworkProvider::new(
    backend.clone(),
    grandpa_link.shared_authority_set().clone(),
    Vec::default(),
  ));

  let (network, system_rpc_tx, start_network) =
    sc_service::build_network(sc_service::BuildNetworkParams {
      config: &parachain_config,
      client: client.clone(),
      transaction_pool: transaction_pool.clone(),
      spawn_handle: task_manager.spawn_handle(),
      import_queue,
      block_announce_validator_builder: None,
      warp_sync: Some(warp_sync),
    })?;

  if parachain_config.offchain_worker.enabled {
    sc_service::build_offchain_workers(
      &parachain_config,
      task_manager.spawn_handle(),
      client.clone(),
      network.clone(),
    );
  }

  let role = parachain_config.role.clone();
  let force_authoring = parachain_config.force_authoring;
  let name = parachain_config.network.node_name.clone();
  let grandpa_enabled = !parachain_config.disable_grandpa;

  let prometheus_registry = parachain_config.prometheus_registry().cloned();

  let network_clone = network.clone();

  let rpc_extensions_builder = move |deny_unsafe, subscription_executor| {
    let r =
      partial_rpc_extensions_builder(deny_unsafe, subscription_executor, network_clone.clone())?;

    Ok(r)
  };

  task_manager.spawn_essential_handle().spawn(
    "frontier-mapping-sync-worker",
    None,
    MappingSyncWorker::new(
      client.import_notification_stream(),
      Duration::new(6, 0),
      client.clone(),
      backend.clone(),
      frontier_backend.clone(),
      SyncStrategy::Normal,
    )
    .for_each(|()| futures::future::ready(())),
  );

  let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
    rpc_builder: Box::new(rpc_extensions_builder),
    client: client.clone(),
    transaction_pool: transaction_pool.clone(),
    task_manager: &mut task_manager,
    config: parachain_config,
    keystore: keystore_container.sync_keystore(),
    backend,
    network: network.clone(),
    system_rpc_tx,
    telemetry: telemetry.as_mut(),
  })?;

  let _announce_block = {
    let network = network.clone();
    Arc::new(move |hash, data| network.announce_block(hash, data))
  };

  // Spawn Frontier EthFilterApi maintenance task.
  if filter_pool.is_some() {
    // Each filter is allowed to stay in the pool for 100 blocks.
    const FILTER_RETAIN_THRESHOLD: u64 = 100;
    task_manager.spawn_essential_handle().spawn(
      "frontier-filter-pool",
      None,
      client
        .import_notification_stream()
        .for_each(move |notification| {
          if let Ok(locked) = &mut filter_pool.clone().unwrap().lock() {
            let imported_number: u64 = notification.header.number as u64;
            for (k, v) in locked.clone().iter() {
              let lifespan_limit = v.at_block + FILTER_RETAIN_THRESHOLD;
              if lifespan_limit <= imported_number {
                locked.remove(&k);
              }
            }
          }
          futures::future::ready(())
        }),
    );
  }

  let backoff_authoring_blocks =
    Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());

	if let sc_service::config::Role::Authority { .. } = &role {
    let proposer = sc_basic_authorship::ProposerFactory::new(
      task_manager.spawn_handle(),
      client.clone(),
      transaction_pool.clone(),
      prometheus_registry.as_ref(),
      telemetry.as_ref().map(|x| x.handle()),
    );

    let can_author_with =
        sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

    let slot_duration = babe_link.config().slot_duration();
    
    let client_clone = client.clone();
    let babe_config = sc_consensus_babe::BabeParams {
      keystore: keystore_container.sync_keystore(),
      client: client.clone(),
      select_chain,
      env: proposer,
      block_import,
      sync_oracle: network.clone(),
      create_inherent_data_providers: move |parent, ()| {
        let client_clone = client_clone.clone();
        async move {
          let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
          
          let slot =
            sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
              *timestamp,
              slot_duration,
            );
          
          let uncles =
            sp_authorship::InherentDataProvider::<<Block as BlockT>::Header>::check_inherents();

          let storage_proof =
						sp_transaction_storage_proof::registration::new_data_provider(
							&*client_clone,
							&parent,
						)?;
  
          Ok((timestamp, slot, uncles, storage_proof))
        }
      },
      force_authoring,
      backoff_authoring_blocks,
      babe_link,
      can_author_with,
      block_proposal_slot_portion: sc_consensus_babe::SlotProportion::new(0.5),
      justification_sync_link: network.clone(),
      max_block_proposal_slot_portion: None,
      telemetry: telemetry.as_ref().map(|x| x.handle()),
    };

    let babe = sc_consensus_babe::start_babe(babe_config)?;

    task_manager
      .spawn_essential_handle()
      .spawn_blocking("babe-proposer", Some("block-authoring"), babe);
  }

  if role.is_authority() {
    let authority_discovery_role =
      sc_authority_discovery::Role::PublishAndDiscover(keystore_container.keystore());
		let dht_event_stream =
			network.event_stream("authority-discovery").filter_map(|e| async move {
				match e {
					Event::Dht(e) => Some(e),
					_ => None,
				}
			});
		let (authority_discovery_worker, _service) =
			sc_authority_discovery::new_worker_and_service_with_config(
				sc_authority_discovery::WorkerConfig {
					publish_non_global_ips: auth_disc_publish_non_global_ips,
					..Default::default()
				},
				client.clone(),
				network.clone(),
				Box::pin(dht_event_stream),
				authority_discovery_role,
				prometheus_registry.clone(),
			);

		task_manager.spawn_handle().spawn(
			"authority-discovery-worker",
			Some("networking"),
			authority_discovery_worker.run(),
		);
  }

  if grandpa_enabled {
    
    // if the node isn't actively participating in consensus then it doesn't
    // need a keystore, regardless of which protocol we use below.
    let keystore = if role.is_authority() { Some(keystore_container.sync_keystore()) } else { None };

    let grandpa_config = sc_finality_grandpa::Config {
      // FIXME #1578 make this available through chainspec
      gossip_duration: Duration::from_millis(333),
      justification_period: 512,
      name: Some(name),
      observer_enabled: false,
      keystore,
      local_role: role,
      telemetry: telemetry.as_ref().map(|x| x.handle()),
      protocol_name: grandpa_protocol_name,
    };

    // start the full GRANDPA voter
    // NOTE: non-authorities could run the GRANDPA observer protocol, but at
    // this point the full voter should provide better guarantees of block
    // and vote data availability than the observer. The observer has not
    // been tested extensively yet and having most nodes in a network run it
    // could lead to finality stalls.
    let grandpa_config = sc_finality_grandpa::GrandpaParams {
      config: grandpa_config,
      link: grandpa_link,
      network,
      voting_rule: sc_finality_grandpa::VotingRulesBuilder::default().build(),
      prometheus_registry,
      shared_voter_state,
      telemetry: telemetry.as_ref().map(|x| x.handle()),
    };

    // the GRANDPA voter task is considered infallible, i.e.
    // if it fails we take down the service with it.
    task_manager.spawn_essential_handle().spawn_blocking(
      "grandpa-voter",
      None,
      sc_finality_grandpa::run_grandpa_voter(grandpa_config)?,
    );
  }


  start_network.start_network();
  Ok((task_manager, client))
}

/// Start a normal parachain node.
pub async fn start_node(
  parachain_config: Configuration,
  hwbench: Option<sc_sysinfo::HwBench>,
  cli: &Cli,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient>)> {
  start_node_impl(
    parachain_config,
    cli,
    hwbench,
  )
  .await
}

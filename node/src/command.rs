// This file is part of Substrate.

// Copyright (C) 2017-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use crate::service;
use crate::service::new_partial;
use crate::{
  chain_spec,
  cli::{Cli, RelayChainCli, Subcommand},
};
use glitch_runtime::Block;
use cumulus_client_cli::generate_genesis_block;
use cumulus_primitives_core::ParaId;
use log::info;
use sc_cli::{
  ChainSpec, CliConfiguration, DefaultConfigurationValues, ImportParams, KeystoreParams,
  NetworkParams, Result, RuntimeVersion, SharedParams, SubstrateCli,
};
use sc_service::{
  config::{BasePath, PrometheusConfig},
  PartialComponents,
};
use sp_core::hexdisplay::HexDisplay;
use sp_core::Encode;
use sp_runtime::traits::{AccountIdConversion, Block as BlockT};
use std::{io::Write, net::SocketAddr};

fn load_spec(
  id: &str,
  para_id: ParaId,
) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
  Ok(match id {
    "dev" => Box::new(chain_spec::development_config(para_id)?),
    "local" => Box::new(chain_spec::local_testnet_config(para_id)?),
    "glitch_testnet" => Box::new(chain_spec::glitch_testnet_config(para_id)?),
    "glitch_mainnet" => Box::new(chain_spec::glitch_mainnet_config(para_id)?),
    "glitch_uat" => Box::new(chain_spec::glitch_uat_config(para_id)?),
    "" | "testnet" => Box::new(chain_spec::glitch_testnet_config(para_id)?),
    "mainnet" => Box::new(chain_spec::glitch_mainnet_config(para_id)?),
    "uat" => Box::new(chain_spec::glitch_uat_config(para_id)?),
    path => Box::new(chain_spec::ChainSpec::from_json_file(
      std::path::PathBuf::from(path),
    )?),
  })
}

impl SubstrateCli for Cli {
  fn impl_name() -> String {
    "Glitch Node".into()
  }

  fn impl_version() -> String {
    env!("SUBSTRATE_CLI_IMPL_VERSION").into()
  }

  fn description() -> String {
    env!("CARGO_PKG_DESCRIPTION").into()
  }

  fn author() -> String {
    env!("CARGO_PKG_AUTHORS").into()
  }

  fn support_url() -> String {
    "support.anonymous.an".into()
  }

  fn copyright_start_year() -> i32 {
    2017
  }

  fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
    load_spec(id, self.run.parachain_id.unwrap_or(2002).into())
  }

  fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
    &glitch_runtime::VERSION
  }
}

impl SubstrateCli for RelayChainCli {
  fn impl_name() -> String {
    format!("Glitch-cc1 Parachain Collator").into()
  }

  fn impl_version() -> String {
    env!("SUBSTRATE_CLI_IMPL_VERSION").into()
  }

  fn description() -> String {
    env!("CARGO_PKG_DESCRIPTION").into()
  }

  fn author() -> String {
    env!("CARGO_PKG_AUTHORS").into()
  }

  fn support_url() -> String {
    "https://github.com/clover-network/clover/issues/new".into()
  }

  fn copyright_start_year() -> i32 {
    2020
  }

  fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
    polkadot_cli::Cli::from_iter([RelayChainCli::executable_name().to_string()].iter())
      .load_spec(id)
  }

  fn native_runtime_version(chain_spec: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
    polkadot_cli::Cli::native_runtime_version(chain_spec)
  }
}

fn extract_genesis_wasm(chain_spec: &Box<dyn sc_service::ChainSpec>) -> Result<Vec<u8>> {
  let mut storage = chain_spec.build_storage()?;

  storage
    .top
    .remove(sp_core::storage::well_known_keys::CODE)
    .ok_or_else(|| "Could not find wasm file in genesis state!".into())
}

/// Parse and run command line arguments
#[allow(dead_code)]
pub fn run() -> sc_cli::Result<()> {
  let cli = Cli::from_args();

  match &cli.subcommand {
    Some(Subcommand::Key(cmd)) => cmd.run(&cli),
    Some(Subcommand::Sign(cmd)) => cmd.run(),
    Some(Subcommand::Verify(cmd)) => cmd.run(),
    Some(Subcommand::Vanity(cmd)) => cmd.run(),
    Some(Subcommand::BuildSpec(cmd)) => {
      let runner = cli.create_runner(cmd)?;
      runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
    }
    Some(Subcommand::CheckBlock(cmd)) => {
      let runner = cli.create_runner(cmd)?;
      runner.async_run(|config| {
        let PartialComponents {
          client,
          task_manager,
          import_queue,
          ..
        } = new_partial(&config, &cli)?;
        Ok((cmd.run(client, import_queue), task_manager))
      })
    }
    Some(Subcommand::ExportBlocks(cmd)) => {
      let runner = cli.create_runner(cmd)?;
      runner.async_run(|config| {
        let PartialComponents {
          client,
          task_manager,
          ..
        } = new_partial(&config, &cli)?;
        Ok((cmd.run(client, config.database), task_manager))
      })
    }

    Some(Subcommand::ExportGenesisState(params)) => {
      let mut builder = sc_cli::LoggerBuilder::new("");
      builder.with_profiling(sc_tracing::TracingReceiver::Log, "");
      let _ = builder.init();

      let spec = load_spec(
        &params.chain.clone().unwrap_or_default(),
        params.parachain_id.into(),
      )?;
      let state_version = Cli::native_runtime_version(&spec).state_version();
      let block: Block = generate_genesis_block(&*spec, state_version)?;
      let raw_header = block.header().encode();
      let output_buf = if params.raw {
        raw_header
      } else {
        format!("0x{:?}", HexDisplay::from(&block.header().encode())).into_bytes()
      };

      if let Some(output) = &params.output {
        std::fs::write(output, output_buf)?;
      } else {
        std::io::stdout().write_all(&output_buf)?;
      }

      Ok(())
    }
    Some(Subcommand::ExportGenesisWasm(params)) => {
      let mut builder = sc_cli::LoggerBuilder::new("");
      builder.with_profiling(sc_tracing::TracingReceiver::Log, "");
      let _ = builder.init();

      let raw_wasm_blob =
        extract_genesis_wasm(&cli.load_spec(&params.chain.clone().unwrap_or_default())?)?;
      let output_buf = if params.raw {
        raw_wasm_blob
      } else {
        format!("0x{:?}", HexDisplay::from(&raw_wasm_blob)).into_bytes()
      };

      if let Some(output) = &params.output {
        std::fs::write(output, output_buf)?;
      } else {
        std::io::stdout().write_all(&output_buf)?;
      }

      Ok(())
    }

    Some(Subcommand::ExportState(cmd)) => {
      let runner = cli.create_runner(cmd)?;
      runner.async_run(|config| {
        let PartialComponents {
          client,
          task_manager,
          ..
        } = new_partial(&config, &cli)?;
        Ok((cmd.run(client, config.chain_spec), task_manager))
      })
    }

    Some(Subcommand::ImportBlocks(cmd)) => {
      let runner = cli.create_runner(cmd)?;

      runner.async_run(|config| {
        let PartialComponents {
          client,
          import_queue,
          task_manager,
          ..
        } = service::new_partial(&config, &cli)?;
        Ok((cmd.run(client, import_queue), task_manager))
      })
    }

    Some(Subcommand::PurgeChain(cmd)) => {
      let runner = cli.create_runner(cmd)?;
      runner.sync_run(|config| {
        let polkadot_cli = RelayChainCli::new(
          &config,
          [RelayChainCli::executable_name().to_string()]
            .iter()
            .chain(cli.relaychain_args.iter()),
        );

        let polkadot_config = SubstrateCli::create_configuration(
          &polkadot_cli,
          &polkadot_cli,
          config.tokio_handle.clone(),
        )
        .map_err(|err| format!("Relay chain argument error: {}", err))?;

        cmd.run(config, polkadot_config)
      })
    }

    Some(Subcommand::Revert(cmd)) => {
      let runner = cli.create_runner(cmd)?;

      runner.async_run(|config| {
        let PartialComponents {
          client,
          task_manager,
          backend,
          ..
        } = service::new_partial(&config, &cli)?;
        Ok((cmd.run(client, backend, None), task_manager))
      })
    }
    None => {
      let runner = cli.create_runner(&cli.run.base.normalize())?;
      let collator_options = cli.run.base.collator_options();

      runner.run_node_until_exit(|config| async move {
        let hwbench = if !cli.no_hardware_benchmarks {
          config.database.path().map(|database_path| {
            let _ = std::fs::create_dir_all(&database_path);
            sc_sysinfo::gather_hwbench(Some(database_path))
          })
        } else {
          None
        };

        let para_id = chain_spec::Extensions::try_get(&*config.chain_spec).map(|e| e.para_id);

        let polkadot_cli = RelayChainCli::new(
          &config,
          [RelayChainCli::executable_name().to_string()]
            .iter()
            .chain(cli.relaychain_args.iter()),
        );

        let id = ParaId::from(cli.run.parachain_id.or(para_id).unwrap_or(2002));

        let parachain_account =
          AccountIdConversion::<polkadot_primitives::v2::AccountId>::into_account_truncating(&id);

        let state_version = Cli::native_runtime_version(&config.chain_spec).state_version();

        let block: Block = generate_genesis_block(&*config.chain_spec, state_version)
          .map_err(|e| format!("{:?}", e))?;
        let genesis_state = format!("0x{:?}", HexDisplay::from(&block.header().encode()));

        let tokio_handle = config.tokio_handle.clone();

        let polkadot_config =
          SubstrateCli::create_configuration(&polkadot_cli, &polkadot_cli, tokio_handle)
            .map_err(|err| format!("Relay chain argument error: {}", err))?;

        let collator = config.role.is_authority();
        info!("Parachain id: {:?}", id);
        info!("Parachain Account: {}", parachain_account);
        info!("Parachain genesis state: {}", genesis_state);
        info!("Is collating: {}", if collator { "yes" } else { "no" });

        crate::service::start_node(config, polkadot_config, collator_options, id, hwbench, &cli)
          .await
          .map(|r| r.0)
          .map_err(Into::into)
      })
    }
  }
}

impl DefaultConfigurationValues for RelayChainCli {
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

impl CliConfiguration<Self> for RelayChainCli {
  fn shared_params(&self) -> &SharedParams {
    self.base.base.shared_params()
  }

  fn import_params(&self) -> Option<&ImportParams> {
    self.base.base.import_params()
  }

  fn network_params(&self) -> Option<&NetworkParams> {
    self.base.base.network_params()
  }

  fn keystore_params(&self) -> Option<&KeystoreParams> {
    self.base.base.keystore_params()
  }

  fn base_path(&self) -> Result<Option<BasePath>> {
    Ok(
      self
        .shared_params()
        .base_path()
        .or_else(|| self.base_path.clone().map(Into::into)),
    )
  }

  fn rpc_http(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
    self.base.base.rpc_http(default_listen_port)
  }

  fn rpc_ipc(&self) -> Result<Option<String>> {
    self.base.base.rpc_ipc()
  }

  fn rpc_ws(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
    self.base.base.rpc_ws(default_listen_port)
  }

  fn prometheus_config(
    &self,
    default_listen_port: u16,
    chain_spec: &Box<dyn ChainSpec>,
  ) -> Result<Option<PrometheusConfig>> {
    self
      .base
      .base
      .prometheus_config(default_listen_port, chain_spec)
  }

  fn init<F>(
    &self,
    _support_url: &String,
    _impl_version: &String,
    _logger_hook: F,
    _config: &sc_service::Configuration,
  ) -> Result<()>
  where
    F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
  {
    unreachable!("PolkadotCli is never initialized; qed");
  }

  fn chain_id(&self, is_dev: bool) -> Result<String> {
    let chain_id = self.base.base.chain_id(is_dev)?;

    Ok(if chain_id.is_empty() {
      self.chain_id.clone().unwrap_or_default()
    } else {
      chain_id
    })
  }

  fn role(&self, is_dev: bool) -> Result<sc_service::Role> {
    self.base.base.role(is_dev)
  }

  fn transaction_pool(&self) -> Result<sc_service::config::TransactionPoolOptions> {
    self.base.base.transaction_pool()
  }

  fn state_cache_child_ratio(&self) -> Result<Option<usize>> {
    self.base.base.state_cache_child_ratio()
  }

  fn rpc_methods(&self) -> Result<sc_service::config::RpcMethods> {
    self.base.base.rpc_methods()
  }

  fn rpc_ws_max_connections(&self) -> Result<Option<usize>> {
    self.base.base.rpc_ws_max_connections()
  }

  fn rpc_cors(&self, is_dev: bool) -> Result<Option<Vec<String>>> {
    self.base.base.rpc_cors(is_dev)
  }

  // fn telemetry_external_transport(&self) -> Result<Option<sc_service::config::ExtTransport>> {
  //   self.base.base.telemetry_external_transport()
  // }

  fn default_heap_pages(&self) -> Result<Option<u64>> {
    self.base.base.default_heap_pages()
  }

  fn force_authoring(&self) -> Result<bool> {
    self.base.base.force_authoring()
  }

  fn disable_grandpa(&self) -> Result<bool> {
    self.base.base.disable_grandpa()
  }

  fn max_runtime_instances(&self) -> Result<Option<usize>> {
    self.base.base.max_runtime_instances()
  }

  fn announce_block(&self) -> Result<bool> {
    self.base.base.announce_block()
  }

  fn telemetry_endpoints(
    &self,
    chain_spec: &Box<dyn ChainSpec>,
  ) -> Result<Option<sc_telemetry::TelemetryEndpoints>> {
    self.base.base.telemetry_endpoints(chain_spec)
  }
}

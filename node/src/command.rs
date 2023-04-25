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
  cli::{Cli, Subcommand},
};
//use cumulus_client_cli::generate_genesis_block;
use sc_cli::{
  ChainSpec, Result, RuntimeVersion, SubstrateCli,
};
use sc_service::{
  PartialComponents,
};
use sp_core::hexdisplay::HexDisplay;
use std::{io::Write};

fn load_spec(
  id: &str,
) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
  Ok(match id {
    "dev" => Box::new(chain_spec::development_config()?),
    "local" => Box::new(chain_spec::local_testnet_config()?),
    "glitch_testnet" => Box::new(chain_spec::glitch_testnet_config()?),
    "glitch_mainnet" => Box::new(chain_spec::glitch_mainnet_config()?),
    "glitch_uat" => Box::new(chain_spec::glitch_uat_config()?),
    "" | "testnet" => Box::new(chain_spec::glitch_testnet_config()?),
    "mainnet" => Box::new(chain_spec::glitch_mainnet_config()?),
    "uat" => Box::new(chain_spec::glitch_uat_config()?),
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
    load_spec(id)
  }

  fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
    &glitch_runtime::VERSION
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

    /*Some(Subcommand::ExportGenesisState(params)) => {
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
    }*/
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

    /*Some(Subcommand::PurgeChain(cmd)) => {
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
    }*/

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
      let runner = cli.create_runner(&cli.run)?;

      runner.run_node_until_exit(|config| async move {
        let hwbench = if !cli.no_hardware_benchmarks {
          config.database.path().map(|database_path| {
            let _ = std::fs::create_dir_all(&database_path);
            sc_sysinfo::gather_hwbench(Some(database_path))
          })
        } else {
          None
        };

        let _state_version = Cli::native_runtime_version(&config.chain_spec).state_version();

        crate::service::start_node(config, hwbench, &cli)
          .await
          .map(|r| r.0)
          .map_err(Into::into)
      })
    }
  }
}

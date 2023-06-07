// This file is part of Clover.
//
// Copyright (C) 2018-2022 Clover Network
// SPDX-License-Identifier: GPL-3.0
//
// Clover is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Clover is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Clover. If not, see <https://www.gnu.org/licenses/>.

// --- crates.io ---
use ethereum_types::H256;
use jsonrpsee::{core::RpcResult, core::async_trait, proc_macros::rpc};
use serde::Deserialize;
// --- Clover-network ---
use fc_tracer::types::single;
use fp_trace_rpc::RequestBlockId;


#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceParams {
	pub disable_storage: Option<bool>,
	pub disable_memory: Option<bool>,
	pub disable_stack: Option<bool>,
	/// Javascript tracer (we just check if it's Blockscout tracer string)
	pub tracer: Option<String>,
	pub timeout: Option<String>,
}

#[rpc(server)]
#[async_trait]
pub trait DebugApi {
	#[method(name = "debug_traceTransaction")]
	async fn trace_transaction(
		&self,
		transaction_hash: H256,
		params: Option<TraceParams>,
	) ->  RpcResult<single::TransactionTrace>;
	#[method(name = "debug_traceBlockByNumber", aliases = ["debug_traceBlockByHash"])]
	async fn trace_block(
		&self,
		id: RequestBlockId,
		params: Option<TraceParams>,
	) -> RpcResult<Vec<single::TransactionTrace>>;
}

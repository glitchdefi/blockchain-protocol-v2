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
use ethereum_types::H160;
use jsonrpsee::{core::RpcResult as Result, proc_macros::rpc};
use serde::Deserialize;
// --- clover-network ---
use fc_tracer::types::block::TransactionTrace;
use fp_trace_rpc::RequestBlockId;

#[rpc(server)]
pub trait TraceApi {
	#[method(name = "trace_filter")]
	async fn filter(
		&self,
		filter: FilterRequest,
	) -> Result<Vec<TransactionTrace>>;
}

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRequest {
	/// (optional?) From this block.
	pub from_block: Option<RequestBlockId>,

	/// (optional?) To this block.
	pub to_block: Option<RequestBlockId>,

	/// (optional) Sent from these addresses.
	pub from_address: Option<Vec<H160>>,

	/// (optional) Sent to these addresses.
	pub to_address: Option<Vec<H160>>,

	/// (optional) The offset trace number
	pub after: Option<u32>,

	/// (optional) Integer number of traces to display in a batch.
	pub count: Option<u32>,
}


pub struct CloverEvmConfig;

impl CloverEvmConfig {
  /// clover configuration,
  /// identical to Istanbul hard fork except higher create_contract_limit
	pub const fn config() -> evm::Config {
		evm::Config {
			gas_ext_code: 700,
			gas_ext_code_hash: 700,
			gas_balance: 700,
			gas_sload: 800,
			gas_sload_cold: 0,
			gas_sstore_set: 20000,
			gas_sstore_reset: 5000,
			refund_sstore_clears: 15000,
			max_refund_quotient: 2,
			gas_suicide: 5000,
			gas_suicide_new_account: 25000,
			gas_call: 700,
			gas_expbyte: 50,
			gas_transaction_create: 53000,
			gas_transaction_call: 21000,
			gas_transaction_zero_data: 4,
			gas_transaction_non_zero_data: 16,
			gas_access_list_address: 0,
			gas_access_list_storage_key: 0,
			gas_account_access_cold: 0,
			gas_storage_read_warm: 0,
			sstore_gas_metering: true,
			sstore_revert_under_stipend: true,
			increase_state_access_gas: false,
			decrease_clears_refund: false,
			disallow_executable_format: false,
			err_on_call_with_more_gas: false,
			empty_considered_exists: false,
			create_increase_nonce: true,
			call_l64_after_gas: true,
			stack_limit: 1024,
			memory_limit: usize::max_value(),
			call_stack_limit: 1024,
			create_contract_limit: None, //we remove limits on contract limit creation
			call_stipend: 2300,
			has_delegate_call: true,
			has_create2: true,
			has_revert: true,
			has_return_data: true,
			has_bitwise_shifting: true,
			has_chain_id: true,
			has_self_balance: true,
			has_ext_code_hash: true,
			has_base_fee: false,
			estimate: false,
		}
	}
}

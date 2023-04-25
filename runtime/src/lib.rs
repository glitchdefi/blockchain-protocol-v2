#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use codec::Decode;
use sp_arithmetic::PerThing;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H160, H256, U256};
use sp_runtime::curve::PiecewiseLinear;
use sp_runtime::traits::{
  BlakeTwo256, Block as BlockT, Convert, ConvertInto, Dispatchable, PostDispatchInfoOf,
  SaturatedConversion, StaticLookup, UniqueSaturatedInto,
};
use sp_runtime::{
  create_runtime_str, generic, impl_opaque_keys,
  transaction_validity::{
    TransactionPriority, TransactionSource, TransactionValidity, TransactionValidityError,
  },
  ApplyExtrinsicResult, FixedPointNumber, OpaqueExtrinsic, Percent, Perquintill,
};
use frame_election_provider_support::{
  onchain, generate_solution_type, SequentialPhragmen, BalancingConfig,
};
use sp_std::{marker::PhantomData, prelude::*};

use sp_api::impl_runtime_apis;

use pallet_contracts::weights::WeightInfo;
use pallet_ethereum::{Call::transact, Transaction as EthereumTransaction};
pub use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
pub use pallet_transaction_payment::{
  Multiplier, TargetedFeeAdjustment, MultiplierUpdate,
};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

// A few exports that help ease life for downstream crates.
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

pub use pallet_staking::StakerStatus;

use codec::Encode;
use evm_accounts::EvmAddressMapping;
use fp_rpc::TransactionStatus;
pub use frame_support::{
  construct_runtime, debug, ensure, log, parameter_types,
	pallet_prelude::Get,
  traits::{
    ConstU128, ConstU16, ConstU32, Currency, EitherOfDiverse, EqualPrivilegeOnly, Everything,
    FindAuthor, Imbalance, KeyOwnerProofSystem, LockIdentifier, Nothing, OnUnbalanced, Randomness,
    U128CurrencyToVote,
  },
  transactional,
  weights::{
    constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
    ConstantMultiplier, DispatchClass, Weight,
  },
  ConsensusEngineId, PalletId, StorageValue,
};
use frame_system::{limits, EnsureRoot};
use pallet_grandpa::{fg_primitives, AuthorityId as GrandpaId};
pub use pallet_balances::Call as BalancesCall;
use pallet_evm::{Account as EVMAccount, EnsureAddressNever, EnsureAddressRoot, FeeCalculator, Runner};
pub use pallet_timestamp::Call as TimestampCall;
pub use sp_runtime::{Perbill, Permill, RuntimeAppPublic};
use pallet_session::historical as session_historical;

use runtime_parachains::{
	configuration as parachains_configuration, disputes as parachains_disputes,
	dmp as parachains_dmp, hrmp as parachains_hrmp, inclusion as parachains_inclusion,
	initializer as parachains_initializer, origin as parachains_origin, paras as parachains_paras,
	paras_inherent as parachains_paras_inherent, reward_points as parachains_reward_points,
	runtime_api_impl::v2 as parachains_runtime_api_impl, scheduler as parachains_scheduler,
	session_info as parachains_session_info, shared as parachains_shared, ump as parachains_ump,
};

use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;

pub use primitives::{
  currency::*, AccountId, AccountIndex, Amount, Balance, BlockNumber, CurrencyId, EraIndex, Hash,
  Index, Moment, Price, Rate, Share, Signature,
};

// Polkadot imports
use pallet_xcm::{EnsureXcm, IsMajorityOfBody};
use polkadot_runtime_common::{
  BlockHashCount, prod_or_fast, CurrencyToVote,
};
use xcm::latest::BodyId;

pub use constants::time::*;
use impls::{MergeAccountEvm, WeightToFee};

mod asset_location;
mod clover_evm_config;
mod constants;
mod impls;
mod mock;
mod parachains_common;
mod tests;
mod weights;

mod bag_thresholds;

mod precompiles;
use precompiles::CloverPrecompiles;

mod asset_trader;

use crate::asset_location::AssetLocation;

pub type AssetId = u64;
type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
  use super::*;

  pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

  /// Opaque block header type.
  pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
  /// Opaque block type.
  pub type Block = generic::Block<Header, UncheckedExtrinsic>;
  /// Opaque block identifier type.
  pub type BlockId = generic::BlockId<Block>;
}

impl_opaque_keys! {
  pub struct SessionKeys {
    pub grandpa: Grandpa,
    pub babe: Babe,
    pub im_online: ImOnline,
    pub authority_discovery: AuthorityDiscovery,
  }
}

pub const VERSION: RuntimeVersion = RuntimeVersion {
  spec_name: create_runtime_str!("glitch"),
  impl_name: create_runtime_str!("glitch"),
  authoring_version: 1,
  spec_version: 114,
  impl_version: 1,
  apis: RUNTIME_API_VERSIONS,
  transaction_version: 1,
  state_version: 0,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
  NativeVersion {
    runtime_version: VERSION,
    can_author_with: Default::default(),
  }
}

pub const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(25);

parameter_types! {
  pub BlockLength: limits::BlockLength =
    limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
  /// We allow for 2 seconds of compute with a 6 second average block time.
  pub BlockWeights: limits::BlockWeights = limits::BlockWeights::builder()
  .base_block(BlockExecutionWeight::get())
  .for_class(DispatchClass::all(), |weights| {
    weights.base_extrinsic = ExtrinsicBaseWeight::get();
  })
  .for_class(DispatchClass::Normal, |weights| {
    weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
  })
  .for_class(DispatchClass::Operational, |weights| {
    weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
    // Operational transactions have an extra reserved space, so that they
    // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
    weights.reserved = Some(
      MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
    );
  })
  .avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
  .build_or_panic();
  pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
  pub const Version: RuntimeVersion = VERSION;
  pub const SS58Prefix: u8 = 42;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
  /// The basic call filter to use in dispatchable.
  type BaseCallFilter = Everything;
  /// The identifier used to distinguish between accounts.
  type AccountId = AccountId;
  /// The aggregated dispatch type that is available for extrinsics.
  type Call = Call;
  /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
  type Lookup = Indices;
  /// The index type for storing how many extrinsics an account has signed.
  type Index = Index;
  /// The index type for blocks.
  type BlockNumber = BlockNumber;
  /// The type for hashing blocks and tries.
  type Hash = Hash;
  /// The hashing algorithm used.
  type Hashing = BlakeTwo256;
  /// The header type.
  type Header = generic::Header<BlockNumber, BlakeTwo256>;
  /// The ubiquitous event type.
  type Event = Event;
  /// The ubiquitous origin type.
  type Origin = Origin;
  /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
  type BlockHashCount = BlockHashCount;
  type BlockWeights = BlockWeights;
  type BlockLength = BlockLength;
  /// The weight of database operations that the runtime can invoke.
  type DbWeight = RocksDbWeight;
  /// Version of the runtime.
  type Version = Version;
  type PalletInfo = PalletInfo;
  /// What to do if a new account is created.
  type OnNewAccount = ();
  /// What to do if an account is fully reaped from the system.
  type OnKilledAccount = (
    // pallet_evm::CallKillAccount<Runtime>,
    evm_accounts::CallKillAccount<Runtime>,
  );
  /// The data to be stored in an account.
  type AccountData = pallet_balances::AccountData<Balance>;
  /// Weight information for the extrinsics of this pallet.
  type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
  type SS58Prefix = SS58Prefix;
  /// The set code logic of the parachain.
  type OnSetCode = ();
  type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
  pub const BasicDeposit: Balance = 10 * DOLLARS;       // 258 bytes on-chain
  pub const FieldDeposit: Balance = 250 * CENTS;        // 66 bytes on-chain
  pub const SubAccountDeposit: Balance = 2 * DOLLARS;   // 53 bytes on-chain
  pub const MaxSubAccounts: u32 = 100;
  pub const MaxAdditionalFields: u32 = 100;
  pub const MaxRegistrars: u32 = 20;
}

type EnsureRootOrHalfCouncil = EitherOfDiverse<
  EnsureRoot<AccountId>,
  pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
>;

impl pallet_identity::Config for Runtime {
  type Event = Event;
  type Currency = Balances;
  type BasicDeposit = BasicDeposit;
  type FieldDeposit = FieldDeposit;
  type SubAccountDeposit = SubAccountDeposit;
  type MaxSubAccounts = MaxSubAccounts;
  type MaxAdditionalFields = MaxAdditionalFields;
  type MaxRegistrars = MaxRegistrars;
  type Slashed = Treasury;
  type ForceOrigin = EnsureRootOrHalfCouncil;
  type RegistrarOrigin = EnsureRootOrHalfCouncil;
  type WeightInfo = pallet_identity::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
  pub const MinVestedTransfer: Balance = 100 * DOLLARS;
}

impl pallet_vesting::Config for Runtime {
  type Event = Event;
  type Currency = Balances;
  type BlockNumberToBalance = ConvertInto;
  type MinVestedTransfer = MinVestedTransfer;
  type WeightInfo = pallet_vesting::weights::SubstrateWeight<Runtime>;
  const MAX_VESTING_SCHEDULES: u32 = 28;
}

parameter_types! {
  pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
  /// A timestamp: milliseconds since the unix epoch.
  type Moment = u64;
  type OnTimestampSet = ();
  type MinimumPeriod = MinimumPeriod;
  type WeightInfo = pallet_timestamp::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
  pub const UncleGenerations: BlockNumber = 0;
}

impl pallet_authorship::Config for Runtime {
  type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
  type UncleGenerations = UncleGenerations;
  type FilterUncle = ();
  type EventHandler = (Staking, ImOnline);
}

/// clover account
impl evm_accounts::Config for Runtime {
  type Event = Event;
  type Currency = Balances;
  type KillAccount = frame_system::Consumer<Runtime>;
  type AddressMapping = EvmAddressMapping<Runtime>;
  type MergeAccount = MergeAccountEvm;
  type WeightInfo = weights::evm_accounts::WeightInfo<Runtime>;
}

static CLOVER_EVM_CONFIG: evm::Config = clover_evm_config::CloverEvmConfig::config();

parameter_types! {
  pub PrecompilesValue: CloverPrecompiles<Runtime> = CloverPrecompiles::<_>::new();
}

pub const GAS_PER_SECOND: u64 = 32_000_000;

/// Approximate ratio of the amount of Weight per Gas.
/// u64 works for approximations because Weight is a very small unit compared to gas.
pub const WEIGHT_PER_GAS: u64 = WEIGHT_PER_SECOND / GAS_PER_SECOND;

pub struct GlitchGasWeightMapping;

impl pallet_evm::GasWeightMapping for GlitchGasWeightMapping {
  fn gas_to_weight(gas: u64) -> Weight {
    Weight::try_from(gas.saturating_mul(WEIGHT_PER_GAS)).unwrap_or(Weight::MAX)
  }
  fn weight_to_gas(weight: Weight) -> u64 {
    u64::try_from(weight.wrapping_div(WEIGHT_PER_GAS)).unwrap_or(u64::MAX)
  }
}

parameter_types! {
    pub const GlitchTestnetChainId: u64 = 2160;
    pub BlockGasLimit: U256 = U256::from(u32::max_value());
}

pub struct FixedGasPrice;

impl FeeCalculator for FixedGasPrice {
    fn min_gas_price() -> U256 {
        let fixed_price: u64 = 10_000_000_000;
        fixed_price.into()
    }
}

impl pallet_evm::Config for Runtime {
  //type FeeCalculator = BaseFee;
  type FeeCalculator = FixedGasPrice;
  type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
  type GasWeightMapping = GlitchGasWeightMapping;
  type CallOrigin = EnsureAddressRoot<AccountId>;
  type WithdrawOrigin = EnsureAddressNever<AccountId>;
  type AddressMapping = EvmAddressMapping<Runtime>;
  type Currency = Balances;
  type Event = Event;
  type Runner = pallet_evm::runner::stack::Runner<Self>;
  type PrecompilesType = CloverPrecompiles<Self>;
  type PrecompilesValue = PrecompilesValue;
  type ChainId = GlitchTestnetChainId;
  type FindAuthor = EthereumFindAuthor<Babe>;
  type BlockGasLimit = BlockGasLimit;
  type OnChargeTransaction = pallet_evm::EVMCurrencyAdapter<Balances, FundBalance>;
  fn config() -> &'static evm::Config {
    &CLOVER_EVM_CONFIG
  }
}

pub struct EthereumFindAuthor<F>(PhantomData<F>);
impl<F: FindAuthor<u32>> FindAuthor<H160> for EthereumFindAuthor<F> {
  fn find_author<'a, I>(digests: I) -> Option<H160>
  where
    I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
  {
    if let Some(author_index) = F::find_author(digests) {
      let (authority_id, _) = Babe::authorities()[author_index as usize].clone();
      return Some(H160::from_slice(&authority_id.to_raw_vec()[4..24]));
    }
    None
  }
}

/// parachain doesn't have an authorship support currently
pub struct PhantomMockAuthorship;

impl FindAuthor<u32> for PhantomMockAuthorship {
  fn find_author<'a, I>(_digests: I) -> Option<u32>
  where
    I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
  {
    Some(0 as u32)
  }
}

impl pallet_ethereum::Config for Runtime {
  type Event = Event;
  // type FindAuthor = EthereumFindAuthor<PhantomMockAuthorship>;
  type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
  type RevenueSharing = Revenue;
}

pub struct TransactionConverter;

impl fp_rpc::ConvertTransaction<UncheckedExtrinsic> for TransactionConverter {
  fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> UncheckedExtrinsic {
    UncheckedExtrinsic::new_unsigned(
      pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
    )
  }
}

impl fp_rpc::ConvertTransaction<OpaqueExtrinsic> for TransactionConverter {
  fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> OpaqueExtrinsic {
    let extrinsic = UncheckedExtrinsic::new_unsigned(
      pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
    );
    let encoded = extrinsic.encode();
    OpaqueExtrinsic::decode(&mut &encoded[..]).expect("Encoded extrinsic is always valid")
  }
}

/// Struct that handles the conversion of Balance -> `u64`. This is used for
/// staking's election calculation.
pub struct CurrencyToVoteHandler;

impl Convert<u64, u64> for CurrencyToVoteHandler {
  fn convert(x: u64) -> u64 {
    x
  }
}
impl Convert<u128, u128> for CurrencyToVoteHandler {
  fn convert(x: u128) -> u128 {
    x
  }
}
impl Convert<u128, u64> for CurrencyToVoteHandler {
  fn convert(x: u128) -> u64 {
    x.saturated_into()
  }
}

impl Convert<u64, u128> for CurrencyToVoteHandler {
  fn convert(x: u64) -> u128 {
    x as u128
  }
}

pallet_staking_reward_curve::build! {
  const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
    min_inflation: 0_025_000,
    max_inflation: 0_100_000,
    // 3:2:1 staked : parachains : float.
    // while there's no parachains, then this is 75% staked : 25% float.
    ideal_stake: 0_750_000,
    falloff: 0_050_000,
    max_piece_count: 40,
    test_precision: 0_005_000,
  );
}

parameter_types! {
  pub const MaxElectingVoters: u32 = 22_500;
}

generate_solution_type!(
  #[compact]
  pub struct NposCompactSolution16::<
    VoterIndex = u32,
    TargetIndex = u16,
    Accuracy = sp_runtime::PerU16,
    MaxVoters = MaxElectingVoters,
  >(16)
);

parameter_types! {
  // Six sessions in an era (24 hours).
  pub const SessionsPerEra: sp_staking::SessionIndex = 6;
  // 28 eras for unbonding (28 days).
  pub const BondingDuration: sp_staking::EraIndex = 28;
  pub const SlashDeferDuration: sp_staking::EraIndex = 27;
  pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
  pub const ElectionLookahead: BlockNumber = EPOCH_DURATION_IN_BLOCKS / 4;
  pub const MaxNominatorRewardedPerValidator: u32 = 256;
  pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
  pub const MaxIterations: u32 = 10;
  // 0.05%. The higher the value, the more strict solution acceptance becomes.
  pub MinSolutionScoreBump: Perbill = PerThing::from_rational(5u32, 10_000);
  pub OffchainSolutionWeightLimit: Weight = BlockWeights::get()
    .get(DispatchClass::Normal)
    .max_extrinsic
    .expect("Normal extrinsics have weight limit configured by default; qed")
    .saturating_sub(BlockExecutionWeight::get());
  pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
  // 16
  pub const MaxNominations: u32 = <NposCompactSolution16 as frame_election_provider_support::NposSolution>::LIMIT as u32;
}

type SlashCancelOrigin = EitherOfDiverse<
  EnsureRoot<AccountId>,
  pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>,
>;

pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
  type System = Runtime;
  type Solver = SequentialPhragmen<AccountId, polkadot_runtime_common::elections::OnChainAccuracy>;
  type DataProvider = Staking;
  type WeightInfo = weights::frame_election_provider_support::WeightInfo<Runtime>;
}

impl onchain::BoundedConfig for OnChainSeqPhragmen {
	type VotersBound = MaxElectingVoters;
	type TargetsBound = ConstU32<2_000>;
}

parameter_types!{
  pub OffchainSolutionLengthLimit: u32 = Perbill::from_rational(90_u32, 100) *
    *BlockLength::get()
    .max
    .get(DispatchClass::Normal);
  pub EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS as u64;
  pub SignedPhase: u32 = (EPOCH_DURATION_IN_SLOTS / 4).saturated_into::<u32>();
  pub UnsignedPhase: u32 = (EPOCH_DURATION_IN_SLOTS / 4).saturated_into::<u32>();
  pub const SignedMaxSubmissions: u32 = 16;
  pub const SignedMaxRefunds: u32 = 16 / 4;
  pub SignedRewardBase: Balance = 1_000_000_000_000;
  pub const SignedDepositBase: Balance = deposit(2, 0);
  pub const SignedDepositByte: Balance = deposit(0, 10) / 1024;
  pub BetterUnsignedThreshold: Perbill = Perbill::from_rational(5u32, 10_000);
  pub OffchainRepeat: BlockNumber = UnsignedPhase::get() / 32;
  pub NposSolutionPriority: TransactionPriority = Perbill::from_percent(90) * TransactionPriority::max_value();
  pub const MaxElectableTargets: u16 = u16::MAX;
}

impl pallet_election_provider_multi_phase::MinerConfig for Runtime {
  type AccountId = AccountId;
  type MaxLength = OffchainSolutionLengthLimit;
  type MaxWeight = OffchainSolutionWeightLimit;
  type Solution = NposCompactSolution16;
  type MaxVotesPerVoter = <
    <Self as pallet_election_provider_multi_phase::Config>::DataProvider
    as
    frame_election_provider_support::ElectionDataProvider
  >::MaxVotesPerVoter;

  // The unsigned submissions have to respect the weight of the submit_unsigned call, thus their
  // weight estimate function is wired to this call's weight.
  fn solution_weight(v: u32, t: u32, a: u32, d: u32) -> Weight {
    <
      <Self as pallet_election_provider_multi_phase::Config>::WeightInfo
      as
      pallet_election_provider_multi_phase::WeightInfo
    >::submit_unsigned(v, t, a, d)
  }
}

/// Maximum number of iterations for balancing that will be executed in the embedded OCW
/// miner of election provider multi phase.
pub const MINER_MAX_ITERATIONS: u32 = 10;

/// A source of random balance for NposSolver, which is meant to be run by the OCW election miner.
pub struct OffchainRandomBalancing;
impl Get<Option<BalancingConfig>> for OffchainRandomBalancing {
	fn get() -> Option<BalancingConfig> {
		use sp_runtime::traits::TrailingZeroInput;
		let iterations = match MINER_MAX_ITERATIONS {
			0 => 0,
			max => {
				let seed = sp_io::offchain::random_seed();
				let random = <u32>::decode(&mut TrailingZeroInput::new(&seed))
					.expect("input is padded with zeroes; qed") %
					max.saturating_add(1);
				random as usize
			},
		};

		let config = BalancingConfig { iterations, tolerance: 0 };
		Some(config)
	}
}

impl pallet_election_provider_multi_phase::Config for Runtime {
  type Event = Event;
  type Currency = Balances;
  type EstimateCallFee = TransactionPayment;
  type SignedPhase = SignedPhase;
  type UnsignedPhase = UnsignedPhase;
  type SignedMaxSubmissions = SignedMaxSubmissions;
  type SignedMaxRefunds = SignedMaxRefunds;
  type SignedRewardBase = SignedRewardBase;
  type SignedDepositBase = SignedDepositBase;
  type SignedDepositByte = SignedDepositByte;
  type SignedDepositWeight = ();
  type SignedMaxWeight =
    <Self::MinerConfig as pallet_election_provider_multi_phase::MinerConfig>::MaxWeight;
  type MinerConfig = Self;
  type SlashHandler = (); // burn slashes
  type RewardHandler = (); // nothing to do upon rewards
  type BetterUnsignedThreshold = BetterUnsignedThreshold;
  type BetterSignedThreshold = ();
  type OffchainRepeat = OffchainRepeat;
  type MinerTxPriority = NposSolutionPriority;
  type DataProvider = Staking;
  type Fallback = onchain::BoundedExecution<OnChainSeqPhragmen>;
  type GovernanceFallback = onchain::UnboundedExecution<OnChainSeqPhragmen>;
  type Solver = SequentialPhragmen<
    AccountId,
    pallet_election_provider_multi_phase::SolutionAccuracyOf<Self>,
    OffchainRandomBalancing,
  >;
  type BenchmarkingConfig = polkadot_runtime_common::elections::BenchmarkConfig;
  type ForceOrigin = EnsureRootOrHalfCouncil;
  type WeightInfo = weights::pallet_election_provider_multi_phase::WeightInfo<Self>;
  type MaxElectingVoters = MaxElectingVoters;
  type MaxElectableTargets = MaxElectableTargets;
}

impl pallet_session::historical::Config for Runtime {
  type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
  type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

impl pallet_staking::Config for Runtime {
  type MaxNominations = MaxNominations;
  type Currency = Balances;
  type CurrencyBalance = Balance;
  type UnixTime = Timestamp;
  type CurrencyToVote = CurrencyToVote;
  type Event = Event;
  type Slash = Treasury;
  //TODO: Take rewards from reward fund.
  type Reward = Fund;
  type SessionsPerEra = SessionsPerEra;
  type BondingDuration = BondingDuration;
  type SlashDeferDuration = SlashDeferDuration;
  // A super-majority of the council can cancel the slash.
  type SlashCancelOrigin = SlashCancelOrigin;
  type SessionInterface = Self;
  type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
  type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
  type NextNewSession = Session;
  type ElectionProvider = ElectionProviderMultiPhase;
  type GenesisElectionProvider = onchain::UnboundedExecution<OnChainSeqPhragmen>;
  type VoterList = VoterList;
  type MaxUnlockingChunks = frame_support::traits::ConstU32<32>;
  type BenchmarkingConfig = polkadot_runtime_common::StakingBenchmarkingConfig;
  type OnStakerSlash = ();
  type WeightInfo = weights::pallet_staking::WeightInfo<Runtime>;
  //TODO:
  type RevenueFund = RevenueFund;
}

parameter_types! {
  pub const ExistentialDeposit: u128 = primitives::ExistentialDeposit;
  pub const MaxLocks: u32 = 50;
  pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
  /// The type for recording an account's balance.
  type Balance = Balance;
  /// The ubiquitous event type.
  type Event = Event;
  //TODO: Send dust to reward fund.
  type DustRemoval = FundBalance;
  type ExistentialDeposit = ExistentialDeposit;
  type AccountStore = System;
  type MaxLocks = MaxLocks;
  type MaxReserves = MaxReserves;
  type ReserveIdentifier = [u8; 8];
  type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
}

pub struct FundBalance;
impl OnUnbalanced<NegativeImbalance> for FundBalance {
    fn on_nonzero_unbalanced(amount: NegativeImbalance) {
        Balances::resolve_creating(&Fund::account_id(), amount);
    }
}

pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance> for DealWithFees {
    fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance>) {
        if let Some(fees) = fees_then_tips.next() {
            // Deposit all fees and tips to fund
            let mut amount = fees;
            if let Some(tips) = fees_then_tips.next() {
                amount = amount.merge(tips);
            }
            FundBalance::on_unbalanced(amount);
        }
    }
}

parameter_types! {
  pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_BLOCKS as _;
  pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
}

parameter_types! {
  pub MaximumSchedulerWeight: Weight = 10_000_000; //For some reason? Previously: Perbill::from_percent(10) * MAXIMUM_BLOCK_WEIGHT;
  pub const MaxScheduledPerBlock: u32 = 50;
  // Retry a scheduled item every 10 blocks (2 minute) until the preimage exists.
  pub const NoPreimagePostponement: Option<u32> = Some(10);
}

// democracy
impl pallet_scheduler::Config for Runtime {
  type Event = Event;
  type Origin = Origin;
  type Call = Call;
  type MaximumWeight = MaximumSchedulerWeight;
  type PalletsOrigin = OriginCaller;
  type ScheduleOrigin = EnsureRoot<AccountId>;
  type MaxScheduledPerBlock = MaxScheduledPerBlock;
  type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
  type OriginPrivilegeCmp = EqualPrivilegeOnly;
  type PreimageProvider = ();
  type NoPreimagePostponement = NoPreimagePostponement;
}

parameter_types! {
  pub const LaunchPeriod: BlockNumber = 28 * DAYS;
  pub const VotingPeriod: BlockNumber = 28 * DAYS;
  pub const FastTrackVotingPeriod: BlockNumber = 3 * HOURS;
  pub const MinimumDeposit: Balance = 100 * DOLLARS;
  pub const EnactmentPeriod: BlockNumber = 28 * DAYS;
  pub const CooloffPeriod: BlockNumber = 7 * DAYS;
  // One cent: $10,000 / MB
  pub const PreimageByteDeposit: Balance = 10 * MILLICENTS;
  pub const InstantAllowed: bool = true;
  pub const MaxVotes: u32 = 100;
  pub const MaxProposals: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
  type Proposal = Call;
  type Event = Event;
  type Currency = Balances;
  type EnactmentPeriod = EnactmentPeriod;
  type VoteLockingPeriod = EnactmentPeriod;
  type LaunchPeriod = LaunchPeriod;
  type VotingPeriod = VotingPeriod;
  type MinimumDeposit = MinimumDeposit;
  /// A straight majority of the council can decide what their next motion is.
  type ExternalOrigin =
    pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>;
  /// A super-majority can have the next scheduled referendum be a straight
  /// majority-carries vote.
  type ExternalMajorityOrigin =
    pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 4, 5>;
  /// A unanimous council can have the next scheduled referendum be a straight
  /// default-carries (NTB) vote.
  type ExternalDefaultOrigin =
    pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>;
  /// Full of the technical committee can have an
  /// ExternalMajority/ExternalDefault vote be tabled immediately and with a
  /// shorter voting/enactment period.
  type FastTrackOrigin =
    pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>;
  type InstantOrigin =
    pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>;
  type InstantAllowed = InstantAllowed;
  type FastTrackVotingPeriod = FastTrackVotingPeriod;
  /// To cancel a proposal which has been passed, all of the council must
  /// agree to it.
  type CancellationOrigin =
    pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>;
  type CancelProposalOrigin = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
  >;
  type OperationalPreimageOrigin = pallet_collective::EnsureMember<AccountId, CouncilCollective>;
  type BlacklistOrigin = EnsureRoot<AccountId>;
  /// Any single technical committee member may veto a coming council
  /// proposal, however they can only do it once and it lasts only for the
  /// cooloff period.
  type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
  type CooloffPeriod = CooloffPeriod;
  type PreimageByteDeposit = PreimageByteDeposit;
  type Slash = Treasury;
  type Scheduler = Scheduler;
  type MaxVotes = MaxVotes;
  type PalletsOrigin = OriginCaller;
  type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
  type MaxProposals = MaxProposals;
}

parameter_types! {
  pub const AssetDeposit: Balance = 10_000 * DOLLARS; // 10_000 DOLLARS deposit to create asset
  pub const AssetAccountDeposit: Balance = 0;
  pub const ApprovalDeposit: Balance = 0;
  pub const AssetsStringLimit: u32 = 50;
  /// Key = 32 bytes, Value = 36 bytes (32+1+1+1+1)
  // https://github.com/paritytech/substrate/blob/069917b/frame/assets/src/lib.rs#L257L271
  pub const MetadataDepositBase: Balance = deposit(1, 68);
  pub const MetadataDepositPerByte: Balance = deposit(0, 1);
  pub const ExecutiveBody: BodyId = BodyId::Executive;
}

impl pallet_utility::Config for Runtime {
  type Event = Event;
  type Call = Call;
  type PalletsOrigin = OriginCaller;
  type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
  // One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
  pub const DepositBase: Balance = deposit(1, 88);
  // Additional storage item size of 32 bytes.
  pub const DepositFactor: Balance = deposit(0, 32);
  pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
  type Event = Event;
  type Call = Call;
  type Currency = Balances;
  type DepositBase = DepositBase;
  type DepositFactor = DepositFactor;
  type MaxSignatories = MaxSignatories;
  type WeightInfo = (); //previously: pallet_multisig::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
  pub const CouncilMotionDuration: BlockNumber = 7 * DAYS;
  pub const CouncilMaxProposals: u32 = 100;
  pub const GeneralCouncilMaxMembers: u32 = 100;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
  type Origin = Origin;
  type Proposal = Call;
  type Event = Event;
  type MotionDuration = CouncilMotionDuration;
  type MaxProposals = CouncilMaxProposals;
  type MaxMembers = GeneralCouncilMaxMembers;
  type DefaultVote = pallet_collective::PrimeDefaultVote;
  type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
}

/// Converter for currencies to votes.
pub struct CurrencyToVoteHandler2<R>(sp_std::marker::PhantomData<R>);

impl<R> CurrencyToVoteHandler2<R>
where
  R: pallet_balances::Config,
  R::Balance: Into<u128>,
{
  fn factor() -> u128 {
    let issuance: u128 = <pallet_balances::Pallet<R>>::total_issuance().into();
    (issuance / u64::max_value() as u128).max(1)
  }
}

impl<R> Convert<u128, u64> for CurrencyToVoteHandler2<R>
where
  R: pallet_balances::Config,
  R::Balance: Into<u128>,
{
  fn convert(x: u128) -> u64 {
    (x / Self::factor()) as u64
  }
}

impl<R> Convert<u128, u128> for CurrencyToVoteHandler2<R>
where
  R: pallet_balances::Config,
  R::Balance: Into<u128>,
{
  fn convert(x: u128) -> u128 {
    x * Self::factor()
  }
}

pub const fn deposit(items: u32, bytes: u32) -> Balance {
  items as Balance * 15 * CENTS + (bytes as Balance) * 6 * CENTS
}

parameter_types! {
  pub const CandidacyBond: Balance = 100 * DOLLARS;
  // 1 storage item created, key size is 32 bytes, value size is 16+16.
  pub const VotingBondBase: Balance = deposit(1, 64);
  // additional data per vote is 32 bytes (account id).
  pub const VotingBondFactor: Balance = deposit(0, 32);
  /// Daily council elections.
  pub const TermDuration: BlockNumber = 7 * DAYS;
  pub const DesiredMembers: u32 = 13;
  pub const DesiredRunnersUp: u32 = 20;
  pub const ElectionsPhragmenModuleId: LockIdentifier = *b"phrelect";
}

impl pallet_elections_phragmen::Config for Runtime {
  type Event = Event;
  type Currency = Balances;
  type ChangeMembers = Council;
  type InitializeMembers = Council;
  type CurrencyToVote = U128CurrencyToVote;
  type CandidacyBond = CandidacyBond;
  type VotingBondBase = VotingBondBase;
  type VotingBondFactor = VotingBondFactor;
  type LoserCandidate = Treasury;
  type KickedMember = Treasury;
  type DesiredMembers = DesiredMembers;
  type DesiredRunnersUp = DesiredRunnersUp;
  type TermDuration = TermDuration;
  type PalletId = ElectionsPhragmenModuleId;
  type WeightInfo = pallet_elections_phragmen::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
  pub const TechnicalMotionDuration: BlockNumber = 7 * DAYS;
  pub const TechnicalMaxProposals: u32 = 100;
  pub const TechnicalMaxMembers:u32 = 100;
}

type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for Runtime {
  type Origin = Origin;
  type Proposal = Call;
  type Event = Event;
  type MotionDuration = TechnicalMotionDuration;
  type MaxProposals = TechnicalMaxProposals;
  type MaxMembers = TechnicalMaxMembers;
  type DefaultVote = pallet_collective::PrimeDefaultVote;
  type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
}

impl pallet_membership::Config<pallet_membership::Instance1> for Runtime {
  type Event = Event;
  type AddOrigin = frame_system::EnsureRoot<AccountId>;
  type RemoveOrigin = frame_system::EnsureRoot<AccountId>;
  type SwapOrigin = frame_system::EnsureRoot<AccountId>;
  type ResetOrigin = frame_system::EnsureRoot<AccountId>;
  type PrimeOrigin = frame_system::EnsureRoot<AccountId>;
  type MaxMembers = GeneralCouncilMaxMembers;
  type MembershipInitialized = TechnicalCommittee;
  type MembershipChanged = TechnicalCommittee;
  type WeightInfo = pallet_membership::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
  pub const ProposalBond: Permill = Permill::from_percent(5);
  pub const ProposalBondMinimum: Balance = 100 * DOLLARS;
  pub const SpendPeriod: BlockNumber = 24 * DAYS;
  pub const Burn: Permill = Permill::from_percent(1);
  pub const TreasuryModuleId: PalletId = PalletId(*b"py/trsry");

  pub const TipCountdown: BlockNumber = 1 * DAYS;
  pub const TipFindersFee: Percent = Percent::from_percent(20);
  pub const TipReportDepositBase: Balance = 1 * DOLLARS;
  pub const DataDepositPerByte: Balance = 10 * MILLICENTS;

  pub const MaximumReasonLength: u32 = 16384;
  pub const BountyDepositBase: Balance = 1 * DOLLARS;
  pub const BountyDepositPayoutDelay: BlockNumber = 8 * DAYS;
  pub const BountyUpdatePeriod: BlockNumber = 90 * DAYS;
  pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
  pub const BountyValueMinimum: Balance = 10 * DOLLARS;
  pub const CuratorDepositMin: Balance = 100 * DOLLARS;
  pub const CuratorDepositMax: Balance = 10000 * DOLLARS;
  pub const MaxApprovals: u32 = 100;
}

impl pallet_treasury::Config for Runtime {
  type Currency = Balances;
  type ApproveOrigin =
    pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 3, 5>;
  type RejectOrigin =
    pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>;
  type Event = Event;
  type OnSlash = Treasury;
  type ProposalBond = ProposalBond;
  type ProposalBondMinimum = ProposalBondMinimum;
  type ProposalBondMaximum = ();
  type SpendPeriod = SpendPeriod;
  type Burn = Burn;
  //TODO: Send burned to reward fund.
  type BurnDestination = FundBalance;
  type SpendFunds = Bounties;
  type PalletId = TreasuryModuleId;
  type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
  type MaxApprovals = MaxApprovals;
  type SpendOrigin = frame_support::traits::NeverEnsureOrigin<u128>;
}

impl pallet_bounties::Config for Runtime {
  type Event = Event;
  type BountyDepositBase = BountyDepositBase;
  type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
  type BountyUpdatePeriod = BountyUpdatePeriod;
  type CuratorDepositMultiplier = CuratorDepositMultiplier;
  type CuratorDepositMin = CuratorDepositMin;
  type CuratorDepositMax = CuratorDepositMax;
  type BountyValueMinimum = BountyValueMinimum;
  type DataDepositPerByte = DataDepositPerByte;
  type MaximumReasonLength = MaximumReasonLength;
  type WeightInfo = pallet_bounties::weights::SubstrateWeight<Runtime>;
  type ChildBountyManager = ();
}

impl pallet_tips::Config for Runtime {
  type Event = Event;
  type DataDepositPerByte = DataDepositPerByte;
  type MaximumReasonLength = MaximumReasonLength;
  type Tippers = ElectionsPhragmen;
  type TipCountdown = TipCountdown;
  type TipFindersFee = TipFindersFee;
  type TipReportDepositBase = TipReportDepositBase;
  type WeightInfo = pallet_tips::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
  pub const TransactionByteFee: Balance = 10 * MILLICENTS;
  pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
  pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
  pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
  pub const OperationalFeeMultiplier: u8 = 5;
}

pub struct ConstantFeeUpdate;
impl MultiplierUpdate for ConstantFeeUpdate {
    fn min() -> Multiplier {
        1.into()
    }
    fn target() -> Perquintill {
        Perquintill::one()
    }
    fn variability() -> Multiplier {
        1.into()
    }
}

impl Convert<Multiplier, Multiplier> for ConstantFeeUpdate{
    fn convert(previous: Multiplier) -> Multiplier {
        previous
    }
}

impl pallet_transaction_payment::Config for Runtime {
  type Event = Event;
  type OnChargeTransaction = pallet_transaction_payment::CurrencyAdapter<Balances, DealWithFees>;
  type OperationalFeeMultiplier = OperationalFeeMultiplier;
  type WeightToFee = WeightToFee<Balance>;
  type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
  type FeeMultiplierUpdate = ConstantFeeUpdate;
}

impl pallet_sudo::Config for Runtime {
  type Event = Event;
  type Call = Call;
}

parameter_types! {
  pub const IndexDeposit: Balance = 10 * DOLLARS;
}

impl pallet_indices::Config for Runtime {
  type AccountIndex = AccountIndex;
  type Event = Event;
  type Currency = Balances;
  type Deposit = IndexDeposit;
  type WeightInfo = pallet_indices::weights::SubstrateWeight<Runtime>;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
  Call: From<LocalCall>,
{
  fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
    call: Call,
    public: <Signature as sp_runtime::traits::Verify>::Signer,
    account: AccountId,
    nonce: Index,
  ) -> Option<(
    Call,
    <UncheckedExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
  )> {
    // take the biggest period possible.
    let period = BlockHashCount::get()
      .checked_next_power_of_two()
      .map(|c| c / 2)
      .unwrap_or(2) as u64;
    let current_block = System::block_number()
      .saturated_into::<u64>()
      // The `System::block_number` is initialized with `n+1`,
      // so the actual block number is `n`.
      .saturating_sub(1);
    let tip = 0;
    let extra: SignedExtra = (
      frame_system::CheckSpecVersion::<Runtime>::new(),
      frame_system::CheckTxVersion::<Runtime>::new(),
      frame_system::CheckGenesis::<Runtime>::new(),
      frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
      frame_system::CheckNonce::<Runtime>::from(nonce),
      frame_system::CheckWeight::<Runtime>::new(),
      pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
    );
    let raw_payload = SignedPayload::new(call, extra)
      .map_err(|e| {
        log::warn!("Unable to create signed payload: {:?}", e);
      })
      .ok()?;
    let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
    let address = Indices::unlookup(account);
    let (call, extra, _) = raw_payload.deconstruct();
    Some((call, (address, signature, extra)))
  }
}

impl frame_system::offchain::SigningTypes for Runtime {
  type Public = <Signature as sp_runtime::traits::Verify>::Signer;
  type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
  Call: From<C>,
{
  type OverarchingCall = Call;
  type Extrinsic = UncheckedExtrinsic;
}

parameter_types! {
  pub const DepositPerItem: Balance = deposit(1, 0);
  pub const DepositPerByte: Balance = deposit(0, 1);
  pub const TombstoneDeposit: Balance = 16 * MILLICENTS;
  pub const SurchargeReward: Balance = 150 * MILLICENTS;
  pub const SignedClaimHandicap: u32 = 2;
  pub const MaxDepth: u32 = 32;
  pub const MaxValueSize: u32 = 16 * 1024;
  pub const RentByteFee: Balance = 4 * MILLICENTS;
  pub const RentDepositOffset: Balance = 1000 * MILLICENTS;
  pub const DepositPerContract: Balance = TombstoneDeposit::get();
  pub const DepositPerStorageByte: Balance = deposit(0, 1);
  pub const DepositPerStorageItem: Balance = deposit(1, 0);
  pub RentFraction: Perbill = PerThing::from_rational(1u32, 30 * DAYS);
  // The lazy deletion runs inside on_initialize.
  pub DeletionWeightLimit: Weight = AVERAGE_ON_INITIALIZE_RATIO *
    BlockWeights::get().max_block;
  // The weight needed for decoding the queue should be less or equal than a fifth
  // of the overall weight dedicated to the lazy deletion.
  pub DeletionQueueDepth: u32 = ((DeletionWeightLimit::get() / (
      <Runtime as pallet_contracts::Config>::WeightInfo::on_initialize_per_queue_item(1) -
      <Runtime as pallet_contracts::Config>::WeightInfo::on_initialize_per_queue_item(0)
    )) / 5) as u32;

  pub const MaxCodeSize: u32 = 2 * 1024 * 1024;
  pub Schedule: pallet_contracts::Schedule<Runtime> = Default::default();
}

impl pallet_contracts::Config for Runtime {
  type Time = Timestamp;
  type Randomness = RandomnessCollectiveFlip;
  type Currency = Balances;
  type Event = Event;
  type Call = Call;
  type CallFilter = Nothing;
  type DepositPerItem = DepositPerItem;
  type DepositPerByte = DepositPerByte;
  type CallStack = [pallet_contracts::Frame<Self>; 31];
  type WeightPrice = pallet_transaction_payment::Pallet<Self>;
  type WeightInfo = pallet_contracts::weights::SubstrateWeight<Self>;
  type ChainExtension = ();
  type DeletionQueueDepth = DeletionQueueDepth;
  type DeletionWeightLimit = DeletionWeightLimit;
  type Schedule = Schedule;
  type AddressGenerator = pallet_contracts::DefaultAddressGenerator;
  type ContractAccessWeight = pallet_contracts::DefaultContractAccessWeight<BlockWeights>;
  type MaxCodeLen = ConstU32<{ 128 * 1024 }>;
  type RelaxedMaxCodeLen = ConstU32<{ 256 * 1024 }>;
  type MaxStorageKeyLen = ConstU32<128>;
}

parameter_types! {
  pub Prefix: &'static [u8] = b"Pay CLVs to the Clover account:";
  pub const ClaimsModuleId: PalletId = PalletId(*b"clvclaim");
}

parameter_types! {
  pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
  pub const Period: u32 = 6 * HOURS;
  pub const Offset: u32 = 0;
  pub const MaxAuthorities: u32 = 100_000;
}

impl pallet_session::Config for Runtime {
  type Event = Event;
  type ValidatorId = <Self as frame_system::Config>::AccountId;
  // we don't have stash and controller, thus we don't need the convert as well.
  type ValidatorIdOf = pallet_staking::StashOf<Self>;
  type ShouldEndSession = Babe;
  type NextSessionRotation = Babe;
  type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
  // Essentially just Aura, but lets be pedantic.
  type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
  type Keys = SessionKeys;
  //type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
  type WeightInfo = ();
}

pub mod currency {
  use super::Balance;

  pub const SUPPLY_FACTOR: Balance = 100;

  pub const WEI: Balance = 1;
  pub const KILOWEI: Balance = 1_000;
  pub const MEGAWEI: Balance = 1_000_000;
  pub const GIGAWEI: Balance = 1_000_000_000;
}

parameter_types! {
  // Tells `pallet_base_fee` whether to calculate a new BaseFee `on_finalize` or not.
  pub IsActive: bool = false;
  pub DefaultBaseFeePerGas: U256 = (1 * currency::GIGAWEI * currency::SUPPLY_FACTOR).into();
}

pub struct BaseFeeThreshold;
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
  fn lower() -> Permill {
    Permill::zero()
  }
  fn ideal() -> Permill {
    Permill::from_parts(500_000)
  }
  fn upper() -> Permill {
    Permill::from_parts(1_000_000)
  }
}

//impl pallet_base_fee::Config for Runtime {
//  type Event = Event;
//  type Threshold = BaseFeeThreshold;
//  type IsActive = IsActive;
//}

impl pallet_randomness_collective_flip::Config for Runtime {}

parameter_types! {
  pub const PotId: PalletId = PalletId(*b"PotStake");
  pub const MaxCandidates: u32 = 1000;
  pub const MinCandidates: u32 = 2;
  pub const MaxInvulnerables: u32 = 100;
}

//
pub type CollatorSelectionUpdateOrigin = EnsureRootOrHalfCouncil;

impl pallet_collator_selection::Config for Runtime {
  type Event = Event;
  type Currency = Balances;
  type UpdateOrigin = CollatorSelectionUpdateOrigin;
  type PotId = PotId;
  type MaxCandidates = MaxCandidates;
  type MinCandidates = MinCandidates;
  type MaxInvulnerables = MaxInvulnerables;
  type ValidatorId = <Self as frame_system::Config>::AccountId;
  type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
  type ValidatorRegistration = Session;
  type KickThreshold = Period;
  type WeightInfo = ();
}

parameter_types! {
  pub const BagThresholds: &'static [u64] = &bag_thresholds::THRESHOLDS;
}

impl pallet_bags_list::Config for Runtime {
  type Event = Event;
  type ScoreProvider = Staking;
  type WeightInfo = weights::pallet_bags_list::WeightInfo<Runtime>;
  type BagThresholds = BagThresholds;
  type Score = sp_npos_elections::VoteWeight;
}

impl asset_config::Config for Runtime {
  type Event = Event;
  type AssetId = AssetId;
  type AssetLocation = AssetLocation;
}

parameter_types! {
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
	pub ReportLongevity: u64 =
		BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
}

impl pallet_babe::Config for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;

	// session module is the trigger
	type EpochChangeTrigger = pallet_babe::ExternalTrigger;

	type DisabledValidators = Session;

	type KeyOwnerProofSystem = Historical;

	type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		pallet_babe::AuthorityId,
	)>>::Proof;

	type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		pallet_babe::AuthorityId,
	)>>::IdentificationTuple;

	type HandleEquivocation =
		pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, Offences, ReportLongevity>;

	type WeightInfo = ();

	type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
	pub const MaxKeys: u32 = 10_000;
	pub const MaxPeerInHeartbeats: u32 = 10_000;
	pub const MaxPeerDataEncodingSize: u32 = 1_000;
}

impl pallet_im_online::Config for Runtime {
  type AuthorityId = ImOnlineId;
  type Event = Event;
  type ValidatorSet = Historical;
  type NextSessionRotation = Babe;
  type ReportUnresponsiveness = Offences;
  type UnsignedPriority = ImOnlineUnsignedPriority;
  type WeightInfo = weights::pallet_im_online::WeightInfo<Runtime>;
  type MaxKeys = MaxKeys;
  type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
  type MaxPeerDataEncodingSize = MaxPeerDataEncodingSize;
}

impl pallet_grandpa::Config for Runtime {
  type Event = Event;
  type Call = Call;

  type KeyOwnerProof =
    <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

  type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
    KeyTypeId,
    GrandpaId,
  )>>::IdentificationTuple;

  type KeyOwnerProofSystem = Historical;

  type HandleEquivocation = pallet_grandpa::EquivocationHandler<
    Self::KeyOwnerIdentification,
    Offences,
    ReportLongevity,
  >;

  type WeightInfo = ();
  type MaxAuthorities = MaxAuthorities;
}

impl pallet_authority_discovery::Config for Runtime {
	type MaxAuthorities = MaxAuthorities;
}

impl pallet_offences::Config for Runtime {
	type Event = Event;
	type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
}

// TODO:
parameter_types! {
    pub const RevenueModuleId: PalletId = PalletId(*b"py/rvnsr");
}

/// Configure the pallet-template in pallets/template.
impl pallet_revenue::Config for Runtime {
    type Event = Event;
}

//Config pallet-fund

impl pallet_fund::Config for Runtime {
    type Currency = Balances;
    type Event = Event;
}

// Config wallet revenue

impl pallet_revenue_fund::Config for Runtime {
    type Currency = Balances;
    type Event = Event;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
  pub enum Runtime where
    Block = Block,
    NodeBlock = opaque::Block,
    UncheckedExtrinsic = UncheckedExtrinsic
  {
    //Core
    System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
    RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Storage},
    Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
    Sudo: pallet_sudo::{Pallet, Call, Config<T>, Storage, Event<T>},

		//Auth
    ImOnline: pallet_im_online::{Pallet, Call, Storage, Event<T>, ValidateUnsigned, Config<T>},
    AuthorityDiscovery: pallet_authority_discovery::{Pallet, Config},
    Offences: pallet_offences::{Pallet, Storage, Event},

		//Account lookup
    Indices: pallet_indices::{Pallet, Call, Storage, Config<T>, Event<T>},

		//Native token
    Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
    TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>},

		//Consensus the order of these 4 is important and shall not change.
    Authorship: pallet_authorship::{Pallet, Call, Storage},
    CollatorSelection: pallet_collator_selection::{Pallet, Call, Storage, Event<T>, Config<T>},
    //Babe must be before session.
    Babe: pallet_babe::{Pallet, Call, Storage, Config, ValidateUnsigned},
    Grandpa: pallet_grandpa::{Pallet, Call, Storage, Config, Event, ValidateUnsigned},
    Staking: pallet_staking::{Pallet, Call, Storage, Config<T>, Event<T>},
    ElectionProviderMultiPhase: pallet_election_provider_multi_phase::{Pallet, Call, Storage, Event<T>, ValidateUnsigned},
    VoterList: pallet_bags_list::{Pallet, Call, Storage, Event<T>},

    Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>},
    Historical: session_historical::{Pallet},

		//Smart contracts
    Contracts: pallet_contracts::{Pallet, Call, Storage, Event<T>},
    Ethereum: pallet_ethereum::{Pallet, Call, Storage, Event, Origin, Config, },
    EVM: pallet_evm::{Pallet, Config, Call, Storage, Event<T>},
    //EVMChainId
    //DynamicFee
    //BaseFee: pallet_base_fee::{Pallet, Call, Storage, Config<T>, Event},

		//Governance
    Council: pallet_collective::<Instance1>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>},
    TechnicalCommittee: pallet_collective::<Instance2>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>},
    Treasury: pallet_treasury::{Pallet, Call, Storage, Event<T>, Config},
    Bounties: pallet_bounties::{Pallet, Call, Storage, Event<T>},
    Democracy: pallet_democracy::{Pallet, Call, Storage, Config<T>, Event<T>},
    ElectionsPhragmen: pallet_elections_phragmen::{Pallet, Call, Storage, Event<T>, Config<T>},
    TechnicalMembership: pallet_membership::<Instance1>::{Pallet, Call, Storage, Event<T>, Config<T>},
    Tips: pallet_tips::{Pallet, Call, Storage, Event<T>},

		//Account module
    EvmAccounts: evm_accounts::{Pallet, Call, Storage, Event<T>},

		//Utility
    Utility: pallet_utility::{Pallet, Call, Event},
    Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>},
    Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},

    Identity: pallet_identity::{Pallet, Call, Storage, Event<T>},
    Vesting: pallet_vesting::{Pallet, Call, Storage, Event<T>, Config<T>},

    AssetConfig: asset_config::{Pallet, Call, Storage, Event<T>},
    
    Revenue: pallet_revenue::{Pallet, Call, Storage, Config<T>, Event<T>} ,
    Fund: pallet_fund::{Pallet, Call, Storage, Event<T>, Config},
    RevenueFund: pallet_revenue_fund::{Pallet, Call, Storage, Event<T>, Config},
  }
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, AccountIndex>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
  frame_system::CheckSpecVersion<Runtime>,
  frame_system::CheckTxVersion<Runtime>,
  frame_system::CheckGenesis<Runtime>,
  frame_system::CheckEra<Runtime>,
  frame_system::CheckNonce<Runtime>,
  frame_system::CheckWeight<Runtime>,
  pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
  fp_self_contained::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = fp_self_contained::CheckedExtrinsic<AccountId, Call, SignedExtra, H160>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
  Runtime,
  Block,
  frame_system::ChainContext<Runtime>,
  Runtime,
  AllPalletsWithSystem,
>;

pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;

impl fp_self_contained::SelfContainedCall for Call {
  type SignedInfo = H160;

  fn is_self_contained(&self) -> bool {
    match self {
      Call::Ethereum(call) => call.is_self_contained(),
      _ => false,
    }
  }

  fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
    match self {
      Call::Ethereum(call) => call.check_self_contained(),
      _ => None,
    }
  }

  fn validate_self_contained(&self, info: &Self::SignedInfo) -> Option<TransactionValidity> {
    match self {
      Call::Ethereum(call) => call.validate_self_contained(info),
      _ => None,
    }
  }

  fn pre_dispatch_self_contained(
    &self,
    info: &Self::SignedInfo,
  ) -> Option<Result<(), TransactionValidityError>> {
    match self {
      Call::Ethereum(call) => call.pre_dispatch_self_contained(info),
      _ => None,
    }
  }

  fn apply_self_contained(
    self,
    info: Self::SignedInfo,
  ) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
    match self {
      call @ Call::Ethereum(pallet_ethereum::Call::transact { .. }) => Some(call.dispatch(
        Origin::from(pallet_ethereum::RawOrigin::EthereumTransaction(info)),
      )),
      _ => None,
    }
  }
}

/// The BABE epoch configuration at genesis.
pub const BABE_GENESIS_EPOCH_CONFIG: babe_primitives::BabeEpochConfiguration =
  babe_primitives::BabeEpochConfiguration {
    c: PRIMARY_PROBABILITY,
    allowed_slots: babe_primitives::AllowedSlots::PrimaryAndSecondaryPlainSlots,
  };

impl_runtime_apis! {
  impl sp_api::Core<Block> for Runtime {
    fn version() -> RuntimeVersion {
      VERSION
    }

    fn execute_block(block: Block) {
      Executive::execute_block(block)
    }

    fn initialize_block(header: &<Block as BlockT>::Header) {
      Executive::initialize_block(header)
    }
  }

  impl sp_api::Metadata<Block> for Runtime {
    fn metadata() -> OpaqueMetadata {
      OpaqueMetadata::new(Runtime::metadata().into())
    }
  }

  impl sp_block_builder::BlockBuilder<Block> for Runtime {
    fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
      Executive::apply_extrinsic(extrinsic)
    }

    fn finalize_block() -> <Block as BlockT>::Header {
      Executive::finalize_block()
    }

    fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
      data.create_extrinsics()
    }

    fn check_inherents(
      block: Block,
      data: sp_inherents::InherentData,
    ) -> sp_inherents::CheckInherentsResult {
      data.check_extrinsics(&block)
    }
  }

  impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
    fn validate_transaction(
      source: TransactionSource,
      tx: <Block as BlockT>::Extrinsic,
      block_hash: <Block as BlockT>::Hash,
    ) -> TransactionValidity {
      Executive::validate_transaction(source, tx, block_hash)
    }
  }

  impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
    fn offchain_worker(header: &<Block as BlockT>::Header) {
      Executive::offchain_worker(header)
    }
  }

  impl fg_primitives::GrandpaApi<Block> for Runtime {
    fn grandpa_authorities() -> Vec<(GrandpaId, u64)> {
      Grandpa::grandpa_authorities()
    }

    fn current_set_id() -> fg_primitives::SetId {
      Grandpa::current_set_id()
    }

    fn submit_report_equivocation_unsigned_extrinsic(
      equivocation_proof: fg_primitives::EquivocationProof<
        <Block as BlockT>::Hash,
        sp_runtime::traits::NumberFor<Block>,
      >,
      key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
    ) -> Option<()> {
      let key_owner_proof = key_owner_proof.decode()?;

      Grandpa::submit_unsigned_equivocation_report(
        equivocation_proof,
        key_owner_proof,
      )
    }

    fn generate_key_ownership_proof(
      _set_id: fg_primitives::SetId,
      authority_id: fg_primitives::AuthorityId,
    ) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
      use codec::Encode;

      Historical::prove((fg_primitives::KEY_TYPE, authority_id))
        .map(|p| p.encode())
        .map(fg_primitives::OpaqueKeyOwnershipProof::new)
    }
  }

  impl babe_primitives::BabeApi<Block> for Runtime {
    fn configuration() -> babe_primitives::BabeGenesisConfiguration {
      // The choice of `c` parameter (where `1 - c` represents the
      // probability of a slot being empty), is done in accordance to the
      // slot duration and expected target block time, for safely
      // resisting network delays of maximum two seconds.
      // <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
      babe_primitives::BabeGenesisConfiguration {
        slot_duration: Babe::slot_duration(),
        epoch_length: EpochDuration::get(),
        c: BABE_GENESIS_EPOCH_CONFIG.c,
        genesis_authorities: Babe::authorities().to_vec(),
        randomness: Babe::randomness(),
        allowed_slots: BABE_GENESIS_EPOCH_CONFIG.allowed_slots,
      }
    }

    fn current_epoch_start() -> babe_primitives::Slot {
      Babe::current_epoch_start()
    }

    fn current_epoch() -> babe_primitives::Epoch {
      Babe::current_epoch()
    }

    fn next_epoch() -> babe_primitives::Epoch {
      Babe::next_epoch()
    }

    fn generate_key_ownership_proof(
      _slot: babe_primitives::Slot,
      authority_id: babe_primitives::AuthorityId,
    ) -> Option<babe_primitives::OpaqueKeyOwnershipProof> {
      use codec::Encode;

      Historical::prove((babe_primitives::KEY_TYPE, authority_id))
        .map(|p| p.encode())
        .map(babe_primitives::OpaqueKeyOwnershipProof::new)
    }

    fn submit_report_equivocation_unsigned_extrinsic(
      equivocation_proof: babe_primitives::EquivocationProof<<Block as BlockT>::Header>,
      key_owner_proof: babe_primitives::OpaqueKeyOwnershipProof,
    ) -> Option<()> {
      let key_owner_proof = key_owner_proof.decode()?;

      Babe::submit_unsigned_equivocation_report(
        equivocation_proof,
        key_owner_proof,
      )
    }
  }

	impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityDiscoveryId> {
			AuthorityDiscovery::authorities()
		}
	}

  impl sp_session::SessionKeys<Block> for Runtime {
    fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
      SessionKeys::generate(seed)
    }

    fn decode_session_keys(
      encoded: Vec<u8>,
    ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
      SessionKeys::decode_into_raw_public_keys(&encoded)
    }
  }

  impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
    fn account_nonce(account: AccountId) -> Index {
      System::account_nonce(account)
    }
  }

  impl pallet_contracts_rpc_runtime_api::ContractsApi<Block, AccountId, Balance, BlockNumber, Hash>
    for Runtime
  {
    fn call(
      origin: AccountId,
      dest: AccountId,
      value: Balance,
      gas_limit: u64,
      storage_deposit_limit: Option<Balance>,
      input_data: Vec<u8>,
    ) -> pallet_contracts_primitives::ContractExecResult<Balance> {
        Contracts::bare_call(origin, dest.into(), value, gas_limit, storage_deposit_limit, input_data, true)
    }

    fn instantiate(
      origin: AccountId,
      endowment: Balance,
      gas_limit: u64,
      storage_deposit_limit: Option<Balance>,
      code: pallet_contracts_primitives::Code<Hash>,
      data: Vec<u8>,
      salt: Vec<u8>,
    ) -> pallet_contracts_primitives::ContractInstantiateResult<AccountId, Balance>
    {
      Contracts::bare_instantiate(origin, endowment, gas_limit, storage_deposit_limit, code, data, salt, true)
    }

    fn upload_code(
      origin: AccountId,
      code: Vec<u8>,
      storage_deposit_limit: Option<Balance>,
    ) -> pallet_contracts_primitives::CodeUploadResult<Hash, Balance>
    {
      Contracts::bare_upload_code(origin, code, storage_deposit_limit)
    }

    fn get_storage(
      address: AccountId,
      key: Vec<u8>,
    ) -> pallet_contracts_primitives::GetStorageResult {
      Contracts::get_storage(address, key)
    }

//    fn rent_projection(
//      address: AccountId,
//    ) -> pallet_contracts_primitives::RentProjectionResult<BlockNumber> {
//      Contracts::rent_projection(address)
//    }
  }

  impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
    fn convert_transaction(transaction: EthereumTransaction) -> <Block as BlockT>::Extrinsic {
      UncheckedExtrinsic::new_unsigned(
        pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
      )
    }
  }

  impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
    fn query_info(
      uxt: <Block as BlockT>::Extrinsic,
      len: u32,
    ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
      TransactionPayment::query_info(uxt, len)
    }

    fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
      TransactionPayment::query_fee_details(uxt, len)
    }
  }

  impl fp_rpc::EthereumRuntimeRPCApi<Block> for Runtime {
    fn chain_id() -> u64 {
        <Runtime as pallet_evm::Config>::ChainId::get()
    }

    fn account_basic(address: H160) -> EVMAccount {
        EVM::account_basic(&address)
    }

    fn gas_price() -> U256 {
        <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price()
    }

    fn account_code_at(address: H160) -> Vec<u8> {
        EVM::account_codes(address)
    }

    fn author() -> H160 {
        <pallet_evm::Pallet<Runtime>>::find_author()
    }

    fn storage_at(address: H160, index: U256) -> H256 {
        let mut tmp = [0u8; 32];
        index.to_big_endian(&mut tmp);
        EVM::account_storages(address, H256::from_slice(&tmp[..]))
    }

    fn call(
        from: H160,
        to: H160,
        data: Vec<u8>,
        value: U256,
        gas_limit: U256,
        max_fee_per_gas: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        nonce: Option<U256>,
        estimate: bool,
        access_list: Option<Vec<(H160, Vec<H256>)>>,
    ) -> Result<pallet_evm::CallInfo, sp_runtime::DispatchError> {
        let config = if estimate {
            let mut config = <Runtime as pallet_evm::Config>::config().clone();
            config.estimate = true;
            Some(config)
        } else {
            None
        };

        <Runtime as pallet_evm::Config>::Runner::call(
            from,
            to,
            data,
            value,
            gas_limit.unique_saturated_into(),
            max_fee_per_gas,
            max_priority_fee_per_gas,
            nonce,
            access_list.unwrap_or_default(),
            config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config()),
        ).map_err(|err| err.into())
    }

    fn create(
        from: H160,
        data: Vec<u8>,
        value: U256,
        gas_limit: U256,
        max_fee_per_gas: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        nonce: Option<U256>,
        estimate: bool,
        access_list: Option<Vec<(H160, Vec<H256>)>>,
    ) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
        let config = if estimate {
            let mut config = <Runtime as pallet_evm::Config>::config().clone();
            config.estimate = true;
            Some(config)
        } else {
            None
        };

        <Runtime as pallet_evm::Config>::Runner::create(
            from,
            data,
            value,
            gas_limit.unique_saturated_into(),
            max_fee_per_gas,
            max_priority_fee_per_gas,
            nonce,
            access_list.unwrap_or_default(),
            config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config()),
        ).map_err(|err| err.into())
    }

    fn current_transaction_statuses() -> Option<Vec<TransactionStatus>> {
        Ethereum::current_transaction_statuses()
    }

    fn current_block() -> Option<pallet_ethereum::Block> {
        Ethereum::current_block()
    }

    fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> {
        Ethereum::current_receipts()
    }

    fn current_all() -> (
        Option<pallet_ethereum::Block>,
        Option<Vec<pallet_ethereum::Receipt>>,
        Option<Vec<TransactionStatus>>
    ) {
        (
            Ethereum::current_block(),
            Ethereum::current_receipts(),
            Ethereum::current_transaction_statuses()
        )
    }

    fn extrinsic_filter(
      xts: Vec<<Block as BlockT>::Extrinsic>,
    ) -> Vec<EthereumTransaction> {
      xts.into_iter().filter_map(|xt| match xt.0.function {
        Call::Ethereum(transact{transaction}) => Some(transaction),
        _ => None
      }).collect::<Vec<EthereumTransaction>>()
    }

    fn elasticity() -> Option<Permill> {
      //Some(BaseFee::elasticity())
      None
    }
  }

  impl fp_trace_apis::DebugRuntimeApi<Block> for Runtime {
    fn trace_transaction(
      _extrinsics: Vec<<Block as BlockT>::Extrinsic>,
      _traced_transaction: &pallet_ethereum::Transaction,
    ) -> Result<
      (),
      sp_runtime::DispatchError,
    > {
      use fp_tracer::tracer::EvmTracer;
      use pallet_ethereum::Call::transact;
      // Apply the a subset of extrinsics: all the substrate-specific or ethereum
      // transactions that preceded the requested transaction.
      for ext in _extrinsics.into_iter() {
        let _ = match &ext.0.function {
          Call::Ethereum(transact { transaction }) => {
            if transaction == _traced_transaction {
              EvmTracer::new().trace(|| Executive::apply_extrinsic(ext));
              return Ok(());
            } else {
              Executive::apply_extrinsic(ext)
            }
          }
          _ => Executive::apply_extrinsic(ext),
        };
      }

      Err(sp_runtime::DispatchError::Other(
        "Failed to find Ethereum transaction among the extrinsics.",
      ))
    }

    fn trace_block(
      _extrinsics: Vec<<Block as BlockT>::Extrinsic>,
      _known_transactions: Vec<H256>,
    ) -> Result<
      (),
      sp_runtime::DispatchError,
    > {
      use fp_tracer::tracer::EvmTracer;
      use sha3::{Digest, Keccak256};
      use pallet_ethereum::Call::transact;

      let mut config = <Runtime as pallet_evm::Config>::config().clone();
      config.estimate = true;

      // Apply all extrinsics. Ethereum extrinsics are traced.
      for ext in _extrinsics.into_iter() {
        match &ext.0.function {
          Call::Ethereum(transact { transaction }) => {
            let eth_extrinsic_hash =
              H256::from_slice(Keccak256::digest(&rlp::encode(transaction)).as_slice());
            if _known_transactions.contains(&eth_extrinsic_hash) {
              // Each known extrinsic is a new call stack.
              EvmTracer::emit_new();
              EvmTracer::new().trace(|| Executive::apply_extrinsic(ext));
            } else {
              let _ = Executive::apply_extrinsic(ext);
            }
          }
          _ => {
            let _ = Executive::apply_extrinsic(ext);
          }
        };
      }

      Ok(())
    }
  }
}

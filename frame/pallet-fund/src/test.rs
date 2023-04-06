use crate::{self as charity, Config, RawEvent};
use frame_support::{
	assert_err, assert_ok, construct_runtime, parameter_types,
	traits::{Currency, OnUnbalanced},
};
use frame_system::{self as system, EventRecord, Phase, RawOrigin};
use pallet_balances;
use sp_core::H256;
use sp_io;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
		Charity: charity::{Module, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(1024);
}
impl frame_system::Config for TestRuntime {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Index = u64;
	type Call = Call;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}
impl pallet_balances::Config for TestRuntime {
	type MaxLocks = ();
	type Balance = u64;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
}

impl Config for TestRuntime {
	type Event = Event;
	type Currency = Balances;
}

// An alternative to `ExternalityBuilder` which includes custom configuration
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = system::GenesisConfig::default()
		.build_storage::<TestRuntime>()
		.unwrap();

	pallet_balances::GenesisConfig::<TestRuntime> {
		// Provide some initial balances
		balances: vec![(1, 13), (2, 11), (3, 1), (4, 3), (5, 19)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	crate::GenesisConfig {}
		.assimilate_storage::<TestRuntime>(&mut t)
		.unwrap();

	let mut ext: sp_io::TestExternalities = t.into();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

/// Charity pot minimum balance is set
#[test]
fn pot_min_balance_is_set() {
	new_test_ext().execute_with(|| {
		assert_eq!(Charity::pot(), Balances::minimum_balance());
	})
}

/// Verifying correct behavior of boilerplate
#[test]
fn new_test_ext_behaves() {
	new_test_ext().execute_with(|| {
		assert_eq!(Balances::free_balance(&1), 13);
	})
}

#[test]
fn donations_work() {
	new_test_ext().execute_with(|| {
		// User 1 donates 10 of her 13 tokens
		let original = Balances::free_balance(&1);
		let donation = 10;
		assert_ok!(Charity::donate(Origin::signed(1), donation));

		// Charity should have 10 tokens
		let new_pot_total = Balances::minimum_balance() + donation;
		assert_eq!(Charity::pot(), new_pot_total);

		// Donor should have 3 remaining
		assert_eq!(Balances::free_balance(&1), original - donation);

		// Check that the correct event is emitted
		let expected_event = Event::charity(RawEvent::DonationReceived(1, donation, new_pot_total));

		assert_eq!(System::events()[1].event, expected_event,);
	})
}

#[test]
fn cant_donate_too_much() {
	new_test_ext().execute_with(|| {
		// User 1 donates 20 toekns but only has 13
		assert_err!(
			Charity::donate(Origin::signed(1), 20),
			"Can't make donation"
		);
	})
}

#[test]
fn imbalances_work() {
	new_test_ext().execute_with(|| {
		let donation = 11;
		assert_ok!(Charity::donate(Origin::signed(1), donation));
		let pot_total = Balances::minimum_balance() + donation;
		assert_eq!(Charity::pot(), pot_total);

		let imb_amt = 5;
		let imb = pallet_balances::PositiveImbalance::new(imb_amt);
		Charity::on_nonzero_unbalanced(imb);

		let new_pot_total = pot_total - imb_amt;
		assert_eq!(Charity::pot(), new_pot_total);

		// testing if the the event come in the correct order
		assert_eq!(
			System::events()[2],
			EventRecord {
				phase: Phase::Initialization,
				event: Event::charity(RawEvent::SpendFund(imb_amt)),
				topics: vec![],
			},
		);
	})
}

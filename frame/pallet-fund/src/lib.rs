#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
use frame_support::{
	traits::{
		Currency, OnUnbalanced, Imbalance,
	},
	PalletId,
};
use sp_runtime::{traits::AccountIdConversion};

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type PositiveImbalanceOf<T> = <<T as Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
	>>::PositiveImbalance;

/// Hardcoded pallet ID; used to create the special Pot Account
/// Must be exactly 8 characters long
const PALLET_ID: PalletId = PalletId(*b"fundreve");

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::*,
		traits::{
			Currency,
		},
	};
	use super::BalanceOf;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// The currency type that the charity deals in
		type Currency: Currency<Self::AccountId>;

	}

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Spend fund
		SpendFund(BalanceOf<T>),
	}

	#[cfg(feature = "std")]
	impl/*<T: Config>*/ Default for GenesisConfig/*<T>*/ {
		fn default() -> Self {
			Self {}
		}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig/*<T: Config>*/ {
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig/*<T>*/ {
		fn build(&self) {
			let account_id = <Pallet<T>>::account_id();
			let _ = T::Currency::make_free_balance_be(
				&account_id,
				T::Currency::minimum_balance(),
			);
		}
	}

	#[pallet::error]
	pub enum Error<T> {
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
	}
}

impl<T: Config> Pallet<T> {
	/// The account ID that holds the Reward's funds
	pub fn account_id() -> T::AccountId {
		PALLET_ID.into_account_truncating()
	}

	/// The Charity's balance
	fn pot() -> BalanceOf<T> {
		T::Currency::free_balance(&Self::account_id())
	}

}

// This implementation allows the charity to be the recipient of funds that are burned elsewhere in
// the runtime. For eample, it could be transaction fees, consensus-related slashing, or burns that
// align incentives in other pallets.
impl<T: Config> OnUnbalanced<PositiveImbalanceOf<T>> for Pallet<T> {
	fn on_nonzero_unbalanced(amount: PositiveImbalanceOf<T>) {
		let numeric_amount = amount.peek();
		/*if let Err(problem) = T::Currency::settle(
			&Self::account_id(),
			amount,
			WithdrawReasons::TRANSFER,
			ExistenceRequirement::KeepAlive
		) {
			print("Inconsistent state - couldn't settle imbalance for funds");
			// Nothing else to do here.
			drop(problem);
			return;
		}*/
		Self::deposit_event(Event::<T>::SpendFund(numeric_amount));
	}
}

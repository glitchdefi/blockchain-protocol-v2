#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
use frame_support::{
	traits::{
		Currency, WithdrawReasons, ExistenceRequirement, OnUnbalanced, Imbalance,
	},
	dispatch::{DispatchError, DispatchResult},
	print, PalletId,
};
use pallet_staking::RevenueWallet;
use sp_runtime::{traits::AccountIdConversion};

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Hardcoded pallet ID; used to create the special Pot Account
/// Must be exactly 8 characters long
const PALLET_ID: PalletId = PalletId(*b"waltfund");

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// The currency type that the charity deals in
		type Currency: Currency<Self::AccountId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Donor has made a charitable donation to the charity
		DonationReceived(T::AccountId, BalanceOf<T>, BalanceOf<T>),

		RewardReceived(BalanceOf<T>),
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
			let account_id = <Module<T>>::account_id();
			let _ = T::Currency::make_free_balance_be(
				&<Module<T>>::account_id(),
				T::Currency::minimum_balance(),
			);
		}
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Donate some funds to the charity
		#[pallet::weight((10, DispatchClass::Normal, Pays::No))]
		pub fn donate(
			origin: OriginFor<T>,
			amount: BalanceOf<T>
		) -> DispatchResult {
			let donor = ensure_signed(origin)?;

			T::Currency::transfer(&donor, &Self::account_id(), amount, ExistenceRequirement::AllowDeath)
				.map_err(|_| DispatchError::Other("Can't make donation"))?;

			Self::deposit_event(Event::DonationReceived(donor, amount, Self::pot()));
			Ok(())
		}

		// Transfer balance from wallet to reward fund
		#[pallet::weight((10, DispatchClass::Normal, Pays::No))]
		pub fn triger(origin: OriginFor<T>) -> DispatchResult {
			ensure_signed(origin)?;
			let amount = Self::pot();
			T::Currency::transfer(&Self::account_id(),&Self::reward_fund_id(), amount,ExistenceRequirement::AllowDeath)
				.map_err(|_| DispatchError::Other("Can't make donation"))?;
			Self::deposit_event(Event::RewardReceived(amount));
			Ok(())
		}
	}
}

impl<T: Config> Module<T> {
	/// The account ID that holds the revenue's funds
	pub fn account_id() -> T::AccountId {
		PALLET_ID.into_account_truncating()
	}
	/// The account ID that holds the Reward's funds
	fn reward_fund_id() -> T::AccountId { PalletId(*b"fundreve").into_account_truncating() }

	/// The wallet's balance
	fn pot() -> BalanceOf<T> {
		T::Currency::free_balance(&Self::account_id())
	}

	// pub fn u64_to_balance_option(input: u64) -> BalanceOf<T> { input.try_into().ok() }
}

impl<T:Config> RevenueWallet for Module<T> {
	fn trigger_wallet() {
		let amount = Self::pot();

		if amount > 0u32.into() {
			T::Currency::transfer(&Self::account_id(),&Self::reward_fund_id(), amount,ExistenceRequirement::AllowDeath)
				.map_err(|_| DispatchError::Other("Can't make donation"));
			Self::deposit_event(Event::RewardReceived(amount));
		}
	}
}


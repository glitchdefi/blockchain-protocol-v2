#![cfg_attr(not(feature = "std"), no_std)]

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;
use pallet_ethereum::RevenueWhiteList;
// use sp_core::H160;
use ethereum_types::H160;
use log::warn;
use frame_support::{
    PalletId,
    traits::{
        Currency,
    },
};
use sp_runtime::traits::AccountIdConversion;

#[cfg(feature = "std")]
use frame_support::traits::GenesisBuild;

const PALLET_ID: PalletId = PalletId(*b"RevShare");

pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use sp_std::vec::Vec;
    // use sp_core::H160;
    use ethereum_types::H160;
    use sp_arithmetic::Percent;

    use frame_support::{
        traits::{
            Currency,
        },
    };

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: Currency<Self::AccountId>;
    }

    #[pallet::event]
    // #[pallet::metadata(H160 = "address")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Event emitted when an address has been added to the whitelist. [address]
        AddressAdded(H160, Percent, T::AccountId),
        /// Event emitted when an address has been removed from the whitelist [address]
        AddressRemoved(H160),
    }

    #[pallet::error]
    pub enum Error<T> {
        NoPermission,
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub(super) type Whitelist<T: Config> = StorageMap<
        _, Blake2_128Concat, H160, Percent, ValueQuery
    >;

    #[pallet::storage]
    pub(super) type WithdrawalAddresses<T: Config> = StorageMap<
        _, Blake2_128Concat, H160, T::AccountId, OptionQuery
    >;

    //#[pallet::storage]
    //#[pallet::getter(fn admin_address)]
    //pub(super) type AdminAccountID<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;


    #[pallet::genesis_config]
    pub struct GenesisConfig/*<T: Config>*/ {
    //    pub admin_genesis: Option<T::AccountId>,
    }

    #[cfg(feature = "std")]
    impl/*<T: Config>*/ Default for GenesisConfig/*<T>*/ {
        fn default() -> Self {
            Self {
                //admin_genesis: None,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig/*<T>*/ {
        fn build(&self) {
            //AdminAccountID::<T>::put(self.admin_genesis.clone());
        }
    }


    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {

        #[pallet::weight(1_000)]
        pub fn add_address(
            origin: OriginFor<T>,
            address: H160,
            share_ratio: Percent,
            withdrawal_address: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            // get account root
            ensure_root(origin)?;
            //let who = ensure_signed(withdrawal_address)?;
            //ensure!(who == withdrawal_address, Error::<T>::NoPermission);
            if !Whitelist::<T>::contains_key(&address) {
                Whitelist::<T>::insert(address, share_ratio);
                WithdrawalAddresses::<T>::insert(address, withdrawal_address.clone());
                Self::deposit_event(Event::AddressAdded(address, share_ratio, withdrawal_address));
            }
            Ok(().into())
        }

        #[pallet::weight(1_000)]
        pub fn remove_address(
            origin: OriginFor<T>,
            address: H160,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            // let who = ensure_signed(origin)?;
            // ensure!(who == Self::admin_address(), Error::<T>::NoPermission);
            if !Whitelist::<T>::contains_key(&address) {
                Whitelist::<T>::remove(address);
                Self::deposit_event(Event::AddressRemoved(address));
            }
            Ok(().into())
        }

    }
}

impl<T: Config> RevenueWhiteList for Module<T> {
    fn is_in_white_list(address: H160) -> bool {
        let result = Whitelist::<T>::contains_key(&address);
        result
    }
}

impl<T: Config> Pallet<T> {
	/// The account ID that holds the Charity's funds
	pub fn account_id() -> T::AccountId {
		PALLET_ID.into_account_truncating()
	}

	/// The Charity's balance
	fn pot() -> BalanceOf<T> {
		T::Currency::free_balance(&Self::account_id())
	}
}

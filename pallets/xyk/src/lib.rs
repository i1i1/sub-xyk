#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::{Hash, StaticLookup};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

impl<T: Config> Pallet<T> {
    fn transfer(
        from: T::AccountId,
        to: T::AccountId,
        asset_id: AssetId<T>,
        amount: Balance<T>,
    ) -> Result<(), Error<T>> {
        pallet_assets::Pallet::<T>::transfer(
            RawOrigin::Signed(from).into(),
            asset_id.into(),
            T::Lookup::unlookup(to),
            amount,
        )
        .map_err(|_| Error::<T>::NotEnoughTokens)
    }

    fn mint_lp(lp: AssetId<T>, amount: Balance<T>, to: T::AccountId) {
        pallet_assets::Pallet::<T>::mint(
            RawOrigin::Signed(T::DEXAddr::get()).into(),
            lp.into(),
            T::Lookup::unlookup(to),
            amount,
        )
        .unwrap()
    }

    fn burn_lp(lp: AssetId<T>, amount: Balance<T>, from: T::AccountId) {
        pallet_assets::Pallet::<T>::burn(
            RawOrigin::Signed(T::DEXAddr::get()).into(),
            lp.into(),
            T::Lookup::unlookup(from),
            amount,
        )
        .unwrap()
    }

    pub fn lp_token(x: AssetId<T>, y: AssetId<T>) -> AssetId<T> {
        let (x, y) = if x > y { (y, x) } else { (x, y) };
        AssetId::<T>::decode(&mut T::AssetIdHash::hash_of(&(x, y)).encode().as_ref()).unwrap()
    }
}

#[frame_support::pallet]
pub mod pallet {
    use codec::{Decode, Encode};
    use frame_support::{
        dispatch::Parameter,
        ensure,
        pallet_prelude::*,
        storage::{with_transaction, TransactionOutcome},
    };
    use frame_system::{pallet_prelude::*, RawOrigin};
    use scale_info::TypeInfo;
    use sp_runtime::traits::{Hash, StaticLookup};

    use core::cmp::Ord;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_assets::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type AssetId: IsType<<Self as pallet_assets::Config>::AssetId> + Parameter + Ord + Copy;
        /// Address where dex stores all the funds
        type DEXAddr: Get<Self::AccountId>;
        /// Minimum balance for issued LP tokens
        type LpMinBalance: Get<Self::Balance>;
        /// Hash for asset ids. It is used to determenistically get LP asset id from 2 asset ids.
        type AssetIdHash: Hash;
    }

    pub type AssetId<T> = <T as Config>::AssetId;
    pub type Balance<T> = <T as pallet_assets::Config>::Balance;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[derive(Debug, Clone, Copy, Encode, Decode, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct Pair<T: Config> {
        x_id: AssetId<T>,
        x_balance: Balance<T>,
        y_id: AssetId<T>,
        y_balance: Balance<T>,
    }

    #[pallet::storage]
    pub type LPPairs<T> = StorageMap<_, Blake2_128Concat, AssetId<T>, Pair<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Swapped {
            who: T::AccountId,
            from: (AssetId<T>, Balance<T>),
            to: (AssetId<T>, Balance<T>),
        },
        Locked {
            who: T::AccountId,
            x: (AssetId<T>, Balance<T>),
            y: (AssetId<T>, Balance<T>),
            lp: (AssetId<T>, Balance<T>),
        },
        Unlocked {
            who: T::AccountId,
            x: (AssetId<T>, Balance<T>),
            y: (AssetId<T>, Balance<T>),
            lp: (AssetId<T>, Balance<T>),
        },
        LPMinted {
            who: T::AccountId,
            x: (AssetId<T>, Balance<T>),
            y: (AssetId<T>, Balance<T>),
            lp: (AssetId<T>, Balance<T>),
        },
    }

    /// Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        /// No liquidity available
        NoLiquidity,
        /// Not enough tokens
        NotEnoughTokens,
        LiquidityAlreadyExists,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Swap 2 tokens
        #[pallet::weight(10_000)]
        pub fn swap(
            origin: OriginFor<T>,
            from_id: AssetId<T>,
            from_balance: Balance<T>,
            to_id: AssetId<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let lp_id = Self::lp_token(from_id, to_id);

            let to_amount = LPPairs::<T>::mutate(lp_id, |pair| {
                let pair = pair.as_mut().ok_or(Error::<T>::NoLiquidity)?;

                Self::transfer(who.clone(), T::DEXAddr::get(), from_id, from_balance)?;

                // x * y = k
                // (x + dx) * (y - dy) = k
                // y - dy = x * y / (x + dx)
                // dy = y - x * y / (x + dx)
                let new_x = pair.x_balance + from_balance;
                let new_y = (pair.x_balance * pair.y_balance) / new_x;

                let dy = pair.y_balance - new_y;

                Self::transfer(T::DEXAddr::get(), who.clone(), to_id, dy)
                    .expect("Must always work by because of x*y=k architecture");

                pair.x_balance = new_x;
                pair.y_balance = new_y;

                Ok::<_, Error<T>>(dy)
            })?;

            let to = (to_id, to_amount);
            let from = (from_id, from_balance);
            Self::deposit_event(Event::Swapped { who, from, to });
            Ok(())
        }

        /// Create new lp pair
        #[pallet::weight(10_000)]
        pub fn create_lp(
            origin: OriginFor<T>,
            x_id: AssetId<T>,
            x_balance: Balance<T>,
            y_id: AssetId<T>,
            y_balance: Balance<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let lp_id = Self::lp_token(x_id, y_id);

            ensure!(!LPPairs::<T>::contains_key(lp_id), Error::<T>::LiquidityAlreadyExists);

            let do_transfer = || -> DispatchResult {
                Self::transfer(who.clone(), T::DEXAddr::get(), x_id, x_balance)?;
                Self::transfer(who.clone(), T::DEXAddr::get(), y_id, y_balance)?;
                Ok(())
            };

            with_transaction(|| match do_transfer() {
                r @ Ok(()) => TransactionOutcome::Commit(r),
                e @ Err(_) => TransactionOutcome::Rollback(e),
            })?;

            pallet_assets::Pallet::<T>::create(
                RawOrigin::Signed(T::DEXAddr::get()).into(),
                lp_id.into(),
                T::Lookup::unlookup(T::DEXAddr::get()),
                T::LpMinBalance::get(),
            )
            .expect("Must always be random. Assume no collisions");

            LPPairs::<T>::insert(lp_id, Pair { x_id, x_balance, y_id, y_balance });
            Self::mint_lp(lp_id, x_balance * y_balance, who.clone());

            let x = (x_id, x_balance);
            let y = (y_id, y_balance);
            let lp = (lp_id, x_balance * y_balance);
            Self::deposit_event(Event::LPMinted { who, x, y, lp });
            Ok(())
        }

        /// Lock liquidity
        #[pallet::weight(10_000)]
        pub fn lock(
            origin: OriginFor<T>,
            x_id: AssetId<T>,
            x_balance: Balance<T>,
            y_id: AssetId<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let lp_id = Self::lp_token(x_id, y_id);

            let (y_amount, lp) = LPPairs::<T>::mutate(lp_id, |pair| {
                let pair = pair.as_mut().ok_or(Error::<T>::NoLiquidity)?;

                // dy / y = dx / x
                // dy = y * dx / x
                let dy = x_balance * pair.y_balance / pair.x_balance;

                let lp_supply = pallet_assets::Pallet::<T>::total_supply(lp_id.into());
                let lp = lp_supply * x_balance / pair.x_balance;

                let do_transfer = || -> Result<(), Error<T>> {
                    Self::transfer(who.clone(), T::DEXAddr::get(), x_id, x_balance)?;
                    Self::transfer(who.clone(), T::DEXAddr::get(), y_id, dy)?;
                    Ok(())
                };

                with_transaction(|| match do_transfer() {
                    r @ Ok(()) => TransactionOutcome::Commit(r),
                    e @ Err(_) => TransactionOutcome::Rollback(e),
                })?;

                pair.x_balance += x_balance;
                pair.y_balance += dy;

                Self::mint_lp(lp_id, lp, who.clone());

                Ok::<_, Error<_>>((dy, (lp_id, lp)))
            })?;

            let x = (x_id, x_balance);
            let y = (y_id, y_amount);
            Self::deposit_event(Event::Locked { who, x, y, lp });
            Ok(())
        }

        /// Unlock liquidity
        #[pallet::weight(10_000)]
        pub fn unlock(origin: OriginFor<T>, lp: (AssetId<T>, Balance<T>)) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let (lp_id, lp_balance) = lp;

            let (x, y) = LPPairs::<T>::mutate(lp_id, |pair| {
                let pair = pair.as_mut().ok_or(Error::<T>::NoLiquidity)?;

                let lp_supply = pallet_assets::Pallet::<T>::total_supply(lp_id.into());

                let dx = pair.x_balance * lp_balance / lp_supply;
                let dy = pair.y_balance * lp_balance / lp_supply;

                Self::burn_lp(lp_id, lp_balance, who.clone());

                Self::transfer(T::DEXAddr::get(), who.clone(), pair.x_id, dx)
                    .expect("Dex should always have enough tokens");
                Self::transfer(T::DEXAddr::get(), who.clone(), pair.y_id, dy)
                    .expect("Dex should always have enough tokens");

                Ok::<_, Error<T>>(((pair.x_id, dx), (pair.y_id, dy)))
            })?;

            Self::deposit_event(Event::Unlocked { who, x, y, lp });
            Ok(())
        }
    }
}

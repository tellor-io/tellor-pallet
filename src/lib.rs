#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod api;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod contracts;
mod types;
pub mod xcm;

#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use super::{
		contracts::{governance, registry},
		types::*,
		xcm::{self, ethereum_xcm},
	};
	use ::xcm::latest::prelude::*;
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::traits::{
			AtLeast32BitUnsigned, BadOrigin, CheckEqual, Hash, MaybeDisplay,
			MaybeSerializeDeserialize, Member, SimpleBitOps,
		},
		traits::{
			fungible::{Inspect, Transfer},
			PalletInfoAccess, Time,
		},
		PalletId,
	};
	use frame_system::pallet_prelude::*;
	use sp_core::{bounded::BoundedBTreeMap, U256};
	use sp_runtime::{
		traits::{AccountIdConversion, SaturatedConversion},
		Saturating,
	};
	use sp_std::{fmt::Debug, prelude::*, result};

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The runtime origin type.
		type RuntimeOrigin: From<<Self as frame_system::Config>::RuntimeOrigin>
			+ Into<result::Result<Origin, <Self as Config>::RuntimeOrigin>>;

		/// The units in which we record amounts.
		type Amount: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// The identifier used for disputes.
		type DisputeId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Debug
			+ MaybeDisplay
			+ Ord
			+ MaxEncodedLen
			+ Default
			+ Into<U256>;

		/// Percentage, 1000 is 100%, 50 is 5%, etc
		#[pallet::constant]
		type Fee: Get<u8>;

		/// The location of the governance controller contract.
		#[pallet::constant]
		type Governance: Get<MultiLocation>;

		/// The output of the `Hasher` function.
		type Hash: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Debug
			+ MaybeDisplay
			+ SimpleBitOps
			+ Ord
			+ Default
			+ Copy
			+ CheckEqual
			+ sp_std::hash::Hash
			+ AsRef<[u8]>
			+ AsMut<[u8]>
			+ MaxEncodedLen;

		/// The hashing system (algorithm) to be used (e.g. keccak256).
		type Hasher: Hash<Output = <Self as Config>::Hash> + TypeInfo;

		/// The maximum number of timestamps per claim.
		#[pallet::constant]
		type MaxClaimTimestamps: Get<u32>;

		/// The maximum number of feeds per query.
		#[pallet::constant]
		type MaxFeedsPerQuery: Get<u32>;

		/// The maximum number of funded feeds.
		#[pallet::constant]
		type MaxFundedFeeds: Get<u32>;

		/// The maximum number of queries (data feeds) per reporter.
		#[pallet::constant]
		type MaxQueriesPerReporter: Get<u32> + TypeInfo;

		/// The maximum length of query data.
		#[pallet::constant]
		type MaxQueryDataLength: Get<u32>;

		/// The maximum number of timestamps per data feed.
		#[pallet::constant]
		type MaxTimestamps: Get<u32>;

		/// The maximum number of tips per query.
		#[pallet::constant]
		type MaxTipsPerQuery: Get<u32>;

		/// The maximum length of an individual value submitted to the oracle.
		#[pallet::constant]
		type MaxValueLength: Get<u32>;

		/// The maximum number of votes.
		#[pallet::constant]
		type MaxVotes: Get<u32>;

		/// The identifier of the pallet within the runtime.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The local parachain's own identifier.
		#[pallet::constant]
		type ParachainId: Get<ParaId>;

		/// The location of the registry controller contract.
		#[pallet::constant]
		type Registry: Get<MultiLocation>;

		/// Base amount of time before a reporter is able to submit a value again.
		#[pallet::constant]
		type ReportingLock: Get<TimestampOf<Self>>;

		/// The location of the staking controller contract.
		#[pallet::constant]
		type Staking: Get<MultiLocation>;

		/// The on-chain time provider.
		type Time: Time;

		type Token: Inspect<Self::AccountId, Balance = Self::Amount> + Transfer<Self::AccountId>;

		type Xcm: SendXcm;
	}

	// AutoPay
	#[pallet::storage]
	pub type CurrentFeeds<T> = StorageMap<
		_,
		Blake2_128Concat,
		QueryIdOf<T>,
		BoundedVec<FeedIdOf<T>, <T as Config>::MaxFeedsPerQuery>,
	>;
	#[pallet::storage]
	pub type DataFeeds<T> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		QueryIdOf<T>,
		Blake2_128Concat,
		FeedIdOf<T>,
		FeedDetailsOf<T>,
	>;
	#[pallet::storage]
	pub type FeedsWithFunding<T> =
		StorageValue<_, BoundedVec<FeedIdOf<T>, <T as Config>::MaxFundedFeeds>>;
	#[pallet::storage]
	pub type QueryIdFromDataFeedId<T> = StorageMap<_, Blake2_128Concat, FeedIdOf<T>, QueryIdOf<T>>;
	#[pallet::storage]
	pub type QueryIdsWithFunding<T> =
		StorageValue<_, BoundedVec<QueryIdOf<T>, <T as Config>::MaxFundedFeeds>>;
	#[pallet::storage]
	pub type QueryIdsWithFundingIndex<T> = StorageMap<_, Blake2_128Concat, QueryIdOf<T>, u32>;
	#[pallet::storage]
	#[pallet::getter(fn tips)]
	pub type Tips<T> = StorageMap<
		_,
		Blake2_128Concat,
		QueryIdOf<T>,
		BoundedVec<TipOf<T>, <T as Config>::MaxTipsPerQuery>,
	>;
	#[pallet::storage]
	pub type UserTipsTotal<T> =
		StorageMap<_, Blake2_128Concat, AccountIdOf<T>, AmountOf<T>, ValueQuery>;
	// Oracle
	#[pallet::storage]
	pub type Reports<T> = StorageMap<_, Blake2_128Concat, QueryIdOf<T>, ReportOf<T>>;
	#[pallet::storage]
	pub type RewardRate<T> = StorageValue<_, AmountOf<T>>;
	#[pallet::storage]
	pub type StakeAmount<T> = StorageValue<_, AmountOf<T>>;
	#[pallet::storage]
	pub type StakerDetails<T> = StorageMap<_, Blake2_128Concat, AccountIdOf<T>, StakeInfoOf<T>>;
	#[pallet::storage]
	pub type StakerAddresses<T> = StorageMap<_, Blake2_128Concat, Address, AccountIdOf<T>>;
	#[pallet::storage]
	pub type TimeOfLastNewValue<T> = StorageValue<_, TimestampOf<T>>;
	#[pallet::storage]
	pub type TotalStakeAmount<T> = StorageValue<_, AmountOf<T>>;
	#[pallet::storage]
	pub type TotalStakers<T> = StorageValue<_, u128>;
	// Governance
	#[pallet::storage]
	pub type DisputeIdsByReporter<T> =
		StorageDoubleMap<_, Blake2_128Concat, AccountIdOf<T>, Blake2_128Concat, DisputeIdOf<T>, ()>;
	#[pallet::storage]
	pub type VoteCount<T> = StorageValue<_, DisputeIdOf<T>, ValueQuery>;
	// Query Data
	#[pallet::storage]
	pub type QueryData<T> = StorageMap<_, Blake2_128Concat, QueryIdOf<T>, QueryDataOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// AutoPay
		DataFeedFunded {
			query_id: QueryIdOf<T>,
			feed_id: FeedIdOf<T>,
			amount: AmountOf<T>,
			feed_funder: AccountIdOf<T>,
			feed_details: FeedDetailsOf<T>,
		},
		NewDataFeed {
			query_id: QueryIdOf<T>,
			feed_id: FeedIdOf<T>,
			query_data: QueryDataOf<T>,
			feed_creator: AccountIdOf<T>,
		},
		OneTimeTipClaimed {
			query_id: QueryIdOf<T>,
			amount: AmountOf<T>,
			reporter: AccountIdOf<T>,
		},
		TipAdded {
			query_id: QueryIdOf<T>,
			amount: AmountOf<T>,
			query_data: QueryDataOf<T>,
			tipper: AccountIdOf<T>,
		},
		TipClaimed {
			feed_id: FeedIdOf<T>,
			query_id: QueryIdOf<T>,
			amount: AmountOf<T>,
			reporter: AccountIdOf<T>,
		},
		// Oracle
		NewReport {
			query_id: QueryIdOf<T>,
			time: TimestampOf<T>,
			value: ValueOf<T>,
			nonce: Nonce,
			query_data: QueryDataOf<T>,
			reporter: AccountIdOf<T>,
		},
		NewStakerReported {
			staker: AccountIdOf<T>,
			amount: AmountOf<T>,
			address: Address,
		},
		SlashReported {
			reporter: AccountIdOf<T>,
			recipient: AccountIdOf<T>,
			amount: AmountOf<T>,
		},
		StakeWithdrawnReported {
			staker: AccountIdOf<T>,
		},
		StakeWithdrawRequestReported {
			reporter: AccountIdOf<T>,
			balance: AmountOf<T>,
			address: Address,
		},
		ValueRemoved {
			query_id: QueryIdOf<T>,
			timestamp: TimestampOf<T>,
		},
		// Governance
		NewDispute {
			dispute_id: DisputeIdOf<T>,
			query_id: QueryIdOf<T>,
			timestamp: TimestampOf<T>,
			reporter: AccountIdOf<T>,
		},
		Voted {
			dispute_id: DisputeIdOf<T>,
			supports: Option<bool>,
			voter: AccountIdOf<T>,
		},
		// Query Data
		QueryDataStored {
			query_id: QueryIdOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		// AutoPay
		/// Tip must be greater than zero.
		InvalidAmount,
		/// Query identifier must be a hash of bytes data.
		InvalidQueryId,
		/// No tips submitted for this query identifier.
		NoTipsSubmitted,

		NoValueExists,
		NotStaking,
		// XCM
		InvalidContractAddress,
		InvalidDestination,
		MaxEthereumXcmInputSizeExceeded,
		SendFailure,
		Unreachable,
	}

	/// Origin for the Tellor module.
	#[pallet::origin]
	#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub enum Origin {
		/// It comes from the governance controller contract.
		Governance,
		/// It comes from the staking controller contract.
		Staking,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Function to claim singular tip.
		///
		/// - `query_id`: Identifier of reported data.
		/// - `timestamps`: Batch of timestamps of reported data eligible for reward.
		#[pallet::call_index(0)]
		pub fn claim_onetime_tip(
			origin: OriginFor<T>,
			query_id: QueryIdOf<T>,
			timestamps: BoundedVec<TimestampOf<T>, T::MaxClaimTimestamps>,
		) -> DispatchResult {
			let reporter = ensure_signed(origin)?;
			ensure!(
				<Tips<T>>::get(query_id).map_or(false, |t| t.len() > 0),
				Error::<T>::NoTipsSubmitted
			);

			let cumulative_reward = AmountOf::<T>::default();
			for _timestamp in timestamps {}
			Self::deposit_event(Event::OneTimeTipClaimed {
				query_id,
				amount: cumulative_reward,
				reporter,
			});
			Ok(())
		}

		/// Allows Tellor reporters to claim their tips in batches.
		///
		/// - `feed_id`: Unique feed identifier.
		/// - `query_id`: Identifier of reported data.
		/// - `timestamps`: Batch of timestamps of reported data eligible for reward.
		#[pallet::call_index(1)]
		pub fn claim_tip(
			_origin: OriginFor<T>,
			_feed_id: FeedIdOf<T>,
			_query_id: QueryIdOf<T>,
			_timestamps: BoundedVec<TimestampOf<T>, T::MaxClaimTimestamps>,
		) -> DispatchResult {
			Ok(())
		}

		/// Allows Tellor reporters to claim their tips in batches.
		///
		/// - `feed_id`: Unique feed identifier.
		/// - `query_id`: Identifier of reported data.
		/// - `timestamps`: Batch of timestamps of reported data eligible for reward.
		#[pallet::call_index(2)]
		pub fn fund_feed(
			_origin: OriginFor<T>,
			_feed_id: FeedIdOf<T>,
			_query_id: QueryIdOf<T>,
			_amount: AmountOf<T>,
		) -> DispatchResult {
			Ok(())
		}

		/// Initializes data feed parameters.
		///
		/// - `query_id`: Unique identifier of desired data feed.
		/// - `reward`: Tip amount per eligible data submission.
		/// - `start_time`: Timestamp of first autopay window.
		/// - `interval`: Amount of time between autopay windows.
		/// - `window`: Amount of time after each new interval when reports are eligible for tips.
		/// - `price_threshold`: Amount price must change to automate update regardless of time (negated if 0, 100 = 1%).
		/// - `reward_increase_per_second`: Amount reward increases per second within a window (0 for flat reward).
		/// - `query_data`: The data used by reporters to fulfil the query.
		/// - `amount`: Optional initial amount to fund it with.
		#[pallet::call_index(3)]
		pub fn setup_data_feed(
			_origin: OriginFor<T>,
			_query_id: QueryIdOf<T>,
			_reward: AmountOf<T>,
			_start_time: TimestampOf<T>,
			_interval: u8,
			_window: u8,
			_price_threshold: AmountOf<T>,
			_reward_increase_per_second: AmountOf<T>,
			_query_data: QueryDataOf<T>,
			_amount: Option<AmountOf<T>>,
		) -> DispatchResult {
			Ok(())
		}

		/// Function to run a single tip.
		///
		/// - `query_id`: Identifier of tipped data.
		/// - `amount`: Amount to tip.
		/// - `query_data`: The data used by reporters to fulfil the query.
		#[pallet::call_index(4)]
		pub fn tip(
			origin: OriginFor<T>,
			query_id: QueryIdOf<T>,
			amount: AmountOf<T>,
			query_data: QueryDataOf<T>,
		) -> DispatchResult {
			let tipper = ensure_signed(origin)?;
			ensure!(query_id == HasherOf::<T>::hash_of(&query_data), Error::<T>::InvalidQueryId);
			ensure!(amount > AmountOf::<T>::default(), Error::<T>::InvalidAmount);

			<Tips<T>>::try_mutate(query_id, |maybe_tips| -> DispatchResult {
				match maybe_tips {
					None => {
						*maybe_tips = Some(
							BoundedVec::try_from(vec![TipOf::<T> {
								amount,
								timestamp: T::Time::now(),
								cumulative_tips: amount,
							}])
							.map_err(|_| Error::<T>::InvalidQueryId)?,
						);
						Self::store_data(query_id, &query_data);
						Ok(())
					},
					Some(tips) => {
						todo!()
					},
				}
			})?;

			T::Token::transfer(
				&tipper,
				&T::PalletId::get().into_account_truncating(),
				amount,
				true,
			)?;
			<UserTipsTotal<T>>::mutate(&tipper, |total| total.saturating_add(amount));
			Self::deposit_event(Event::TipAdded { query_id, amount, query_data, tipper });
			Ok(())
		}

		/// Removes a value from the oracle.
		///
		/// - `query_id`: Identifier of the specific data feed.
		/// - `timestamp`: The timestamp of the value to remove.
		#[pallet::call_index(5)]
		pub fn remove_value(
			origin: OriginFor<T>,
			query_id: QueryIdOf<T>,
			timestamp: Timestamp,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;

			let timestamp = timestamp
				.saturated_into::<u128>() // todo: handle in single call skipping u128
				.saturated_into::<TimestampOf<T>>();

			Self::deposit_event(Event::ValueRemoved { query_id, timestamp });
			Ok(())
		}

		/// Allows a reporter to submit a value to the oracle.
		///
		/// - `query_id`: Identifier of the specific data feed.
		/// - `value`: Value the user submits to the oracle.
		/// - `nonce`: The current value count for the query identifier.
		/// - `query_data`: The data used to fulfil the data query.
		#[pallet::call_index(6)]
		pub fn submit_value(
			origin: OriginFor<T>,
			query_id: QueryIdOf<T>,
			_value: ValueOf<T>,
			_nonce: Nonce,
			_query_data: QueryDataOf<T>,
		) -> DispatchResult {
			let _reporter = ensure_signed(origin)?;

			let mut timestamps = BoundedVec::default();
			timestamps.try_push(T::Time::now()).unwrap(); // todo: return error

			<Reports<T>>::insert(
				query_id,
				ReportOf::<T> {
					timestamps,
					timestamp_index: BoundedBTreeMap::default(),
					timestamp_to_block_number: BoundedBTreeMap::default(),
					value_by_timestamp: BoundedBTreeMap::default(),
					reporter_by_timestamp: BoundedBTreeMap::default(),
					is_disputed: BoundedBTreeMap::default(),
				},
			);

			Ok(())
		}

		/// Initialises a dispute/vote in the system.
		///
		/// - `query_id`: Query identifier being disputed.
		/// - `timestamp`: Timestamp being disputed.
		#[pallet::call_index(7)]
		pub fn begin_dispute(
			origin: OriginFor<T>,
			query_id: QueryIdOf<T>,
			timestamp: TimestampOf<T>,
		) -> DispatchResult {
			let dispute_initiator = ensure_signed(origin)?;
			ensure!(<StakerDetails<T>>::contains_key(&dispute_initiator), <Error<T>>::NotStaking);
			ensure!(
				<Reports<T>>::get(query_id).map_or(false, |r| r.timestamps.contains(&timestamp)),
				<Error<T>>::NoValueExists
			);

			let dispute = DisputeOf::<T> {
				query_id,
				timestamp,
				value: <ValueOf<T>>::default(),
				dispute_reporter: dispute_initiator.clone(),
			};

			let _dispute_id = <VoteCount<T>>::get();
			let _query_id = [0u8; 32];
			let _timestamp = 12345;
			let _disputed_reporter = Address::default();
			let _dispute_initiator = Address::default();

			const GAS_LIMIT: u32 = 71_000;

			let governance = T::Governance::get();
			// Balances pallet on destination chain
			let self_reserve = MultiLocation { parents: 0, interior: X1(PalletInstance(3)) };
			let message = xcm::transact(
				MultiAsset {
					id: Concrete(self_reserve),
					fun: Fungible(1_000_000_000_000_000_u128),
				},
				WeightLimit::Unlimited,
				5_000_000_000u64,
				ethereum_xcm::transact(
					xcm::contract_address(&governance)
						.ok_or(Error::<T>::InvalidContractAddress)?
						.into(),
					governance::begin_parachain_dispute(
						T::ParachainId::get(),
						&_query_id.into(),
						_timestamp,
						_dispute_id.clone(),
						&dispute.value,
						_disputed_reporter,
						_dispute_initiator,
					)
					.try_into()
					.map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
					GAS_LIMIT.into(),
					None,
				),
			);
			Self::send_xcm(
				xcm::destination(&governance).ok_or(Error::<T>::InvalidDestination)?,
				message,
			)?;

			Self::deposit_event(Event::NewDispute {
				dispute_id: _dispute_id,
				query_id,
				timestamp,
				reporter: dispute_initiator, // todo: update
			});
			Ok(())
		}

		/// Enables the caller to cast a vote.
		///
		/// - `dispute_id`: The identifier of the dispute.
		/// - `supports`: Whether the caller supports or is against the vote. None indicates the caller’s classification of the dispute as invalid.
		#[pallet::call_index(8)]
		pub fn vote(
			origin: OriginFor<T>,
			dispute_id: DisputeIdOf<T>,
			supports: Option<bool>,
		) -> DispatchResult {
			let voter = ensure_signed(origin)?;
			Ok(())
		}

		/// Reports a stake deposited by a reporter.
		///
		/// - `reporter`: The reporter who deposited a stake.
		/// - `amount`: The amount staked.
		/// - `address`: The corresponding address on the controlling chain.
		#[pallet::call_index(9)]
		pub fn report_stake_deposited(
			origin: OriginFor<T>,
			reporter: AccountIdOf<T>,
			amount: Amount,
			address: Address,
		) -> DispatchResult {
			// ensure origin is staking controller contract
			ensure_staking(<T as Config>::RuntimeOrigin::from(origin))?;

			let amount = amount
				.saturated_into::<u128>() // todo: handle in single call skipping u128
				.saturated_into::<AmountOf<T>>();

			<StakerDetails<T>>::insert(
				&reporter,
				StakeInfoOf::<T> {
					address,
					start_date: T::Time::now(),
					staked_balance: amount,
					locked_balance: T::Amount::default(),
					reward_debt: T::Amount::default(),
					reporter_last_timestamp: <MomentOf<T>>::default(),
					reports_submitted: 0,
					start_vote_count: 0,
					start_vote_tally: 0,
					staked: false,
					reports_submitted_by_query_id: BoundedBTreeMap::default(),
				},
			);

			Self::deposit_event(Event::NewStakerReported { staker: reporter, amount, address });
			Ok(())
		}

		/// Reports a staking withdrawal request by a reporter.
		///
		/// - `reporter`: The reporter who requested a withdrawal.
		/// - `amount`: The amount requested to withdraw.
		/// - `address`: The corresponding address on the controlling chain.
		#[pallet::call_index(10)]
		pub fn report_staking_withdraw_request(
			origin: OriginFor<T>,
			reporter: AccountIdOf<T>,
			amount: Amount,
			address: Address,
		) -> DispatchResult {
			// ensure origin is staking controller contract
			ensure_staking(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}

		/// Reports a stake withdrawal by a reporter.
		///
		/// - `reporter`: The reporter who withdrew a stake.
		/// - `amount`: The total amount withdrawn.
		/// - `address`: The corresponding address on the controlling chain.
		#[pallet::call_index(11)]
		pub fn report_stake_withdrawal(
			origin: OriginFor<T>,
			reporter: AccountIdOf<T>,
			amount: Amount,
			address: Address,
		) -> DispatchResult {
			// ensure origin is staking controller contract
			ensure_staking(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}

		/// Reports a slashing of a reporter.
		///
		/// - `reporter`: The address of the slashed reporter.
		/// - `recipient`: The address of the recipient.
		/// - `amount`: The slashed amount.
		#[pallet::call_index(12)]
		pub fn report_slash(
			origin: OriginFor<T>,
			reporter: Address,
			recipient: Address,
			amount: Amount,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}

		#[pallet::call_index(13)]
		pub fn report_invalid_dispute(
			origin: OriginFor<T>,
			dispute_id: DisputeIdOf<T>,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}

		#[pallet::call_index(14)]
		pub fn slash_dispute_initiator(
			origin: OriginFor<T>,
			dispute_id: DisputeIdOf<T>,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}

		#[pallet::call_index(15)]
		pub fn register(origin: OriginFor<T>) -> DispatchResult {
			ensure_root(origin)?; // todo: use configurable origin

			const GAS_LIMIT: u32 = 15_000_000; // todo: make configurable

			let registry = T::Registry::get();

			// Balances pallet on destination chain
			let self_reserve = MultiLocation { parents: 0, interior: X1(PalletInstance(3)) };
			let message = xcm::transact(
				MultiAsset {
					id: Concrete(self_reserve),
					fun: Fungible(1_000_000_000_000_000_u128),
				},
				WeightLimit::Unlimited,
				50_000_000_000u64,
				ethereum_xcm::transact(
					xcm::contract_address(&registry)
						.ok_or(Error::<T>::InvalidContractAddress)?
						.into(),
					registry::register(T::ParachainId::get(), Pallet::<T>::index() as u8, 100)
						.try_into()
						.map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
					GAS_LIMIT.into(),
					None,
				),
			);
			Self::send_xcm(
				xcm::destination(&registry).ok_or(Error::<T>::InvalidDestination)?,
				message,
			)?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn send_xcm(
			destination: impl Into<MultiLocation>,
			mut message: Xcm<()>,
		) -> Result<(), Error<T>> {
			// Descend origin to signify pallet call
			message
				.0
				.insert(0, DescendOrigin(X1(PalletInstance(Pallet::<T>::index() as u8))));

			T::Xcm::send_xcm(destination, message).map_err(|e| match e {
				SendError::CannotReachDestination(..) => Error::<T>::Unreachable,
				_ => Error::<T>::SendFailure,
			})
		}

		fn store_data(query_id: QueryIdOf<T>, query_data: &QueryDataOf<T>) {
			QueryData::<T>::insert(query_id, query_data);
			Self::deposit_event(Event::QueryDataStored { query_id });
		}
	}

	/// Ensure that the origin `o` represents is the governance controller contract.
	/// Returns `Ok` if it does or an `Err` otherwise.
	fn ensure_governance<OuterOrigin>(o: OuterOrigin) -> Result<(), BadOrigin>
	where
		OuterOrigin: Into<Result<Origin, OuterOrigin>>,
	{
		match o.into() {
			Ok(Origin::Governance) => Ok(()),
			_ => Err(BadOrigin),
		}
	}

	/// Ensure that the origin `o` represents is the staking controller contract.
	/// Returns `Ok` if it does or an `Err` otherwise.
	fn ensure_staking<OuterOrigin>(o: OuterOrigin) -> Result<(), BadOrigin>
	where
		OuterOrigin: Into<Result<Origin, OuterOrigin>>,
	{
		match o.into() {
			Ok(Origin::Staking) => Ok(()),
			_ => Err(BadOrigin),
		}
	}
}

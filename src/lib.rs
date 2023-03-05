#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	ensure,
	traits::{Len, Time},
};
pub use pallet::*;
use sp_core::Get;
use sp_runtime::Saturating;
use sp_std::vec::Vec;
use traits::UsingTellor;
use types::*;
pub use types::{
	autopay::{FeedDetails, Tip},
	oracle::StakeInfo,
	Address,
};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod contracts;
pub mod traits;
mod types;
pub mod xcm;

#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use super::{
		contracts::{governance, registry},
		types::*,
		xcm::{self, ethereum_xcm},
		*,
	};
	use crate::{types::oracle::Report, Tip};
	use ::xcm::latest::prelude::*;
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::traits::{
			AtLeast32BitUnsigned, BadOrigin, CheckEqual, Hash, MaybeDisplay,
			MaybeSerializeDeserialize, Member, SimpleBitOps,
		},
		traits::{
			fungible::{Inspect, Transfer},
			PalletInfoAccess,
		},
		PalletId,
	};
	use frame_system::pallet_prelude::*;
	use sp_core::{bounded::BoundedBTreeMap, U256};
	use sp_runtime::traits::{AccountIdConversion, CheckedDiv, SaturatedConversion};
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
			+ TypeInfo
			+ Into<U256>;

		/// The claim buffer time.
		#[pallet::constant]
		type ClaimBuffer: Get<<Self::Time as Time>::Moment>;

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
		type Fee: Get<u16>;

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
		type MaxQueriesPerReporter: Get<u32>;

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

		type Xcm: traits::Xcm;
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
	pub type StakeAmount<T> = StorageValue<_, AmountOf<T>, ValueQuery>;
	#[pallet::storage]
	pub type StakerDetails<T> = StorageMap<_, Blake2_128Concat, AccountIdOf<T>, StakeInfoOf<T>>;
	#[pallet::storage]
	pub type StakerAddresses<T> = StorageMap<_, Blake2_128Concat, Address, AccountIdOf<T>>;
	#[pallet::storage]
	pub type TimeOfLastNewValue<T> = StorageValue<_, TimestampOf<T>>;
	#[pallet::storage]
	pub type TotalStakeAmount<T> = StorageValue<_, AmountOf<T>, ValueQuery>;
	#[pallet::storage]
	pub type TotalStakers<T> = StorageValue<_, u128, ValueQuery>;
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
		/// Claim buffer time has not passed.
		ClaimBufferNotPassed,
		FeeCalculationError,
		/// Feed must not be set up already.
		FeedAlreadyExists,
		/// Tip must be greater than zero.
		InvalidAmount,
		/// Claimer must be the reporter.
		InvalidClaimer,
		/// Query identifier must be a hash of bytes data.
		InvalidQueryId,
		/// The maximum number of feeds have been funded,
		MaxFeedsFunded,
		/// The maximum number of tips has been reached,
		MaxTipsReached,
		/// No tips submitted for this query identifier.
		NoTipsSubmitted,
		/// Timestamp not eligible for tip.
		TimestampIneligibleForTip,
		/// Tip already claimed.
		TipAlreadyClaimed,
		/// Tip earned by previous submission.
		TipAlreadyEarned,
		/// Value disputed.
		ValueDisputed,

		// Oracle
		/// Balance must be greater than stake amount.
		InsufficientStake,
		/// Nonce must match the timestamp index.
		InvalidNonce,
		/// Value must be submitted.
		InvalidValue,
		/// The maximum number of queries has been reached.
		MaxQueriesReached,
		/// The maximum number of timestamps has been reached,
		MaxTimestampsReached,
		/// Still in reporter time lock, please wait!
		ReporterTimeLocked,
		ReportingLockCalculationError,
		/// Timestamp already reported.
		TimestampAlreadyReported,

		NoValueExists,
		NotStaking,
		// Governance
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
		#[pallet::call_index(0)]
		pub fn register(
			origin: OriginFor<T>,
			stake_amount: AmountOf<T>,
			require_weight_at_most: u64,
			gas_limit: u128,
		) -> DispatchResult {
			ensure_root(origin)?; // todo: use configurable origin

			<StakeAmount<T>>::set(stake_amount);

			let registry = T::Registry::get();

			// Balances pallet on destination chain
			let self_reserve = MultiLocation { parents: 0, interior: X1(PalletInstance(3)) };
			let message = xcm::transact(
				MultiAsset { id: Concrete(self_reserve), fun: Fungible(300_000_000_000_000_u128) },
				WeightLimit::Unlimited,
				require_weight_at_most,
				ethereum_xcm::transact(
					xcm::contract_address(&registry)
						.ok_or(Error::<T>::InvalidContractAddress)?
						.into(),
					registry::register(
						T::ParachainId::get(),
						Pallet::<T>::index() as u8,
						stake_amount,
					)
					.try_into()
					.map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
					gas_limit.into(),
					None,
				),
			);
			Self::send_xcm(
				xcm::destination(&registry).ok_or(Error::<T>::InvalidDestination)?,
				message,
			)?;

			Ok(())
		}

		/// Function to claim singular tip.
		///
		/// - `query_id`: Identifier of reported data.
		/// - `timestamps`: Batch of timestamps of reported data eligible for reward.
		#[pallet::call_index(1)]
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

			let mut cumulative_reward = AmountOf::<T>::default();
			for timestamp in timestamps {
				cumulative_reward += Self::get_onetime_tip_amount(query_id, timestamp, &reporter)?;
			}
			let fee = (cumulative_reward.saturating_mul(T::Fee::get().into()))
				.checked_div(&1000u16.into())
				.ok_or(Error::<T>::FeeCalculationError)?;
			T::Token::transfer(
				&reporter,
				&T::PalletId::get().into_account_truncating(),
				cumulative_reward - fee,
				true,
			)?;
			Self::add_staking_rewards(fee);
			if Self::get_current_tip(query_id) == <AmountOf<T>>::default() {
				// todo: replace with if let once guards stable
				match <QueryIdsWithFundingIndex<T>>::get(query_id) {
					Some(index) if index != 0 => {
						let idx: usize = index as usize - 1;
						// Replace unfunded feed in array with last element
						<QueryIdsWithFunding<T>>::mutate(|maybe| match maybe {
							None => {
								todo!()
							},
							Some(query_ids_with_funding) => {
								// todo: safe indexing
								query_ids_with_funding[idx] =
									query_ids_with_funding[query_ids_with_funding.len() - 1];
								let query_id_last_funded = query_ids_with_funding[idx];
								<QueryIdsWithFundingIndex<T>>::set(
									query_id_last_funded,
									Some((idx + 1).saturated_into()),
								);
								<QueryIdsWithFundingIndex<T>>::remove(query_id);
								query_ids_with_funding.pop();
							},
						});
					},
					_ => {},
				}
			}
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
		#[pallet::call_index(2)]
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
		#[pallet::call_index(3)]
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
		#[pallet::call_index(4)]
		pub fn setup_data_feed(
			_origin: OriginFor<T>,
			_query_id: QueryIdOf<T>,
			_reward: AmountOf<T>,
			_start_time: TimestampOf<T>,
			_interval: u32,
			_window: u32,
			_price_threshold: u16,
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
		#[pallet::call_index(5)]
		pub fn tip(
			origin: OriginFor<T>,
			query_id: QueryIdOf<T>,
			amount: AmountOf<T>,
			query_data: QueryDataOf<T>,
		) -> DispatchResult {
			let tipper = ensure_signed(origin)?;
			ensure!(
				query_id == HasherOf::<T>::hash(&query_data.as_ref()),
				Error::<T>::InvalidQueryId
			);
			ensure!(amount > AmountOf::<T>::default(), Error::<T>::InvalidAmount);

			<Tips<T>>::try_mutate(query_id, |mut maybe_tips| -> DispatchResult {
				match &mut maybe_tips {
					None => {
						*maybe_tips = Some(
							BoundedVec::try_from(vec![TipOf::<T> {
								amount,
								timestamp: T::Time::now(),
								cumulative_tips: amount,
							}])
							.map_err(|_| Error::<T>::MaxTipsReached)?,
						);
						Self::store_data(query_id, &query_data);
						Ok(())
					},
					Some(tips) => {
						let timestamp_retrieved = Self::_get_current_value(query_id)
							.map_or(<TimestampOf<T>>::default(), |v| v.1);
						match tips.last_mut() {
							Some(last_tip) if timestamp_retrieved < last_tip.timestamp => {
								last_tip.timestamp = T::Time::now();
								last_tip.amount = last_tip.amount.saturating_add(amount);
								last_tip.cumulative_tips =
									last_tip.cumulative_tips.saturating_add(amount);
							},
							_ => {
								let cumulative_tips = tips
									.last()
									.map_or(<AmountOf<T>>::default(), |t| t.cumulative_tips);
								tips.try_push(Tip {
									amount,
									timestamp: T::Time::now(),
									cumulative_tips: cumulative_tips.saturating_add(amount),
								})
								.map_err(|_| Error::<T>::MaxTipsReached)?;
							},
						}
						Ok(())
					},
				}
			})?;

			if <QueryIdsWithFundingIndex<T>>::get(query_id).unwrap_or_default() == 0 &&
				Self::get_current_tip(query_id) > <AmountOf<T>>::default()
			{
				let mut len: u32 = 0;
				<QueryIdsWithFunding<T>>::try_mutate(|maybe| match maybe {
					Some(query_ids) => {
						query_ids.try_push(query_id).map_err(|_| Error::<T>::MaxFeedsFunded)?;
						len = query_ids.len() as u32;
						Ok::<(), Error<T>>(())
					},
					None => {
						*maybe = Some(
							BoundedVec::try_from(vec![query_id])
								.map_err(|_| Error::<T>::MaxFeedsFunded)?,
						);
						Ok(())
					},
				})?;
				<QueryIdsWithFundingIndex<T>>::set(query_id, Some(len));
			}
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
		#[pallet::call_index(6)]
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
		#[pallet::call_index(7)]
		pub fn submit_value(
			origin: OriginFor<T>,
			query_id: QueryIdOf<T>,
			value: ValueOf<T>,
			nonce: Nonce,
			query_data: QueryDataOf<T>,
		) -> DispatchResult {
			let reporter = ensure_signed(origin)?;
			ensure!(
				HasherOf::<T>::hash(value.as_ref()) != HasherOf::<T>::hash(&[]),
				Error::<T>::InvalidValue
			);
			let report = <Reports<T>>::get(query_id);
			ensure!(
				nonce ==
					report.as_ref().map_or(Nonce::default(), |r| r
						.timestamps
						.len()
						.saturated_into::<Nonce>()),
				Error::<T>::InvalidNonce
			);
			let mut staker =
				<StakerDetails<T>>::get(&reporter).ok_or(Error::<T>::InsufficientStake)?;
			ensure!(
				staker.staked_balance >= <StakeAmount<T>>::get(),
				Error::<T>::InsufficientStake
			);
			// Require reporter to abide by given reporting lock
			let timestamp = T::Time::now();
			ensure!(
				// todo: refactor
				(timestamp - staker.reporter_last_timestamp)
					.saturated_into::<u128>()
					.saturating_mul(1000) >
					(T::ReportingLock::get().saturated_into::<u128>().saturating_mul(1000))
						.checked_div(
							staker
								.staked_balance
								.checked_div(&<StakeAmount<T>>::get())
								.ok_or(Error::<T>::ReportingLockCalculationError)?
								.saturated_into::<u128>()
						)
						.ok_or(Error::<T>::ReportingLockCalculationError)?,
				Error::<T>::ReporterTimeLocked
			);
			ensure!(
				query_id == HasherOf::<T>::hash(query_data.as_ref()),
				Error::<T>::InvalidQueryId
			);
			staker.reporter_last_timestamp = timestamp;
			// Checks for no double reporting of timestamps
			ensure!(
				report
					.as_ref()
					.map_or(true, |r| r.reporter_by_timestamp.contains_key(&timestamp)),
				Error::<T>::TimestampAlreadyReported
			);

			// Update number of timestamps, value for given timestamp, and reporter for timestamp
			let mut report = report.unwrap_or(Report::new());
			report
				.timestamp_index
				.try_insert(timestamp, report.timestamps.len().saturated_into::<u32>())
				.map_err(|_| Error::<T>::MaxTimestampsReached)?;
			report
				.timestamps
				.try_push(timestamp)
				.map_err(|_| Error::<T>::MaxTimestampsReached)?;
			report
				.timestamp_to_block_number
				.try_insert(timestamp, frame_system::Pallet::<T>::block_number())
				.map_err(|_| Error::<T>::MaxTimestampsReached)?;
			report
				.value_by_timestamp
				.try_insert(timestamp, value.clone())
				.map_err(|_| Error::<T>::MaxTimestampsReached)?;
			report
				.reporter_by_timestamp
				.try_insert(timestamp, reporter.clone())
				.map_err(|_| Error::<T>::MaxTimestampsReached)?;
			<Reports<T>>::insert(query_id, report);

			// todo: Disperse Time Based Reward
			// uint256 _reward = ((block.timestamp - timeOfLastNewValue) * timeBasedReward) / 300; //.5 TRB per 5 minutes
			// uint256 _totalTimeBasedRewardsBalance =
			// 	token.balanceOf(address(this)) -
			// 		(totalStakeAmount + stakingRewardsBalance + toWithdraw);
			// if (_totalTimeBasedRewardsBalance > 0 && _reward > 0) {
			// 	if (_totalTimeBasedRewardsBalance < _reward) {
			// 		token.transfer(msg.sender, _totalTimeBasedRewardsBalance);
			// 	} else {
			// 		token.transfer(msg.sender, _reward);
			// 	}
			// }

			// Update last oracle value and number of values submitted by a reporter
			<TimeOfLastNewValue<T>>::set(Some(timestamp));
			staker.reports_submitted += 1;
			staker
				.reports_submitted_by_query_id
				.try_insert(
					query_id,
					staker
						.reports_submitted_by_query_id
						.get(&query_id)
						.copied()
						.unwrap_or_default()
						.saturating_add(1),
				)
				.map_err(|_| Error::<T>::MaxQueriesReached)?;
			<StakerDetails<T>>::insert(&reporter, staker);
			Self::deposit_event(Event::NewReport {
				query_id,
				time: timestamp,
				value,
				nonce,
				query_data,
				reporter,
			});
			Ok(())
		}

		/// Initialises a dispute/vote in the system.
		///
		/// - `query_id`: Query identifier being disputed.
		/// - `timestamp`: Timestamp being disputed.
		#[pallet::call_index(8)]
		pub fn begin_dispute(
			origin: OriginFor<T>,
			query_id: QueryIdOf<T>,
			timestamp: TimestampOf<T>,
		) -> DispatchResult {
			let dispute_initiator = ensure_signed(origin)?;
			ensure!(<StakerDetails<T>>::contains_key(&dispute_initiator), Error::<T>::NotStaking);
			ensure!(
				<Reports<T>>::get(query_id).map_or(false, |r| r.timestamps.contains(&timestamp)),
				Error::<T>::NoValueExists
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
		#[pallet::call_index(9)]
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
		#[pallet::call_index(10)]
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
		#[pallet::call_index(11)]
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
		#[pallet::call_index(12)]
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
		#[pallet::call_index(13)]
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

		#[pallet::call_index(14)]
		pub fn report_invalid_dispute(
			origin: OriginFor<T>,
			dispute_id: DisputeIdOf<T>,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}

		#[pallet::call_index(15)]
		pub fn slash_dispute_initiator(
			origin: OriginFor<T>,
			dispute_id: DisputeIdOf<T>,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn send_xcm(
			destination: impl Into<MultiLocation>,
			mut message: Xcm<()>,
		) -> Result<(), Error<T>> {
			let interior = X1(PalletInstance(Pallet::<T>::index() as u8));
			<T::Xcm as traits::Xcm>::send_xcm(interior, destination, message).map_err(|e| match e {
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

impl<T: Config> Pallet<T> {
	pub fn get_block_number_by_timestamp(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<BlockNumberOf<T>> {
		todo!()
	}

	fn add_staking_rewards(amount: AmountOf<T>) {
		// todo: allocate to sub-account, separate from xcm fees
	}

	/// Read current onetime tip by query identifier.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// # Returns
	/// Amount of tip.
	pub fn get_current_tip(query_id: QueryIdOf<T>) -> AmountOf<T> {
		// todo: optimise
		// if no tips, return 0
		if <Tips<T>>::get(query_id).map_or(0, |t| t.len()) == 0 {
			return AmountOf::<T>::default()
		}
		let timestamp_retrieved =
			Self::_get_current_value(query_id).map_or(TimestampOf::<T>::default(), |v| v.1);
		match <Tips<T>>::get(query_id) {
			Some(tips) => match tips.last() {
				Some(last_tip) if timestamp_retrieved < last_tip.timestamp => last_tip.amount,
				_ => AmountOf::<T>::default(),
			},
			_ => AmountOf::<T>::default(),
		}
	}

	/// Allows the user to get the latest value for the query identifier specified.
	/// # Arguments
	/// * `query_id` - Identifier to look up the value for
	/// # Returns
	/// The value retrieved, along with its timestamp, if found.
	fn _get_current_value(query_id: QueryIdOf<T>) -> Option<(ValueOf<T>, TimestampOf<T>)> {
		let mut count = Self::get_new_value_count_by_query_id(query_id);
		if count == 0 {
			return None
		}
		//loop handles for dispute (value = None if disputed)
		while count > 0 {
			count -= 1;
			let value =
				Self::get_timestamp_by_query_id_and_index(query_id, count).and_then(|timestamp| {
					Self::retrieve_data(query_id, timestamp)
						.and_then(|value| Some((value, timestamp)))
				});
			if value.is_some() {
				return value
			}
		}
		None
	}

	pub fn get_current_value(query_id: QueryIdOf<T>) -> Option<ValueOf<T>> {
		// todo: implement properly
		<Reports<T>>::get(query_id)
			.and_then(|r| r.value_by_timestamp.last_key_value().and_then(|kv| Some(kv.1.clone())))
	}

	fn get_data_before(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<(ValueOf<T>, TimestampOf<T>)> {
		Self::get_index_for_data_before(query_id, timestamp)
			.and_then(|index| Self::get_timestamp_by_query_id_and_index(query_id, index))
			.and_then(|timestamp_retrieved| {
				Self::retrieve_data(query_id, timestamp_retrieved)
					.and_then(|value| Some((value, timestamp_retrieved)))
			})
	}

	fn get_index_for_data_before(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<usize> {
		let count = Self::get_new_value_count_by_query_id(query_id);
		if count > 0 {
			let mut middle;
			let mut start = 0;
			let mut end = count - 1;
			let mut time;
			// Checking Boundaries to short-circuit the algorithm
			time = Self::get_timestamp_by_query_id_and_index(query_id, start)?;
			if time >= timestamp {
				return None
			}
			time = Self::get_timestamp_by_query_id_and_index(query_id, end)?;
			if time < timestamp {
				while Self::is_in_dispute(query_id, time) && end > 0 {
					end -= 1;
					time = Self::get_timestamp_by_query_id_and_index(query_id, end)?;
				}
				if end == 0 && Self::is_in_dispute(query_id, time) {
					return None
				}
				return Some(end)
			}
			// Since the value is within our boundaries, do a binary search
			loop {
				middle = (end - start) / 2 + 1 + start;
				time = Self::get_timestamp_by_query_id_and_index(query_id, middle)?;
				if time < timestamp {
					//get immediate next value
					let next_time =
						Self::get_timestamp_by_query_id_and_index(query_id, middle + 1)?;
					if next_time >= timestamp {
						if !Self::is_in_dispute(query_id, time) {
							// _time is correct
							return Some(middle)
						} else {
							// iterate backwards until we find a non-disputed value
							while Self::is_in_dispute(query_id, time) && middle > 0 {
								middle -= 1;
								time = Self::get_timestamp_by_query_id_and_index(query_id, middle)?;
							}
							if middle == 0 && Self::is_in_dispute(query_id, time) {
								return None
							}
							// _time is correct
							return Some(middle)
						}
					} else {
						//look from middle + 1(next value) to end
						start = middle + 1;
					}
				} else {
					let mut previous_time =
						Self::get_timestamp_by_query_id_and_index(query_id, middle - 1)?;
					if previous_time < timestamp {
						if !Self::is_in_dispute(query_id, previous_time) {
							// _prevTime is correct
							return Some(middle - 1)
						} else {
							// iterate backwards until we find a non-disputed value
							middle -= 1;
							while Self::is_in_dispute(query_id, previous_time) && middle > 0 {
								middle -= 1;
								previous_time =
									Self::get_timestamp_by_query_id_and_index(query_id, middle)?;
							}
							if middle == 0 && Self::is_in_dispute(query_id, previous_time) {
								return None
							}
							// _prevtime is correct
							return Some(middle)
						}
					} else {
						//look from start to middle -1(prev value)
						end = middle - 1;
					}
				}
			}
		}
		return None
	}

	/// Determines tip eligibility for a given oracle submission.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// * `timestamp` - Timestamp of one time tip.
	/// # Returns
	/// Amount of tip.
	fn get_onetime_tip_amount(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
		claimer: &AccountIdOf<T>,
	) -> Result<AmountOf<T>, Error<T>> {
		ensure!(
			T::Time::now().saturating_sub(timestamp) > T::ClaimBuffer::get(),
			Error::<T>::ClaimBufferNotPassed
		);
		ensure!(!Self::is_in_dispute(query_id, timestamp), Error::<T>::ValueDisputed);
		ensure!(
			Self::get_reporter_by_timestamp(query_id, timestamp)
				.map_or(false, |reporter| claimer == &reporter),
			Error::<T>::InvalidClaimer
		);
		<Tips<T>>::try_mutate(query_id, |maybe_tips| {
			match maybe_tips {
				None => Err(Error::<T>::NoTipsSubmitted),
				Some(tips) => {
					let mut min = 0;
					let mut max = tips.len();
					let mut mid;
					while max - min > 1 {
						mid = (max.saturating_add(min)).saturating_div(2);
						if tips.get(mid).map_or(<TimestampOf<T>>::default(), |t| t.timestamp) >
							timestamp
						{
							max = mid;
						} else {
							min = mid;
						}
					}

					let (_, timestamp_before) =
						Self::get_data_before(query_id, timestamp).unwrap_or_default();
					let min_tip = &mut tips[min]; // todo: convert to tips::get(min)
					ensure!(timestamp_before < min_tip.timestamp, Error::<T>::TipAlreadyEarned);
					ensure!(timestamp >= min_tip.timestamp, Error::<T>::TimestampIneligibleForTip);
					ensure!(
						min_tip.amount > <AmountOf<T>>::default(),
						Error::<T>::TipAlreadyClaimed
					);

					// todo: add test to ensure storage updated accordingly
					let mut tip_amount = min_tip.amount;
					min_tip.amount = <AmountOf<T>>::default();
					let min_backup = min;

					// check whether eligible for previous tips in array due to disputes
					let index_now = Self::get_index_for_data_before(
						query_id,
						timestamp.saturating_add(1u32.into()),
					);
					let index_before = Self::get_index_for_data_before(
						query_id,
						timestamp_before.saturating_add(1u32.into()),
					);
					if index_now
						.unwrap_or_default()
						.saturating_sub(index_before.unwrap_or_default()) >
						1 || index_before.is_none()
					{
						if index_before.is_none() {
							tip_amount = tips[min_backup].cumulative_tips;
						} else {
							max = min;
							min = 0;
							let mut mid;
							while max.saturating_sub(min) > 1 {
								mid = (max.saturating_add(min)).saturating_div(2);
								if tips[mid].timestamp > timestamp_before {
									max = mid;
								} else {
									min = mid;
								}
							}
							min = min.saturating_add(1);
							if min < min_backup {
								tip_amount = tips[min_backup].cumulative_tips -
									tips[min].cumulative_tips + tips[min].amount;
							}
						}
					}

					Ok(tip_amount)
				},
			}
		})
	}

	pub fn get_query_data(query_id: QueryIdOf<T>) -> Option<QueryDataOf<T>> {
		<QueryData<T>>::get(query_id)
	}

	pub fn get_reporter_by_timestamp(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<AccountIdOf<T>> {
		<Reports<T>>::get(query_id)
			.and_then(|report| report.reporter_by_timestamp.get(&timestamp).cloned())
	}

	pub fn get_reporting_lock() -> TimestampOf<T> {
		T::ReportingLock::get()
	}

	pub fn get_stake_amount() -> AmountOf<T> {
		<StakeAmount<T>>::get()
	}

	pub fn get_staker_info(staker: AccountIdOf<T>) -> Option<StakeInfoOf<T>> {
		<StakerDetails<T>>::get(staker)
	}

	pub fn get_timestamp_by_query_id_and_index(
		query_id: QueryIdOf<T>,
		index: usize,
	) -> Option<TimestampOf<T>> {
		<Reports<T>>::get(query_id).and_then(|report| report.timestamps.get(index).copied())
	}

	pub fn get_total_stake_amount() -> AmountOf<T> {
		<TotalStakeAmount<T>>::get()
	}

	pub fn get_total_stakers() -> u128 {
		<TotalStakers<T>>::get()
	}

	pub fn get_new_value_count_by_query_id(query_id: QueryIdOf<T>) -> usize {
		<Reports<T>>::get(query_id).map_or(usize::default(), |r| r.timestamps.len())
	}

	pub fn is_in_dispute(query_id: QueryIdOf<T>, timestamp: TimestampOf<T>) -> bool {
		<Reports<T>>::get(query_id)
			.map_or(false, |report| report.is_disputed.contains_key(&timestamp))
	}

	pub fn retrieve_data(query_id: QueryIdOf<T>, timestamp: TimestampOf<T>) -> Option<ValueOf<T>> {
		<Reports<T>>::get(query_id)
			.and_then(|report| report.value_by_timestamp.get(&timestamp).cloned())
	}
}

impl<T: Config> UsingTellor<AccountIdOf<T>, QueryIdOf<T>, TimestampOf<T>, ValueOf<T>>
	for Pallet<T>
{
	fn get_data_after(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<(ValueOf<T>, TimestampOf<T>)> {
		Self::get_index_for_data_after(query_id, timestamp)
			.and_then(|index| Self::get_timestamp_by_query_id_and_index(query_id, index))
			.and_then(|timestamp_retrieved| {
				Self::retrieve_data(query_id, timestamp_retrieved)
					.map(|value| (value, timestamp_retrieved))
			})
	}

	fn get_data_before(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<(ValueOf<T>, TimestampOf<T>)> {
		Self::get_data_before(query_id, timestamp)
	}

	fn get_index_for_data_after(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<usize> {
		todo!()
	}

	fn get_index_for_data_before(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<usize> {
		Self::get_index_for_data_before(query_id, timestamp)
	}

	fn get_multiple_values_before(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
		max_age: TimestampOf<T>,
	) -> Vec<(ValueOf<T>, TimestampOf<T>)> {
		todo!()
	}

	fn get_new_value_count_by_query_id(query_id: QueryIdOf<T>) -> usize {
		Self::get_new_value_count_by_query_id(query_id)
	}

	fn get_reporter_by_timestamp(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<AccountIdOf<T>> {
		Self::get_reporter_by_timestamp(query_id, timestamp)
	}

	fn get_timestamp_by_query_id_and_index(
		query_id: QueryIdOf<T>,
		index: usize,
	) -> Option<TimestampOf<T>> {
		Self::get_timestamp_by_query_id_and_index(query_id, index)
	}

	fn is_in_dispute(query_id: QueryIdOf<T>, timestamp: TimestampOf<T>) -> bool {
		Self::is_in_dispute(query_id, timestamp)
	}

	fn retrieve_data(query_id: QueryIdOf<T>, timestamp: TimestampOf<T>) -> Option<ValueOf<T>> {
		Self::retrieve_data(query_id, timestamp)
	}
}

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Encode;
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure,
	traits::{fungible::Transfer, Len, Time},
};
pub use pallet::*;
use sp_core::Get;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedDiv, Convert},
	SaturatedConversion, Saturating,
};
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
	use sp_runtime::traits::{AccountIdConversion, SaturatedConversion};
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
			+ Into<U256>
			+ From<<Self::Time as Time>::Moment>;

		/// The claim buffer time.
		#[pallet::constant]
		type ClaimBuffer: Get<<Self::Time as Time>::Moment>;

		/// The claim period.
		#[pallet::constant]
		type ClaimPeriod: Get<<Self::Time as Time>::Moment>;

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

		/// The maximum number of reward claims.
		#[pallet::constant]
		type MaxRewardClaims: Get<u32>;

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

		/// Conversion from submitted value to an amount.
		type ValueConverter: Convert<ValueOf<Self>, Option<Self::Amount>>;

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
		FeedOf<T>,
	>;
	#[pallet::storage]
	pub type FeedsWithFunding<T> =
		StorageValue<_, BoundedVec<FeedIdOf<T>, <T as Config>::MaxFundedFeeds>, ValueQuery>;
	#[pallet::storage]
	pub type QueryIdFromDataFeedId<T> = StorageMap<_, Blake2_128Concat, FeedIdOf<T>, QueryIdOf<T>>;
	#[pallet::storage]
	pub type QueryIdsWithFunding<T> =
		StorageValue<_, BoundedVec<QueryIdOf<T>, <T as Config>::MaxFundedFeeds>, ValueQuery>;
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
		/// Timestamp too old to claim tip.
		ClaimPeriodExpired,
		FeeCalculationError,
		/// Feed must not be set up already.
		FeedAlreadyExists,
		/// No funds available for this feed or insufficient balance for all submitted timestamps.
		InsufficientFeedBalance,
		IntervalCalculationError,
		/// Amount must be greater than zero.
		InvalidAmount,
		/// Claimer must be the reporter.
		InvalidClaimer,
		/// Feed not set up.
		InvalidFeed,
		/// Interval must be greater than zero.
		InvalidInterval,
		/// Reward must be greater than zero.
		InvalidReward,
		/// Query identifier must be a hash of bytes data.
		InvalidQueryId,
		/// No value exists at timestamp.
		InvalidTimestamp,
		/// Window must be less than interval length.
		InvalidWindow,
		/// The maximum number of feeds have been funded.
		MaxFeedsFunded,
		/// The maximum number of reward claims has been reached,
		MaxRewardClaimsReached,
		/// The maximum number of tips has been reached,
		MaxTipsReached,
		/// No tips submitted for this query identifier.
		NoTipsSubmitted,
		PriceChangeCalculationError,
		/// Price threshold not met.
		PriceThresholdNotMet,
		/// Timestamp not eligible for tip.
		TimestampIneligibleForTip,
		/// Tip already claimed.
		TipAlreadyClaimed,
		/// Tip earned by previous submission.
		TipAlreadyEarned,
		ValueConversionError,
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
				cumulative_reward.saturating_accrue(Self::get_onetime_tip_amount(
					query_id, timestamp, &reporter,
				)?);
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
			Self::add_staking_rewards(fee)?;
			if Self::get_current_tip(query_id) == <AmountOf<T>>::default() {
				// todo: replace with if let once guards stable
				match <QueryIdsWithFundingIndex<T>>::get(query_id) {
					Some(index) if index != 0 => {
						let idx: usize = index as usize - 1;
						// Replace unfunded feed in array with last element
						<QueryIdsWithFunding<T>>::mutate(|query_ids_with_funding| {
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
			origin: OriginFor<T>,
			feed_id: FeedIdOf<T>,
			query_id: QueryIdOf<T>,
			timestamps: BoundedVec<TimestampOf<T>, T::MaxClaimTimestamps>,
		) -> DispatchResult {
			let reporter = ensure_signed(origin)?;

			let mut feed = <DataFeeds<T>>::get(query_id, feed_id).ok_or(Error::<T>::InvalidFeed)?;
			let balance = feed.details.balance;
			ensure!(balance > AmountOf::<T>::default(), Error::<T>::InsufficientFeedBalance);

			let mut cumulative_reward = AmountOf::<T>::default();
			for timestamp in &timestamps {
				ensure!(
					T::Time::now().saturating_sub(*timestamp) > T::ClaimBuffer::get(),
					Error::<T>::ClaimBufferNotPassed
				);
				ensure!(
					Some(&reporter) ==
						Self::get_reporter_by_timestamp(query_id, *timestamp).as_ref(),
					Error::<T>::InvalidClaimer
				);
				cumulative_reward
					.saturating_accrue(Self::_get_reward_amount(feed_id, query_id, *timestamp)?);

				if cumulative_reward >= balance {
					ensure!(
						Some(timestamp) == timestamps.last(),
						Error::<T>::InsufficientFeedBalance
					);
					cumulative_reward = balance;
					// Adjust currently funded feeds
					<FeedsWithFunding<T>>::mutate(|feeds_with_funding| {
						if feeds_with_funding.len() > 1 {
							let index = feed.details.feeds_with_funding_index - 1;
							// Replace unfunded feed in array with last element
							feeds_with_funding[index as usize] =
								feeds_with_funding[feeds_with_funding.len() - 1];
							let feed_id_last_funded = feeds_with_funding[index as usize];
							match <QueryIdFromDataFeedId<T>>::get(feed_id_last_funded) {
								None => todo!(),
								Some(query_id_last_funded) => {
									<DataFeeds<T>>::mutate(
										query_id_last_funded,
										feed_id_last_funded,
										|f| {
											if let Some(f) = f {
												f.details.feeds_with_funding_index = index + 1
											}
										},
									);
								},
							}
						}
						feeds_with_funding.pop();
					});
					feed.details.feeds_with_funding_index = 0;
				}
				feed.reward_claimed
					.try_insert(*timestamp, true)
					.map_err(|_| Error::<T>::MaxRewardClaimsReached)?;
			}

			feed.details.balance -= cumulative_reward;
			<DataFeeds<T>>::set(query_id, feed_id, Some(feed));
			let fee = (cumulative_reward.saturating_mul(T::Fee::get().into()))
				.checked_div(&1000u16.into())
				.ok_or(Error::<T>::FeeCalculationError)?;
			T::Token::transfer(
				&T::PalletId::get().into_account_truncating(),
				&reporter,
				cumulative_reward - fee,
				true,
			)?;
			Self::add_staking_rewards(fee)?;
			Self::deposit_event(Event::TipClaimed {
				feed_id,
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
		#[pallet::call_index(3)]
		pub fn fund_feed(
			origin: OriginFor<T>,
			feed_id: FeedIdOf<T>,
			query_id: QueryIdOf<T>,
			amount: AmountOf<T>,
		) -> DispatchResult {
			let feed_funder = ensure_signed(origin)?;
			Self::_fund_feed(feed_funder, feed_id, query_id, amount)
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
			origin: OriginFor<T>,
			query_id: QueryIdOf<T>,
			reward: AmountOf<T>,
			start_time: TimestampOf<T>,
			interval: TimestampOf<T>,
			window: TimestampOf<T>,
			price_threshold: u16,
			reward_increase_per_second: AmountOf<T>,
			query_data: QueryDataOf<T>,
			amount: AmountOf<T>,
		) -> DispatchResult {
			let feed_creator = ensure_signed(origin)?;
			ensure!(
				query_id == HasherOf::<T>::hash(query_data.as_ref()),
				Error::<T>::InvalidQueryId
			);
			let feed_id = HasherOf::<T>::hash(
				&contracts::Abi::default()
					.fixed_bytes(query_id.as_ref())
					.uint(reward)
					.uint(start_time.saturated_into::<u128>())
					.uint(interval.saturated_into::<u128>())
					.uint(window.saturated_into::<u128>())
					.uint(price_threshold as u128)
					.uint(reward_increase_per_second.into())
					.encode(),
			);
			let feed = <DataFeeds<T>>::get(query_id, feed_id);
			ensure!(feed.is_none(), Error::<T>::FeedAlreadyExists);
			ensure!(reward > <AmountOf<T>>::default(), Error::<T>::InvalidReward);
			ensure!(interval > <TimestampOf<T>>::default(), Error::<T>::InvalidInterval);
			ensure!(window < interval, Error::<T>::InvalidWindow);

			let feed = FeedDetailsOf::<T> {
				reward,
				balance: <AmountOf<T>>::default(),
				start_time,
				interval,
				window,
				price_threshold,
				reward_increase_per_second,
				feeds_with_funding_index: 0,
			};
			<CurrentFeeds<T>>::try_mutate(query_id, |maybe| -> Result<(), DispatchError> {
				match maybe {
					None => {
						let mut feeds = BoundedVec::default();
						feeds.try_push(feed_id).map_err(|_| Error::<T>::MaxFeedsFunded)?;
						*maybe = Some(feeds);
					},
					Some(feeds) => {
						feeds.try_push(feed_id).map_err(|_| Error::<T>::MaxFeedsFunded)?;
					},
				}
				Ok(())
			})?;
			<QueryIdFromDataFeedId<T>>::insert(feed_id, query_id);
			Self::store_data(query_id, &query_data);
			<DataFeeds<T>>::insert(
				query_id,
				feed_id,
				FeedOf::<T> { details: feed, reward_claimed: BoundedBTreeMap::default() },
			);
			Self::deposit_event(Event::NewDataFeed {
				query_id,
				feed_id,
				query_data,
				feed_creator: feed_creator.clone(),
			});
			if amount > <AmountOf<T>>::default() {
				Self::_fund_feed(feed_creator, feed_id, query_id, amount)?;
			}
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
				query_id == HasherOf::<T>::hash(query_data.as_ref()),
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
				let len = <QueryIdsWithFunding<T>>::try_mutate(
					|query_ids| -> Result<u32, DispatchError> {
						query_ids.try_push(query_id).map_err(|_| Error::<T>::MaxFeedsFunded)?;
						Ok(query_ids.len() as u32)
					},
				)?;
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
				// todo: refactor to remove saturated_into()
				(timestamp.saturating_sub(staker.reporter_last_timestamp))
					.saturated_into::<u128>()
					.saturating_mul(1_000) >
					(T::ReportingLock::get().saturated_into::<u128>().saturating_mul(1_000))
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
					.map_or(true, |r| !r.reporter_by_timestamp.contains_key(&timestamp)),
				Error::<T>::TimestampAlreadyReported
			);

			// Update number of timestamps, value for given timestamp, and reporter for timestamp
			let mut report = report.unwrap_or_else(Report::new);
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
			staker.reports_submitted = staker.reports_submitted.saturating_add(1);
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
						&_query_id,
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
			_dispute_id: DisputeIdOf<T>,
			_supports: Option<bool>,
		) -> DispatchResult {
			let _voter = ensure_signed(origin)?;
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
			_reporter: AccountIdOf<T>,
			_amount: Amount,
			_address: Address,
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
			_reporter: AccountIdOf<T>,
			_amount: Amount,
			_address: Address,
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
			_reporter: Address,
			_recipient: Address,
			_amount: Amount,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}

		#[pallet::call_index(14)]
		pub fn report_invalid_dispute(
			origin: OriginFor<T>,
			_dispute_id: DisputeIdOf<T>,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}

		#[pallet::call_index(15)]
		pub fn slash_dispute_initiator(
			origin: OriginFor<T>,
			_dispute_id: DisputeIdOf<T>,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn send_xcm(
			destination: impl Into<MultiLocation>,
			message: Xcm<()>,
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
		_query_id: QueryIdOf<T>,
		_timestamp: TimestampOf<T>,
	) -> Option<BlockNumberOf<T>> {
		todo!()
	}

	fn add_staking_rewards(amount: AmountOf<T>) -> DispatchResult {
		let pallet_id = T::PalletId::get();
		let source = pallet_id.into_account_truncating();
		let dest = pallet_id.into_sub_account_truncating(b"staking");
		T::Token::transfer(&source, &dest, amount, true)?;
		Ok(())
	}

	fn _fund_feed(
		feed_funder: AccountIdOf<T>,
		feed_id: FeedIdOf<T>,
		query_id: QueryIdOf<T>,
		amount: AmountOf<T>,
	) -> DispatchResult {
		let Some(mut feed) = <DataFeeds<T>>::get(query_id, feed_id) else {
			return Err(Error::<T>::InvalidFeed.into());
		};

		ensure!(amount > <AmountOf<T>>::default(), Error::<T>::InvalidAmount);
		feed.details.balance = feed.details.balance.saturating_add(amount);
		T::Token::transfer(
			&feed_funder,
			&T::PalletId::get().into_account_truncating(),
			amount,
			true,
		)?;
		// Add to array of feeds with funding
		if feed.details.feeds_with_funding_index == 0 &&
			feed.details.balance > <AmountOf<T>>::default()
		{
			let index = <FeedsWithFunding<T>>::try_mutate(
				|feeds_with_funding| -> Result<usize, DispatchError> {
					feeds_with_funding.try_push(feed_id).map_err(|_| Error::<T>::MaxFeedsFunded)?;
					Ok(feeds_with_funding.len())
				},
			)?;
			feed.details.feeds_with_funding_index = index.saturated_into::<u32>();
		}
		let feed_details = feed.details.clone();
		<DataFeeds<T>>::insert(query_id, feed_id, feed);
		<UserTipsTotal<T>>::mutate(&feed_funder, |total| total.saturating_add(amount));
		Self::deposit_event(Event::DataFeedFunded {
			feed_id,
			query_id,
			amount,
			feed_funder,
			feed_details,
		});

		Ok(())
	}

	/// Read current data feeds.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// # Returns
	/// Feed identifiers for query identifier.
	pub fn get_current_feeds(query_id: QueryIdOf<T>) -> Vec<FeedIdOf<T>> {
		<CurrentFeeds<T>>::get(query_id).map_or_else(Vec::default, |f| f.to_vec())
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
					Self::retrieve_data(query_id, timestamp).map(|value| (value, timestamp))
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
			.and_then(|r| r.value_by_timestamp.last_key_value().map(|kv| kv.1.clone()))
	}

	/// Retrieves the latest value for the query identifier before the specified timestamp.
	/// # Arguments
	/// * `query_id` - The query identifier to look up the value for.
	/// * `timestamp` - The timestamp before which to search for the latest value.
	/// # Returns
	/// The value retrieved and its timestamp, if found.
	pub fn get_data_before(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<(ValueOf<T>, TimestampOf<T>)> {
		Self::get_index_for_data_before(query_id, timestamp)
			.and_then(|index| Self::get_timestamp_by_query_id_and_index(query_id, index))
			.and_then(|timestamp_retrieved| {
				Self::retrieve_data(query_id, timestamp_retrieved)
					.map(|value| (value, timestamp_retrieved))
			})
	}

	/// Read a specific data feed.
	/// # Arguments
	/// * `query_id` - Unique feed identifier of parameters.
	/// # Returns
	/// Details of the specified feed.
	pub fn get_data_feed(feed_id: FeedIdOf<T>) -> Option<FeedDetailsOf<T>> {
		<QueryIdFromDataFeedId<T>>::get(feed_id)
			.and_then(|query_id| <DataFeeds<T>>::get(query_id, feed_id))
			.map(|f| f.details)
	}

	/// Read currently funded feed details.
	/// # Arguments
	/// * `query_id` - Unique feed identifier of parameters.
	/// # Returns
	/// Details of the specified feed.
	pub fn get_funded_feed_details(
		_feed_id: FeedIdOf<T>,
	) -> Vec<(FeedDetailsOf<T>, QueryDataOf<T>)> {
		Self::get_funded_feeds()
			.into_iter()
			.filter_map(|feed_id| {
				Self::get_data_feed(feed_id).and_then(|feed_detail| {
					Self::get_query_id_from_feed_id(feed_id).and_then(|query_id| {
						Self::get_query_data(query_id).map(|query_data| (feed_detail, query_data))
					})
				})
			})
			.collect()
	}

	/// Read currently funded feeds.
	/// # Returns
	/// The currently funded feeds
	pub fn get_funded_feeds() -> Vec<FeedIdOf<T>> {
		<FeedsWithFunding<T>>::get().to_vec()
	}

	/// Read query identifiers with current one-time tips.
	/// # Returns
	/// Query identifiers with current one-time tips.
	pub fn get_funded_query_ids() -> Vec<QueryIdOf<T>> {
		<QueryIdsWithFunding<T>>::get().to_vec()
	}

	/// Read currently funded single tips with query data.
	/// # Returns
	/// The current single tips.
	pub fn get_funded_single_tips_info() -> Vec<(QueryDataOf<T>, AmountOf<T>)> {
		Self::get_funded_query_ids()
			.into_iter()
			.filter_map(|query_id| {
				Self::get_query_data(query_id)
					.map(|query_data| (query_data, Self::get_current_tip(query_id)))
			})
			.collect()
	}

	/// Retrieves latest index of data before the specified timestamp for the query identifier.
	/// # Arguments
	/// * `query_id` - The query identifier to look up the index for.
	/// * `timestamp` - The timestamp before which to search for the latest index.
	/// # Returns
	/// Whether the index was found along with the latest index found before the supplied timestamp.
	pub fn get_index_for_data_before(
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
		None
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

	/// Read the number of past tips for a query identifier.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// # Returns
	/// The number of past tips.
	pub fn get_past_tip_count(query_id: QueryIdOf<T>) -> u32 {
		<Tips<T>>::get(query_id).map_or(0, |t| t.len() as u32)
	}

	/// Read the past tips for a query identifier.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// # Returns
	/// All past tips.
	pub fn get_past_tips(query_id: QueryIdOf<T>) -> Vec<Tip<AmountOf<T>, TimestampOf<T>>> {
		<Tips<T>>::get(query_id).map_or_else(Vec::default, |t| t.to_vec())
	}

	/// Read a past tip for a query identifier and index.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// * `index` - The index of the tip.
	/// # Returns
	/// The past tip, if found.
	pub fn get_past_tip_by_index(
		query_id: QueryIdOf<T>,
		index: u32,
	) -> Option<Tip<AmountOf<T>, TimestampOf<T>>> {
		<Tips<T>>::get(query_id).and_then(|t| t.get(index as usize).cloned())
	}

	pub fn get_query_data(query_id: QueryIdOf<T>) -> Option<QueryDataOf<T>> {
		<QueryData<T>>::get(query_id)
	}

	/// Look up a query identifier from a data feed identifier.
	/// # Arguments
	/// * `feed_id` - Data feed unique identifier.
	/// # Returns
	/// Corresponding query identifier, if found.
	pub fn get_query_id_from_feed_id(feed_id: FeedIdOf<T>) -> Option<QueryIdOf<T>> {
		<QueryIdFromDataFeedId<T>>::get(feed_id)
	}

	/// Returns reporter and whether a value was disputed for a given query identifier and timestamp.
	/// # Arguments
	/// * `query_id` - The query identifier to look up.
	/// * `timestamp` - The timestamp of the value to look up.
	/// # Returns
	/// The reporter who submitted the value and whether the value was disputed, provided a value exists.
	pub fn get_report_details(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<(AccountIdOf<T>, bool)> {
		<Reports<T>>::get(query_id).and_then(|report| {
			report.reporter_by_timestamp.get(&timestamp).map(|reporter| {
				(reporter.clone(), report.is_disputed.get(&timestamp).cloned().unwrap_or_default())
			})
		})
	}

	/// Returns the reporter who submitted a value for a query identifier at a specific time.
	/// # Arguments
	/// * `query_id` - The identifier of the specific data feed.
	/// * `timestamp` - The timestamp to find a corresponding reporter for.
	/// # Returns
	/// Identifier of the reporter who reported the value for the query identifier at the given timestamp.
	pub fn get_reporter_by_timestamp(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<AccountIdOf<T>> {
		<Reports<T>>::get(query_id)
			.and_then(|report| report.reporter_by_timestamp.get(&timestamp).cloned())
	}

	/// Returns the timestamp of the reporter's last submission.
	/// # Arguments
	/// * `reporter` - The identifier of the reporter.
	/// # Returns
	/// The timestamp of the reporter's last submission, if one exists.
	pub fn get_reporter_last_timestamp(reporter: AccountIdOf<T>) -> Option<TimestampOf<T>> {
		<StakerDetails<T>>::get(reporter).map(|stake_info| stake_info.reporter_last_timestamp)
	}

	/// Returns the reporting lock time, the amount of time a reporter must wait to submit again.
	/// # Returns
	/// The reporting lock time.
	pub fn get_reporting_lock() -> TimestampOf<T> {
		T::ReportingLock::get()
	}

	/// Returns the number of values submitted by a specific reporter.
	/// # Arguments
	/// * `reporter` - The identifier of the reporter.
	/// # Returns
	/// The number of values submitted by the given reporter.
	pub fn get_reports_submitted_by_address(reporter: AccountIdOf<T>) -> u128 {
		<StakerDetails<T>>::get(reporter)
			.map(|stake_info| stake_info.reports_submitted)
			.unwrap_or_default()
	}

	/// Returns the number of values submitted to a specific query identifier by a specific reporter.
	/// # Arguments
	/// * `reporter` - The identifier of the reporter.
	/// * `query_id` - Identifier of the specific data feed.
	/// # Returns
	/// The number of values submitted by the given reporter to the given query identifier.
	pub fn get_reports_submitted_by_address_and_query_id(
		reporter: AccountIdOf<T>,
		query_id: QueryIdOf<T>,
	) -> u128 {
		<StakerDetails<T>>::get(reporter)
			.and_then(|stake_info| stake_info.reports_submitted_by_query_id.get(&query_id).copied())
			.unwrap_or_default()
	}

	fn _get_reward_amount(
		feed_id: FeedIdOf<T>,
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Result<AmountOf<T>, Error<T>> {
		ensure!(
			T::Time::now().saturating_sub(timestamp) < T::ClaimPeriod::get(),
			Error::<T>::ClaimPeriodExpired
		);

		let feed = <DataFeeds<T>>::get(query_id, feed_id).ok_or(Error::<T>::InvalidFeed)?;
		ensure!(!feed.reward_claimed.get(&timestamp).unwrap_or(&false), Error::TipAlreadyClaimed);
		let n = (timestamp.saturating_sub(feed.details.start_time))
			.checked_div(&feed.details.interval)
			.ok_or(Error::<T>::IntervalCalculationError)?; // finds closest interval n to timestamp
		let c = feed.details.start_time + feed.details.interval * n; // finds start timestamp c of interval n
		let value_retrieved = Self::retrieve_data(query_id, timestamp);
		ensure!(value_retrieved.as_ref().map_or(0, |v| v.len()) != 0, Error::<T>::InvalidTimestamp);
		let (value_retrieved_before, timestamp_before) =
			Self::get_data_before(query_id, timestamp).unwrap_or_default();
		let mut price_change = 0; // price change from last value to current value
		if feed.details.price_threshold != 0 {
			let v1 = T::ValueConverter::convert(
				value_retrieved.expect("value retrieved checked above; qed"),
			)
			.ok_or(Error::<T>::ValueConversionError)?;
			let v2 = T::ValueConverter::convert(value_retrieved_before)
				.ok_or(Error::<T>::ValueConversionError)?;
			if v2 == <AmountOf<T>>::default() {
				price_change = 10_000;
			} else if v1 >= v2 {
				price_change = (<AmountOf<T>>::from(10_000u16)
					.saturating_mul(v1.saturating_sub(v2)))
				.checked_div(&v2)
				.ok_or(Error::<T>::PriceChangeCalculationError)?
				.saturated_into();
			} else {
				price_change = (<AmountOf<T>>::from(10_000u16)
					.saturating_mul(v2.saturating_sub(v1)))
				.checked_div(&v2)
				.ok_or(Error::<T>::PriceChangeCalculationError)?
				.saturated_into();
			}
		}
		let mut reward_amount = feed.details.reward;
		let time_diff = timestamp.saturating_sub(c); // time difference between report timestamp and start of interval

		// ensure either report is first within a valid window, or price change threshold is met
		if time_diff < feed.details.window && timestamp_before < c {
			// add time based rewards if applicable
			reward_amount = reward_amount.saturating_add(
				feed.details.reward_increase_per_second.saturating_mul(time_diff.into()),
			);
		} else {
			ensure!(price_change > feed.details.price_threshold, Error::<T>::PriceThresholdNotMet);
		}

		if feed.details.balance < reward_amount {
			reward_amount = feed.details.balance;
		}
		Ok(reward_amount)
	}

	/// Read potential reward for a set of oracle submissions.
	/// # Arguments
	/// * `feed_id` - Data feed unique identifier.
	/// * `query_id` - Identifier of reported data.
	/// * `timestamps` - Timestamps of oracle submissions.
	/// # Returns
	/// Potential reward for a set of oracle submissions.
	pub fn get_reward_amount(
		feed_id: FeedIdOf<T>,
		query_id: QueryIdOf<T>,
		timestamps: Vec<TimestampOf<T>>,
	) -> AmountOf<T> {
		// todo: use boundedvec for timestamps

		let Some(feed) = <DataFeeds<T>>::get(query_id, feed_id) else { return <AmountOf<T>>::default()};
		let mut cumulative_reward = <AmountOf<T>>::default();
		for timestamp in timestamps {
			cumulative_reward.saturating_accrue(
				Self::_get_reward_amount(feed_id, query_id, timestamp).unwrap_or_default(),
			)
		}
		if cumulative_reward > feed.details.balance {
			cumulative_reward = feed.details.balance;
		}
		cumulative_reward.saturating_reduce(
			(cumulative_reward.saturating_mul(T::Fee::get().into())) / 1000u16.into(),
		);
		cumulative_reward
	}

	/// Read whether a reward has been claimed.
	/// # Arguments
	/// * `feed_id` - Data feed unique identifier.
	/// * `query_id` - Identifier of reported data.
	/// * `timestamp` - Timestamp of reported data.
	/// # Returns
	/// Whether a reward has been claimed, if timestamp exists.
	pub fn get_reward_claimed_status(
		feed_id: FeedIdOf<T>,
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<bool> {
		<DataFeeds<T>>::get(query_id, feed_id)
			.and_then(|f| f.reward_claimed.get(&timestamp).copied())
	}

	/// Read whether rewards have been claimed.
	/// # Arguments
	/// * `feed_id` - Data feed unique identifier.
	/// * `query_id` - Identifier of reported data.
	/// * `timestamps` - Timestamps of oracle submissions.
	/// # Returns
	/// Whether rewards have been claimed.
	pub fn get_reward_claim_status_list(
		feed_id: FeedIdOf<T>,
		query_id: QueryIdOf<T>,
		timestamps: Vec<TimestampOf<T>>,
	) -> Vec<Option<bool>> {
		// todo: use boundedvec for timestamps
		<DataFeeds<T>>::get(query_id, feed_id).map_or_else(Vec::default, |feed| {
			timestamps
				.into_iter()
				.map(|timestamp| feed.reward_claimed.get(&timestamp).copied())
				.collect()
		})
	}

	/// Returns the amount required to report oracle values.
	/// # Returns
	/// The stake amount.
	pub fn get_stake_amount() -> AmountOf<T> {
		<StakeAmount<T>>::get()
	}

	/// Returns all information about a staker.
	/// # Arguments
	/// * `staker` - The identifier of the staker inquiring about.
	/// # Returns
	/// All information about a staker, if found.
	pub fn get_staker_info(staker: AccountIdOf<T>) -> Option<StakeInfoOf<T>> {
		<StakerDetails<T>>::get(staker)
	}

	/// Returns the timestamp for the last value of any identifier from the oracle.
	/// # Returns
	/// The timestamp of the last oracle value.
	pub fn get_time_of_last_new_value() -> Option<TimestampOf<T>> {
		<TimeOfLastNewValue<T>>::get()
	}

	/// Gets the timestamp for the value based on their index.
	/// # Arguments
	/// * `query_id` - The query identifier to look up.
	/// * `index` - The value index to look up.
	/// # Returns
	/// A timestamp if found.
	pub fn get_timestamp_by_query_id_and_index(
		query_id: QueryIdOf<T>,
		index: usize,
	) -> Option<TimestampOf<T>> {
		<Reports<T>>::get(query_id).and_then(|report| report.timestamps.get(index).copied())
	}

	/// Returns the index of a reporter timestamp in the timestamp array for a specific query identifier.
	/// # Arguments
	/// * `query_id` - Unique identifier of the data feed.
	/// * `timestamp` - The timestamp to find within the available timestamps.
	/// # Returns
	/// The index of the reporter timestamp within the available timestamps for specific query identifier.
	pub fn get_timestamp_index_by_timestamp(
		query_id: QueryIdOf<T>,
		timestamp: TimestampOf<T>,
	) -> Option<u32> {
		<Reports<T>>::get(query_id)
			.and_then(|report| report.timestamp_index.get(&timestamp).copied())
	}

	/// Read the total amount of tips paid by a user.
	/// # Arguments
	/// * `user` - Address of user to query.
	/// # Returns
	/// Total amount of tips paid by a user.
	pub fn get_tips_by_address(user: AccountIdOf<T>) -> AmountOf<T> {
		<UserTipsTotal<T>>::get(user)
	}

	/// Returns the total amount staked for reporting.
	/// # Returns
	/// The total amount of token staked.
	pub fn get_total_stake_amount() -> AmountOf<T> {
		<TotalStakeAmount<T>>::get()
	}

	/// Returns the total number of current stakers.
	/// # Returns
	/// The total number of current stakers.
	pub fn get_total_stakers() -> u128 {
		<TotalStakers<T>>::get()
	}

	/// Counts the number of values that have been submitted for the query identifier.
	/// # Arguments
	/// * `query_id` - The query identifier to look up.
	/// # Returns
	/// Count of the number of values received for the query identifier.
	pub fn get_new_value_count_by_query_id(query_id: QueryIdOf<T>) -> usize {
		<Reports<T>>::get(query_id).map_or(usize::default(), |r| r.timestamps.len())
	}

	/// Returns whether a given value is disputed.
	/// # Arguments
	/// * `query_id` - Unique identifier of the data feed.
	/// * `timestamp` - Timestamp of the value.
	/// # Returns
	/// Whether the value is disputed.
	pub fn is_in_dispute(query_id: QueryIdOf<T>, timestamp: TimestampOf<T>) -> bool {
		<Reports<T>>::get(query_id)
			.map_or(false, |report| report.is_disputed.contains_key(&timestamp))
	}

	/// Retrieve value from the oracle based on timestamp.
	/// # Arguments
	/// * `query_id` - Identifier being requested.
	/// * `timestamp` - Timestamp to retrieve data/value from.
	/// # Returns
	/// Value for timestamp submitted, if found.
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
		_query_id: QueryIdOf<T>,
		_timestamp: TimestampOf<T>,
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
		_query_id: QueryIdOf<T>,
		_timestamp: TimestampOf<T>,
		_max_age: TimestampOf<T>,
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

#![cfg_attr(not(feature = "std"), no_std)]

pub use crate::xcm::{ContractLocation, LocationToAccount, LocationToOrigin};
use codec::Encode;
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure,
	traits::{fungible::Transfer, EnsureOrigin, Len, Time},
};
pub use pallet::*;
use sp_core::Get;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedDiv, Convert},
	SaturatedConversion, Saturating,
};
use sp_std::vec::Vec;
pub use traits::{UsingTellor, Xcm};
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
mod impls;
pub mod traits;
mod types;
pub mod xcm;

pub const MINUTE_IN_MILLISECONDS: u64 = 60 * 1_000;
pub const HOUR_IN_MILLISECONDS: u64 = 60 * MINUTE_IN_MILLISECONDS;
pub const DAY_IN_MILLISECONDS: u64 = 24 * HOUR_IN_MILLISECONDS;
pub const WEEK_IN_MILLISECONDS: u64 = 7 * DAY_IN_MILLISECONDS;

#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use super::{
		contracts::{governance, registry},
		types::*,
		xcm::{self, ethereum_xcm},
		*,
	};
	use crate::{types::oracle::Report, xcm::ContractLocation, Tip};
	use ::xcm::latest::prelude::*;
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::traits::{
			AtLeast32BitUnsigned, CheckEqual, Hash, MaybeDisplay, MaybeSerializeDeserialize,
			Member, SimpleBitOps,
		},
		traits::{
			fungible::{Inspect, Transfer},
			PalletInfoAccess,
		},
		PalletId,
	};
	use frame_system::pallet_prelude::*;
	use sp_core::{bounded::BoundedBTreeMap, U256};
	use sp_runtime::traits::{AccountIdConversion, CheckedAdd, SaturatedConversion};
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
		type DisputeId: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ Debug
			+ MaybeDisplay
			+ Ord
			+ MaxEncodedLen
			+ Into<U256>;

		/// Percentage, 1000 is 100%, 50 is 5%, etc
		#[pallet::constant]
		type Fee: Get<u16>;

		/// The location of the governance controller contract.
		#[pallet::constant]
		type Governance: Get<ContractLocation>;

		/// Origin that handles dispute resolution (governance).
		type GovernanceOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

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

		/// The maximum number of vote rounds (per dispute).
		#[pallet::constant]
		type MaxVoteRounds: Get<u32>;

		/// The identifier of the pallet within the runtime.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The local parachain's own identifier.
		#[pallet::constant]
		type ParachainId: Get<ParaId>;

		type Price: AtLeast32BitUnsigned + Copy + Default;

		/// Origin that manages registration and deregistration from the controller contracts.
		type RegistrationOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// The location of the registry controller contract.
		#[pallet::constant]
		type Registry: Get<ContractLocation>;

		/// Base amount of time before a reporter is able to submit a value again.
		#[pallet::constant]
		type ReportingLock: Get<TimestampOf<Self>>;

		/// The location of the staking controller contract.
		#[pallet::constant]
		type Staking: Get<ContractLocation>;

		/// Origin that handles staking.
		type StakingOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// The on-chain time provider.
		type Time: Time;

		type Token: Inspect<Self::AccountId, Balance = Self::Amount> + Transfer<Self::AccountId>;

		/// Conversion from submitted value (bytes) to a price for price threshold evaluation.
		type ValueConverter: Convert<Vec<u8>, Option<Self::Price>>;

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
	#[pallet::getter(fn query_ids_with_funding_index)]
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
	pub type DisputeInfo<T> = StorageMap<_, Blake2_128Concat, DisputeIdOf<T>, DisputeOf<T>>;
	#[pallet::storage]
	pub type VoteCount<T> = StorageValue<_, DisputeIdOf<T>, ValueQuery>;
	#[pallet::storage]
	pub type VoteInfo<T> = StorageMap<_, Blake2_128Concat, DisputeIdOf<T>, VoteOf<T>>;
	#[pallet::storage]
	pub type VoteRounds<T> = StorageMap<
		_,
		Blake2_128Concat,
		VoteIdOf<T>,
		BoundedVec<DisputeIdOf<T>, <T as Config>::MaxVoteRounds>,
	>;
	// Query Data
	#[pallet::storage]
	pub type QueryData<T> = StorageMap<_, Blake2_128Concat, QueryIdOf<T>, QueryDataOf<T>>;
	// XCM
	#[pallet::storage]
	pub type XcmConfig<T> = StorageValue<_, xcm::XcmConfig>;

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
		InvalidIndex,
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
		/// The maximum number of timestamps has been reached.
		MaxTimestampsReached,
		/// Still in reporter time lock, please wait!
		ReporterTimeLocked,
		ReportingLockCalculationError,
		/// Timestamp already reported.
		TimestampAlreadyReported,

		// Governance
		/// Dispute must be started within reporting lock time.
		DisputeReportingPeriodExpired,
		/// The maximum number of disputes has been reached.
		MaxDisputesReached,
		/// The maximum number of vote rounds has been reached.
		MaxVoteRoundsReached,
		/// Dispute initiator is not a reporter.
		NotReporter,
		/// No value exists at given timestamp.
		NoValueExists,
		// XCM
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
			fees: Box<MultiAsset>,
			weight_limit: WeightLimit,
			require_weight_at_most: u64,
			gas_limit: u128,
		) -> DispatchResult {
			T::RegistrationOrigin::ensure_origin(origin)?;

			<StakeAmount<T>>::set(stake_amount);
			<XcmConfig<T>>::set(Some(xcm::XcmConfig {
				fees: *fees.clone(),
				weight_limit: weight_limit.clone(),
				require_weight_at_most,
				gas_limit,
			}));

			let registry_contract = T::Registry::get();
			let message = xcm::transact(
				*fees,
				weight_limit,
				require_weight_at_most,
				ethereum_xcm::transact(
					registry_contract.address,
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
			Self::send_xcm(registry_contract.para_id, message)?;

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
				&T::PalletId::get().into_account_truncating(),
				&reporter,
				// todo: safe math
				cumulative_reward - fee,
				false,
			)?;
			Self::add_staking_rewards(fee)?;
			if Self::get_current_tip(query_id) == <AmountOf<T>>::default() {
				// todo: replace with if let once guards stable
				match <QueryIdsWithFundingIndex<T>>::get(query_id) {
					Some(index) if index != 0 => {
						// todo: safe math
						let idx: usize = index as usize - 1;
						// Replace unfunded feed in array with last element
						<QueryIdsWithFunding<T>>::try_mutate(
							|query_ids_with_funding| -> DispatchResult {
								// todo: safe math
								let qid = *query_ids_with_funding
									.last()
									.ok_or(Error::<T>::InvalidIndex)?;
								query_ids_with_funding
									.get_mut(idx)
									.map(|i| *i = qid)
									.ok_or(Error::<T>::InvalidIndex)?;
								let query_id_last_funded = query_ids_with_funding
									.get(idx)
									.ok_or(Error::<T>::InvalidIndex)?;
								<QueryIdsWithFundingIndex<T>>::set(
									query_id_last_funded,
									// todo: safe math
									Some((idx + 1).saturated_into()),
								);
								<QueryIdsWithFundingIndex<T>>::remove(query_id);
								query_ids_with_funding.pop();
								Ok(())
							},
						)?;
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
					<FeedsWithFunding<T>>::try_mutate(|feeds_with_funding| -> DispatchResult {
						if feeds_with_funding.len() > 1 {
							// todo: safe math
							let index = feed.details.feeds_with_funding_index - 1;
							// Replace unfunded feed in array with last element
							let fid = *feeds_with_funding.last().ok_or(Error::<T>::InvalidIndex)?;
							feeds_with_funding
								.get_mut(index as usize)
								.map(|i| *i = fid)
								.ok_or(Error::<T>::InvalidIndex)?;
							let feed_id_last_funded = feeds_with_funding
								.get(index as usize)
								.ok_or(Error::<T>::InvalidIndex)?;
							match <QueryIdFromDataFeedId<T>>::get(feed_id_last_funded) {
								None => todo!(),
								Some(query_id_last_funded) => {
									<DataFeeds<T>>::mutate(
										query_id_last_funded,
										feed_id_last_funded,
										|f| {
											if let Some(f) = f {
												// todo: safe math
												f.details.feeds_with_funding_index = index + 1
											}
										},
									);
								},
							}
						}
						feeds_with_funding.pop();
						Ok(())
					})?;
					feed.details.feeds_with_funding_index = 0;
				}
				feed.reward_claimed
					.try_insert(*timestamp, true)
					.map_err(|_| Error::<T>::MaxRewardClaimsReached)?;
			}

			feed.details.balance.saturating_reduce(cumulative_reward);
			<DataFeeds<T>>::set(query_id, feed_id, Some(feed));
			let fee = (cumulative_reward.saturating_mul(T::Fee::get().into()))
				.checked_div(&1000u16.into())
				.ok_or(Error::<T>::FeeCalculationError)?;
			T::Token::transfer(
				&T::PalletId::get().into_account_truncating(),
				&reporter,
				// todo: safe math
				cumulative_reward - fee,
				false,
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
			<CurrentFeeds<T>>::try_mutate(query_id, |maybe| -> DispatchResult {
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
								timestamp: T::Time::now().saturating_add(1u8.into()),
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
								last_tip.timestamp = T::Time::now().saturating_add(1u8.into());
								last_tip.amount.saturating_accrue(amount);
								last_tip.cumulative_tips.saturating_accrue(amount);
							},
							_ => {
								let cumulative_tips = tips
									.last()
									.map_or(<AmountOf<T>>::default(), |t| t.cumulative_tips);
								tips.try_push(Tip {
									amount,
									timestamp: T::Time::now().saturating_add(1u8.into()),
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
			_origin: OriginFor<T>,
			_query_id: QueryIdOf<T>,
			_timestamp: Timestamp,
		) -> DispatchResult {
			todo!("remove function and adjust call indices")
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
			staker.reports_submitted.saturating_inc();
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
			// todo: complete implementation
			// todo: ensure registered
			let dispute_initiator = ensure_signed(origin)?;
			// Only reporters can begin disputes due to requiring an account on staking chain to potentially receive slash amount if dispute successful
			ensure!(<StakerDetails<T>>::contains_key(&dispute_initiator), Error::<T>::NotReporter);
			// Ensure value actually exists
			ensure!(
				<Reports<T>>::get(query_id).map_or(false, |r| r.timestamps.contains(&timestamp)),
				Error::<T>::NoValueExists
			);
			let vote_id: VoteIdOf<T> = HasherOf::<T>::hash(
				&contracts::Abi::default()
					.fixed_bytes(query_id.as_ref())
					.uint(timestamp.saturated_into::<u128>())
					.encode(),
			);
			// Push new vote round
			let dispute_id = <VoteCount<T>>::get()
				.checked_add(&1u8.into())
				.ok_or(Error::<T>::MaxDisputesReached)?;
			let vote_rounds =
				<VoteRounds<T>>::try_mutate(vote_id, |maybe| -> Result<usize, DispatchError> {
					match maybe {
						None => {
							let mut vote_rounds = BoundedVec::default();
							vote_rounds
								.try_push(dispute_id)
								.map_err(|_| Error::<T>::MaxVoteRoundsReached)?;
							let len = vote_rounds.len();
							*maybe = Some(vote_rounds);
							Ok(len)
						},
						Some(vote_rounds) => {
							vote_rounds
								.try_push(dispute_id)
								.map_err(|_| Error::<T>::MaxVoteRoundsReached)?;
							Ok(vote_rounds.len())
						},
					}
				})?;

			// Create new vote and dispute
			let _vote = <VoteInfo<T>>::get(dispute_id).unwrap_or_else(|| VoteOf::<T> {
				identifier: vote_id,
				// todo: improve to u32?
				vote_round: vote_rounds as u8,
				start_date: T::Time::now(),
				block_number: frame_system::Pallet::<T>::block_number(),
				fee: Self::get_dispute_fee(),
				initiator: dispute_initiator.clone(),
				voted: BoundedBTreeMap::default(),
			});
			let dispute = <DisputeInfo<T>>::get(dispute_id).map_or_else(
				|| -> Result<DisputeOf<T>, DispatchError> {
					let disputed_reporter = Self::get_reporter_by_timestamp(query_id, timestamp)
						.ok_or(Error::<T>::NoValueExists)?;
					Ok(DisputeOf::<T> {
						query_id,
						timestamp,
						value: <ValueOf<T>>::default(),
						disputed_reporter,
					})
				},
				Ok,
			)?;

			if vote_rounds == 1 {
				ensure!(
					T::Time::now().saturating_sub(timestamp) < T::ReportingLock::get(),
					Error::<T>::DisputeReportingPeriodExpired
				);
				Self::_remove_value(query_id, timestamp)?;
			} else {
				todo!()
			}

			{
				// Lookup corresponding addresses on controller chain
				let dispute_initiator = <StakerDetails<T>>::get(&dispute_initiator)
					.ok_or(Error::<T>::NotReporter)?
					.address;
				let disputed_reporter = <StakerDetails<T>>::get(dispute.disputed_reporter)
					.ok_or(Error::<T>::NotReporter)?
					.address;

				let xcm_config = <XcmConfig<T>>::get().unwrap(); // todo: add error

				// todo: charge dispute initiator corresponding fees

				let governance_contract = T::Governance::get();
				let message = xcm::transact(
					xcm_config.fees,
					xcm_config.weight_limit,
					xcm_config.require_weight_at_most,
					ethereum_xcm::transact(
						governance_contract.address,
						governance::begin_parachain_dispute(
							T::ParachainId::get(),
							query_id.as_ref(),
							timestamp.saturated_into::<u128>(),
							dispute_id,
							&dispute.value,
							disputed_reporter,
							dispute_initiator,
						)
						.try_into()
						.map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
						xcm_config.gas_limit.into(),
						None,
					),
				);
				Self::send_xcm(governance_contract.para_id, message)?;
			}

			Self::deposit_event(Event::NewDispute {
				dispute_id,
				query_id,
				timestamp,
				reporter: dispute_initiator,
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
			T::StakingOrigin::ensure_origin(origin)?;

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
			T::StakingOrigin::ensure_origin(origin)?;
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
			T::StakingOrigin::ensure_origin(origin)?;
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
			T::GovernanceOrigin::ensure_origin(origin)?;
			Ok(())
		}

		#[pallet::call_index(14)]
		pub fn report_invalid_dispute(
			origin: OriginFor<T>,
			_dispute_id: DisputeIdOf<T>,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			T::GovernanceOrigin::ensure_origin(origin)?;
			Ok(())
		}

		#[pallet::call_index(15)]
		pub fn slash_dispute_initiator(
			origin: OriginFor<T>,
			_dispute_id: DisputeIdOf<T>,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			T::GovernanceOrigin::ensure_origin(origin)?;
			Ok(())
		}
	}
}

/// Ensure the origin is the governance controller contract.
pub struct EnsureGovernance;
impl<O: Into<Result<Origin, O>> + From<Origin>> EnsureOrigin<O> for EnsureGovernance {
	type Success = ();
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			Origin::Governance => Ok(()),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<O, ()> {
		Ok(O::from(Origin::Governance))
	}
}

/// Ensure the origin is the staking controller contract.
pub struct EnsureStaking;
impl<O: Into<Result<Origin, O>> + From<Origin>> EnsureOrigin<O> for EnsureStaking {
	type Success = ();
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			Origin::Staking => Ok(()),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<O, ()> {
		Ok(O::from(Origin::Staking))
	}
}

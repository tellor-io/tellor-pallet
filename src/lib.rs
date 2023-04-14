// Copyright 2023 Tellor Inc.
// This file is part of Tellor.

// Tellor is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Tellor is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Tellor. If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

use crate::constants::{DAYS, HOURS, REPORTING_LOCK, WEEKS};
pub use crate::xcm::{ContractLocation, LocationToAccount, LocationToOrigin};
use codec::Encode;
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure,
	traits::{fungible::Transfer, EnsureOrigin, Len, UnixTime},
};
pub use pallet::*;
use sp_core::Get;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedDiv, Convert, Zero},
	SaturatedConversion, Saturating,
};
use sp_std::vec::Vec;
pub use traits::{SendXcm, UsingTellor};
use types::*;
pub use types::{
	autopay::{FeedDetails, Tip},
	governance::VoteResult,
	oracle::StakeInfo,
	Address, Amount, DisputeId, FeedId, QueryId, Timestamp,
};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod constants;
mod contracts;
mod impls;
pub mod traits;
mod types;
pub mod xcm;

#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use super::{
		contracts::{governance, registry},
		types::{QueryId, *},
		xcm::{self, ethereum_xcm},
		*,
	};
	use crate::{contracts::staking, types::oracle::Report, xcm::ContractLocation, Tip};
	use ::xcm::latest::prelude::*;
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::traits::{AtLeast32BitUnsigned, Hash},
		traits::{
			fungible::{Inspect, Transfer},
			tokens::Balance,
			PalletInfoAccess,
		},
		PalletId,
	};
	use frame_system::pallet_prelude::*;
	use sp_core::{bounded::BoundedBTreeMap, U256};
	use sp_runtime::{traits::SaturatedConversion, ArithmeticError};
	use sp_std::{prelude::*, result};

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

		/// The units in which we record balances.
		type Balance: Balance + From<u64> + Into<U256>;

		/// The number of decimals used by the balance unit.
		#[pallet::constant]
		type Decimals: Get<u8>;

		/// Percentage, 1000 is 100%, 50 is 5%, etc
		#[pallet::constant]
		type Fee: Get<u16>;

		/// The location of the governance controller contract.
		#[pallet::constant]
		type Governance: Get<ContractLocation>;

		/// Origin that handles dispute resolution (governance).
		type GovernanceOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

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

		type Price: AtLeast32BitUnsigned + Copy + Default;

		/// Origin that manages registration and deregistration from the controller contracts.
		type RegistrationOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// The location of the registry controller contract.
		#[pallet::constant]
		type Registry: Get<ContractLocation>;

		/// The location of the staking controller contract.
		#[pallet::constant]
		type Staking: Get<ContractLocation>;

		/// Origin that handles staking.
		type StakingOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// The on-chain time provider.
		type Time: UnixTime;

		type Token: Inspect<Self::AccountId, Balance = Self::Balance> + Transfer<Self::AccountId>;

		/// Conversion from submitted value (bytes) to a price for price threshold evaluation.
		type ValueConverter: Convert<Vec<u8>, Result<Self::Price, DispatchError>>;

		type Xcm: traits::SendXcm;
	}

	// AutoPay
	#[pallet::storage]
	pub(super) type CurrentFeeds<T> = StorageMap<
		_,
		Blake2_128Concat,
		QueryId,
		BoundedVec<FeedId, <T as Config>::MaxFeedsPerQuery>,
	>;
	#[pallet::storage]
	pub(super) type DataFeeds<T> =
		StorageDoubleMap<_, Blake2_128Concat, QueryId, Blake2_128Concat, FeedId, FeedOf<T>>;
	#[pallet::storage]
	pub(super) type FeedsWithFunding<T> =
		StorageValue<_, BoundedVec<FeedId, <T as Config>::MaxFundedFeeds>, ValueQuery>;
	#[pallet::storage]
	pub(super) type QueryIdFromDataFeedId<T> = StorageMap<_, Blake2_128Concat, FeedId, QueryId>;
	#[pallet::storage]
	pub(super) type QueryIdsWithFunding<T> =
		StorageValue<_, BoundedVec<QueryId, <T as Config>::MaxFundedFeeds>, ValueQuery>;
	#[pallet::storage]
	#[pallet::getter(fn query_ids_with_funding_index)]
	pub(super) type QueryIdsWithFundingIndex<T> = StorageMap<_, Blake2_128Concat, QueryId, u32>;
	#[pallet::storage]
	pub(super) type Tips<T> = StorageMap<
		_,
		Blake2_128Concat,
		QueryId,
		BoundedVec<TipOf<T>, <T as Config>::MaxTipsPerQuery>,
	>;
	#[pallet::storage]
	pub(super) type UserTipsTotal<T> =
		StorageMap<_, Blake2_128Concat, AccountIdOf<T>, BalanceOf<T>, ValueQuery>;
	// Oracle
	/// Accumulated staking reward per staked token
	#[pallet::storage]
	#[pallet::getter(fn accumulated_reward_per_share)]
	pub(super) type AccumulatedRewardPerShare<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;
	/// Mapping of query identifiers to a report.
	#[pallet::storage]
	pub(super) type Reports<T> = StorageMap<_, Blake2_128Concat, QueryId, ReportOf<T>>;
	/// Total staking rewards released per second.
	#[pallet::storage]
	#[pallet::getter(fn reward_rate)]
	pub(super) type RewardRate<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;
	/// Minimum amount required to be a staker.
	#[pallet::storage]
	pub(super) type StakeAmount<T> = StorageValue<_, Amount>;
	/// Mapping from a staker's account identifier to their staking info.
	#[pallet::storage]
	pub(super) type StakerDetails<T> =
		StorageMap<_, Blake2_128Concat, AccountIdOf<T>, StakeInfoOf<T>>;
	#[pallet::storage]
	pub(super) type StakerAddresses<T> = StorageMap<_, Blake2_128Concat, Address, AccountIdOf<T>>;
	/// The time of last update to AccumulatedRewardPerShare.
	#[pallet::storage]
	#[pallet::getter(fn time_of_last_allocation)]
	pub(super) type TimeOfLastAllocation<T> = StorageValue<_, Timestamp, ValueQuery>;
	/// The time of the last new submitted value.
	#[pallet::storage]
	#[pallet::getter(fn time_of_last_new_value)]
	pub(super) type TimeOfLastNewValue<T> = StorageValue<_, Timestamp>;
	/// Staking reward debt, used to calculate real staking rewards balance.
	#[pallet::storage]
	#[pallet::getter(fn total_reward_debt)]
	pub(super) type TotalRewardDebt<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;
	/// Total amount of tokens locked in the staking controller contract.
	#[pallet::storage]
	pub(super) type TotalStakeAmount<T> = StorageValue<_, Amount, ValueQuery>;
	/// Total number of stakers with at least StakeAmount staked, not exact.
	#[pallet::storage]
	pub(super) type TotalStakers<T> = StorageValue<_, u128, ValueQuery>;
	/// Amount locked for withdrawal.
	#[pallet::storage]
	pub(super) type ToWithdraw<T> = StorageValue<_, Amount, ValueQuery>;
	// Governance
	#[pallet::storage]
	pub(super) type DisputeIdsByReporter<T> =
		StorageDoubleMap<_, Blake2_128Concat, AccountIdOf<T>, Blake2_128Concat, DisputeId, ()>;
	#[pallet::storage]
	pub(super) type DisputeInfo<T> = StorageMap<_, Blake2_128Concat, DisputeId, DisputeOf<T>>;
	#[pallet::storage]
	pub(super) type OpenDisputesOnId<T> = StorageMap<_, Blake2_128Concat, QueryId, u128>;
	#[pallet::storage]
	pub(super) type VoteCount<T> = StorageValue<_, u128, ValueQuery>;
	#[pallet::storage]
	pub(super) type VoteInfo<T> =
		StorageDoubleMap<_, Blake2_128Concat, DisputeId, Blake2_128Concat, u8, VoteOf<T>>;
	#[pallet::storage]
	pub(super) type VoteRounds<T> = StorageMap<_, Blake2_128Concat, DisputeId, u8, ValueQuery>;
	#[pallet::storage]
	pub(super) type VoteTallyByAddress<T> =
		StorageMap<_, Blake2_128Concat, AccountIdOf<T>, u128, ValueQuery>;
	// Query Data
	#[pallet::storage]
	pub(super) type QueryData<T> = StorageMap<_, Blake2_128Concat, QueryId, QueryDataOf<T>>;
	// Configuration
	#[pallet::storage]
	pub(super) type Configuration<T> = StorageValue<_, types::Configuration>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// AutoPay
		/// Emitted when a data feed is funded.
		DataFeedFunded {
			query_id: QueryId,
			feed_id: FeedId,
			amount: BalanceOf<T>,
			feed_funder: AccountIdOf<T>,
			feed_details: FeedDetailsOf<T>,
		},
		/// Emitted when a data feed is set up.
		NewDataFeed {
			query_id: QueryId,
			feed_id: FeedId,
			query_data: QueryDataOf<T>,
			feed_creator: AccountIdOf<T>,
		},
		/// Emitted when a onetime tip is claimed.
		OneTimeTipClaimed { query_id: QueryId, amount: BalanceOf<T>, reporter: AccountIdOf<T> },
		/// Emitted when a tip is added.
		TipAdded {
			query_id: QueryId,
			amount: BalanceOf<T>,
			query_data: QueryDataOf<T>,
			tipper: AccountIdOf<T>,
		},
		/// Emitted when a tip is claimed.
		TipClaimed {
			feed_id: FeedId,
			query_id: QueryId,
			amount: BalanceOf<T>,
			reporter: AccountIdOf<T>,
		},

		// Oracle
		/// Emitted when a new value is submitted.
		NewReport {
			query_id: QueryId,
			time: Timestamp,
			value: ValueOf<T>,
			nonce: Nonce,
			query_data: QueryDataOf<T>,
			reporter: AccountIdOf<T>,
		},
		/// Emitted when a new staker is reported.
		NewStakerReported { staker: AccountIdOf<T>, amount: Amount, address: Address },
		/// Emitted when a stake slash is reported.
		SlashReported { reporter: AccountIdOf<T>, recipient: AccountIdOf<T>, amount: Amount },
		/// Emitted when a stake withdrawal is reported.
		StakeWithdrawnReported { staker: AccountIdOf<T> },
		/// Emitted when a stake withdrawal request is reported.
		StakeWithdrawRequestReported { reporter: AccountIdOf<T>, amount: Amount, address: Address },
		/// Emitted when a value is removed (via governance).
		ValueRemoved { query_id: QueryId, timestamp: Timestamp },

		// Governance
		/// Emitted when a new dispute is opened.
		NewDispute {
			dispute_id: DisputeId,
			query_id: QueryId,
			timestamp: Timestamp,
			reporter: AccountIdOf<T>,
		},
		/// Emitted when an address casts their vote.
		Voted { dispute_id: DisputeId, supports: Option<bool>, voter: AccountIdOf<T> },
		/// Emitted when all casting for a vote is tallied.
		VoteTallied { dispute_id: DisputeId, initiator: AccountIdOf<T>, reporter: AccountIdOf<T> },
		/// Emitted when a vote is executed.
		VoteExecuted { dispute_id: DisputeId, result: VoteResult },

		// Query Data
		/// Emitted when query data is stored.
		QueryDataStored { query_id: QueryId },

		// Registration
		/// Emitted when the pallet is (re-)configured.
		Configured { stake_amount: Amount },
		/// Emitted when registration with the controller contracts is attempted.
		RegistrationAttempted { para_id: u32, contract_address: Address },
		/// Emitted when deregistration from the controller contracts is attempted.
		DeregistrationAttempted { para_id: u32, contract_address: Address },
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
		/// Cannot deregister due to active stake.
		ActiveStake,
		InvalidAddress,
		/// Balance must be greater than stake amount.
		InsufficientStake,
		/// Nonce must match the timestamp index.
		InvalidNonce,
		InvalidPrice,
		/// Value must be submitted.
		InvalidValue,
		/// The maximum number of queries has been reached.
		MaxQueriesReached,
		/// The maximum number of timestamps has been reached.
		MaxTimestampsReached,
		/// Reporter not locked for withdrawal.
		NoWithdrawalRequested,
		/// Still in reporter time lock, please wait!
		ReporterTimeLocked,
		/// Timestamp already reported.
		TimestampAlreadyReported,
		/// Withdrawal period didn't pass.
		WithdrawalPeriodPending,

		// Governance
		/// Voter has already voted.
		AlreadyVoted,
		DisputeFeeCalculationError,
		/// Dispute must be started within reporting lock time.
		DisputeReportingPeriodExpired,
		/// New dispute round must be started within a day.
		DisputeRoundReportingPeriodExpired,
		/// Dispute does not exist.
		InvalidDispute,
		/// Vote does not exist.
		InvalidVote,
		/// The maximum number of disputes has been reached.
		MaxDisputesReached,
		/// The maximum number of vote rounds has been reached.
		MaxVoteRoundsReached,
		/// The maximum number of votes has been reached.
		MaxVotesReached,
		/// Dispute initiator is not a reporter.
		NotReporter,
		/// No value exists at given timestamp.
		NoValueExists,
		/// One day has to pass after tally to allow for disputes.
		TallyDisputePeriodActive,
		/// Vote has already been executed.
		VoteAlreadyExecuted,
		/// Vote has already been tallied.
		VoteAlreadyTallied,
		/// Vote must be tallied.
		VoteNotTallied,
		/// Time for voting has not elapsed.
		VotingPeriodActive,

		// Registration
		NotRegistered,

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

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: T::BlockNumber) -> Weight {
			// todo: check for any pending votes to be tallied and sent to governance controller contract
			Weight::zero()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Registers the parachain with the Tellor controller contracts.
		///
		/// - `stake_amount`: The stake amount required to report oracle data to the parachain.
		/// - `fees`: The asset(s) to pay for cross-chain message fees.
		/// - `weight_limit`: The maximum amount of weight to purchase for remote execution of messages.
		/// - `require_weight_at_most`: The maximum weight of any remote call.
		/// - `gas_limit`: Gas limit to be consumed by remote EVM execution.
		#[pallet::call_index(0)]
		pub fn register(
			origin: OriginFor<T>,
			stake_amount: Amount,
			fees: Box<MultiAsset>,
			weight_limit: WeightLimit,
			require_weight_at_most: u64,
			gas_limit: u128,
		) -> DispatchResult {
			T::RegistrationOrigin::ensure_origin(origin)?;

			// Update local configuration
			<StakeAmount<T>>::set(Some(stake_amount));
			let config = types::Configuration {
				xcm_config: xcm::XcmConfig {
					fees: *fees.clone(),
					weight_limit: weight_limit.clone(),
					require_weight_at_most,
				},
				gas_limit,
			};
			<Configuration<T>>::set(Some(config));
			Self::deposit_event(Event::Configured { stake_amount });

			// Register relevant supplied config with parachain registry contract
			let registry_contract = T::Registry::get();
			let message = xcm::transact(
				fees,
				weight_limit,
				require_weight_at_most,
				ethereum_xcm::transact(
					registry_contract.address,
					registry::register(T::ParachainId::get(), Pallet::<T>::index() as u8)
						.try_into()
						.map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
					gas_limit,
					None,
				),
			);
			Self::send_xcm(registry_contract.para_id, message)?;
			Self::deposit_event(Event::RegistrationAttempted {
				para_id: registry_contract.para_id,
				contract_address: registry_contract.address.into(),
			});
			Ok(())
		}

		/// Function to claim singular tip.
		///
		/// - `query_id`: Identifier of reported data.
		/// - `timestamps`: Batch of timestamps of reported data eligible for reward.
		#[pallet::call_index(1)]
		pub fn claim_onetime_tip(
			origin: OriginFor<T>,
			query_id: QueryId,
			timestamps: BoundedVec<Timestamp, T::MaxClaimTimestamps>,
		) -> DispatchResult {
			let reporter = ensure_signed(origin)?;
			ensure!(
				<Tips<T>>::get(query_id).map_or(false, |t| t.len() > 0),
				Error::<T>::NoTipsSubmitted
			);

			let mut cumulative_reward = BalanceOf::<T>::zero();
			for timestamp in timestamps {
				cumulative_reward.saturating_accrue(Self::get_onetime_tip_amount(
					query_id, timestamp, &reporter,
				)?);
			}
			let fee = (cumulative_reward.saturating_mul(T::Fee::get().into()))
				.checked_div(&1000u16.into())
				.ok_or(Error::<T>::FeeCalculationError)?;
			T::Token::transfer(
				&Self::tips(),
				&reporter,
				// todo: safe math
				cumulative_reward - fee,
				false,
			)?;
			Self::add_staking_rewards(fee)?;
			if Self::get_current_tip(query_id) == Zero::zero() {
				let index = <QueryIdsWithFundingIndex<T>>::get(query_id).unwrap_or_default();
				if index != 0 {
					let idx = (index as usize).checked_sub(1).ok_or(Error::<T>::InvalidIndex)?;
					// Replace unfunded feed in array with last element
					<QueryIdsWithFunding<T>>::try_mutate(
						|query_ids_with_funding| -> DispatchResult {
							// todo: safe math
							let qid =
								*query_ids_with_funding.last().ok_or(Error::<T>::InvalidIndex)?;
							query_ids_with_funding
								.get_mut(idx)
								.map(|i| *i = qid)
								.ok_or(Error::<T>::InvalidIndex)?;
							let query_id_last_funded =
								query_ids_with_funding.get(idx).ok_or(Error::<T>::InvalidIndex)?;
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
			feed_id: FeedId,
			query_id: QueryId,
			timestamps: BoundedVec<Timestamp, T::MaxClaimTimestamps>,
		) -> DispatchResult {
			let reporter = ensure_signed(origin)?;

			let mut feed = <DataFeeds<T>>::get(query_id, feed_id).ok_or(Error::<T>::InvalidFeed)?;
			let balance = feed.details.balance;
			ensure!(balance > Zero::zero(), Error::<T>::InsufficientFeedBalance);

			let mut cumulative_reward = BalanceOf::<T>::zero();
			for timestamp in &timestamps {
				ensure!(
					Self::now().saturating_sub(*timestamp) > 12 * HOURS,
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
				&Self::tips(),
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

		/// Allows data feed account to be filled with tokens.
		///
		/// - `feed_id`: Unique feed identifier.
		/// - `query_id`: Identifier of reported data type associated with feed.
		/// - `amount`: Quantity of tokens to fund feed.
		#[pallet::call_index(3)]
		pub fn fund_feed(
			origin: OriginFor<T>,
			feed_id: FeedId,
			query_id: QueryId,
			amount: BalanceOf<T>,
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
			query_id: QueryId,
			reward: BalanceOf<T>,
			start_time: Timestamp,
			interval: Timestamp,
			window: Timestamp,
			price_threshold: u16,
			reward_increase_per_second: BalanceOf<T>,
			query_data: QueryDataOf<T>,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let feed_creator = ensure_signed(origin)?;
			ensure!(query_id == Keccak256::hash(query_data.as_ref()), Error::<T>::InvalidQueryId);
			let feed_id = Keccak256::hash(
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
			ensure!(reward > Zero::zero(), Error::<T>::InvalidReward);
			ensure!(interval > 0, Error::<T>::InvalidInterval);
			ensure!(window < interval, Error::<T>::InvalidWindow);

			let feed = FeedDetailsOf::<T> {
				reward,
				balance: Zero::zero(),
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
						*maybe = Some(
							BoundedVec::try_from(vec![feed_id])
								.map_err(|_| Error::<T>::MaxFeedsFunded)?,
						);
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
			if amount > Zero::zero() {
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
			query_id: QueryId,
			amount: BalanceOf<T>,
			query_data: QueryDataOf<T>,
		) -> DispatchResult {
			let tipper = ensure_signed(origin)?;
			ensure!(query_id == Keccak256::hash(query_data.as_ref()), Error::<T>::InvalidQueryId);
			ensure!(amount > Zero::zero(), Error::<T>::InvalidAmount);

			<Tips<T>>::try_mutate(query_id, |mut maybe_tips| -> DispatchResult {
				match &mut maybe_tips {
					None => {
						*maybe_tips = Some(
							BoundedVec::try_from(vec![TipOf::<T> {
								amount,
								timestamp: Self::now().saturating_add(1u8.into()),
								cumulative_tips: amount,
							}])
							.map_err(|_| Error::<T>::MaxTipsReached)?,
						);
						Self::store_data(query_id, &query_data);
						Ok(())
					},
					Some(tips) => {
						let timestamp_retrieved =
							Self::_get_current_value(query_id).map_or(0, |v| v.1);
						match tips.last_mut() {
							Some(last_tip) if timestamp_retrieved < last_tip.timestamp => {
								last_tip.timestamp = Self::now().saturating_add(1u8.into());
								last_tip.amount.saturating_accrue(amount);
								last_tip.cumulative_tips.saturating_accrue(amount);
							},
							_ => {
								let cumulative_tips =
									tips.last().map_or(Zero::zero(), |t| t.cumulative_tips);
								tips.try_push(Tip {
									amount,
									timestamp: Self::now().saturating_add(1u8.into()),
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
				Self::get_current_tip(query_id) > Zero::zero()
			{
				let len = <QueryIdsWithFunding<T>>::try_mutate(
					|query_ids| -> Result<u32, DispatchError> {
						query_ids.try_push(query_id).map_err(|_| Error::<T>::MaxFeedsFunded)?;
						Ok(query_ids.len() as u32)
					},
				)?;
				<QueryIdsWithFundingIndex<T>>::set(query_id, Some(len));
			}
			T::Token::transfer(&tipper, &Self::tips(), amount, true)?;
			<UserTipsTotal<T>>::mutate(&tipper, |total| total.saturating_accrue(amount));
			Self::deposit_event(Event::TipAdded { query_id, amount, query_data, tipper });
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
			query_id: QueryId,
			value: ValueOf<T>,
			nonce: Nonce,
			query_data: QueryDataOf<T>,
		) -> DispatchResult {
			let reporter = ensure_signed(origin)?;
			ensure!(
				// todo: confirm replacement with Tellor
				//Keccak256::hash(value.as_ref()) != Keccak256::<T>::hash(&[]),
				!value.is_empty(),
				Error::<T>::InvalidValue
			);
			let report = <Reports<T>>::get(query_id);
			ensure!(
				nonce ==
					report
						.as_ref()
						.map_or(Nonce::zero(), |r| r.timestamps.len().saturated_into::<Nonce>()) ||
					nonce == 0, // todo: query || nonce == 0 check
				Error::<T>::InvalidNonce
			);
			let mut staker =
				<StakerDetails<T>>::get(&reporter).ok_or(Error::<T>::InsufficientStake)?;
			ensure!(
				staker.staked_balance >=
					<StakeAmount<T>>::get().ok_or(Error::<T>::NotRegistered)?,
				Error::<T>::InsufficientStake
			);
			// Require reporter to abide by given reporting lock
			let timestamp = Self::now();
			ensure!(
				// todo: refactor to remove saturated_into()
				(timestamp.saturating_sub(staker.reporter_last_timestamp))
					.saturated_into::<u128>()
					.saturating_mul(1_000) >
					((REPORTING_LOCK as u128).saturating_mul(1_000))
						.checked_div(
							staker
								.staked_balance
								.checked_div(
									<StakeAmount<T>>::get().ok_or(Error::<T>::NotRegistered)?
								)
								.ok_or(ArithmeticError::DivisionByZero)?
								.saturated_into::<u128>()
						)
						.ok_or(ArithmeticError::DivisionByZero)?,
				Error::<T>::ReporterTimeLocked
			);
			ensure!(query_id == Keccak256::hash(query_data.as_ref()), Error::<T>::InvalidQueryId);
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
		/// - 'beneficiary`: address on controller chain to potentially receive the slash amount if dispute successful
		#[pallet::call_index(7)]
		pub fn begin_dispute(
			origin: OriginFor<T>,
			query_id: QueryId,
			timestamp: Timestamp,
			beneficiary: Option<Address>,
		) -> DispatchResult {
			let dispute_initiator = ensure_signed(origin)?;

			// Lookup dispute initiator's corresponding address on controller chain (if available) when no beneficiary address specified
			let beneficiary = beneficiary
				.or_else(|| <StakerDetails<T>>::get(&dispute_initiator).map(|s| s.address))
				.ok_or(Error::<T>::NotReporter)?;

			// Ensure value actually exists
			ensure!(
				<Reports<T>>::get(query_id).map_or(false, |r| r.timestamps.contains(&timestamp)),
				Error::<T>::NoValueExists
			);
			let dispute_id: DisputeId = Keccak256::hash(
				&contracts::Abi::default()
					.uint(T::ParachainId::get())
					.fixed_bytes(query_id.as_ref())
					.uint(timestamp.saturated_into::<u128>())
					.encode(),
			);
			// Push new vote round
			let vote_round = <VoteRounds<T>>::try_mutate(
				dispute_id,
				|vote_rounds| -> Result<u8, DispatchError> {
					*vote_rounds =
						vote_rounds.checked_add(1).ok_or(Error::<T>::MaxVoteRoundsReached)?;
					Ok(*vote_rounds)
				},
			)?;

			// Create new vote and dispute
			let mut vote = VoteOf::<T> {
				identifier: dispute_id,
				vote_round,
				start_date: Self::now(),
				block_number: frame_system::Pallet::<T>::block_number(),
				fee: Self::get_dispute_fee().ok_or(Error::<T>::DisputeFeeCalculationError)?,
				tally_date: 0,
				users: Tally::default(),
				reporters: Tally::default(),
				executed: false,
				result: None,
				initiator: dispute_initiator.clone(),
				voted: BoundedBTreeMap::default(),
			};
			// todo: optimise to only write if not already existing
			let mut dispute = DisputeOf::<T> {
				query_id,
				timestamp,
				value: <ValueOf<T>>::default(),
				disputed_reporter: Self::get_reporter_by_timestamp(query_id, timestamp)
					.ok_or(Error::<T>::NoValueExists)?,
				slashed_amount: <StakeAmount<T>>::get().ok_or(Error::<T>::NotRegistered)?,
			};
			<DisputeIdsByReporter<T>>::insert(&dispute.disputed_reporter, dispute_id, ());
			if vote_round == 1 {
				ensure!(
					Self::now().saturating_sub(timestamp) < REPORTING_LOCK,
					Error::<T>::DisputeReportingPeriodExpired
				);
				<OpenDisputesOnId<T>>::mutate(query_id, |open_disputes| {
					*open_disputes =
						Some(open_disputes.take().unwrap_or_default().saturating_add(1));
				});
				// calculate dispute fee based on number of open disputes on query id
				vote.fee = vote.fee.saturating_mul(
					<BalanceOf<T>>::from(2u8).saturating_pow(
						<OpenDisputesOnId<T>>::get(query_id)
							.ok_or(Error::<T>::InvalidIndex)?
							.saturating_sub(1)
							.saturated_into(),
					),
				);
				dispute.value =
					Self::retrieve_data(query_id, timestamp).ok_or(Error::<T>::InvalidTimestamp)?;
				Self::remove_value(query_id, timestamp)?;
			} else {
				let prev_id = vote_round.saturating_sub(1);
				ensure!(
					Self::now() -
						<VoteInfo<T>>::get(dispute_id, prev_id)
							.ok_or(Error::<T>::InvalidVote)?
							.tally_date < 1 * DAYS,
					Error::<T>::DisputeRoundReportingPeriodExpired
				);
				vote.fee = vote.fee.saturating_mul(
					<BalanceOf<T>>::from(2u8)
						.saturating_pow(vote_round.saturating_sub(1).saturated_into()),
				);
				dispute.value =
					<DisputeInfo<T>>::get(dispute_id).ok_or(Error::<T>::InvalidDispute)?.value;
			}
			let stake_amount = U256ToBalance::<T>::convert(Self::convert(
				<StakeAmount<T>>::get().ok_or(Error::<T>::NotRegistered)?,
			)?);
			if vote.fee > stake_amount {
				vote.fee = stake_amount;
			}
			<VoteCount<T>>::mutate(|count| count.saturating_inc());
			let dispute_fee = vote.fee;
			T::Token::transfer(&dispute_initiator, &Self::dispute_fees(), dispute_fee, false)?;
			<VoteInfo<T>>::insert(dispute_id, vote_round, vote);
			<DisputeInfo<T>>::insert(dispute_id, &dispute);
			Self::deposit_event(Event::NewDispute {
				dispute_id,
				query_id,
				timestamp,
				reporter: dispute_initiator,
			});

			// Lookup corresponding address on controller chain
			let disputed_reporter = <StakerDetails<T>>::get(&dispute.disputed_reporter)
				.ok_or(Error::<T>::NotReporter)?
				.address;

			let config = <Configuration<T>>::get().ok_or(Error::<T>::NotRegistered)?;

			// todo: charge dispute initiator corresponding xcm fees

			let governance_contract = T::Governance::get();
			let message = xcm::transact_with_config(
				ethereum_xcm::transact(
					governance_contract.address,
					governance::begin_parachain_dispute(
						query_id.as_ref(),
						timestamp.saturated_into::<u128>(),
						&dispute.value,
						disputed_reporter,
						beneficiary,
						dispute.slashed_amount,
					)
					.try_into()
					.map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
					config.gas_limit,
					None,
				),
				config.xcm_config,
			);
			Self::send_xcm(governance_contract.para_id, message)?;
			// todo: emit event such as GovernanceBeginDisputeAttempted?
			Ok(())
		}

		/// Enables the caller to cast a vote.
		///
		/// - `dispute_id`: The identifier of the dispute.
		/// - `supports`: Whether the caller supports or is against the vote. None indicates the caller’s classification of the dispute as invalid.
		#[pallet::call_index(8)]
		pub fn vote(
			origin: OriginFor<T>,
			dispute_id: DisputeId,
			supports: Option<bool>,
		) -> DispatchResult {
			let voter = ensure_signed(origin)?;
			// Ensure that dispute has not been executed and that vote does not exist and is not tallied
			ensure!(
				dispute_id != <DisputeId>::default() &&
					dispute_id != Keccak256::hash(&[]) &&
					<DisputeInfo<T>>::contains_key(dispute_id),
				Error::<T>::InvalidVote
			);
			let vote_round = <VoteRounds<T>>::get(dispute_id); // use most recent round todo: check whether this should be a parameter
			<VoteInfo<T>>::try_mutate(dispute_id, vote_round, |maybe| -> DispatchResult {
				match maybe {
					None => Err(Error::<T>::InvalidVote.into()),
					Some(vote) => {
						ensure!(vote.tally_date == 0, Error::<T>::VoteAlreadyTallied);
						ensure!(!vote.voted.contains_key(&voter), Error::<T>::AlreadyVoted);
						// Update voting status and increment total queries for support, invalid, or against based on vote
						vote.voted
							.try_insert(voter.clone(), true)
							.map_err(|_| Error::<T>::MaxVotesReached)?;
						let reports = Self::get_reports_submitted_by_address(&voter);
						let user_tips = Self::get_tips_by_address(&voter);
						match supports {
							// Invalid
							None => {
								vote.reporters.invalid_query.saturating_accrue(reports);
								vote.users.invalid_query.saturating_accrue(user_tips);
							},
							Some(supports) =>
								if supports {
									vote.reporters.does_support.saturating_accrue(reports);
									vote.users.does_support.saturating_accrue(user_tips);
								} else {
									vote.reporters.against.saturating_accrue(reports);
									vote.users.against.saturating_accrue(user_tips);
								},
						};
						Ok(())
					},
				}
			})?;
			<VoteTallyByAddress<T>>::mutate(&voter, |total| total.saturating_inc());
			Self::deposit_event(Event::Voted { dispute_id, supports, voter });
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
			T::StakingOrigin::ensure_origin(origin)?;

			<StakerDetails<T>>::try_mutate(&reporter, |maybe| -> DispatchResult {
				let mut staker = maybe.take().unwrap_or_else(|| <StakeInfoOf<T>>::new(address));
				ensure!(address == staker.address, Error::<T>::InvalidAddress);
				let staked_balance = staker.staked_balance;
				let locked_balance = staker.locked_balance;
				if locked_balance > U256::zero() {
					if locked_balance >= amount {
						// if staker's locked balance covers full amount, use that
						staker.locked_balance = staker.locked_balance.saturating_sub(amount);
						<ToWithdraw<T>>::mutate(|locked| *locked = locked.saturating_sub(amount));
					} else {
						// otherwise, stake the whole locked balance
						<ToWithdraw<T>>::mutate(|locked| {
							*locked = locked.saturating_sub(staker.locked_balance)
						});
						staker.locked_balance = U256::zero();
					}
				} else {
					if staked_balance == U256::zero() {
						// if staked balance and locked balance equal 0, save current vote tally.
						// voting participation used for calculating rewards
						staker.start_vote_count = Self::get_vote_count();
						staker.start_vote_tally = Self::get_vote_tally_by_address(&reporter);
					}
				}
				Self::update_stake_and_pay_rewards(
					(&reporter, &mut staker),
					staked_balance + amount,
				)?;
				staker.start_date = Self::now(); // This resets the staker start date to now
				*maybe = Some(staker);
				Ok(())
			})?;
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
			T::StakingOrigin::ensure_origin(origin)?;

			<StakerDetails<T>>::try_mutate(&reporter, |maybe| -> DispatchResult {
				match maybe {
					None => Err(Error::<T>::InsufficientStake.into()),
					Some(staker) => {
						ensure!(address == staker.address, Error::<T>::InvalidAddress);
						ensure!(staker.staked_balance >= amount, Error::<T>::InsufficientStake);
						// todo: safe math
						let stake_amount = staker.staked_balance - amount;
						Self::update_stake_and_pay_rewards((&reporter, staker), stake_amount)?;
						staker.start_date = Self::now();
						staker.locked_balance = staker.locked_balance.saturating_add(amount);
						<ToWithdraw<T>>::mutate(|locked| *locked = locked.saturating_add(amount));
						Ok(())
					},
				}
			})?;
			Self::deposit_event(Event::StakeWithdrawRequestReported { reporter, amount, address });

			// Confirm staking withdraw request
			let staking_contract = T::Staking::get();
			let config = <Configuration<T>>::get().ok_or(Error::<T>::NotRegistered)?;
			let message = xcm::transact_with_config(
				ethereum_xcm::transact(
					staking_contract.address,
					staking::confirm_parachain_stake_withdraw_request(address, amount)
						.try_into()
						.map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
					config.gas_limit,
					None,
				),
				config.xcm_config,
			);
			Self::send_xcm(staking_contract.para_id, message)?;
			// todo: emit StakeWithRequestConfirmationSent event?
			Ok(())
		}

		/// Reports a stake withdrawal by a reporter.
		///
		/// - `reporter`: The reporter who withdrew a stake.
		/// - `amount`: The total amount withdrawn.
		/// - `address`: The corresponding address on the controlling chain.
		#[pallet::call_index(11)]
		pub fn report_stake_withdrawn(
			origin: OriginFor<T>,
			reporter: AccountIdOf<T>,
			amount: Amount,
			// todo: consider removal of address
			address: Address,
		) -> DispatchResult {
			// ensure origin is staking controller contract
			T::StakingOrigin::ensure_origin(origin)?;

			<StakerDetails<T>>::try_mutate(&reporter, |maybe| -> DispatchResult {
				match maybe {
					None => Err(Error::<T>::InsufficientStake.into()),
					Some(staker) => {
						// Ensure reporter is locked and that enough time has passed
						ensure!(
							staker.locked_balance > U256::zero(),
							Error::<T>::NoWithdrawalRequested
						);
						ensure!(
							Self::now().saturating_sub(staker.start_date) >= 7 * DAYS,
							Error::<T>::WithdrawalPeriodPending
						);
						<ToWithdraw<T>>::mutate(|locked| {
							*locked = locked.saturating_sub(staker.locked_balance)
						});
						staker.locked_balance = staker.locked_balance.saturating_sub(amount);
						Ok(())
					},
				}
			})?;
			Self::deposit_event(Event::StakeWithdrawnReported { staker: reporter });
			Ok(())
		}

		/// Reports a slashing of a reporter, due to a passing vote.
		///
		/// - `reporter`: The address of the slashed reporter.
		/// - `recipient`: The address of the recipient.
		/// - `amount`: The slashed amount.
		#[pallet::call_index(12)]
		pub fn report_slash(
			origin: OriginFor<T>,
			reporter: AccountIdOf<T>,
			recipient: AccountIdOf<T>,
			amount: Amount,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			T::GovernanceOrigin::ensure_origin(origin)?;

			<StakerDetails<T>>::try_mutate(&reporter, |maybe| -> DispatchResult {
				match maybe {
					None => Err(Error::<T>::InsufficientStake.into()),
					Some(staker) => {
						let staked_balance = staker.staked_balance;
						let locked_balance = staker.locked_balance;
						ensure!(
							staked_balance.saturating_add(locked_balance) > U256::zero(),
							Error::<T>::InsufficientStake
						);
						if locked_balance >= amount {
							// if locked balance is at least stakeAmount, slash from locked balance
							staker.locked_balance = staker.locked_balance.saturating_sub(amount);
							<ToWithdraw<T>>::mutate(|locked| {
								*locked = locked.saturating_sub(amount)
							});
						} else if locked_balance.saturating_add(staked_balance) >= amount {
							// if locked balance + staked balance is at least stake amount,
							// slash from locked balance and slash remainder from staked balance
							Self::update_stake_and_pay_rewards(
								(&reporter, staker),
								staked_balance
									.saturating_sub(amount.saturating_sub(locked_balance)),
							)?;
							<ToWithdraw<T>>::mutate(|locked| {
								*locked = locked.saturating_sub(locked_balance)
							});
							staker.locked_balance = U256::zero();
						} else {
							// if sum(locked balance + staked balance) is less than stakeAmount, slash sum
							<ToWithdraw<T>>::mutate(|locked| {
								*locked = locked.saturating_sub(locked_balance)
							});
							Self::update_stake_and_pay_rewards((&reporter, staker), U256::zero())?;
							staker.locked_balance = U256::zero();
						}
						Ok(())
					},
				}
			})?;
			Self::deposit_event(Event::SlashReported { reporter, recipient, amount });
			Ok(())
		}

		/// Reports the result of a dispute.
		///
		/// - `dispute_id`: The identifier of the dispute.
		/// - `result`: The result of the dispute.
		#[pallet::call_index(13)]
		pub fn report_vote_executed(
			origin: OriginFor<T>,
			dispute_id: DisputeId,
			result: VoteResult,
		) -> DispatchResult {
			// ensure origin is governance controller contract
			T::GovernanceOrigin::ensure_origin(origin)?;
			// execute vote
			Self::execute_vote(dispute_id, result)
		}

		/// Deregisters the parachain from the Tellor controller contracts.
		#[pallet::call_index(14)]
		pub fn deregister(origin: OriginFor<T>) -> DispatchResult {
			T::RegistrationOrigin::ensure_origin(origin)?;
			ensure!(Self::get_total_stake_amount() == U256::zero(), Error::<T>::ActiveStake);

			// Update local configuration
			<StakeAmount<T>>::set(None);
			Self::deposit_event(Event::Configured { stake_amount: U256::zero() });

			// Register relevant supplied config with parachain registry contract
			let config = <Configuration<T>>::take().ok_or(Error::<T>::NotRegistered)?;
			let registry_contract = T::Registry::get();
			let message = xcm::transact_with_config(
				ethereum_xcm::transact(
					registry_contract.address,
					registry::deregister()
						.try_into()
						.map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
					config.gas_limit,
					None,
				),
				config.xcm_config,
			);
			Self::send_xcm(registry_contract.para_id, message)?;
			Self::deposit_event(Event::DeregistrationAttempted {
				para_id: registry_contract.para_id,
				contract_address: registry_contract.address.into(),
			});
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

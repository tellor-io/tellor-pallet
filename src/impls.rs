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

use super::*;
use crate::constants::DECIMALS;
use ::xcm::prelude::Parachain;
use frame_support::traits::{fungible::Inspect, tokens::Preservation};
use sp_runtime::{
	traits::{CheckedAdd, CheckedMul, CheckedSub, Hash},
	ArithmeticError, SaturatedConversion,
};
use sp_std::cmp::Ordering;

impl<T: Config> Pallet<T> {
	/// The primary account used by the pallet.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub(super) fn account() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Calculates the latest dispute fee based on the supplied price.
	/// # Arguments
	/// * `price` - The current staking token to local balance price.
	/// # Returns
	/// The latest dispute fee.
	pub(super) fn calculate_dispute_fee(
		price: impl Into<U256>,
	) -> Result<BalanceOf<T>, DispatchError> {
		Self::convert(
			<StakeAmount<T>>::get()
				.checked_div(10.into())
				.expect("other is non-zero; qed")
				.checked_mul(price.into())
				.ok_or(ArithmeticError::Overflow)?
				.checked_div(U256::from(10u128.pow(DECIMALS)))
				.expect("other is non-zero; qed"),
		)
		.map(<U256ToBalance<T>>::convert)
	}

	/// Converts a stake amount to a local balance amount.
	/// # Arguments
	/// * `stake_amount` - The amount staked.
	/// # Returns
	/// A stake amount as a local balance amount if successful.
	pub(super) fn convert(stake_amount: Tributes) -> Result<U256, DispatchError> {
		// Convert to local number of decimals
		Self::convert_to_decimals(stake_amount, T::Decimals::get() as u32)
	}

	/// Converts the supplied amount to the supplied number of decimals.
	/// # Arguments
	/// * `amount` - The amount to be converted.
	/// * `decimals` - The number of decimals.
	/// # Returns
	/// The converted amount if successful.
	pub(super) fn convert_to_decimals(amount: U256, decimals: u32) -> Result<U256, DispatchError> {
		if amount == U256::zero() {
			return Ok(amount);
		}
		match DECIMALS.cmp(&decimals) {
			Ordering::Greater => U256::from(10)
				.checked_pow(U256::from(DECIMALS - decimals))
				.ok_or_else(|| ArithmeticError::Overflow.into())
				.map(|r| {
					amount.checked_div(r).expect("result is non-zero, provided non-overflow; qed")
				}),
			Ordering::Less => U256::from(10)
				.checked_pow(U256::from(decimals - DECIMALS))
				.ok_or_else(|| ArithmeticError::Overflow.into())
				.and_then(|r| {
					amount.checked_mul(r).ok_or_else(|| ArithmeticError::Overflow.into())
				}),
			Ordering::Equal => Ok(amount),
		}
	}

	/// Determines if an account voted for a specific dispute round.
	/// # Arguments
	/// * `dispute_id` - The identifier of the dispute.
	/// * `vote_round` - The vote round.
	/// * `voter` - The account of the voter to check.
	/// # Returns
	/// Whether or not the account voted for the specific dispute round.
	pub fn did_vote(dispute_id: DisputeId, vote_round: u8, voter: AccountIdOf<T>) -> bool {
		<Votes<T>>::get((dispute_id, vote_round, voter))
	}

	/// The account identifier of the sub-account used to hold dispute fees.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub(super) fn dispute_fees() -> T::AccountId {
		T::PalletId::get().into_sub_account_truncating(b"dispute")
	}

	/// Funds the staking account with staking rewards from the source account.
	/// # Arguments
	/// * `source` - The source account.
	/// * `amount` - The amount of tokens to fund the staking account with.
	pub(super) fn do_add_staking_rewards(
		source: &AccountIdOf<T>,
		amount: BalanceOf<T>,
	) -> DispatchResult {
		let staking_rewards = Self::staking_rewards();
		if amount > Zero::zero() {
			T::Asset::transfer(source, &staking_rewards, amount, Preservation::Expendable)?;
		}
		Self::update_rewards()?;
		let staking_rewards_balance = T::Asset::balance(&staking_rewards).into();
		// update reward rate = real staking rewards balance / 30 days
		let total_stake_amount = Self::convert(<TotalStakeAmount<T>>::get())?;
		<RewardRate<T>>::set(U256ToBalance::<T>::convert(
			(staking_rewards_balance
				.checked_sub(
					(<AccumulatedRewardPerShare<T>>::get()
						.into()
						.checked_mul(total_stake_amount)
						.ok_or(ArithmeticError::Overflow)?)
					.checked_div(U256::from(Self::unit()?))
					.ok_or(ArithmeticError::DivisionByZero)?
					.checked_sub(<TotalRewardDebt<T>>::get().into())
					.ok_or(ArithmeticError::Underflow)?,
				)
				.ok_or(ArithmeticError::Underflow)?)
			.checked_div(U256::from(30 * DAYS))
			.expect("days constant is greater than zero; qed"),
		));
		Ok(())
	}

	/// Allows data feed account to be filled with tokens.
	/// # Arguments
	/// * `feed_funder`: Account funding the feed.
	/// * `feed_id`: Unique feed identifier.
	/// * `query_id`: Identifier of reported data type associated with feed.
	/// * `amount`: Quantity of tokens to fund feed.
	pub(super) fn do_fund_feed(
		feed_funder: AccountIdOf<T>,
		feed_id: FeedId,
		query_id: QueryId,
		amount: BalanceOf<T>,
	) -> DispatchResult {
		let Some(mut feed) = <DataFeeds<T>>::get(query_id, feed_id) else {
			return Err(Error::<T>::InvalidFeed.into());
		};

		ensure!(amount > Zero::zero(), Error::<T>::InvalidAmount);
		feed.balance.saturating_accrue(amount);
		T::Asset::transfer(&feed_funder, &Self::tips(), amount, Preservation::Expendable)?;
		// Add to feeds with funding
		<FeedsWithFunding<T>>::insert(feed_id, ());
		<DataFeeds<T>>::insert(query_id, feed_id, &feed);
		<UserTipsTotal<T>>::mutate(&feed_funder, |total| total.saturating_accrue(amount));
		Self::deposit_event(Event::DataFeedFunded {
			feed_id,
			query_id,
			amount,
			feed_funder,
			feed_details: feed,
		});
		Ok(())
	}

	/// Read potential reward for an oracle submission.
	/// # Arguments
	/// * `feed_id` - Data feed unique identifier.
	/// * `query_id` - Identifier of reported data.
	/// * `timestamp` - Timestamp of oracle submission.
	/// # Returns
	/// Potential reward for an oracle submission.
	pub(super) fn do_get_reward_amount(
		feed_id: FeedId,
		query_id: QueryId,
		timestamp: Timestamp,
	) -> Result<BalanceOf<T>, DispatchError> {
		ensure!(
			Self::now().checked_sub(timestamp).ok_or(ArithmeticError::Underflow)? < 4 * WEEKS,
			Error::<T>::ClaimPeriodExpired
		);

		let feed = <DataFeeds<T>>::get(query_id, feed_id).ok_or(Error::<T>::InvalidFeed)?;
		ensure!(
			!<DataFeedRewardClaimed<T>>::contains_key((query_id, feed_id, timestamp)),
			Error::<T>::TipAlreadyClaimed
		);
		let n = (timestamp.checked_sub(feed.start_time).ok_or(ArithmeticError::Underflow)?)
			.checked_div(feed.interval)
			.ok_or(ArithmeticError::DivisionByZero)?; // finds closest interval n to timestamp
		let c = feed
			.start_time
			.checked_add(feed.interval.checked_mul(n).ok_or(ArithmeticError::Overflow)?)
			.ok_or(ArithmeticError::Overflow)?; // finds start timestamp c of interval n
		let report = <Reports<T>>::get(query_id, timestamp).ok_or(Error::<T>::InvalidTimestamp)?;
		ensure!(!report.is_disputed, Error::<T>::ValueDisputed);
		let timestamp_before = report.previous.unwrap_or_default();
		let mut price_change = 0; // price change from last value to current value
		if feed.price_threshold != 0 {
			// v1 is value retrieved at supplied timestamp
			let value = <ReportedValuesByTimestamp<T>>::get(query_id, timestamp)
				.ok_or(Error::<T>::InvalidValue)?;
			ensure!(value.len() != 0, Error::<T>::InvalidValue);
			let v1 =
				BytesToU256::convert(value.into_inner()).ok_or(Error::<T>::ValueConversionError)?;
			// v2 is latest value retrieved BEFORE supplied timestamp
			let value_before =
				<ReportedValuesByTimestamp<T>>::get(query_id, timestamp_before).unwrap_or_default();
			let v2 = BytesToU256::convert(value_before.into_inner())
				.ok_or(Error::<T>::ValueConversionError)?;
			if v2 == U256::zero() {
				price_change = 10_000;
			} else if v1 >= v2 {
				price_change = (U256::from(10_000)
					.checked_mul(v1.checked_sub(v2).ok_or(ArithmeticError::Underflow)?)
					.ok_or(ArithmeticError::Overflow)?)
				.checked_div(v2)
				.expect("v2 checked against zero above; qed")
				.saturated_into();
			} else {
				price_change = (U256::from(10_000)
					.checked_mul(v2.checked_sub(v1).ok_or(ArithmeticError::Underflow)?)
					.ok_or(ArithmeticError::Overflow)?)
				.checked_div(v2)
				.expect("v2 checked against zero above; qed")
				.saturated_into();
			}
		}
		let mut reward_amount = feed.reward;
		let time_diff = timestamp.checked_sub(c).ok_or(ArithmeticError::Underflow)?; // time difference between report timestamp and start of interval

		// ensure either report is first within a valid window, or price change threshold is met
		if time_diff < feed.window && timestamp_before < c {
			// add time based rewards if applicable
			reward_amount.saturating_accrue(
				feed.reward_increase_per_second
					.checked_mul(&time_diff.into())
					.ok_or(ArithmeticError::Overflow)?,
			);
		} else {
			ensure!(price_change > feed.price_threshold, Error::<T>::PriceThresholdNotMet);
		}

		if feed.balance < reward_amount {
			reward_amount = feed.balance;
		}
		Ok(reward_amount)
	}

	/// Sends any pending dispute votes due to the governance controller contract for tallying.
	/// # Arguments
	/// * `timestamp` - Data feed unique identifier.
	/// * `max` - The maximum number of pending dispute votes to be sent.
	pub(super) fn do_send_votes(timestamp: Timestamp, max: u8) -> Result<u32, DispatchError> {
		let governance_contract = T::Governance::get();
		const GAS_LIMIT: u64 = gas_limits::VOTE;
		// Check for any pending votes to be sent to governance controller contract
		let mut pending_votes: Vec<_> = <PendingVotes<T>>::iter()
			.filter(|(_, (_, scheduled))| &timestamp >= scheduled)
			.collect();
		pending_votes.sort_by_key(|(_, (_, scheduled))| *scheduled);
		let mut pending_votes_len: u32 = 0;
		for (dispute_id, (vote_round, _)) in pending_votes.into_iter().take(max.into()) {
			pending_votes_len.saturating_inc();
			let _ = <VoteInfo<T>>::try_mutate(dispute_id, vote_round, |maybe| -> DispatchResult {
				let vote = maybe.as_mut().ok_or(Error::<T>::InvalidVote)?;
				ensure!(!vote.sent, Error::<T>::VoteAlreadySent);
				let message = xcm::transact::<T>(
					Parachain(governance_contract.para_id),
					xcm::ethereum_xcm::transact(
						T::EthereumXcmPalletIndex::get(),
						governance_contract.address,
						contracts::governance::vote(
							dispute_id.as_ref(),
							vote.users.does_support,
							vote.users.against,
							vote.users.invalid_query,
							vote.reporters.does_support,
							vote.reporters.against,
							vote.reporters.invalid_query,
						)
						.try_into()
						.map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
						GAS_LIMIT,
					),
					GAS_LIMIT,
				)?;
				Self::send_xcm(
					governance_contract.para_id,
					message,
					Event::VoteSent {
						para_id: governance_contract.para_id,
						contract_address: governance_contract.address.into(),
						dispute_id,
						vote_round,
					},
				)?;
				vote.sent = true;
				<PendingVotes<T>>::remove(dispute_id);
				Ok(())
			});
		}
		Ok(pending_votes_len)
	}

	// Updates the stake amount after retrieving the latest token price from oracle.
	pub(super) fn do_update_stake_amount() -> Result<u32, DispatchError> {
		let (Some((value, _)), iterations) = Self::get_data_before_with_start(
			T::StakingTokenPriceQueryId::get(),
			Self::now().checked_sub(12 * HOURS).ok_or(ArithmeticError::Underflow)?,
			0,
		) else {
			return Err(Error::<T>::InvalidStakingTokenPrice.into());
		};
		let Some(staking_token_price) = BytesToU256::convert(value.into_inner()) else {
			return Err(Error::<T>::InvalidStakingTokenPrice.into());
		};
		ensure!(
			staking_token_price >= 10u128.pow(16).into()
				&& staking_token_price < 10u128.pow(24).into(),
			Error::<T>::InvalidStakingTokenPrice
		);
		let adjusted_stake_amount = (Tributes::from(T::StakeAmountCurrencyTarget::get())
			.checked_mul(Tributes::from(10u128.pow(18)))
			.ok_or(ArithmeticError::Overflow)?)
		.checked_div(staking_token_price)
		.expect("price range checked above; qed");

		let amount = <StakeAmount<T>>::mutate(|amount| {
			let minimum_stake_amount = T::MinimumStakeAmount::get().into();
			if adjusted_stake_amount < minimum_stake_amount {
				*amount = minimum_stake_amount;
				minimum_stake_amount
			} else {
				*amount = adjusted_stake_amount;
				adjusted_stake_amount
			}
		});
		Self::deposit_event(Event::NewStakeAmount { amount });
		Ok(iterations)
	}

	/// Enables the caller to cast a vote.
	/// # Arguments
	/// * `dispute_id` - The identifier of the dispute.
	/// * `supports` - Whether the caller supports or is against the vote. None indicates the caller’s classification of the dispute as invalid.
	pub(super) fn do_vote(
		voter: &AccountIdOf<T>,
		dispute_id: DisputeId,
		supports: Option<bool>,
	) -> DispatchResult {
		// Ensure that dispute has not been executed and that vote does not exist and is not tallied
		ensure!(
			dispute_id != <DisputeId>::default()
				&& dispute_id != Keccak256::hash(&[])
				&& <DisputeInfo<T>>::contains_key(dispute_id),
			Error::<T>::InvalidDispute
		);
		let vote_round = <VoteRounds<T>>::get(dispute_id); // use most recent round
		<VoteInfo<T>>::try_mutate(dispute_id, vote_round, |maybe| -> DispatchResult {
			match maybe {
				None => Err(Error::<T>::InvalidVote.into()),
				Some(vote) => {
					ensure!(vote.tally_date == 0, Error::<T>::VoteAlreadyTallied);
					ensure!(
						!<Votes<T>>::get((dispute_id, vote.vote_round, voter)),
						Error::<T>::AlreadyVoted
					);
					ensure!(!vote.sent, Error::<T>::VoteAlreadySent);
					// Update voting status and increment total queries for support, invalid, or against based on vote
					<Votes<T>>::set((dispute_id, vote_round, voter), true);
					let reports = Self::get_reports_submitted_by_address(voter);
					let user_tips = Self::get_tips_by_address(voter);
					match supports {
						// Invalid
						None => {
							vote.reporters.invalid_query.saturating_accrue(reports.into());
							vote.users.invalid_query.saturating_accrue(user_tips);
						},
						Some(supports) => {
							if supports {
								vote.reporters.does_support.saturating_accrue(reports.into());
								vote.users.does_support.saturating_accrue(user_tips);
							} else {
								vote.reporters.against.saturating_accrue(reports.into());
								vote.users.against.saturating_accrue(user_tips);
							}
						},
					};
					Ok(())
				},
			}
		})?;
		<VoteTallyByAddress<T>>::mutate(voter, |total| total.saturating_inc());
		Self::deposit_event(Event::Voted { dispute_id, supports, voter: voter.clone() });
		Ok(())
	}

	/// Executes the vote and transfers corresponding dispute fees to initiator/reporter.
	/// # Arguments
	/// * `dispute_id` - The identifier of the dispute.
	#[allow(clippy::identity_op)]
	pub(super) fn execute_vote(dispute_id: DisputeId) -> Result<u8, DispatchError> {
		// Ensure validity of dispute id, vote has been executed, and vote must be tallied
		ensure!(
			dispute_id != <DisputeId>::default()
				&& dispute_id != Keccak256::hash(&[])
				&& <DisputeInfo<T>>::contains_key(dispute_id),
			Error::<T>::InvalidDispute
		);
		let final_vote_round = <VoteRounds<T>>::get(dispute_id);
		ensure!(final_vote_round > 0, Error::<T>::InvalidVote);
		let result = <VoteInfo<T>>::try_mutate(
			dispute_id,
			final_vote_round,
			|maybe| -> Result<VoteResult, DispatchError> {
				match maybe {
					None => Err(Error::<T>::InvalidVote.into()),
					Some(vote) => {
						// Ensure vote has not already been executed, and vote must be tallied
						ensure!(!vote.executed, Error::<T>::VoteAlreadyExecuted);
						ensure!(vote.tally_date > 0, Error::<T>::VoteNotTallied);
						let result = vote.result.ok_or(Error::<T>::VoteNotTallied)?;
						// Ensure that time has to be passed after the vote is tallied (86,400 = 24 * 60 * 60 for seconds in a day)
						ensure!(
							Self::now()
								.checked_sub(vote.tally_date)
								.ok_or(ArithmeticError::Underflow)?
								>= 1 * DAYS,
							Error::<T>::TallyDisputePeriodActive
						);
						vote.executed = true;
						let dispute =
							<DisputeInfo<T>>::get(dispute_id).ok_or(Error::<T>::InvalidDispute)?;
						<OpenDisputesOnId<T>>::mutate(dispute.query_id, |maybe| {
							if let Some(disputes) = maybe {
								disputes.saturating_dec();
							}
						});
						// iterate through each vote round and process the dispute fee based on result
						let dispute_fees = &Self::dispute_fees();
						for vote_round in (1..=final_vote_round).rev() {
							// Get dispute initiator and fee for vote round
							let (dispute_initiator, dispute_fee) = if vote_round == final_vote_round
							{
								(vote.initiator.clone(), vote.fee) // use info from final vote round already read above
							} else {
								<VoteInfo<T>>::get(dispute_id, vote_round)
									.map(|v| (v.initiator, v.fee))
									.ok_or(Error::<T>::InvalidVote)?
							};

							// handling transfer of dispute fee
							let dest = match result {
								// If vote passed or invalid, transfer the dispute to initiator
								VoteResult::Passed | VoteResult::Invalid => &dispute_initiator,
								// If vote failed, transfer the dispute fee to disputed reporter
								VoteResult::Failed => &dispute.disputed_reporter,
							};
							T::Asset::transfer(
								dispute_fees,
								dest,
								dispute_fee,
								Preservation::Protect,
							)?;
						}
						Ok(result)
					},
				}
			},
		)?;
		Self::deposit_event(Event::VoteExecuted { dispute_id, result });
		Ok(final_vote_round)
	}

	/// Returns the block number at a given timestamp.
	/// # Arguments
	/// * `query_id` - The identifier of the specific data feed.
	/// * `timestamp` - The timestamp to find the corresponding block number for.
	/// # Returns
	/// Block number of the timestamp for the given query identifier and timestamp, if found.
	pub fn get_block_number_by_timestamp(
		query_id: QueryId,
		timestamp: Timestamp,
	) -> Option<BlockNumberOf<T>> {
		<Reports<T>>::get(query_id, timestamp).map(|r| r.block_number)
	}

	/// Read current data feeds.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// # Returns
	/// Feed identifiers for query identifier, in no particular order.
	pub fn get_current_feeds(query_id: QueryId) -> Vec<FeedId> {
		<DataFeeds<T>>::iter_key_prefix(query_id).collect()
	}

	/// Read current onetime tip by query identifier.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// # Returns
	/// Amount of tip.
	pub fn get_current_tip(query_id: QueryId) -> BalanceOf<T> {
		// if no tips, return 0
		match <TipCount<T>>::get(query_id) {
			0 => Zero::zero(),
			tip_count => <Tips<T>>::get(
				query_id,
				tip_count.checked_sub(1).expect("tip_count greater than zero; qed"),
			)
			.map(|last_tip| {
				let last_reported_timestamp =
					<LastReportedTimestamp<T>>::get(query_id).unwrap_or_default();
				if last_reported_timestamp < last_tip.timestamp {
					last_tip.amount
				} else {
					Zero::zero()
				}
			})
			.unwrap_or_default(),
		}
	}

	/// Returns the current value of a data feed given a specific identifier.
	/// # Arguments
	/// * `query_id` - The identifier of the specific data feed.
	/// # Returns
	/// The latest submitted value for the given identifier.
	pub fn get_current_value(query_id: QueryId) -> Option<ValueOf<T>> {
		<LastReportedTimestamp<T>>::get(query_id)
			.and_then(|t| <ReportedValuesByTimestamp<T>>::get(query_id, t))
	}

	/// Retrieves the latest value for the query identifier before the specified timestamp.
	/// # Arguments
	/// * `query_id` - The query identifier to look up the value for.
	/// * `timestamp` - The timestamp before which to search for the latest value.
	/// # Returns
	/// The value retrieved and its timestamp, if found.
	pub fn get_data_before(
		query_id: QueryId,
		timestamp: Timestamp,
	) -> Option<(ValueOf<T>, Timestamp)> {
		Self::get_index_for_data_before(query_id, timestamp)
			.and_then(|index| Self::get_timestamp_by_query_id_and_index(query_id, index))
			.and_then(|timestamp_retrieved| {
				Self::retrieve_data(query_id, timestamp_retrieved)
					.map(|value| (value, timestamp_retrieved))
			})
	}

	/// Retrieves the latest value for the query identifier before the specified timestamp.
	/// # Arguments
	/// * `query_id` - The query identifier to look up the value for.
	/// * `timestamp` - The timestamp before which to search for the latest value.
	/// * `start` - The start index at which to to begin the search.
	/// # Returns
	/// The value retrieved and its timestamp, if found, along with the number of iterations taken.
	pub(super) fn get_data_before_with_start(
		query_id: QueryId,
		timestamp: Timestamp,
		start: u32,
	) -> (Option<(ValueOf<T>, Timestamp)>, u32) {
		let (index_before, iterations) =
			Self::get_index_for_data_before_with_start(query_id, timestamp, start);
		(
			index_before
				.and_then(|index| <ReportedTimestampsByIndex<T>>::get(query_id, index))
				.and_then(|timestamp_retrieved| {
					<ReportedValuesByTimestamp<T>>::get(query_id, timestamp_retrieved)
						.map(|value| (value, timestamp_retrieved))
				}),
			iterations,
		)
	}

	/// Read a specific data feed.
	/// # Arguments
	/// * `query_id` - Unique feed identifier of parameters.
	/// # Returns
	/// Details of the specified feed.
	pub fn get_data_feed(feed_id: FeedId) -> Option<FeedOf<T>> {
		<QueryIdFromDataFeedId<T>>::get(feed_id)
			.and_then(|query_id| <DataFeeds<T>>::get(query_id, feed_id))
	}

	/// Get the latest dispute fee.
	/// # Returns
	/// The latest dispute fee.
	pub fn get_dispute_fee() -> BalanceOf<T> {
		<DisputeFee<T>>::get()
	}

	/// Returns information on a dispute for a given identifier.
	/// # Arguments
	/// * `dispute_id` - Identifier of the specific dispute.
	/// # Returns
	/// Returns information on a dispute for a given dispute identifier including:
	/// query identifier of disputed value, timestamp of disputed value, value being disputed,
	/// reporter of the disputed value.
	pub fn get_dispute_info(
		dispute_id: DisputeId,
	) -> Option<(QueryId, Timestamp, ValueOf<T>, AccountIdOf<T>)> {
		<DisputeInfo<T>>::get(dispute_id)
			.map(|d| (d.query_id, d.timestamp, d.value, d.disputed_reporter))
	}

	/// Returns the dispute identifiers for a reporter.
	/// # Arguments
	/// * `reporter` - The account of the reporter to check for.
	/// # Returns
	/// Dispute identifiers for a reporter, in no particular order.
	pub fn get_disputes_by_reporter(reporter: AccountIdOf<T>) -> Vec<DisputeId> {
		<DisputeIdsByReporter<T>>::iter_key_prefix(reporter).collect()
	}

	/// Read currently funded feed details.
	/// # Returns
	/// Details for funded feeds.
	pub fn get_funded_feed_details() -> Vec<(FeedOf<T>, QueryDataOf<T>)> {
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
	/// The currently funded feeds, in no particular order.
	pub fn get_funded_feeds() -> Vec<FeedId> {
		<FeedsWithFunding<T>>::iter_keys().collect()
	}

	/// Read query identifiers with current one-time tips.
	/// # Returns
	/// Query identifiers with current one-time tips, in no particular order.
	pub fn get_funded_query_ids() -> Vec<QueryId> {
		<QueryIdsWithFunding<T>>::iter_keys().collect()
	}

	/// Read currently funded single tips with query data.
	/// # Returns
	/// The current single tips.
	pub fn get_funded_single_tips_info() -> Vec<(QueryDataOf<T>, BalanceOf<T>)> {
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
	pub fn get_index_for_data_before(query_id: QueryId, timestamp: Timestamp) -> Option<u32> {
		Self::get_index_for_data_before_with_start(query_id, timestamp, 0).0
	}

	/// Retrieves latest index of data before the specified timestamp for the query identifier.
	/// # Arguments
	/// * `query_id` - The query identifier to look up the index for.
	/// * `timestamp` - The timestamp before which to search for the latest index.
	/// * `start` - The start index at which to to begin the search.
	/// # Returns
	/// Whether the index was found along with the latest index found before the supplied timestamp,
	/// along with the number of iterations taken.
	pub(super) fn get_index_for_data_before_with_start(
		query_id: QueryId,
		timestamp: Timestamp,
		start: u32,
	) -> (Option<u32>, u32) {
		let mut iterations = 0;
		// Use closure to simply append iterations to result, whilst retaining clean ? syntax within closure
		let mut get_index = |query_id, timestamp, mut start| {
			let last_reported_timestamp = <LastReportedTimestamp<T>>::get(query_id)?;
			// Checking Boundaries to short-circuit the algorithm
			let mut time = <ReportedTimestampsByIndex<T>>::get(query_id, start)?;
			if time >= timestamp {
				return None;
			}
			let mut end = <Reports<T>>::get(query_id, last_reported_timestamp)?.index;
			if last_reported_timestamp < timestamp {
				return Some(end);
			}
			// Since the value is within our boundaries, do a binary search
			let mut middle;
			loop {
				iterations.saturating_inc();
				middle =
					(end.checked_sub(start)?).checked_div(2)?.checked_add(1)?.checked_add(start)?;
				time = <ReportedTimestampsByIndex<T>>::get(query_id, middle)?;
				if time < timestamp {
					// get immediate next value
					let next_time = <ReportedTimestampsByIndex<T>>::get(query_id, middle + 1)?;
					if next_time >= timestamp {
						let report = <Reports<T>>::get(query_id, time)?;
						return if !report.is_disputed {
							// _time is correct
							Some(middle)
						} else {
							report
								.previous
								.and_then(|t| <Reports<T>>::get(query_id, t).map(|r| r.index))
						};
					} else {
						// look from middle + 1(next value) to end
						start = middle.checked_add(1)?;
					}
				} else {
					let previous_time =
						<ReportedTimestampsByIndex<T>>::get(query_id, middle.checked_sub(1)?)?;
					if previous_time < timestamp {
						let report = <Reports<T>>::get(query_id, previous_time)?;
						return if !report.is_disputed {
							// previous_time is correct
							Some(middle.checked_sub(1)?)
						} else {
							report
								.previous
								.and_then(|t| <Reports<T>>::get(query_id, t).map(|r| r.index))
						};
					} else {
						// look from start to middle -1(prev value)
						end = middle.checked_sub(1)?;
					}
				}
			}
		};
		(get_index(query_id, timestamp, start), iterations)
	}

	/// Determines tip eligibility for a given oracle submission.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// * `timestamp` - Timestamp of one time tip.
	/// # Returns
	/// Amount of tip.
	pub(super) fn get_onetime_tip_amount(
		query_id: QueryId,
		timestamp: Timestamp,
		claimer: &AccountIdOf<T>,
	) -> Result<BalanceOf<T>, DispatchError> {
		ensure!(
			Self::now().checked_sub(timestamp).ok_or(ArithmeticError::Underflow)? > 12 * HOURS,
			Error::<T>::ClaimBufferNotPassed
		);
		let report = <Reports<T>>::get(query_id, timestamp).ok_or(Error::<T>::InvalidTimestamp)?;
		ensure!(!report.is_disputed, Error::<T>::ValueDisputed);
		ensure!(claimer == &report.reporter, Error::<T>::InvalidClaimer);
		let tip_count = <TipCount<T>>::get(query_id);
		if tip_count == 0 {
			Err(Error::<T>::NoTipsSubmitted.into())
		} else {
			let mut min = 0;
			let mut max = tip_count;
			let mut mid;
			while max.checked_sub(min).ok_or(ArithmeticError::Underflow)? > 1 {
				mid = (max.checked_add(min).ok_or(ArithmeticError::Overflow)?)
					.checked_div(2)
					.expect("divisor is non-zero");
				if <Tips<T>>::get(query_id, mid).map_or(0, |t| t.timestamp) > timestamp {
					max = mid;
				} else {
					min = mid;
				}
			}

			let timestamp_before = report.previous.unwrap_or_default();
			let min_tip = &mut <Tips<T>>::get(query_id, min).ok_or(Error::<T>::InvalidIndex)?;
			ensure!(timestamp_before < min_tip.timestamp, Error::<T>::TipAlreadyEarned);
			ensure!(timestamp >= min_tip.timestamp, Error::<T>::TimestampIneligibleForTip);
			ensure!(min_tip.amount > Zero::zero(), Error::<T>::TipAlreadyClaimed);

			let mut tip_amount = min_tip.amount;
			min_tip.amount = Zero::zero();
			<Tips<T>>::insert(query_id, min, min_tip);
			let min_backup = min;

			// check whether eligible for previous tips in array due to disputes
			let index_before = <Reports<T>>::get(query_id, timestamp_before).map(|r| r.index);
			if report
				.index
				.checked_sub(index_before.unwrap_or_default())
				.ok_or(ArithmeticError::Underflow)?
				> 1 || index_before.is_none()
			{
				if index_before.is_none() {
					tip_amount = <Tips<T>>::get(query_id, min_backup)
						.ok_or(Error::<T>::InvalidIndex)?
						.cumulative_tips;
				} else {
					max = min;
					min = 0;
					let mut mid;
					while max.checked_sub(min).ok_or(ArithmeticError::Underflow)? > 1 {
						mid = (max.checked_add(min).ok_or(ArithmeticError::Overflow)?)
							.checked_div(2)
							.expect("divisor is non-zero");
						if <Tips<T>>::get(query_id, mid).ok_or(Error::<T>::InvalidIndex)?.timestamp
							> timestamp_before
						{
							max = mid;
						} else {
							min = mid;
						}
					}
					min.saturating_inc();
					if min < min_backup {
						let min_backup_tip =
							<Tips<T>>::get(query_id, min_backup).ok_or(Error::<T>::InvalidIndex)?;
						let min_tip =
							<Tips<T>>::get(query_id, min).ok_or(Error::<T>::InvalidIndex)?;
						tip_amount = min_backup_tip
							.cumulative_tips
							.checked_sub(&min_tip.cumulative_tips)
							.ok_or(ArithmeticError::Underflow)?
							.checked_add(&min_tip.amount)
							.ok_or(ArithmeticError::Overflow)?;
					}
				}
			}

			Ok(tip_amount)
		}
	}

	/// Returns the number of open disputes for a specific query identifier.
	/// # Arguments
	/// * `query_id` - Identifier of a specific data feed.
	/// # Returns
	/// The number of open disputes for the query identifier.
	pub fn get_open_disputes_on_id(query_id: QueryId) -> u32 {
		<OpenDisputesOnId<T>>::get(query_id).unwrap_or_default()
	}

	/// Read the number of past tips for a query identifier.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// # Returns
	/// The number of past tips.
	pub fn get_past_tip_count(query_id: QueryId) -> u32 {
		<TipCount<T>>::get(query_id)
	}

	/// Read the past tips for a query identifier.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// # Returns
	/// All past tips, in no particular order.
	pub fn get_past_tips(query_id: QueryId) -> Vec<Tip<BalanceOf<T>>> {
		<Tips<T>>::iter_prefix_values(query_id).collect()
	}

	/// Read a past tip for a query identifier and index.
	/// # Arguments
	/// * `query_id` - Identifier of reported data.
	/// * `index` - The index of the tip.
	/// # Returns
	/// The past tip, if found.
	pub fn get_past_tip_by_index(query_id: QueryId, index: u32) -> Option<Tip<BalanceOf<T>>> {
		<Tips<T>>::get(query_id, index)
	}

	pub fn get_query_data(query_id: QueryId) -> Option<QueryDataOf<T>> {
		<QueryData<T>>::get(query_id)
	}

	/// Look up a query identifier from a data feed identifier.
	/// # Arguments
	/// * `feed_id` - Data feed unique identifier.
	/// # Returns
	/// Corresponding query identifier, if found.
	pub fn get_query_id_from_feed_id(feed_id: FeedId) -> Option<QueryId> {
		<QueryIdFromDataFeedId<T>>::get(feed_id)
	}

	/// Returns reporter and whether a value was disputed for a given query identifier and timestamp.
	/// # Arguments
	/// * `query_id` - The query identifier to look up.
	/// * `timestamp` - The timestamp of the value to look up.
	/// # Returns
	/// The reporter who submitted the value and whether the value was disputed, provided a value exists.
	pub fn get_report_details(
		query_id: QueryId,
		timestamp: Timestamp,
	) -> Option<(AccountIdOf<T>, bool)> {
		<Reports<T>>::get(query_id, timestamp).map(|r| (r.reporter, r.is_disputed))
	}

	/// Returns the reporter who submitted a value for a query identifier at a specific time.
	/// # Arguments
	/// * `query_id` - The identifier of the specific data feed.
	/// * `timestamp` - The timestamp to find a corresponding reporter for.
	/// # Returns
	/// Identifier of the reporter who reported the value for the query identifier at the given timestamp.
	pub fn get_reporter_by_timestamp(
		query_id: QueryId,
		timestamp: Timestamp,
	) -> Option<AccountIdOf<T>> {
		<Reports<T>>::get(query_id, timestamp).map(|r| r.reporter)
	}

	/// Returns the timestamp of the reporter's last submission.
	/// # Arguments
	/// * `reporter` - The identifier of the reporter.
	/// # Returns
	/// The timestamp of the reporter's last submission, if one exists.
	pub fn get_reporter_last_timestamp(reporter: AccountIdOf<T>) -> Option<Timestamp> {
		<StakerDetails<T>>::get(reporter).map(|stake_info| stake_info.reporter_last_timestamp)
	}

	/// Returns the reporting lock time, the amount of time a reporter must wait to submit again.
	/// # Returns
	/// The reporting lock time.
	pub fn get_reporting_lock() -> Timestamp {
		REPORTING_LOCK
	}

	/// Returns the number of values submitted by a specific reporter.
	/// # Arguments
	/// * `reporter` - The identifier of the reporter.
	/// # Returns
	/// The number of values submitted by the given reporter.
	pub fn get_reports_submitted_by_address(reporter: &AccountIdOf<T>) -> u32 {
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
		query_id: QueryId,
	) -> u32 {
		<StakerReportsSubmittedByQueryId<T>>::get(reporter, query_id)
	}

	/// Read potential reward for a set of oracle submissions.
	/// # Arguments
	/// * `feed_id` - Data feed unique identifier.
	/// * `query_id` - Identifier of reported data.
	/// * `timestamps` - Timestamps of oracle submissions.
	/// # Returns
	/// Potential reward for a set of oracle submissions.
	pub fn get_reward_amount(
		feed_id: FeedId,
		query_id: QueryId,
		timestamps: Vec<Timestamp>,
	) -> BalanceOf<T> {
		let Some(feed) = <DataFeeds<T>>::get(query_id, feed_id) else { return Zero::zero() };
		let mut cumulative_reward = <BalanceOf<T>>::zero();
		for timestamp in timestamps.into_iter().take(T::MaxClaimTimestamps::get() as usize) {
			cumulative_reward.saturating_accrue(
				Self::do_get_reward_amount(feed_id, query_id, timestamp).unwrap_or_default(),
			)
		}
		if cumulative_reward > feed.balance {
			cumulative_reward = feed.balance;
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
		feed_id: FeedId,
		query_id: QueryId,
		timestamp: Timestamp,
	) -> bool {
		<DataFeedRewardClaimed<T>>::contains_key((query_id, feed_id, timestamp))
	}

	/// Read whether rewards have been claimed.
	/// # Arguments
	/// * `feed_id` - Data feed unique identifier.
	/// * `query_id` - Identifier of reported data.
	/// * `timestamps` - Timestamps of oracle submissions.
	/// # Returns
	/// Whether rewards have been claimed.
	pub fn get_reward_claim_status_list(
		feed_id: FeedId,
		query_id: QueryId,
		timestamps: Vec<Timestamp>,
	) -> Vec<bool> {
		timestamps
			.into_iter()
			.take(T::MaxClaimTimestamps::get() as usize)
			.map(|timestamp| {
				<DataFeedRewardClaimed<T>>::contains_key((query_id, feed_id, timestamp))
			})
			.collect()
	}

	/// Returns the amount required to report oracle values.
	/// # Returns
	/// The stake amount.
	pub fn get_stake_amount() -> Tributes {
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
	pub fn get_time_of_last_new_value() -> Option<Timestamp> {
		<TimeOfLastNewValue<T>>::get()
	}

	/// Gets the timestamp for the value based on their index.
	/// # Arguments
	/// * `query_id` - The query identifier to look up.
	/// * `index` - The value index to look up.
	/// # Returns
	/// A timestamp if found.
	pub fn get_timestamp_by_query_id_and_index(query_id: QueryId, index: u32) -> Option<Timestamp> {
		<ReportedTimestampsByIndex<T>>::get(query_id, index)
	}

	/// Returns the index of a reporter timestamp in the timestamp array for a specific query identifier.
	/// # Arguments
	/// * `query_id` - Unique identifier of the data feed.
	/// * `timestamp` - The timestamp to find within the available timestamps.
	/// # Returns
	/// The index of the reporter timestamp within the available timestamps for specific query identifier.
	pub fn get_timestamp_index_by_timestamp(
		query_id: QueryId,
		timestamp: Timestamp,
	) -> Option<u32> {
		<Reports<T>>::get(query_id, timestamp).map(|r| r.index)
	}

	/// Read the total amount of tips paid by a user.
	/// # Arguments
	/// * `user` - Address of user to query.
	/// # Returns
	/// Total amount of tips paid by a user.
	pub fn get_tips_by_address(user: &AccountIdOf<T>) -> BalanceOf<T> {
		<UserTipsTotal<T>>::get(user)
	}

	/// Returns the total amount staked for reporting.
	/// # Returns
	/// The total amount of token staked.
	pub fn get_total_stake_amount() -> Tributes {
		<TotalStakeAmount<T>>::get()
	}

	/// Returns the total number of current stakers.
	/// # Returns
	/// The total number of current stakers.
	pub fn get_total_stakers() -> u64 {
		<TotalStakers<T>>::get()
	}

	/// Counts the number of values that have been submitted for the query identifier.
	/// # Arguments
	/// * `query_id` - The query identifier to look up.
	/// # Returns
	/// Count of the number of values received for the query identifier.
	pub fn get_new_value_count_by_query_id(query_id: QueryId) -> u32 {
		<ReportedTimestampCount<T>>::get(query_id)
	}

	/// Returns the total number of votes
	/// # Returns
	/// The total number of votes.
	pub fn get_vote_count() -> u64 {
		<VoteCount<T>>::get()
	}

	/// Returns info on a vote for a given dispute identifier.
	/// # Arguments
	/// * `dispute_id` - Identifier of a specific dispute.
	/// * `vote_round` - The vote round.
	/// # Returns
	/// Information on a vote for a given dispute identifier including: the vote identifier, the
	/// vote information, whether it has been executed, the vote result and the dispute initiator.
	pub fn get_vote_info(dispute_id: DisputeId, vote_round: u8) -> Option<VoteOf<T>> {
		<VoteInfo<T>>::get(dispute_id, vote_round)
	}

	/// Returns the voting rounds for a given dispute identifier.
	/// # Arguments
	/// * `dispute_id` - Identifier for a dispute.
	/// # Returns
	/// The number of vote rounds for the dispute identifier.
	pub fn get_vote_rounds(dispute_id: DisputeId) -> u8 {
		<VoteRounds<T>>::get(dispute_id)
	}

	/// Returns the total number of votes cast by a voter.
	/// # Arguments
	/// * `voter` - The account of the voter to check for.
	/// # Returns
	/// The total number of votes cast by the voter.
	pub fn get_vote_tally_by_address(voter: &AccountIdOf<T>) -> u32 {
		<VoteTallyByAddress<T>>::get(voter)
	}

	/// Returns whether a given value is disputed.
	/// # Arguments
	/// * `query_id` - Unique identifier of the data feed.
	/// * `timestamp` - Timestamp of the value.
	/// # Returns
	/// Whether the value is disputed.
	pub fn is_in_dispute(query_id: QueryId, timestamp: Timestamp) -> bool {
		<Reports<T>>::get(query_id, timestamp)
			.map(|r| r.is_disputed)
			.unwrap_or_default()
	}

	/// Returns the duration since UNIX_EPOCH, in seconds.
	/// # Returns
	/// The duration since UNIX_EPOCH, in seconds.
	pub(super) fn now() -> u64 {
		// Use seconds to match EVM smart contracts
		T::Time::now().as_secs()
	}

	/// Removes a value from the oracle.
	/// # Arguments
	/// * `query_id` - Identifier of the specific data feed.
	/// * `timestamp` - The timestamp of the value to remove.
	pub(super) fn remove_value(
		query_id: QueryId,
		timestamp: Timestamp,
	) -> Result<u32, DispatchError> {
		let iterations =
			<Reports<T>>::try_mutate(query_id, timestamp, |maybe| -> Result<u32, DispatchError> {
				let Some(report) = maybe else { return Err(Error::<T>::InvalidTimestamp.into()) };
				ensure!(!report.is_disputed, Error::<T>::ValueDisputed);
				ensure!(
					Some(timestamp) == <ReportedTimestampsByIndex<T>>::get(query_id, report.index),
					Error::<T>::InvalidTimestamp
				);
				report.is_disputed = true;

				// Update last reported timestamp, if applicable
				let _ = <LastReportedTimestamp<T>>::try_mutate(query_id, |lrt| match lrt {
					// Check if last reported timestamp value is being removed
					Some(last_reported_timestamp) if *last_reported_timestamp == timestamp => {
						*lrt = report.previous; // set last reported to previous
						Ok(())
					},
					_ => Err(()), // No mutation
				});

				// Update next valid timestamp in series to point to previous valid timestamp (before one being removed)
				let start = report.index.checked_add(1).ok_or(ArithmeticError::Overflow)?;
				let end = start.saturating_add(T::MaxDisputedTimeSeries::get());
				let mut iterations = 0;
				for index in start..=end {
					iterations.saturating_inc();
					if iterations > T::MaxDisputedTimeSeries::get() {
						return Err(Error::<T>::MaxDisputedTimeSeriesReached.into());
					}
					let Some(timestamp) = <ReportedTimestampsByIndex<T>>::get(query_id, index)
					else {
						break;
					};
					let mut next_report = <Reports<T>>::get(query_id, timestamp)
						.ok_or(Error::<T>::InvalidTimestamp)?;
					next_report.previous = report.previous;
					<Reports<T>>::insert(query_id, timestamp, &next_report);
					if !next_report.is_disputed {
						break;
					}
				}
				Ok(iterations)
			})?;
		<ReportedValuesByTimestamp<T>>::remove(query_id, timestamp);
		Self::deposit_event(Event::ValueRemoved { query_id, timestamp });
		Ok(iterations)
	}

	/// Retrieve value from the oracle based on timestamp.
	/// # Arguments
	/// * `query_id` - Identifier being requested.
	/// * `timestamp` - Timestamp to retrieve data/value from.
	/// # Returns
	/// Value for timestamp submitted, if found.
	pub fn retrieve_data(query_id: QueryId, timestamp: Timestamp) -> Option<ValueOf<T>> {
		<ReportedValuesByTimestamp<T>>::get(query_id, timestamp)
	}

	/// The account identifier of the sub-account used to hold staking rewards.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub(super) fn staking_rewards() -> T::AccountId {
		T::PalletId::get().into_sub_account_truncating(b"staking")
	}

	pub(super) fn store_data(query_id: QueryId, query_data: &QueryDataOf<T>) {
		QueryData::<T>::insert(query_id, query_data);
		Self::deposit_event(Event::QueryDataStored { query_id });
	}

	/// Tallies the votes and begins the challenge period.
	/// # Arguments
	/// * `dispute_id` - The dispute identifier.
	/// * `result` - The outcome of the vote, as determined by governance.
	pub(super) fn tally_votes(dispute_id: DisputeId, result: VoteResult) -> DispatchResult {
		// Get current vote round for dispute
		let vote_round = <VoteRounds<T>>::get(dispute_id);
		let initiator = <VoteInfo<T>>::try_mutate(
			dispute_id,
			vote_round,
			|maybe| -> Result<AccountIdOf<T>, DispatchError> {
				match maybe {
					Some(vote) => {
						// Ensure vote has not been executed and that vote has not been tallied
						ensure!(!vote.executed, Error::<T>::VoteAlreadyExecuted);
						ensure!(vote.tally_date == 0, Error::<T>::VoteAlreadyTallied);
						// Determine appropriate vote duration dispute round
						// Vote time increases as rounds increase but only up to 6 days (withdrawal period)
						ensure!(
							Self::now()
								.checked_sub(vote.start_date)
								.ok_or(ArithmeticError::Underflow)?
								>= (vote.vote_round as Timestamp)
									.checked_mul(DAYS)
									.expect("cannot overflow based on types; qed")
								|| Self::now()
									.checked_sub(vote.start_date)
									.ok_or(ArithmeticError::Underflow)? >= 6
									.checked_mul(&DAYS)
									.expect("specified values cannot overflow; qed"),
							Error::<T>::VotingPeriodActive
						);
						// Note: main tallying functionality determining result takes place within
						// governance controller contract
						vote.result = Some(result);
						vote.tally_date = Self::now(); // Update time vote was tallied
						Ok(vote.initiator.clone())
					},
					None => Err(Error::<T>::InvalidDispute.into()),
				}
			},
		)?;
		Self::deposit_event(Event::VoteTallied {
			dispute_id,
			result,
			initiator,
			reporter: <DisputeInfo<T>>::get(dispute_id)
				.ok_or(Error::<T>::InvalidDispute)?
				.disputed_reporter,
		});
		Ok(())
	}

	/// The account identifier of the sub-account used to hold tips.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub(super) fn tips() -> T::AccountId {
		T::PalletId::get().into_sub_account_truncating(b"tips")
	}

	/// A unit in which balances are recorded.
	fn unit() -> Result<u128, DispatchError> {
		10u128
			.checked_pow(T::Decimals::get().into())
			.ok_or_else(|| ArithmeticError::Overflow.into())
	}

	// Updates the dispute fee after retrieving the latest token price from oracle.
	pub(super) fn update_dispute_fee() -> Result<u32, DispatchError> {
		let (Some((value, _)), iterations) = Self::get_data_before_with_start(
			T::StakingToLocalTokenPriceQueryId::get(),
			Self::now().checked_sub(12 * HOURS).ok_or(ArithmeticError::Underflow)?,
			0,
		) else {
			return Err(Error::<T>::InvalidPrice.into());
		};
		let Some(token_price) = BytesToU256::convert(value.into_inner()) else {
			return Err(Error::<T>::InvalidPrice.into());
		};
		ensure!(
			token_price >= 10u128.pow(16).into() && token_price < 10u128.pow(24).into(),
			Error::<T>::InvalidPrice
		);
		let new_dispute_fee = Self::calculate_dispute_fee(token_price)?;
		let _ = <DisputeFee<T>>::try_mutate(|dispute_fee| {
			// Only update and deposit event if value has changed
			if new_dispute_fee != *dispute_fee {
				*dispute_fee = new_dispute_fee;
				Self::deposit_event(Event::NewDisputeFee { dispute_fee: new_dispute_fee });
				Ok(())
			} else {
				Err(())
			}
		});
		Ok(iterations)
	}

	/// Updates accumulated staking rewards per staked token.
	pub(crate) fn update_rewards() -> DispatchResult {
		let timestamp = Self::now();
		let time_of_last_allocation = <TimeOfLastAllocation<T>>::get();
		if time_of_last_allocation == timestamp {
			return Ok(());
		}
		let total_stake_amount = Self::convert(<TotalStakeAmount<T>>::get())?;
		let reward_rate = <RewardRate<T>>::get();
		if total_stake_amount == U256::zero() || reward_rate == Zero::zero() {
			<TimeOfLastAllocation<T>>::set(timestamp);
			return Ok(());
		}

		// calculate accumulated reward per token staked
		let unit: U256 = Self::unit()?.into();
		let accumulated_reward_per_share = <AccumulatedRewardPerShare<T>>::get().into();
		let new_accumulated_reward_per_share: U256 = accumulated_reward_per_share
			+ (U256::from(timestamp - time_of_last_allocation)
				.checked_mul(reward_rate.into())
				.ok_or(ArithmeticError::Overflow)?
				.checked_mul(unit)
				.ok_or(ArithmeticError::Overflow)?)
			.checked_div(total_stake_amount)
			.expect("total stake amount checked against zero above; qed");
		// calculate accumulated reward with new_accumulated_reward_per_share
		let total_reward_debt = <TotalRewardDebt<T>>::get().into();
		let accumulated_reward = (new_accumulated_reward_per_share
			.checked_mul(total_stake_amount)
			.ok_or(ArithmeticError::Overflow)?)
		.checked_div(unit)
		.ok_or(ArithmeticError::DivisionByZero)?
		.checked_sub(total_reward_debt)
		.ok_or(ArithmeticError::Underflow)?;
		let staking_rewards_balance = T::Asset::balance(&Self::staking_rewards()).into();
		if accumulated_reward >= staking_rewards_balance {
			// if staking rewards run out, calculate remaining reward per staked token and set
			// RewardRate to 0
			let new_pending_rewards = staking_rewards_balance
				.checked_sub(
					(accumulated_reward_per_share
						.checked_mul(total_stake_amount)
						.ok_or(ArithmeticError::Overflow)?)
					.checked_div(unit)
					.ok_or(ArithmeticError::DivisionByZero)?
					.checked_sub(total_reward_debt)
					.ok_or(ArithmeticError::Underflow)?,
				)
				.ok_or(ArithmeticError::Underflow)?;
			<AccumulatedRewardPerShare<T>>::try_mutate(|value| -> DispatchResult {
				*value = value
					.checked_add(&U256ToBalance::<T>::convert(
						(new_pending_rewards.checked_mul(unit).ok_or(ArithmeticError::Overflow)?)
							.checked_div(total_stake_amount)
							.expect("total stake amount checked against zero above; qed"),
					))
					.ok_or(ArithmeticError::Overflow)?;
				Ok(())
			})?;
			<RewardRate<T>>::set(Zero::zero());
		} else {
			<AccumulatedRewardPerShare<T>>::set(U256ToBalance::<T>::convert(
				new_accumulated_reward_per_share,
			));
		}
		<TimeOfLastAllocation<T>>::set(timestamp);
		Ok(())
	}

	/// Called whenever a user's stake amount changes. First updates staking rewards, transfers
	/// pending rewards to user's address, and finally updates user's stake amount and other relevant
	/// variables.
	/// # Arguments
	/// * `staker` - Staker whose stake is being updated.
	/// * `new_staked_balance` - The new staked balance of the staker.
	pub(super) fn update_stake_and_pay_rewards(
		staker: (&AccountIdOf<T>, &mut StakeInfoOf<T>),
		new_staked_balance: Tributes,
	) -> DispatchResult {
		Self::update_rewards()?;
		let (staker, stake_info) = staker;
		let staking_rewards = Self::staking_rewards();
		let unit = Self::unit()?.into();
		if stake_info.staked_balance > U256::zero() {
			// if address already has a staked balance, calculate and transfer pending rewards
			let mut pending_reward = <U256ToBalance<T>>::convert(
				Self::convert(stake_info.staked_balance)?
					.checked_mul(<AccumulatedRewardPerShare<T>>::get().into())
					.ok_or(ArithmeticError::Overflow)?
					.checked_div(unit)
					.ok_or(ArithmeticError::DivisionByZero)?
					.checked_sub(stake_info.reward_debt.into())
					.ok_or(ArithmeticError::Underflow)?,
			);
			// get staker voting participation rate
			let number_of_votes = Self::get_vote_count()
				.checked_sub(stake_info.start_vote_count)
				.ok_or(ArithmeticError::Underflow)?;
			if number_of_votes > 0 {
				// staking reward = pending reward * voting participation rate
				let vote_tally = Self::get_vote_tally_by_address(staker);
				let temp_pending_reward = (pending_reward
					.checked_mul(
						&(vote_tally
							.checked_sub(stake_info.start_vote_tally)
							.ok_or(ArithmeticError::Underflow)?
							.into()),
					)
					.ok_or(ArithmeticError::Overflow)?)
				.checked_div(&number_of_votes.into())
				.ok_or(ArithmeticError::DivisionByZero)?;
				if temp_pending_reward < pending_reward {
					pending_reward = temp_pending_reward;
				}
			}
			T::Asset::transfer(&staking_rewards, staker, pending_reward, Preservation::Protect)?;
			<TotalRewardDebt<T>>::try_mutate(|debt| -> DispatchResult {
				*debt =
					debt.checked_sub(&stake_info.reward_debt).ok_or(ArithmeticError::Underflow)?;
				Ok(())
			})?;
			<TotalStakeAmount<T>>::try_mutate(|total| -> DispatchResult {
				*total = total
					.checked_sub(stake_info.staked_balance)
					.ok_or(ArithmeticError::Underflow)?;
				Ok(())
			})?;
		}
		stake_info.staked_balance = new_staked_balance;
		// Update total stakers
		<TotalStakers<T>>::try_mutate(|total| -> Result<(), Error<T>> {
			if stake_info.staked_balance >= <StakeAmount<T>>::get() {
				if !stake_info.staked {
					total.saturating_inc();
				}
				stake_info.staked = true;
			} else {
				if stake_info.staked && *total > 0 {
					total.saturating_dec();
				}
				stake_info.staked = false;
			}
			Ok(())
		})?;
		// tracks rewards accumulated before stake amount updated
		let accumulated_reward_per_share = <AccumulatedRewardPerShare<T>>::get().into();
		stake_info.reward_debt = U256ToBalance::<T>::convert(
			Self::convert(stake_info.staked_balance)?
				.checked_mul(accumulated_reward_per_share)
				.ok_or(ArithmeticError::Overflow)?
				.checked_div(unit)
				.ok_or(ArithmeticError::DivisionByZero)?,
		);
		let total_reward_debt =
			<TotalRewardDebt<T>>::mutate(|debt| -> Result<BalanceOf<T>, DispatchError> {
				*debt =
					debt.checked_add(&stake_info.reward_debt).ok_or(ArithmeticError::Overflow)?;
				Ok(*debt)
			})?;
		let total_stake_amount = Self::convert(<TotalStakeAmount<T>>::mutate(
			|total| -> Result<Tributes, DispatchError> {
				*total = total
					.checked_add(stake_info.staked_balance)
					.ok_or(ArithmeticError::Overflow)?;
				Ok(*total)
			},
		)?)?;
		// update reward rate if staking rewards are available given staker's updated parameters
		<RewardRate<T>>::try_mutate(|reward_rate| -> DispatchResult {
			if *reward_rate == Zero::zero() {
				*reward_rate = U256ToBalance::<T>::convert(
					T::Asset::balance(&staking_rewards)
						.into()
						.checked_sub(
							accumulated_reward_per_share
								.checked_mul(total_stake_amount)
								.ok_or(ArithmeticError::Overflow)?
								.checked_div(unit)
								.ok_or(ArithmeticError::DivisionByZero)?
								.checked_sub(total_reward_debt.into())
								.ok_or(ArithmeticError::Underflow)?,
						)
						.ok_or(ArithmeticError::Underflow)?
						.checked_div((30 * DAYS).into())
						.expect("days constant is greater than zero; qed"),
				);
			}
			Ok(())
		})?;
		Ok(())
	}
}

impl<T: Config> UsingTellor<AccountIdOf<T>> for Pallet<T> {
	fn bytes_to_uint(bytes: Vec<u8>) -> Option<U256> {
		BytesToU256::convert(bytes)
	}

	fn get_data_after(query_id: QueryId, timestamp: Timestamp) -> Option<(Vec<u8>, Timestamp)> {
		Self::get_index_for_data_after(query_id, timestamp)
			.and_then(|index| Self::get_timestamp_by_query_id_and_index(query_id, index))
			.and_then(|timestamp_retrieved| {
				Self::retrieve_data(query_id, timestamp_retrieved)
					.map(|value| (value.into_inner(), timestamp_retrieved))
			})
	}

	fn get_data_before(query_id: QueryId, timestamp: Timestamp) -> Option<(Vec<u8>, Timestamp)> {
		Self::get_data_before(query_id, timestamp).map(|(v, t)| (v.into_inner(), t))
	}

	fn get_index_for_data_after(query_id: QueryId, timestamp: Timestamp) -> Option<u32> {
		let mut count = Self::get_new_value_count_by_query_id(query_id);
		if count == 0 {
			return None;
		}
		count.saturating_dec();
		let mut search = true; // perform binary search
		let mut middle = 0;
		let mut start = 0;
		let mut end = count;
		// checking boundaries to short-circuit the algorithm
		let mut timestamp_retrieved =
			Self::get_timestamp_by_query_id_and_index(query_id, end).unwrap_or_default();
		if timestamp_retrieved <= timestamp {
			return None;
		}
		timestamp_retrieved =
			Self::get_timestamp_by_query_id_and_index(query_id, start).unwrap_or_default();
		if timestamp_retrieved > timestamp {
			// candidate found, check for disputes
			search = false;
		}
		// since the value is within our boundaries, do a binary search
		while search {
			middle = (end.saturating_add(start)).checked_div(2).expect("divisor is non-zero; qed");
			timestamp_retrieved =
				Self::get_timestamp_by_query_id_and_index(query_id, middle).unwrap_or_default();
			if timestamp_retrieved > timestamp {
				// get immediate previous value
				let previous_time =
					Self::get_timestamp_by_query_id_and_index(query_id, middle.saturating_sub(1))
						.unwrap_or_default();
				if previous_time <= timestamp {
					// candidate found, check for disputes
					search = false;
				} else {
					// look from start to middle -1(prev value)
					end = middle.saturating_sub(1);
				}
			} else {
				// get immediate next value
				let next_time =
					Self::get_timestamp_by_query_id_and_index(query_id, middle.saturating_add(1))
						.unwrap_or_default();
				if next_time > timestamp {
					// candidate found, check for disputes
					search = false;
					middle.saturating_inc();
					timestamp_retrieved = next_time;
				} else {
					// look from middle + 1(next value) to end
					start = middle.saturating_add(1);
				}
			}
		}
		// candidate found, check for disputed values
		if !Self::is_in_dispute(query_id, timestamp_retrieved) {
			// timestamp_retrieved is correct
			Some(middle)
		} else {
			// iterate forward until we find a non-disputed value
			while Self::is_in_dispute(query_id, timestamp_retrieved) && middle < count {
				middle.saturating_inc();
				timestamp_retrieved =
					Self::get_timestamp_by_query_id_and_index(query_id, middle).unwrap_or_default();
			}
			if middle == count && Self::is_in_dispute(query_id, timestamp_retrieved) {
				return None;
			}
			// timestamp_retrieved is correct
			Some(middle)
		}
	}

	fn get_index_for_data_before(query_id: QueryId, timestamp: Timestamp) -> Option<u32> {
		Self::get_index_for_data_before(query_id, timestamp)
	}

	fn get_multiple_values_before(
		query_id: QueryId,
		timestamp: Timestamp,
		max_age: Timestamp,
		max_count: u32,
	) -> Vec<(Vec<u8>, Timestamp)> {
		// get index of first possible value
		let Some(start_index) =
			Self::get_index_for_data_after(query_id, timestamp.saturating_sub(max_age))
		else {
			// no value within range
			return Vec::default();
		};
		// get index of last possible value
		let Some(end_index) = Self::get_index_for_data_before(query_id, timestamp) else {
			// no value before timestamp
			return Vec::default();
		};
		let mut value_count: usize = 0;
		let mut index = 0;
		let max_count = max_count as usize;
		let mut timestamps = Vec::with_capacity(max_count);
		// generate array of non-disputed timestamps within range
		while value_count < max_count
			&& end_index.saturating_add(1).saturating_sub(index) > start_index
		{
			if let Some(timestamp_retrieved) =
				Self::get_timestamp_by_query_id_and_index(query_id, end_index.saturating_sub(index))
			{
				if !Self::is_in_dispute(query_id, timestamp_retrieved) {
					timestamps.push(timestamp_retrieved);
					value_count.saturating_inc();
				}
			}
			index.saturating_inc();
		}

		// retrieve values and reverse timestamps order
		let mut result = Vec::new();
		for i in 0..value_count {
			let timestamp = timestamps[value_count - 1 - i];
			if let Some(data) = Self::retrieve_data(query_id, timestamp) {
				result.push((data.into_inner(), timestamp));
			}
		}
		result
	}

	fn get_new_value_count_by_query_id(query_id: QueryId) -> u32 {
		Self::get_new_value_count_by_query_id(query_id)
	}

	fn get_reporter_by_timestamp(
		query_id: QueryId,
		timestamp: Timestamp,
	) -> Option<AccountIdOf<T>> {
		Self::get_reporter_by_timestamp(query_id, timestamp)
	}

	fn get_timestamp_by_query_id_and_index(query_id: QueryId, index: u32) -> Option<Timestamp> {
		Self::get_timestamp_by_query_id_and_index(query_id, index)
	}

	fn is_in_dispute(query_id: QueryId, timestamp: Timestamp) -> bool {
		Self::is_in_dispute(query_id, timestamp)
	}

	fn now() -> Timestamp {
		Self::now()
	}

	fn retrieve_data(query_id: QueryId, timestamp: Timestamp) -> Option<Vec<u8>> {
		Self::retrieve_data(query_id, timestamp).map(|v| v.into_inner())
	}
}

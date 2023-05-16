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

//! Runtime API definition for Tellor pallet.

#![cfg_attr(not(feature = "std"), no_std)]

pub use autopay::{FeedDetailsWithQueryData, SingleTipWithQueryData};
use codec::Codec;
pub use governance::VoteInfo;
use sp_std::vec::Vec;
use tellor::{DisputeId, Feed, FeedId, QueryId, Timestamp, Tip, Tributes, VoteResult};

mod autopay;
mod governance;
#[cfg(test)]
mod tests;

sp_api::decl_runtime_apis! {
	pub trait TellorAutoPay<AccountId: Codec, Balance: Codec>
	{
		/// Read current data feeds.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// # Returns
		/// Feed identifiers for query identifier, in no particular order.
		fn get_current_feeds(query_id: QueryId) -> Vec<FeedId>;

		/// Read current onetime tip by query identifier.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// # Returns
		/// Amount of tip.
		fn get_current_tip(query_id: QueryId) -> Balance;

		/// Read a specific data feed.
		/// # Arguments
		/// * `query_id` - Unique feed identifier of parameters.
		/// # Returns
		/// Details of the specified feed.
		fn get_data_feed(feed_id: FeedId) -> Option<Feed<Balance>>;

		/// Read currently funded feed details.
		/// # Returns
		/// Details for funded feeds.
		fn get_funded_feed_details() -> Vec<FeedDetailsWithQueryData<Balance>>;

		/// Read currently funded feeds.
		/// # Returns
		/// The currently funded feeds, in no particular order.
		fn get_funded_feeds() -> Vec<FeedId>;

		/// Read query identifiers with current one-time tips.
		/// # Returns
		/// Query identifiers with current one-time tips, in no particular order.
		fn get_funded_query_ids() -> Vec<QueryId>;

		/// Read currently funded single tips with query data.
		/// # Returns
		/// The current single tips.
		fn get_funded_single_tips_info() -> Vec<SingleTipWithQueryData<Balance>>;

		/// Read the number of past tips for a query identifier.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// # Returns
		/// The number of past tips.
		fn get_past_tip_count(query_id: QueryId) -> u128;

		/// Read the past tips for a query identifier.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// # Returns
		/// All past tips, in no particular order.
		fn get_past_tips(query_id: QueryId) -> Vec<Tip<Balance>>;

		/// Read a past tip for a query identifier and index.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// * `index` - The index of the tip.
		/// # Returns
		/// The past tip, if found.
		fn get_past_tip_by_index(query_id: QueryId, index: u128) -> Option<Tip<Balance>>;

		/// Look up a query identifier from a data feed identifier.
		/// # Arguments
		/// * `feed_id` - Data feed unique identifier.
		/// # Returns
		/// Corresponding query identifier, if found.
		fn get_query_id_from_feed_id(feed_id: FeedId) -> Option<QueryId>;

		/// Read potential reward for a set of oracle submissions.
		/// # Arguments
		/// * `feed_id` - Data feed unique identifier.
		/// * `query_id` - Identifier of reported data.
		/// * `timestamps` - Timestamps of oracle submissions.
		/// # Returns
		/// Potential reward for a set of oracle submissions.
		fn get_reward_amount(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> Balance;

		/// Read whether a reward has been claimed.
		/// # Arguments
		/// * `feed_id` - Data feed unique identifier.
		/// * `query_id` - Identifier of reported data.
		/// * `timestamp` - Timestamp of reported data.
		/// # Returns
		/// Whether a reward has been claimed, if timestamp exists.
		fn get_reward_claimed_status(feed_id: FeedId, query_id: QueryId, timestamp: Timestamp) -> bool;

		/// Read whether rewards have been claimed.
		/// # Arguments
		/// * `feed_id` - Data feed unique identifier.
		/// * `query_id` - Identifier of reported data.
		/// * `timestamps` - Timestamps of oracle submissions.
		/// # Returns
		/// Whether rewards have been claimed.
		fn get_reward_claim_status_list(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> Vec<bool>;

		/// Read the total amount of tips paid by a user.
		/// # Arguments
		/// * `user` - Address of user to query.
		/// # Returns
		/// Total amount of tips paid by a user.
		fn get_tips_by_address(user: AccountId) -> Balance;
	}

	pub trait TellorOracle<AccountId: Codec, BlockNumber: Codec, StakeInfo: Codec, Value: Codec> where
	{
		/// Returns the block number at a given timestamp.
		/// # Arguments
		/// * `query_id` - The identifier of the specific data feed.
		/// * `timestamp` - The timestamp to find the corresponding block number for.
		/// # Returns
		/// Block number of the timestamp for the given query identifier and timestamp, if found.
		fn get_block_number_by_timestamp(query_id: QueryId, timestamp: Timestamp) -> Option<BlockNumber>;

		/// Returns the current value of a data feed given a specific identifier.
		/// # Arguments
		/// * `query_id` - The identifier of the specific data feed.
		/// # Returns
		/// The latest submitted value for the given identifier.
		fn get_current_value(query_id: QueryId) -> Option<Value>;

		/// Retrieves the latest value for the query identifier before the specified timestamp.
		/// # Arguments
		/// * `query_id` - The query identifier to look up the value for.
		/// * `timestamp` - The timestamp before which to search for the latest value.
		/// # Returns
		/// The value retrieved and its timestamp, if found.
		fn get_data_before(query_id: QueryId, timestamp: Timestamp) -> Option<(Value, Timestamp)>;

		/// Counts the number of values that have been submitted for the query identifier.
		/// # Arguments
		/// * `query_id` - The query identifier to look up.
		/// # Returns
		/// Count of the number of values received for the query identifier.
		fn get_new_value_count_by_query_id(query_id: QueryId) -> u32;

		/// Returns reporter and whether a value was disputed for a given query identifier and timestamp.
		/// # Arguments
		/// * `query_id` - The query identifier to look up.
		/// * `timestamp` - The timestamp of the value to look up.
		/// # Returns
		/// The reporter who submitted the value and whether the value was disputed, provided a value exists.
		fn get_report_details(query_id: QueryId, timestamp: Timestamp) -> Option<(AccountId, bool)>;

		/// Returns the reporter who submitted a value for a query identifier at a specific time.
		/// # Arguments
		/// * `query_id` - The identifier of the specific data feed.
		/// * `timestamp` - The timestamp to find a corresponding reporter for.
		/// # Returns
		/// Identifier of the reporter who reported the value for the query identifier at the given timestamp.
		fn get_reporter_by_timestamp(query_id: QueryId, timestamp: Timestamp) -> Option<AccountId>;

		/// Returns the timestamp of the reporter's last submission.
		/// # Arguments
		/// * `reporter` - The identifier of the reporter.
		/// # Returns
		/// The timestamp of the reporter's last submission, if one exists.
		fn get_reporter_last_timestamp(reporter: AccountId) -> Option<Timestamp>;

		/// Returns the reporting lock time, the amount of time a reporter must wait to submit again.
		/// # Returns
		/// The reporting lock time.
		fn get_reporting_lock() -> Timestamp;

		/// Returns the number of values submitted by a specific reporter.
		/// # Arguments
		/// * `reporter` - The identifier of the reporter.
		/// # Returns
		/// The number of values submitted by the given reporter.
		fn get_reports_submitted_by_address(reporter: AccountId) -> u128;

		/// Returns the number of values submitted to a specific query identifier by a specific reporter.
		/// # Arguments
		/// * `reporter` - The identifier of the reporter.
		/// * `query_id` - Identifier of the specific data feed.
		/// # Returns
		/// The number of values submitted by the given reporter to the given query identifier.
		fn get_reports_submitted_by_address_and_query_id(reporter: AccountId, query_id: QueryId) -> u128;

		/// Returns the amount required to report oracle values.
		/// # Returns
		/// The stake amount.
		fn get_stake_amount() -> Tributes;

		/// Returns all information about a staker.
		/// # Arguments
		/// * `staker` - The identifier of the staker inquiring about.
		/// # Returns
		/// All information about a staker, if found.
		fn get_staker_info(staker: AccountId) -> Option<StakeInfo>;

		/// Returns the timestamp for the last value of any identifier from the oracle.
		/// # Returns
		/// The timestamp of the last oracle value.
		fn get_time_of_last_new_value() -> Option<Timestamp>;

		/// Gets the timestamp for the value based on their index.
		/// # Arguments
		/// * `query_id` - The query identifier to look up.
		/// * `index` - The value index to look up.
		/// # Returns
		/// A timestamp if found.
		fn get_timestamp_by_query_id_and_index(query_id: QueryId, index: u32) -> Option<Timestamp>;

		/// Retrieves latest index of data before the specified timestamp for the query identifier.
		/// # Arguments
		/// * `query_id` - The query identifier to look up the index for.
		/// * `timestamp` - The timestamp before which to search for the latest index.
		/// # Returns
		/// Whether the index was found along with the latest index found before the supplied timestamp.
		fn get_index_for_data_before(query_id: QueryId, timestamp: Timestamp) -> Option<u32>;

		/// Returns the index of a reporter timestamp in the timestamp array for a specific query identifier.
		/// # Arguments
		/// * `query_id` - Unique identifier of the data feed.
		/// * `timestamp` - The timestamp to find within the available timestamps.
		/// # Returns
		/// The index of the reporter timestamp within the available timestamps for specific query identifier.
		fn get_timestamp_index_by_timestamp(query_id: QueryId, timestamp: Timestamp) -> Option<u32>;

		/// Returns the total amount staked for reporting.
		/// # Returns
		/// The total amount of token staked.
		fn get_total_stake_amount() -> Tributes;

		/// Returns the total number of current stakers.
		/// # Returns
		/// The total number of current stakers.
		fn get_total_stakers() -> u128;

		/// Returns whether a given value is disputed.
		/// # Arguments
		/// * `query_id` - Unique identifier of the data feed.
		/// * `timestamp` - Timestamp of the value.
		/// # Returns
		/// Whether the value is disputed.
		fn is_in_dispute(query_id: QueryId, timestamp: Timestamp) -> bool;

		/// Retrieve value from the oracle based on timestamp.
		/// # Arguments
		/// * `query_id` - Identifier being requested.
		/// * `timestamp` - Timestamp to retrieve data/value from.
		/// # Returns
		/// Value for timestamp submitted, if found.
		fn retrieve_data(query_id: QueryId, timestamp: Timestamp) -> Option<Value>;
	}

	pub trait TellorGovernance<AccountId: Codec, Balance: Codec, BlockNumber: Codec, Value: Codec> where
	{
		/// Determines if an account voted for a specific dispute round.
		/// # Arguments
		/// * `dispute_id` - The identifier of the dispute.
		/// * `vote_round` - The vote round.
		/// * `voter` - The account of the voter to check.
		/// # Returns
		/// Whether or not the account voted for the specific dispute round.
		fn did_vote(dispute_id: DisputeId, vote_round: u8, voter: AccountId) -> bool;

		/// Get the latest dispute fee.
		/// # Returns
		/// The latest dispute fee.
		fn get_dispute_fee() -> Balance;

		/// Returns the dispute identifiers for a reporter.
		/// # Arguments
		/// * `reporter` - The account of the reporter to check for.
		/// # Returns
		/// Dispute identifiers for a reporter.
		fn get_disputes_by_reporter(reporter: AccountId) -> Vec<DisputeId>;

		/// Returns information on a dispute for a given identifier.
		/// # Arguments
		/// * `dispute_id` - Identifier of the specific dispute.
		/// # Returns
		/// Returns information on a dispute for a given dispute identifier including:
		/// query identifier of disputed value, timestamp of disputed value, value being disputed,
		/// reporter of the disputed value.
		fn get_dispute_info(dispute_id: DisputeId) -> Option<(QueryId, Timestamp, Value, AccountId)>;

		/// Returns the number of open disputes for a specific query identifier.
		/// # Arguments
		/// * `query_id` - Identifier of a specific data feed.
		/// # Returns
		/// The number of open disputes for the query identifier.
		fn get_open_disputes_on_id(query_id: QueryId) -> u128;

		/// Returns the total number of votes
		/// # Returns
		/// The total number of votes.
		fn get_vote_count() -> u128;

		/// Returns info on a vote for a given dispute identifier.
		/// # Arguments
		/// * `dispute_id` - Identifier of a specific dispute.
		/// * `vote_round` - The vote round.
		/// # Returns
		/// Information on a vote for a given dispute identifier including: the vote identifier, the
		/// vote information, whether it has been executed, the vote result and the dispute initiator.
		fn get_vote_info(dispute_id: DisputeId, vote_round: u8) -> Option<(VoteInfo<Balance,BlockNumber, Timestamp>,bool,Option<VoteResult>,AccountId)>;

		/// Returns the voting rounds for a given dispute identifier.
		/// # Arguments
		/// * `dispute_id` - Identifier for a dispute.
		/// # Returns
		/// The number of vote rounds for the dispute identifier.
		fn get_vote_rounds(dispute_id: DisputeId) -> u8;

		/// Returns the total number of votes cast by a voter.
		/// # Arguments
		/// * `voter` - The account of the voter to check for.
		/// # Returns
		/// The total number of votes cast by the voter.
		fn get_vote_tally_by_address(voter: AccountId) -> u128;
	}
}

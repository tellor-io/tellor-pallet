use autopay::*;
use codec::Codec;
use sp_std::vec::Vec;
use tellor::{FeedDetails, Tip};

mod autopay;

sp_api::decl_runtime_apis! {
	pub trait AutoPayApi<AccountId: Codec, Amount: Codec, FeedId: Codec, QueryId: Codec, Timestamp: Codec>
	{
		/// Read current data feeds.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// # Returns
		/// Feed identifiers for query identifier.
		fn get_current_feeds(query_id: QueryId) -> Vec<FeedId>;

		/// Read current onetime tip by query identifier.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// # Returns
		/// Amount of tip.
		fn get_current_tip(query_id: QueryId) -> Amount;

		/// Read a specific data feed.
		/// # Arguments
		/// * `query_id` - Unique feed identifier of parameters.
		/// # Returns
		/// Details of the specified feed.
		fn get_data_feed(feed_id: FeedId) -> Option<FeedDetails<Amount, Timestamp>>;

		/// Read currently funded feed details.
		/// # Arguments
		/// * `query_id` - Unique feed identifier of parameters.
		/// # Returns
		/// Details of the specified feed.
		fn get_funded_feed_details(feed_id: FeedId) -> Vec<FeedDetailsWithQueryData<Amount, Timestamp>>;

		/// Read currently funded feeds.
		/// # Returns
		/// The currently funded feeds
		fn get_funded_feeds() -> Vec<FeedId>;

		/// Read query identifiers with current one-time tips.
		/// # Returns
		/// Query identifiers with current one-time tips.
		fn get_funded_query_ids() -> Vec<QueryId>;

		/// Read currently funded single tips with query data.
		/// # Returns
		/// The current single tips.
		fn get_funded_single_tips_info() -> Vec<SingleTipWithQueryData<Amount>>;

		/// Read the number of past tips for a query identifier.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// # Returns
		/// The number of past tips.
		fn get_past_tip_count(query_id: QueryId) -> u32;

		/// Read the past tips for a query identifier.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// # Returns
		/// All past tips.
		fn get_past_tips(query_id: QueryId) -> Vec<Tip<Amount, Timestamp>>;

		/// Read a past tip for a query identifier and index.
		/// # Arguments
		/// * `query_id` - Identifier of reported data.
		/// * `index` - The index of the tip.
		/// # Returns
		/// The past tip, if found.
		fn get_past_tip_by_index(query_id: QueryId, index: u32) -> Option<Tip<Amount, Timestamp>>;

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
		fn get_reward_amount(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> Amount;

		/// Read whether a reward has been claimed.
		/// # Arguments
		/// * `feed_id` - Data feed unique identifier.
		/// * `query_id` - Identifier of reported data.
		/// * `timestamp` - Timestamp of reported data.
		/// # Returns
		/// Whether a reward has been claimed, if timestamp exists.
		fn get_reward_claimed_status(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> Option<bool>;

		/// Read whether rewards have been claimed.
		/// # Arguments
		/// * `feed_id` - Data feed unique identifier.
		/// * `query_id` - Identifier of reported data.
		/// * `timestamps` - Timestamps of oracle submissions.
		/// # Returns
		/// Whether rewards have been claimed.
		fn get_reward_claim_status_list(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> Vec<Option<bool>>;

		/// Read the total amount of tips paid by a user.
		/// # Arguments
		/// * `user` - Address of user to query.
		/// # Returns
		/// Total amount of tips paid by a user.
		fn get_tips_by_address(user: AccountId) -> Amount;
	}

	pub trait OracleApi<BlockNumber: Codec, QueryId: Codec, Timestamp: Codec, Value: Codec> where
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

		// todo: add remaining functions
	}

	pub trait GovernanceApi<AccountId: Codec, DisputeId: Codec, QueryId: Codec, Timestamp: Codec> where
	{
		/// Determines if an account voted for a specific dispute.
		/// # Arguments
		/// * `dispute_id` - The identifier of the dispute.
		/// * `voter` - The account of the voter to check.
		/// # Returns
		/// Whether or not the account voted for the specific dispute.
		fn did_vote(dispute_id: DisputeId, voter: AccountId) -> Option<bool>;

		// todo: add remaining functions
	}
}

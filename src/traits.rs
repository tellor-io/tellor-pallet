use crate::types::{QueryId, Timestamp};
use sp_std::vec::Vec;
use xcm::latest::prelude::*;

// Simple trait to avoid taking a hard dependency on pallet-xcm.
pub trait SendXcm {
	fn send_xcm(
		interior: impl Into<Junctions>,
		dest: impl Into<MultiLocation>,
		message: Xcm<()>,
	) -> Result<(), SendError>;
}

/// This trait helps pallets read data from Tellor
pub trait UsingTellor<AccountId, Price> {
	/// Retrieves the next value for the query identifier after the specified timestamp.
	/// # Arguments
	/// * `query_id` - The query identifier to look up the value for.
	/// * `timestamp` - The timestamp after which to search for next value.
	/// # Returns
	/// The value retrieved, along with timestamp, if found.
	fn get_data_after(query_id: QueryId, timestamp: Timestamp) -> Option<(Vec<u8>, Timestamp)>;

	/// Retrieves the latest value for the query identifier before the specified timestamp.
	/// # Arguments
	/// * `query_id` - The query identifier to look up the value for.
	/// * `timestamp` - The timestamp before which to search for the latest value.
	/// # Returns
	/// The value retrieved and its timestamp, if found.
	fn get_data_before(query_id: QueryId, timestamp: Timestamp) -> Option<(Vec<u8>, Timestamp)>;

	/// Retrieves the latest index of data after the specified timestamp for the query identifier.
	/// # Arguments
	/// * `query_id` - The query identifier to look up the index for.
	/// * `timestamp` - The timestamp after which to search for latest index.
	/// # Returns
	/// The latest index after the specified timestamp, if found.
	fn get_index_for_data_after(query_id: QueryId, timestamp: Timestamp) -> Option<usize>;

	/// Retrieves the latest index of data before the specified timestamp for the query identifier.
	/// # Arguments
	/// * `query_id` - The query identifier to look up the index for.
	/// * `timestamp` - The timestamp before which to search for latest index.
	/// # Returns
	/// The latest index before the specified timestamp, if found.
	fn get_index_for_data_before(query_id: QueryId, timestamp: Timestamp) -> Option<usize>;

	/// Retrieves multiple values before the specified timestamp.
	/// # Arguments
	/// * `query_id` - The unique identifier of the data query.
	/// * `timestamp` - The timestamp before which to search for values.
	/// * `max_age` - The maximum number of units of time before the timestamp to search for values.
	/// * `max_count` - The maximum number of values to return.
	/// # Returns
	/// The values retrieved along with timestamp, ordered from oldest to newest, if any.
	fn get_multiple_values_before(
		query_id: QueryId,
		timestamp: Timestamp,
		max_age: Timestamp,
	) -> Vec<(Vec<u8>, Timestamp)>;

	/// Counts the number of values that have been submitted for the query identifier.
	/// # Arguments
	/// * `query_id` - The query identifier to look up.
	/// # Returns
	/// Count of the number of values received for the query identifier.
	fn get_new_value_count_by_query_id(query_id: QueryId) -> usize;

	/// Returns the reporter who submitted a value for a query identifier at a specific time.
	/// # Arguments
	/// * `query_id` - The identifier of the specific data feed.
	/// * `timestamp` - The timestamp to find a corresponding reporter for.
	/// # Returns
	/// Identifier of the reporter who reported the value for the query identifier at the given timestamp.
	fn get_reporter_by_timestamp(query_id: QueryId, timestamp: Timestamp) -> Option<AccountId>;

	/// Gets the timestamp for the value based on their index.
	/// # Arguments
	/// * `query_id` - The query identifier to look up.
	/// * `index` - The value index to look up.
	/// # Returns
	/// A timestamp if found.
	fn get_timestamp_by_query_id_and_index(query_id: QueryId, index: usize) -> Option<Timestamp>;

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
	fn retrieve_data(query_id: QueryId, timestamp: Timestamp) -> Option<Vec<u8>>;

	/// Attempts to convert value to a price.
	/// # Arguments
	/// * `value` - Value to be converted to a price.
	/// # Returns
	/// A price converted from the value, if successful.
	fn value_to_price(value: Vec<u8>) -> Option<Price>;
}

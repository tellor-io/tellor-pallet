use codec::{Decode, Encode};
use tellor::FeedDetails;

#[derive(Encode, Decode)]
pub struct FeedDetailsWithQueryData<Amount, Timestamp> {
	/// Feed details for feed identifier with funding.
	details: FeedDetails<Amount, Timestamp>,
	/// Query data for requested data
	query_data: Vec<u8>,
}

#[derive(Encode, Decode)]
pub struct SingleTipWithQueryData<Amount> {
	/// Query data with single tip for requested data.
	query_data: Vec<u8>,
	/// Reward amount for request.
	tip: Amount,
}

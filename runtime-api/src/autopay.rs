use codec::{Decode, Encode};
use sp_std::vec::Vec;
use tellor::FeedDetails;

#[derive(Encode, Debug, Decode, Eq, PartialEq)]
pub struct FeedDetailsWithQueryData<Amount, Timestamp> {
	/// Feed details for feed identifier with funding.
	pub details: FeedDetails<Amount, Timestamp>,
	/// Query data for requested data
	pub query_data: Vec<u8>,
}

#[derive(Encode, Debug, Decode, Eq, PartialEq)]
pub struct SingleTipWithQueryData<Amount> {
	/// Query data with single tip for requested data.
	pub query_data: Vec<u8>,
	/// Reward amount for request.
	pub tip: Amount,
}

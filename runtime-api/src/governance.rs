use codec::{Decode, Encode};

#[derive(Encode, Debug, Decode, Eq, PartialEq)]
pub struct VoteInfo<Amount, BlockNumber, Timestamp> {
	pub vote_round: u32,
	pub start_date: Timestamp,
	pub block_number: BlockNumber,
	pub fee: Amount,
	pub tally_date: Timestamp,
	pub users_does_support: Amount,
	pub users_against: Amount,
	pub users_invalid_query: Amount,
	pub reporters_does_support: u128,
	pub reporters_against: u128,
	pub reporters_invalid_query: u128,
}

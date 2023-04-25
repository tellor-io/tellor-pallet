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

use super::Config;
use frame_support::pallet_prelude::*;
pub(crate) use governance::Tally;
pub(crate) use sp_core::U256;
use sp_core::{bounded::BoundedBTreeMap, H160, H256};
pub(crate) use sp_runtime::traits::Keccak256;
use sp_runtime::{
	traits::{Convert, Zero},
	SaturatedConversion,
};

pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
/// Address of a reporter on controller chain.
pub type Address = H160;
/// TRB stake amount as reported from controller chain.
pub type Tributes = U256;
/// Local currency used for onetime tips, funding feeds, accumulated rewards and dispute fees.
pub(crate) type BalanceOf<T> = <T as Config>::Balance;
pub(crate) type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
pub type DisputeId = H256;
pub(crate) type DisputeOf<T> = governance::Dispute<AccountIdOf<T>, ValueOf<T>>;
pub type FeedId = H256;
pub(crate) type FeedOf<T> = autopay::Feed<BalanceOf<T>, <T as Config>::MaxRewardClaims>;
pub(crate) type FeedDetailsOf<T> = autopay::FeedDetails<BalanceOf<T>>;
pub(crate) type Nonce = u128;
pub(crate) type ParaId = u32;
pub(crate) type PriceOf<T> = <T as Config>::Price;
pub(crate) type QueryDataOf<T> = BoundedVec<u8, <T as Config>::MaxQueryDataLength>;
pub type QueryId = H256;
pub(crate) type ReportOf<T> =
	oracle::Report<AccountIdOf<T>, BlockNumberOf<T>, ValueOf<T>, <T as Config>::MaxTimestamps>;
pub(crate) type StakeInfoOf<T> =
	oracle::StakeInfo<BalanceOf<T>, <T as Config>::MaxQueriesPerReporter>;
pub type Timestamp = u64;
pub(crate) type TipOf<T> = autopay::Tip<BalanceOf<T>>;
pub(crate) type ValueOf<T> = BoundedVec<u8, <T as Config>::MaxValueLength>;
pub(crate) type VoteOf<T> =
	governance::Vote<AccountIdOf<T>, BalanceOf<T>, BlockNumberOf<T>, <T as Config>::MaxVotes>;

pub(crate) mod autopay {
	use super::*;

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(MaxRewardClaims))]
	pub struct Feed<Balance, MaxRewardClaims: Get<u32>> {
		pub(crate) details: FeedDetails<Balance>,
		/// Tracks which tips were already paid out.
		pub(crate) reward_claimed: BoundedBTreeMap<Timestamp, bool, MaxRewardClaims>,
	}

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct FeedDetails<Balance> {
		/// Amount paid for each eligible data submission.
		pub(crate) reward: Balance,
		/// Account remaining balance.
		pub(crate) balance: Balance,
		/// Time of first payment window.
		pub(crate) start_time: Timestamp,
		/// Time between pay periods.
		pub(crate) interval: Timestamp,
		/// Amount of time data can be submitted per interval.
		pub(crate) window: Timestamp,
		/// Change in price necessitating an update 100 = 1%.
		pub(crate) price_threshold: u16,
		/// Amount reward increases per second within the window (0 for flat rewards).
		pub(crate) reward_increase_per_second: Balance,
		/// Index plus one of data feed identifier in FeedsWithFunding storage (0 if not included).
		pub(crate) feeds_with_funding_index: u32,
	}

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Tip<Balance> {
		/// Amount tipped.
		pub(crate) amount: Balance,
		/// Time tipped.
		pub(crate) timestamp: Timestamp,
		/// Cumulative tips for query identifier.
		pub(crate) cumulative_tips: Balance,
	}
}

pub(crate) mod oracle {
	use super::*;

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(MaxTimestamps))]
	pub struct Report<AccountId, BlockNumber, Value, MaxTimestamps: Get<u32>> {
		/// All timestamps reported.
		pub(crate) timestamps: BoundedVec<Timestamp, MaxTimestamps>,
		/// Mapping of timestamps to respective indices.
		pub(crate) timestamp_index: BoundedBTreeMap<Timestamp, u32, MaxTimestamps>,
		/// Mapping of timestamp to block number.
		pub(crate) timestamp_to_block_number:
			BoundedBTreeMap<Timestamp, BlockNumber, MaxTimestamps>,
		/// Mapping of timestamps to values.
		pub(crate) value_by_timestamp: BoundedBTreeMap<Timestamp, Value, MaxTimestamps>,
		/// Mapping of timestamps to reporters.
		pub(crate) reporter_by_timestamp: BoundedBTreeMap<Timestamp, AccountId, MaxTimestamps>,
		/// Mapping of timestamps to whether they have been disputed.
		pub(crate) is_disputed: BoundedBTreeMap<Timestamp, bool, MaxTimestamps>,
	}

	impl<AccountId, BlockNumber, Value, MaxTimestamps: Get<u32>>
		Report<AccountId, BlockNumber, Value, MaxTimestamps>
	{
		pub(crate) fn new() -> Self {
			Report {
				timestamps: BoundedVec::default(),
				timestamp_index: BoundedBTreeMap::default(),
				timestamp_to_block_number: BoundedBTreeMap::default(),
				value_by_timestamp: BoundedBTreeMap::default(),
				reporter_by_timestamp: BoundedBTreeMap::default(),
				is_disputed: BoundedBTreeMap::default(),
			}
		}
	}

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(MaxQueries))]
	pub struct StakeInfo<Balance, MaxQueries: Get<u32>> {
		/// The address on the staking chain.
		pub(crate) address: Address,
		/// Stake or withdrawal request start date.
		pub(crate) start_date: Timestamp,
		/// Staked token balance
		pub(crate) staked_balance: Tributes,
		/// Amount locked for withdrawal.
		pub(crate) locked_balance: Tributes,
		/// Used for staking reward calculation.
		pub(crate) reward_debt: Balance,
		/// Timestamp of reporter's last reported value.
		pub(crate) reporter_last_timestamp: Timestamp,
		/// Total number of reports submitted by reporter.
		pub(crate) reports_submitted: u128,
		/// Total number of governance votes when stake deposited.
		pub(crate) start_vote_count: u128,
		/// Staker vote tally when stake deposited.
		pub(crate) start_vote_tally: u128,
		/// Used to keep track of total stakers.
		pub(crate) staked: bool,
		/// Mapping of query identifier to number of reports submitted by reporter.
		pub(crate) reports_submitted_by_query_id: BoundedBTreeMap<QueryId, u128, MaxQueries>,
	}

	impl<Balance: Zero, MaxQueries: Get<u32>> StakeInfo<Balance, MaxQueries> {
		pub(crate) fn new(address: Address) -> Self {
			Self {
				address,
				start_date: Zero::zero(),
				staked_balance: U256::zero(),
				locked_balance: U256::zero(),
				reward_debt: Zero::zero(),
				reporter_last_timestamp: Zero::zero(),
				reports_submitted: 0,
				start_vote_count: 0,
				start_vote_tally: 0,
				staked: false,
				reports_submitted_by_query_id: BoundedBTreeMap::default(),
			}
		}
	}
}

pub(crate) mod governance {
	use super::*;

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Dispute<AccountId, Value> {
		/// Query identifier of disputed value
		pub(crate) query_id: QueryId,
		/// Timestamp of disputed value.
		pub(crate) timestamp: Timestamp,
		/// Disputed value.
		pub(crate) value: Value,
		/// Reporter who submitted the disputed value.
		pub(crate) disputed_reporter: AccountId,
		/// Amount slashed from reporter.
		pub(crate) slashed_amount: Tributes,
	}

	#[derive(
		Clone, Encode, Decode, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen,
	)]
	pub struct Tally<Number> {
		/// Number of votes in favor.
		pub does_support: Number,
		/// Number of votes against.
		pub against: Number,
		/// Number of votes for invalid.
		pub invalid_query: Number,
	}

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(MaxVotes))]
	pub struct Vote<AccountId, Balance, BlockNumber, MaxVotes: Get<u32>> {
		/// Identifier of the dispute.
		pub identifier: DisputeId,
		/// The round of voting on a given dispute or proposal.
		pub vote_round: u8,
		/// Timestamp of when vote was initiated.
		pub start_date: Timestamp,
		/// Block number of when vote was initiated.
		pub block_number: BlockNumber,
		/// Fee paid to initiate the vote round.
		pub fee: Balance,
		/// Timestamp of when the votes were tallied.
		pub tally_date: Timestamp,
		/// Vote tally of users.
		pub users: Tally<Balance>,
		/// Vote tally of reporters.
		pub reporters: Tally<u128>,
		/// Whether the vote was sent to be tallied.
		pub sent: bool,
		/// Whether the vote was executed.
		pub executed: bool,
		/// Result after votes were tallied.
		pub result: Option<VoteResult>,
		/// Address which initiated dispute/proposal.
		pub initiator: AccountId,
		/// Mapping of accounts to whether they voted or not.
		pub(crate) voted: BoundedBTreeMap<AccountId, bool, MaxVotes>,
	}

	/// The status of a potential vote.
	#[derive(Clone, Copy, Encode, Debug, Decode, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
	pub enum VoteResult {
		Failed,
		Passed,
		Invalid,
	}
}

pub(super) struct U256ToBalance<T>(PhantomData<T>);
impl<T: Config> Convert<U256, BalanceOf<T>> for U256ToBalance<T> {
	fn convert(a: U256) -> BalanceOf<T> {
		a.saturated_into::<u128>().saturated_into()
	}
}

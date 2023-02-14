use super::Config;
use frame_support::{pallet_prelude::*, traits::Time};
use sp_core::{bounded::BoundedBTreeMap, H160, U256};

pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type Address = H160;
pub(crate) type Amount = U256;
pub(crate) type AmountOf<T> = <T as Config>::Amount;
pub(crate) type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
pub(crate) type DisputeIdOf<T> = <T as Config>::DisputeId;
pub(crate) type DisputeOf<T> =
	governance::Dispute<AccountIdOf<T>, QueryIdOf<T>, TimestampOf<T>, ValueOf<T>>;
pub(crate) type FeedIdOf<T> = <T as Config>::Hash;
pub(crate) type FeedDetailsOf<T> = autopay::FeedDetails<AccountIdOf<T>, TimestampOf<T>>;
pub(crate) type HashOf<T> = <T as Config>::Hash;
pub(crate) type HasherOf<T> = <T as Config>::Hasher;
pub(crate) type MomentOf<T> = <<T as Config>::Time as Time>::Moment;
pub(crate) type Nonce = u128;
pub(crate) type ParaId = u32;
pub(crate) type QueryDataOf<T> = BoundedVec<u8, <T as Config>::MaxQueryDataLength>;
pub(crate) type QueryIdOf<T> = <T as Config>::Hash;
pub(crate) type ReportOf<T> = oracle::Report<
	AccountIdOf<T>,
	BlockNumberOf<T>,
	TimestampOf<T>,
	ValueOf<T>,
	<T as Config>::MaxTimestamps,
>;
pub(crate) type StakeInfoOf<T> = oracle::StakeInfo<
	AmountOf<T>,
	<T as Config>::MaxQueriesPerReporter,
	QueryIdOf<T>,
	TimestampOf<T>,
>;
pub(crate) type Timestamp = U256;
pub(crate) type TimestampOf<T> = <<T as Config>::Time as Time>::Moment;
pub(crate) type TipOf<T> = autopay::Tip<AmountOf<T>, TimestampOf<T>>;
pub(crate) type ValueOf<T> = BoundedVec<u8, <T as Config>::MaxValueLength>;
pub(crate) type VoteIdOf<T> = <T as Config>::Hash;

pub mod autopay {

	use super::*;

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Feed<Amount, Timestamp, MaxRewards: Get<u32>> {
		details: FeedDetails<Amount, Timestamp>,
		/// Tracks which tips were already paid out.
		reward_claimed: BoundedBTreeMap<Timestamp, bool, MaxRewards>,
	}

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct FeedDetails<Amount, Timestamp> {
		/// Amount paid for each eligible data submission.
		reward: Amount,
		/// Account remaining balance.
		balance: Amount,
		/// Time of first payment window.
		start_time: Timestamp,
		/// Time between pay periods.
		interval: u8,
		/// Amount of time data can be submitted per interval.
		window: u8,
		/// Change in price necessitating an update 100 = 1%.
		price_threshold: u16,
		/// Amount reward increases per second within the window (0 for flat rewards).
		reward_increase_per_second: Amount,
		/// Index plus one of data feed identifier in FeedsWithFunding storage (0 if not included).
		feeds_with_funding_index: u32,
	}

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Tip<Amount, Timestamp> {
		/// Amount tipped.
		pub(crate) amount: Amount,
		/// Time tipped.
		pub(crate) timestamp: Timestamp,
		/// Cumulative tips for query identifier.
		pub(crate) cumulative_tips: Amount,
	}
}

pub(crate) mod oracle {
	use super::*;

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(MaxTimestamps))]
	pub struct Report<AccountId, BlockNumber, Timestamp, Value, MaxTimestamps: Get<u32>> {
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

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(MaxQueries))]
	pub struct StakeInfo<Amount, MaxQueries: Get<u32>, QueryId, Timestamp> {
		/// The address on the staking chain.
		pub(crate) address: Address,
		/// Stake or withdrawal request start date.
		pub(crate) start_date: Timestamp,
		/// Staked token balance
		pub(crate) staked_balance: Amount,
		/// Amount locked for withdrawal.
		pub(crate) locked_balance: Amount,
		/// Used for staking reward calculation.
		pub(crate) reward_debt: Amount,
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
}

mod governance {
	use super::*;

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub(crate) struct Dispute<AccountId, QueryId, Timestamp, Value> {
		/// Query identifier of disputed value
		pub(crate) query_id: QueryId,
		/// Timestamp of disputed value.
		pub(crate) timestamp: Timestamp,
		/// Disputed value.
		pub(crate) value: Value,
		/// Reporter who submitted the disputed value.
		pub(crate) dispute_reporter: AccountId,
	}

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	struct Vote<AccountId, Amount, BlockNumber, Timestamp, VoteId, MaxVotes: Get<u32>> {
		/// Identifier of the vote.
		identifier: VoteId,
		/// The round of voting on a given dispute or proposal.
		vote_round: u8,
		/// Timestamp of when vote was initiated.
		start_date: Timestamp,
		/// Block number of when vote was initiated.
		block_number: BlockNumber,
		/// Fee paid to initiate the vote round.
		fee: Amount,
		/// Address which initiated dispute/proposal.
		initiator: AccountId,
		/// Mapping of accounts to whether they voted or not.
		voted: BoundedBTreeMap<AccountId, bool, MaxVotes>,
	}

	/// The status of a potential vote.
	enum VoteResult {
		Failed,
		Passed,
		Invalid,
	}
}

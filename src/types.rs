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
pub use sp_core::U256;
use sp_core::{H160, H256};
pub(crate) use sp_runtime::traits::Keccak256;
use sp_runtime::{traits::Convert, SaturatedConversion};
use sp_std::vec::Vec;

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
pub(crate) type FeedOf<T> = autopay::Feed<BalanceOf<T>>;
pub(crate) type Nonce = u32;
pub(crate) type ParaId = u32;
pub(crate) type QueryDataOf<T> = BoundedVec<u8, <T as Config>::MaxQueryDataLength>;
pub type QueryId = H256;
pub(crate) type StakeInfoOf<T> = oracle::StakeInfo<BalanceOf<T>>;
pub type Timestamp = u64;
pub(crate) type TipOf<T> = autopay::Tip<BalanceOf<T>>;
pub(crate) type ValueOf<T> = BoundedVec<u8, <T as Config>::MaxValueLength>;
pub(crate) type VoteOf<T> = governance::Vote<AccountIdOf<T>, BalanceOf<T>, BlockNumberOf<T>>;

pub(crate) mod autopay {
	use super::*;

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Feed<Balance> {
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

	#[derive(
		Clone, Default, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen,
	)]
	pub struct StakeInfo<Balance> {
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
		pub(crate) reports_submitted: u32,
		/// Total number of governance votes when stake deposited.
		pub(crate) start_vote_count: u64,
		/// Staker vote tally when stake deposited.
		pub(crate) start_vote_tally: u32,
		/// Used to keep track of total stakers.
		pub(crate) staked: bool,
	}

	impl<Balance: Default> StakeInfo<Balance> {
		pub(crate) fn new(address: Address) -> Self {
			Self { address, ..Default::default() }
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
	pub struct Vote<AccountId, Balance, BlockNumber> {
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
	}

	/// The status of a potential vote.
	#[derive(Clone, Copy, Encode, Debug, Decode, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
	pub enum VoteResult {
		Failed,
		Passed,
		Invalid,
	}
}

/// Storing weights of extrinsics, required in parachain registration
#[derive(Clone, Encode, Decode, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Weights {
	pub report_stake_deposited: u64,
	pub report_staking_withdraw_request: u64,
	pub report_stake_withdrawn: u64,
	pub report_vote_tallied: u64,
	pub report_vote_executed: u64,
	pub report_slash: u64,
}

pub(super) struct U256ToBalance<T>(PhantomData<T>);
impl<T: Config> Convert<U256, BalanceOf<T>> for U256ToBalance<T> {
	fn convert(a: U256) -> BalanceOf<T> {
		a.saturated_into::<u128>().saturated_into()
	}
}

pub struct BytesToU256;
impl Convert<Vec<u8>, Option<U256>> for BytesToU256 {
	fn convert(b: Vec<u8>) -> Option<U256> {
		// From https://github.com/tellor-io/usingtellor/blob/cfc56240e0f753f452d2f376b5ab126fa95222ad/contracts/UsingTellor.sol#L357
		let mut number = Some(U256::zero());
		for i in b {
			number = number
				.and_then(|n| n.checked_mul(256.into()))
				.and_then(|n| n.checked_add(i.into()))
		}
		number
	}
}

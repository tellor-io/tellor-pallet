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

use crate::{
	autopay::{FeedDetailsWithQueryData, SingleTipWithQueryData},
	governance::VoteInfo,
	TellorAutoPay, TellorGovernance, TellorOracle,
};
use codec::Encode;
use frame_support::{
	parameter_types,
	traits::{ConstU16, ConstU64},
	BoundedVec, PalletId,
};
use sp_api::mock_impl_runtime_apis;
use sp_core::{ConstU128, ConstU32, H256};
use sp_runtime::{
	generic::BlockId,
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use std::time::{SystemTime, UNIX_EPOCH};
use tellor::{
	Amount, DisputeId, EnsureGovernance, EnsureStaking, FeedDetails, FeedId, QueryId, Timestamp,
	Tip, VoteResult,
};
use xcm::latest::prelude::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

type AccountId = u64;
type BlockNumber = u64;
type MaxValueLength = ConstU32<4>;
type StakeInfo = tellor::StakeInfo<<Test as tellor::Config>::MaxQueriesPerReporter>;
type Value = BoundedVec<u8, MaxValueLength>;

// Configure a mock runtime to test implementation of the runtime-api
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances,
		Time: pallet_timestamp,
		Tellor: tellor,
	}
);
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}
impl pallet_balances::Config for Test {
	type Balance = u128;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
}
impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<1>;
	type WeightInfo = ();
}
parameter_types! {
	pub const TellorPalletId: PalletId = PalletId(*b"py/tellr");
}
impl tellor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Fee = ();
	type Governance = ();
	type GovernanceOrigin = EnsureGovernance;
	type MaxClaimTimestamps = ();
	type MaxFeedsPerQuery = ();
	type MaxFundedFeeds = ();
	type MaxQueriesPerReporter = ConstU32<100>;
	type MaxQueryDataLength = ();
	type MaxRewardClaims = ();
	type MaxTimestamps = ();
	type MaxTipsPerQuery = ();
	type MaxValueLength = MaxValueLength;
	type MaxVotes = ();
	type PalletId = TellorPalletId;
	type ParachainId = ();
	type Price = u32;
	type RegistrationOrigin = frame_system::EnsureRoot<AccountId>;
	type Registry = ();
	type Staking = ();
	type StakingOrigin = EnsureStaking;
	type Time = Time;
	type Token = Balances;
	type ValueConverter = ();
	type Xcm = TestSendXcm;
}
pub struct TestSendXcm;
impl tellor::traits::SendXcm for TestSendXcm {
	fn send_xcm(
		_interior: impl Into<Junctions>,
		_dest: impl Into<MultiLocation>,
		_message: Xcm<()>,
	) -> Result<(), SendError> {
		todo!()
	}
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}

mock_impl_runtime_apis! {
	impl crate::TellorAutoPay<Block, AccountId> for Test {
		fn get_current_feeds(query_id: QueryId) -> Vec<FeedId>{
			tellor::Pallet::<Test>::get_current_feeds(query_id)
		}

		fn get_current_tip(query_id: QueryId) -> Amount {
			tellor::Pallet::<Test>::get_current_tip(query_id)
		}

		fn get_data_feed(feed_id: FeedId) -> Option<FeedDetails> {
			tellor::Pallet::<Test>::get_data_feed(feed_id)
		}

		fn get_funded_feed_details() -> Vec<FeedDetailsWithQueryData> {
			tellor::Pallet::<Test>::get_funded_feed_details().into_iter()
			.map(|(details, query_data)| FeedDetailsWithQueryData {
				details: details,
				query_data: query_data.to_vec()})
			.collect()
		}

		fn get_funded_feeds() -> Vec<FeedId> {
			tellor::Pallet::<Test>::get_funded_feeds()
		}

		fn get_funded_query_ids() -> Vec<QueryId>{
			tellor::Pallet::<Test>::get_funded_query_ids()
		}

		fn get_funded_single_tips_info() -> Vec<SingleTipWithQueryData> {
			tellor::Pallet::<Test>::get_funded_single_tips_info().into_iter()
			.map(|( query_data, tip)| SingleTipWithQueryData {
				query_data: query_data.to_vec(),
				tip
			})
			.collect()
		}

		fn get_past_tip_count(query_id: QueryId) -> u32 {
			tellor::Pallet::<Test>::get_past_tip_count(query_id)
		}

		fn get_past_tips(query_id: QueryId) -> Vec<Tip> {
			tellor::Pallet::<Test>::get_past_tips(query_id)
		}

		fn get_past_tip_by_index(query_id: QueryId, index: u32) -> Option<Tip>{
			tellor::Pallet::<Test>::get_past_tip_by_index(query_id, index)
		}

		fn get_query_id_from_feed_id(feed_id: FeedId) -> Option<QueryId>{
			tellor::Pallet::<Test>::get_query_id_from_feed_id(feed_id)
		}

		fn get_reward_amount(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> Amount{
			tellor::Pallet::<Test>::get_reward_amount(feed_id, query_id, timestamps)
		}

		fn get_reward_claimed_status(feed_id: FeedId, query_id: QueryId, timestamp: Timestamp) -> Option<bool>{
			tellor::Pallet::<Test>::get_reward_claimed_status(feed_id, query_id, timestamp)
		}

		fn get_reward_claim_status_list(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> Vec<bool>{
			tellor::Pallet::<Test>::get_reward_claim_status_list(feed_id, query_id, timestamps)
		}

		fn get_tips_by_address(user: AccountId) -> Amount {
			tellor::Pallet::<Test>::get_tips_by_address(&user)
		}
	}

	impl crate::TellorOracle<Block, AccountId, BlockNumber, StakeInfo, Value> for Test {
		fn get_block_number_by_timestamp(query_id: QueryId, timestamp: Timestamp) -> Option<BlockNumber> {
			tellor::Pallet::<Test>::get_block_number_by_timestamp(query_id, timestamp)
		}

		fn get_current_value(query_id: QueryId) -> Option<Value> {
			tellor::Pallet::<Test>::get_current_value(query_id)
		}

		fn get_data_before(query_id: QueryId, timestamp: Timestamp) -> Option<(Value, Timestamp)>{
			tellor::Pallet::<Test>::get_data_before(query_id, timestamp)
		}

		fn get_new_value_count_by_query_id(query_id: QueryId) -> u32 {
			tellor::Pallet::<Test>::get_new_value_count_by_query_id(query_id) as u32
		}

		fn get_report_details(query_id: QueryId, timestamp: Timestamp) -> Option<(AccountId, bool)>{
			tellor::Pallet::<Test>::get_report_details(query_id, timestamp)
		}

		fn get_reporter_by_timestamp(query_id: QueryId, timestamp: Timestamp) -> Option<AccountId>{
			tellor::Pallet::<Test>::get_reporter_by_timestamp(query_id, timestamp)
		}

		fn get_reporter_last_timestamp(reporter: AccountId) -> Option<Timestamp>{
			tellor::Pallet::<Test>::get_reporter_last_timestamp(reporter)
		}

		fn get_reporting_lock() -> Timestamp {
			tellor::Pallet::<Test>::get_reporting_lock()
		}

		fn get_reports_submitted_by_address(reporter: AccountId) -> u128 {
			tellor::Pallet::<Test>::get_reports_submitted_by_address(&reporter)
		}

		fn get_reports_submitted_by_address_and_query_id(reporter: AccountId, query_id: QueryId) -> u128 {
			tellor::Pallet::<Test>::get_reports_submitted_by_address_and_query_id(reporter, query_id)
		}

		fn get_stake_amount() -> Amount {
			tellor::Pallet::<Test>::get_stake_amount()
		}

		fn get_staker_info(staker: AccountId) -> Option<StakeInfo>{
			tellor::Pallet::<Test>::get_staker_info(staker)
		}

		fn get_time_of_last_new_value() -> Option<Timestamp> {
			tellor::Pallet::<Test>::get_time_of_last_new_value()
		}

		fn get_timestamp_by_query_id_and_index(query_id: QueryId, index: u32) -> Option<Timestamp>{
			tellor::Pallet::<Test>::get_timestamp_by_query_id_and_index(query_id, index as usize)
		}

		fn get_index_for_data_before(query_id: QueryId, timestamp: Timestamp) -> Option<u32> {
			tellor::Pallet::<Test>::get_index_for_data_before(query_id, timestamp).map(|index| index as u32)
		}

		fn get_timestamp_index_by_timestamp(query_id: QueryId, timestamp: Timestamp) -> Option<u32> {
			tellor::Pallet::<Test>::get_timestamp_index_by_timestamp(query_id, timestamp)
		}

		fn get_total_stake_amount() -> Amount {
			tellor::Pallet::<Test>::get_total_stake_amount()
		}

		fn get_total_stakers() -> u128 {
			tellor::Pallet::<Test>::get_total_stakers()
		}

		fn is_in_dispute(query_id: QueryId, timestamp: Timestamp) -> bool{
			tellor::Pallet::<Test>::is_in_dispute(query_id, timestamp)
		}

		fn retrieve_data(query_id: QueryId, timestamp: Timestamp) -> Option<Value>{
			tellor::Pallet::<Test>::retrieve_data(query_id, timestamp)
		}
	}

	impl crate::TellorGovernance<Block, AccountId, BlockNumber, Value> for Test {
		fn did_vote(dispute_id: DisputeId, vote_round: u8, voter: AccountId) -> bool {
			tellor::Pallet::<Test>::did_vote(dispute_id, vote_round, voter)
		}

		fn get_dispute_fee() -> Amount {
			tellor::Pallet::<Test>::get_dispute_fee()
		}

		fn get_disputes_by_reporter(reporter: AccountId) -> Vec<DisputeId> {
			tellor::Pallet::<Test>::get_disputes_by_reporter(reporter)
		}

		fn get_dispute_info(dispute_id: DisputeId) -> Option<(QueryId, Timestamp, Value, AccountId)> {
			tellor::Pallet::<Test>::get_dispute_info(dispute_id)
		}

		fn get_open_disputes_on_id(query_id: QueryId) -> u128 {
			tellor::Pallet::<Test>::get_open_disputes_on_id(query_id)
		}

		fn get_vote_count() -> u128 {
			tellor::Pallet::<Test>::get_vote_count()
		}

		fn get_vote_info(dispute_id: DisputeId, vote_round: u8) -> Option<(VoteInfo<BlockNumber>,bool,Option<VoteResult>,AccountId)> {
			tellor::Pallet::<Test>::get_vote_info(dispute_id, vote_round).map(|v| (
			VoteInfo{
					vote_round: v.vote_round,
					start_date: v.start_date,
					block_number: v.block_number,
					fee: v.fee,
					tally_date: v.tally_date,
					users_does_support: v.users.does_support,
					users_against: v.users.against,
					users_invalid_query: v.users.invalid_query,
					reporters_does_support: v.reporters.does_support,
					reporters_against: v.reporters.against,
					reporters_invalid_query: v.reporters.invalid_query,
				},
			v.executed,
			v.result,
			v.initiator))
		}

		fn get_vote_rounds(dispute_id: DisputeId) -> u8 {
			tellor::Pallet::<Test>::get_vote_rounds(dispute_id)
		}

		fn get_vote_tally_by_address(voter: AccountId) -> u128 {
			tellor::Pallet::<Test>::get_vote_tally_by_address(voter)
		}
	}
}

const BLOCKID: BlockId<Block> = BlockId::Number(0);

// Tests simply ensure that required API functions are accessible to a runtime

mod autopay {
	use super::*;

	#[test]
	fn get_current_feeds() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_current_feeds(&BLOCKID, QueryId::random()).unwrap(),
				Vec::default()
			);
		});
	}

	#[test]
	fn get_current_tip() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_current_tip(&BLOCKID, QueryId::random()).unwrap(), 0);
		});
	}

	#[test]
	fn get_data_feed() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_data_feed(&BLOCKID, FeedId::random()).unwrap(), None);
		});
	}

	#[test]
	fn get_funded_feed_details() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_funded_feed_details(&BLOCKID).unwrap(), Vec::default());
		});
	}

	#[test]
	fn get_funded_feeds() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_funded_feeds(&BLOCKID).unwrap(), Vec::default());
		});
	}

	#[test]
	fn get_funded_query_ids() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_funded_query_ids(&BLOCKID).unwrap(), Vec::default());
		});
	}

	#[test]
	fn get_funded_single_tips_info() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_funded_single_tips_info(&BLOCKID).unwrap(), Vec::default());
		});
	}

	#[test]
	fn get_past_tip_count() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_past_tip_count(&BLOCKID, QueryId::random()).unwrap(), 0);
		});
	}

	#[test]
	fn get_past_tips() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_past_tips(&BLOCKID, QueryId::random()).unwrap(), Vec::default());
		});
	}

	#[test]
	fn get_past_tip_by_index() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_past_tip_by_index(&BLOCKID, QueryId::random(), 0).unwrap(), None);
		});
	}

	#[test]
	fn get_query_id_from_feed_id() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_query_id_from_feed_id(&BLOCKID, FeedId::random()).unwrap(), None);
		});
	}

	#[test]
	fn get_reward_amount() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_reward_amount(&BLOCKID, FeedId::random(), QueryId::random(), vec![])
					.unwrap(),
				0
			);
		});
	}

	#[test]
	fn get_reward_claimed_status() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_reward_claimed_status(
					&BLOCKID,
					FeedId::random(),
					QueryId::random(),
					Time::get()
				)
				.unwrap(),
				None
			);
		});
	}

	#[test]
	fn get_reward_claim_status_list() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_reward_claim_status_list(
					&BLOCKID,
					FeedId::random(),
					QueryId::random(),
					vec![]
				)
				.unwrap(),
				Vec::<bool>::default()
			);
		});
	}

	#[test]
	fn get_tips_by_address() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_tips_by_address(&BLOCKID, AccountId::default()).unwrap(), 0);
		});
	}
}

mod oracle {
	use super::*;

	#[test]
	fn get_block_number_by_timestamp() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_block_number_by_timestamp(&BLOCKID, QueryId::random(), 0).unwrap(),
				None
			);
		});
	}

	#[test]
	fn get_current_value() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_current_value(&BLOCKID, QueryId::random()).unwrap(), None);
		});
	}

	#[test]
	fn get_data_before() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_data_before(&BLOCKID, QueryId::random(), 0).unwrap(), None);
		});
	}

	#[test]
	fn get_new_value_count_by_query_id() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_new_value_count_by_query_id(&BLOCKID, QueryId::random()).unwrap(),
				0
			);
		});
	}

	#[test]
	fn get_report_details() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_report_details(&BLOCKID, QueryId::random(), 0).unwrap(), None);
		});
	}

	#[test]
	fn get_reporter_by_timestamp() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_reporter_by_timestamp(&BLOCKID, QueryId::random(), 0).unwrap(),
				None
			);
		});
	}

	#[test]
	fn get_reporter_last_timestamp() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_reporter_last_timestamp(&BLOCKID, AccountId::default()).unwrap(),
				None
			);
		});
	}

	#[test]
	fn get_reporting_lock() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_reporting_lock(&BLOCKID).unwrap(), 43200);
		});
	}

	#[test]
	fn get_reports_submitted_by_address() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_reports_submitted_by_address(&BLOCKID, AccountId::default()).unwrap(),
				0
			);
		});
	}

	#[test]
	fn get_reports_submitted_by_address_and_query_id() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_reports_submitted_by_address_and_query_id(
					&BLOCKID,
					AccountId::default(),
					QueryId::random()
				)
				.unwrap(),
				0
			);
		});
	}

	#[test]
	fn get_stake_amount() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_stake_amount(&BLOCKID).unwrap(), 0);
		});
	}

	#[test]
	fn get_staker_info() {
		new_test_ext().execute_with(|| {
			assert!(Test.get_staker_info(&BLOCKID, 0).unwrap().is_none());
		});
	}

	#[test]
	fn get_time_of_last_new_value() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_time_of_last_new_value(&BLOCKID).unwrap(), None);
		});
	}

	#[test]
	fn get_timestamp_by_query_id_and_index() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_timestamp_by_query_id_and_index(&BLOCKID, QueryId::random(), 0)
					.unwrap(),
				None
			);
		});
	}

	#[test]
	fn get_index_for_data_before() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_index_for_data_before(&BLOCKID, QueryId::random(), 0).unwrap(),
				None
			);
		});
	}

	#[test]
	fn get_timestamp_index_by_timestamp() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_timestamp_index_by_timestamp(&BLOCKID, QueryId::random(), 0).unwrap(),
				None
			);
		});
	}

	#[test]
	fn get_total_stake_amount() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_total_stake_amount(&BLOCKID).unwrap(), 0);
		});
	}

	#[test]
	fn get_total_stakers() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_total_stakers(&BLOCKID).unwrap(), 0);
		});
	}

	#[test]
	fn is_in_dispute() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.is_in_dispute(&BLOCKID, QueryId::random(), 0).unwrap(), false);
		});
	}

	#[test]
	fn retrieve_data() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.retrieve_data(&BLOCKID, QueryId::random(), 0).unwrap(), None);
		});
	}
}

mod governance {
	use super::*;

	#[test]
	fn did_vote() {
		new_test_ext().execute_with(|| {
			assert!(!Test
				.did_vote(&BLOCKID, DisputeId::default(), 0, AccountId::default())
				.unwrap());
		});
	}

	#[test]
	fn get_dispute_fee() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_dispute_fee(&BLOCKID).unwrap(), 0);
		});
	}

	#[test]
	fn get_disputes_by_reporter() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_disputes_by_reporter(&BLOCKID, AccountId::default()).unwrap(),
				vec![]
			);
		});
	}

	#[test]
	fn get_dispute_info() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_dispute_info(&BLOCKID, DisputeId::default()).unwrap(), None);
		});
	}

	#[test]
	fn get_open_disputes_on_id() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_open_disputes_on_id(&BLOCKID, H256::random()).unwrap(), 0);
		});
	}

	#[test]
	fn get_vote_count() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_vote_count(&BLOCKID).unwrap(), 0);
		});
	}

	#[test]
	fn get_vote_info() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_vote_info(&BLOCKID, DisputeId::default(), 0).unwrap(), None);
		});
	}

	#[test]
	fn get_vote_rounds() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_vote_rounds(&BLOCKID, H256::random()).unwrap(), 0);
		});
	}

	#[test]
	fn get_vote_tally_by_address() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_vote_tally_by_address(&BLOCKID, 0).unwrap(), 0);
		});
	}
}

#[test]
#[ignore]
fn state_call_encoding() {
	fn call(api: &str, function: &str, data: &[u8]) {
		println!("{}_{}: 0x{}", api, function, hex::encode(data));
	}

	let query_id = QueryId::random();
	let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

	// Example encoding of runtime-api calls via state.call rpc
	const AUTOPAY: &str = "TellorAutoPay";
	const GOVERNANCE: &str = "TellorGovernance";

	const ORACLE: &str = "TellorOracle";
	call(ORACLE, "get_block_number_by_timestamp", &(query_id, timestamp).encode());
	call(ORACLE, "get_current_value", &query_id.encode());
}

use crate::{
	autopay::{FeedDetailsWithQueryData, SingleTipWithQueryData},
	TellorAutoPay, TellorGovernance, TellorOracle,
};
use codec::Encode;
use frame_support::{
	parameter_types,
	sp_runtime::traits::Keccak256,
	traits::{ConstU16, ConstU64},
	BoundedVec, PalletId,
};
use sp_api::mock_impl_runtime_apis;
use sp_core::{ConstU32, H256};
use sp_runtime::{
	generic::BlockId,
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use std::time::{SystemTime, UNIX_EPOCH};
use tellor::{EnsureGovernance, EnsureStaking, FeedDetails, Tip};
use xcm::latest::prelude::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

type AccountId = u64;
type Amount = u64;
type BlockNumber = u64;
type DisputeId = u128;
type QueryId = H256;
type MaxValueLength = ConstU32<4>;
type Moment = u64;
type FeedId = H256;
type StakeInfo =
	tellor::StakeInfo<Amount, <Test as tellor::Config>::MaxQueriesPerReporter, QueryId, Moment>;
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
		Timestamp: pallet_timestamp,
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
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}
impl pallet_balances::Config for Test {
	type Balance = u64;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU64<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
}
impl pallet_timestamp::Config for Test {
	type Moment = Moment;
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
	type Amount = Amount;
	type ClaimBuffer = ();
	type ClaimPeriod = ();
	type DisputeId = DisputeId;
	type Fee = ();
	type Governance = ();
	type GovernanceOrigin = EnsureGovernance;
	type Hash = H256;
	type Hasher = Keccak256;
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
	type MaxVoteRounds = ();
	type PalletId = TellorPalletId;
	type ParachainId = ();
	type Price = u32;
	type RegistrationOrigin = frame_system::EnsureRoot<AccountId>;
	type Registry = ();
	type ReportingLock = ConstU64<42>;
	type Staking = ();
	type StakingOrigin = EnsureStaking;
	type Time = Timestamp;
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
	impl crate::TellorAutoPay<Block, AccountId, Amount, FeedId, QueryId, Moment> for Test {
		fn get_current_feeds(query_id: QueryId) -> Vec<FeedId>{
			tellor::Pallet::<Test>::get_current_feeds(query_id)
		}

		fn get_current_tip(query_id: QueryId) -> Amount {
			tellor::Pallet::<Test>::get_current_tip(query_id)
		}

		fn get_data_feed(feed_id: FeedId) -> Option<FeedDetails<Amount, Moment>> {
			tellor::Pallet::<Test>::get_data_feed(feed_id)
		}

		fn get_funded_feed_details(feed_id: FeedId) -> Vec<FeedDetailsWithQueryData<Amount, Moment>> {
			tellor::Pallet::<Test>::get_funded_feed_details(feed_id).into_iter()
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

		fn get_funded_single_tips_info() -> Vec<SingleTipWithQueryData<Amount>> {
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

		fn get_past_tips(query_id: QueryId) -> Vec<Tip<Amount, Moment>> {
			tellor::Pallet::<Test>::get_past_tips(query_id)
		}

		fn get_past_tip_by_index(query_id: QueryId, index: u32) -> Option<Tip<Amount, Moment>>{
			tellor::Pallet::<Test>::get_past_tip_by_index(query_id, index)
		}

		fn get_query_id_from_feed_id(feed_id: FeedId) -> Option<QueryId>{
			tellor::Pallet::<Test>::get_query_id_from_feed_id(feed_id)
		}

		fn get_reward_amount(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Moment>) -> Amount{
			tellor::Pallet::<Test>::get_reward_amount(feed_id, query_id, timestamps)
		}

		fn get_reward_claimed_status(feed_id: FeedId, query_id: QueryId, timestamp: Moment) -> Option<bool>{
			tellor::Pallet::<Test>::get_reward_claimed_status(feed_id, query_id, timestamp)
		}

		fn get_reward_claim_status_list(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Moment>) -> Vec<Option<bool>>{
			tellor::Pallet::<Test>::get_reward_claim_status_list(feed_id, query_id, timestamps)
		}

		fn get_tips_by_address(user: AccountId) -> Amount {
			tellor::Pallet::<Test>::get_tips_by_address(user)
		}
	}

	impl crate::TellorOracle<Block, AccountId, Amount, BlockNumber, QueryId, StakeInfo, Moment, Value> for Test {
		fn get_block_number_by_timestamp(query_id: QueryId, timestamp: Moment) -> Option<BlockNumber> {
			tellor::Pallet::<Test>::get_block_number_by_timestamp(query_id, timestamp)
		}

		fn get_current_value(query_id: QueryId) -> Option<Value> {
			tellor::Pallet::<Test>::get_current_value(query_id)
		}

		fn get_data_before(query_id: QueryId, timestamp: Moment) -> Option<(Value, Moment)>{
			tellor::Pallet::<Test>::get_data_before(query_id, timestamp)
		}

		fn get_new_value_count_by_query_id(query_id: QueryId) -> u32 {
			tellor::Pallet::<Test>::get_new_value_count_by_query_id(query_id) as u32
		}

		fn get_report_details(query_id: QueryId, timestamp: Moment) -> Option<(AccountId, bool)>{
			tellor::Pallet::<Test>::get_report_details(query_id, timestamp)
		}

		fn get_reporter_by_timestamp(query_id: QueryId, timestamp: Moment) -> Option<AccountId>{
			tellor::Pallet::<Test>::get_reporter_by_timestamp(query_id, timestamp)
		}

		fn get_reporter_last_timestamp(reporter: AccountId) -> Option<Moment>{
			tellor::Pallet::<Test>::get_reporter_last_timestamp(reporter)
		}

		fn get_reporting_lock() -> Moment {
			tellor::Pallet::<Test>::get_reporting_lock()
		}

		fn get_reports_submitted_by_address(reporter: AccountId) -> u128 {
			tellor::Pallet::<Test>::get_reports_submitted_by_address(reporter)
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

		fn get_time_of_last_new_value() -> Option<Moment> {
			tellor::Pallet::<Test>::get_time_of_last_new_value()
		}

		fn get_timestamp_by_query_id_and_index(query_id: QueryId, index: u32) -> Option<Moment>{
			tellor::Pallet::<Test>::get_timestamp_by_query_id_and_index(query_id, index as usize)
		}

		fn get_index_for_data_before(query_id: QueryId, timestamp: Moment) -> Option<u32> {
			tellor::Pallet::<Test>::get_index_for_data_before(query_id, timestamp).map(|index| index as u32)
		}

		fn get_timestamp_index_by_timestamp(query_id: QueryId, timestamp: Moment) -> Option<u32> {
			tellor::Pallet::<Test>::get_timestamp_index_by_timestamp(query_id, timestamp)
		}

		fn get_total_stake_amount() -> Amount {
			tellor::Pallet::<Test>::get_total_stake_amount()
		}

		fn get_total_stakers() -> u128 {
			tellor::Pallet::<Test>::get_total_stakers()
		}

		fn is_in_dispute(query_id: QueryId, timestamp: Moment) -> bool{
			tellor::Pallet::<Test>::is_in_dispute(query_id, timestamp)
		}

		fn retrieve_data(query_id: QueryId, timestamp: Moment) -> Option<Value>{
			tellor::Pallet::<Test>::retrieve_data(query_id, timestamp)
		}
	}

	impl crate::TellorGovernance<Block, AccountId, Amount, DisputeId, QueryId, Moment> for Test {
		fn did_vote(dispute_id: DisputeId, voter: AccountId) -> Option<bool>{
			tellor::Pallet::<Test>::did_vote(dispute_id, voter)
		}

		fn get_dispute_fee() -> Amount {
			tellor::Pallet::<Test>::get_dispute_fee()
		}
	}
}

const BLOCKID: BlockId<Block> = BlockId::Number(0);

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
			assert_eq!(
				Test.get_funded_feed_details(&BLOCKID, FeedId::random()).unwrap(),
				Vec::default()
			);
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
					Timestamp::get()
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
				Vec::default()
			);
		});
	}

	#[test]
	fn get_tips_by_address() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_tips_by_address(&BLOCKID, AccountId::default()).unwrap(),
				Amount::default()
			);
		});
	}
}

mod oracle {
	use super::*;

	#[test]
	#[should_panic]
	fn get_block_number_by_timestamp() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_block_number_by_timestamp(&BLOCKID, QueryId::random(), Moment::default())
					.unwrap(),
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
			assert_eq!(
				Test.get_data_before(&BLOCKID, QueryId::random(), Moment::default()).unwrap(),
				None
			);
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
			assert_eq!(
				Test.get_report_details(&BLOCKID, QueryId::random(), Moment::default()).unwrap(),
				None
			);
		});
	}

	#[test]
	fn get_reporter_by_timestamp() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.get_reporter_by_timestamp(&BLOCKID, QueryId::random(), Moment::default())
					.unwrap(),
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
			assert_eq!(Test.get_reporting_lock(&BLOCKID).unwrap(), 42);
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
			assert_eq!(
				Test.is_in_dispute(&BLOCKID, QueryId::random(), Moment::default()).unwrap(),
				false
			);
		});
	}

	#[test]
	fn retrieve_data() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.retrieve_data(&BLOCKID, QueryId::random(), Moment::default()).unwrap(),
				None
			);
		});
	}
}

mod governance {
	use super::*;

	#[test]
	fn did_vote() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				Test.did_vote(&BLOCKID, DisputeId::default(), AccountId::default()).unwrap(),
				None
			);
		});
	}

	#[test]
	fn get_dispute_fee() {
		new_test_ext().execute_with(|| {
			assert_eq!(Test.get_dispute_fee(&BLOCKID).unwrap(), 0);
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
	const ORACLE: &str = "TellorOracle";
	call(ORACLE, "get_block_number_by_timestamp", &(query_id, timestamp).encode());
	call(ORACLE, "get_current_value", &query_id.encode());
}

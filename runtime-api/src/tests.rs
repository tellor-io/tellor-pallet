use crate::TellorOracle;
use frame_support::{
	assert_ok, parameter_types,
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

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

type BlockNumber = u64;
type QueryId = H256;
type Moment = u64;
type Value = BoundedVec<u8, ConstU32<100>>;

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
	type AccountId = u64;
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
impl tellor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Amount = u64;
	type DisputeId = u128;
	type Fee = ();
	type Governance = ();
	type Hash = H256;
	type Hasher = Keccak256;
	type MaxClaimTimestamps = ();
	type MaxFeedsPerQuery = ();
	type MaxFundedFeeds = ();
	type MaxQueriesPerReporter = ();
	type MaxQueryDataLength = ();
	type MaxTimestamps = ();
	type MaxTipsPerQuery = ();
	type MaxValueLength = ConstU32<100>;
	type MaxVotes = ();
	type PalletId = TellotPalletId;
	type ParachainId = ();
	type Registry = ();
	type ReportingLock = ();
	type Staking = ();
	type Time = Timestamp;
	type Token = Balances;
	type Xcm = ();
}
parameter_types! {
	pub const TellotPalletId: PalletId = PalletId(*b"py/tellr");
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}

mock_impl_runtime_apis! {
	impl crate::TellorOracle<Block, BlockNumber, QueryId, Moment, Value> for Test {
		fn get_block_number_by_timestamp(query_id: QueryId, timestamp: Moment) -> Option<BlockNumber> {
			tellor::Pallet::<Test>::get_block_number_by_timestamp(query_id, timestamp)
		}

		fn get_current_value(query_id: QueryId) -> Option<Value> {
			tellor::Pallet::<Test>::get_current_value(query_id)
		}
	}
}

#[test]
#[should_panic]
fn gets_block_number_by_timestamp() {
	new_test_ext().execute_with(|| {
		assert_ok!(Test.get_block_number_by_timestamp(
			&BlockId::Number(0),
			QueryId::random(),
			Moment::default()
		));
	});
}

#[test]
fn gets_current_value() {
	new_test_ext().execute_with(|| {
		assert_ok!(Test.get_current_value(&BlockId::Number(0), QueryId::random()));
	});
}

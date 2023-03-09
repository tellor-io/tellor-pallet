use crate as tellor;
use crate::types::{Address, MomentOf};
use ::xcm::latest::MultiLocation;
use frame_support::{
	assert_ok, parameter_types,
	traits::{ConstU16, ConstU64, OnFinalize},
	PalletId,
};
use frame_system as system;
use sp_core::{bounded::BoundedVec, ConstU32, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, Convert, IdentityLookup, Keccak256},
};
use sp_std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};
use xcm::latest::prelude::*;

type AccountId = u128; // u64 is not enough to hold bytes used to generate bounty account
type Balance = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) const UNIT: u64 = 1_000_000_000_000;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Tellor: tellor,
	}
);

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	// todo: enforce AccountId = u128 in pallet config
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
	type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
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
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<1>;
	type WeightInfo = ();
}

const PARA_ID: u32 = 2000;

parameter_types! {
	pub const TellorPalletId: PalletId = PalletId(*b"py/tellr");
	pub TellorRegistry: MultiLocation = crate::xcm::controller(PARA_ID, Address::random().0);
	pub TellorGovernance: MultiLocation = crate::xcm::controller(PARA_ID, Address::random().0);
	pub TellorStaking: MultiLocation = crate::xcm::controller(PARA_ID, Address::random().0);
}

pub(crate) const HOUR_IN_MILLISECONDS: u64 = 3_600_000;
const WEEK_IN_MILLISECONDS: u64 = HOUR_IN_MILLISECONDS * 168;

impl tellor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Amount = u64;
	type ClaimBuffer = ConstU64<{ 12 * HOUR_IN_MILLISECONDS }>;
	type ClaimPeriod = ConstU64<{ 4 * WEEK_IN_MILLISECONDS }>;
	type DisputeId = u32;
	type Fee = ConstU16<10>; // 1%
	type Governance = TellorGovernance;
	type Hash = H256;
	type Hasher = Keccak256;
	type MaxClaimTimestamps = ConstU32<10>;
	type MaxFeedsPerQuery = ConstU32<10>;
	type MaxFundedFeeds = ConstU32<10>;
	type MaxQueriesPerReporter = ConstU32<10>;
	type MaxQueryDataLength = ConstU32<1000>;
	type MaxRewardClaims = ConstU32<10>;
	type MaxTimestamps = ConstU32<10>;
	type MaxTipsPerQuery = ConstU32<10>;
	type MaxValueLength = ConstU32<128>; // Chain may want to store any raw bytes, so ValueConverter needs to handle conversion to price for threshold checks
	type MaxVotes = ();
	type MaxVoteRounds = ConstU32<10>;
	type PalletId = TellorPalletId;
	type ParachainId = ();
	type Price = u128;
	type RegistrationOrigin = system::EnsureRoot<AccountId>;
	type Registry = TellorRegistry;
	type ReportingLock = ConstU64<{ 12 * HOUR_IN_MILLISECONDS }>;
	type Staking = TellorStaking;
	type Time = Timestamp;
	type Token = Balances;
	type ValueConverter = ValueConverter;
	type Xcm = TestSendXcm;
}

pub struct ValueConverter;
impl Convert<BoundedVec<u8, ConstU32<128>>, Option<u128>> for ValueConverter {
	fn convert(a: BoundedVec<u8, ConstU32<128>>) -> Option<u128> {
		// Should be more advanced depending on chain config
		match a[16..].try_into() {
			Ok(v) => Some(u128::from_be_bytes(v)),
			Err(_) => None,
		}
	}
}

thread_local! {
	pub static SENT_XCM: RefCell<Vec<(MultiLocation, Xcm<()>)>> = RefCell::new(Vec::new());
}
pub fn sent_xcm() -> Vec<(MultiLocation, opaque::Xcm)> {
	SENT_XCM.with(|q| (*q.borrow()).clone())
}
/// Sender that never returns error, always sends
pub struct TestSendXcm;
impl SendXcm for TestSendXcm {
	fn send_xcm(dest: impl Into<MultiLocation>, msg: Xcm<()>) -> SendResult {
		SENT_XCM.with(|q| q.borrow_mut().push((dest.into(), msg)));
		Ok(())
	}
}
impl tellor::traits::Xcm for TestSendXcm {
	fn send_xcm(
		interior: impl Into<Junctions>,
		dest: impl Into<MultiLocation>,
		mut message: Xcm<()>,
	) -> Result<(), SendError> {
		let interior = interior.into();
		let dest = dest.into();
		if interior != Here {
			message.0.insert(0, DescendOrigin(interior))
		};
		<Self as SendXcm>::send_xcm(dest, message)
	}
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}

/// Starts a new block, executing the supplied closure thereafter.
pub(crate) fn with_block<R>(execute: impl FnOnce() -> R) -> (MomentOf<Test>, R) {
	with_block_after(0, execute)
}

/// Starts a new block after some time, executing the supplied closure thereafter.
pub(crate) fn with_block_after<R>(
	time: MomentOf<Test>,
	execute: impl FnOnce() -> R,
) -> (MomentOf<Test>, R) {
	let block = System::block_number();
	match block {
		0 => {
			System::set_block_number(1);
			assert_ok!(Timestamp::set(
				RuntimeOrigin::none(),
				SystemTime::now()
					.duration_since(UNIX_EPOCH)
					.expect("Current time is always after unix epoch; qed")
					.as_millis() as u64
			));
		},
		_ => {
			Timestamp::on_finalize(block);
			System::set_block_number(block + 1);
			assert_ok!(Timestamp::set(RuntimeOrigin::none(), Timestamp::get() + 1 + time));
		},
	}
	(Timestamp::get(), execute())
}

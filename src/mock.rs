use crate as tellor;
use crate::{
	types::{Address, MomentOf},
	xcm::ContractLocation,
	EnsureGovernance, EnsureStaking, HOUR_IN_MILLISECONDS, WEEK_IN_MILLISECONDS,
};
use frame_support::{
	assert_ok, log, parameter_types,
	traits::{ConstU16, ConstU64, OnFinalize},
	PalletId,
};
use frame_system as system;
use sp_core::{ConstU32, H256};
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
	pub TellorRegistry: ContractLocation = (PARA_ID, Address::random().into()).into();
	pub TellorGovernance: ContractLocation = (PARA_ID, Address::random().into()).into();
	pub TellorStaking: ContractLocation = (PARA_ID, Address::random().into()).into();
}

impl tellor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Amount = u64;
	type ClaimBuffer = ConstU64<{ 12 * HOUR_IN_MILLISECONDS }>;
	type ClaimPeriod = ConstU64<{ 4 * WEEK_IN_MILLISECONDS }>;
	type DisputeId = u32;
	type Fee = ConstU16<10>; // 1%
	type Governance = TellorGovernance;
	type GovernanceOrigin = EnsureGovernance;
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
	type StakingOrigin = EnsureStaking;
	type Time = Timestamp;
	type Token = Balances;
	type ValueConverter = ValueConverter;
	type Xcm = TestSendXcm;
}

pub struct ValueConverter;
impl Convert<Vec<u8>, Option<u128>> for ValueConverter {
	fn convert(a: Vec<u8>) -> Option<u128> {
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
impl tellor::traits::Xcm for TestSendXcm {
	fn send_xcm(
		interior: impl Into<Junctions>,
		dest: impl Into<MultiLocation>,
		mut message: Xcm<()>,
	) -> Result<(), SendError> {
		// From https://github.com/paritytech/polkadot/blob/645723987cf9662244be8faf4e9b63e8b9a1b3a3/xcm/pallet-xcm/src/lib.rs#L1085-L1090
		let interior = interior.into();
		let dest = dest.into();
		if interior != Junctions::Here {
			message.0.insert(0, DescendOrigin(interior))
		};
		log::trace!(target: "xcm::send_xcm", "dest: {:?}, message: {:?}", &dest, &message);

		// From https://github.com/paritytech/polkadot/blob/645723987cf9662244be8faf4e9b63e8b9a1b3a3/xcm/pallet-xcm/src/mock.rs#L154
		SENT_XCM.with(|q| q.borrow_mut().push((dest.into(), message)));
		Ok(())
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

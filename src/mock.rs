use crate as tellor;
use frame_support::traits::{ConstU16, ConstU64};
use frame_support::{parameter_types, PalletId};
use frame_system as system;
use sp_core::H256;
use sp_runtime::traits::Keccak256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

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
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<1>;
    type WeightInfo = ();
}

parameter_types! {
    pub const TellotPalletId: PalletId = PalletId(*b"py/tellr");
}

impl tellor::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type Amount = u64;
    type DisputeId = u128;
    type Fee = ();
    type Hash = H256;
    type Hasher = Keccak256;
    type MaxClaimTimestamps = ();
    type MaxFeedsPerQuery = ();
    type MaxFundedFeeds = ();
    type MaxQueriesPerReporter = ();
    type MaxQueryDataLength = ();
    type MaxTimestamps = ();
    type MaxTipsPerQuery = ();
    type MaxValueLength = ();
    type MaxVotes = ();
    type PalletId = TellotPalletId;
    type ReportingLock = ();
    type Time = Timestamp;
    type Token = Balances;
    type Xcm = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into()
}

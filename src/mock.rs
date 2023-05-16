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

use crate as tellor;
use crate::{
	constants::HOURS, types::Address, xcm::ContractLocation, EnsureGovernance, EnsureStaking,
};
use frame_support::{
	assert_ok, log, parameter_types,
	traits::{ConstU16, ConstU64, OnFinalize, UnixTime},
	Hashable, PalletId,
};
#[cfg(feature = "runtime-benchmarks")]
use frame_support::{traits::Currency, BoundedVec};
use frame_system as system;
use once_cell::sync::Lazy;
use sp_core::{ConstU128, ConstU32, ConstU8, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use sp_std::cell::RefCell;
use std::{
	convert::Into,
	time::{Duration, SystemTime, UNIX_EPOCH},
};
use xcm::latest::prelude::*;

pub(crate) type AccountId = u128; // u64 is not enough to hold bytes used to generate sub accounts
type Balance = u128;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) const EVM_PARA_ID: u32 = 2000;
pub(crate) const PALLET_INDEX: u8 = 3;
pub(crate) const PARA_ID: u32 = 3000;

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
		Tellor: tellor = 3
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
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
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

pub(crate) static REGISTRY: Lazy<[u8; 20]> = Lazy::new(|| Address::random().into());
pub(crate) static GOVERNANCE: Lazy<[u8; 20]> = Lazy::new(|| Address::random().into());
pub(crate) static STAKING: Lazy<[u8; 20]> = Lazy::new(|| Address::random().into());

parameter_types! {
	pub const MinimumStakeAmount: u128 = 100 * 10u128.pow(18); // 100 TRB
	pub const TellorPalletId: PalletId = PalletId(*b"py/tellr");
	pub const ParachainId: u32 = PARA_ID;
	pub TellorRegistry: ContractLocation = (EVM_PARA_ID, *REGISTRY).into();
	pub TellorGovernance: ContractLocation = (EVM_PARA_ID, *GOVERNANCE).into();
	pub TellorStaking: ContractLocation = (EVM_PARA_ID, *STAKING).into();
	pub StakingTokenPriceQueryId: H256 = H256([211,194,112,119,36,198,191,243,89,99,24,187,3,60,229,109,166,126,119,8,208,251,201,107,66,216,126,12,172,199,241,136]);
	pub StakingToLocalTokenPriceQueryId: H256 = H256([252, 212, 53, 69, 139, 47, 79, 224, 14, 207, 98, 192, 81, 195, 123, 170, 138, 241, 23, 4, 53, 70, 22, 191, 191, 171, 11, 101, 130, 16, 61, 30]);
	pub XcmFeesAsset : AssetId = AssetId::Concrete(PalletInstance(3).into()); // Balances pallet on EVM parachain
	pub FeeLocation : Junctions = Junctions::Here;
}

impl tellor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Asset = Balances;
	type Balance = Balance;
	type Decimals = ConstU8<12>;
	type Fee = ConstU16<10>; // 1%
	type FeeLocation = FeeLocation;
	type Governance = TellorGovernance;
	type GovernanceOrigin = EnsureGovernance;
	type InitialDisputeFee = ConstU128<{ 50 * 10u128.pow(12) }>; // (100 TRB / 10) * 5, where TRB 1:5 OCP
	type MaxClaimTimestamps = ConstU32<100>; // 100 timestamps per claim
	type MaxQueryDataLength = ConstU32<1024>;
	type MaxValueLength = ConstU32<256>;
	type MaxVotes = ConstU32<10>; // 10 votes max when voting on multiple disputes
	type MinimumStakeAmount = MinimumStakeAmount;
	type PalletId = TellorPalletId;
	type ParachainId = ParachainId;
	type RegisterOrigin = system::EnsureRoot<AccountId>;
	type Registry = TellorRegistry;
	type StakeAmountCurrencyTarget = ConstU128<{ 500 * 10u128.pow(18) }>;
	type Staking = TellorStaking;
	type StakingOrigin = EnsureStaking;
	type StakingTokenPriceQueryId = StakingTokenPriceQueryId;
	type StakingToLocalTokenPriceQueryId = StakingToLocalTokenPriceQueryId;
	type Time = Timestamp;
	type UpdateStakeAmountInterval = ConstU64<{ 12 * HOURS }>;
	type WeightToFee = ConstU128<10_000>;
	type Xcm = TestSendXcm;
	type XcmFeesAsset = XcmFeesAsset;
	type XcmWeightToAsset = ConstU128<50_000>; // Moonbase Alpha: https://github.com/PureStake/moonbeam/blob/f19ba9de013a1c789425d3b71e8a92d54f2191af/runtime/moonbase/src/lib.rs#L135
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = TestBenchmarkHelper;
	type WeightInfo = ();
}

thread_local! {
	pub static SENT_XCM: RefCell<Vec<(MultiLocation, Xcm<()>)>> = RefCell::new(Vec::new());
}
pub fn sent_xcm() -> Vec<(MultiLocation, opaque::Xcm)> {
	SENT_XCM.with(|q| (*q.borrow()).clone())
}
/// Sender that never returns error, always sends
pub struct TestSendXcm;
impl tellor::traits::SendXcm for TestSendXcm {
	fn send_xcm(
		interior: impl Into<Junctions>,
		dest: impl Into<MultiLocation>,
		mut message: Xcm<()>,
	) -> Result<XcmHash, SendError> {
		// From https://github.com/paritytech/polkadot/blob/1203b2519fed1727256556fb879c6c03c27a830d/xcm/pallet-xcm/src/lib.rs#L1450
		let interior = interior.into();
		let dest = dest.into();
		let _maybe_fee_payer: Option<MultiLocation> = if interior != Junctions::Here {
			message.0.insert(0, DescendOrigin(interior));
			Some(interior.into())
		} else {
			None
		};
		log::debug!(target: "xcm::send_xcm", "dest: {:?}, message: {:?}", &dest, &message);

		// From https://github.com/paritytech/polkadot/blob/645723987cf9662244be8faf4e9b63e8b9a1b3a3/xcm/pallet-xcm/src/mock.rs#L154
		let xcm_hash = message.twox_256();
		SENT_XCM.with(|q| q.borrow_mut().push((dest.into(), message)));
		Ok(xcm_hash)
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub struct TestBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl<MaxQueryDataLength: sp_core::Get<u32>>
	tellor::traits::BenchmarkHelper<AccountId, MaxQueryDataLength> for TestBenchmarkHelper
{
	fn set_time(time_in_secs: u64) {
		let block = System::block_number();
		match block {
			0 => {
				System::set_block_number(1);
				let timestamp = (<Timestamp as UnixTime>::now() +
					Duration::from_secs(1 + time_in_secs))
				.as_millis() as u64;
				pallet_timestamp::Now::<Test>::put(timestamp);
			},
			_ => {
				System::set_block_number(block + 1);
				let timestamp = (<Timestamp as UnixTime>::now() +
					Duration::from_secs(1 + time_in_secs))
				.as_millis() as u64;
				pallet_timestamp::Now::<Test>::put(timestamp);
			},
		}
	}

	fn set_balance(account_id: AccountId, amount: u128) {
		Balances::make_free_balance_be(&account_id, Balance::from_be(amount));
	}

	fn get_staking_token_price_query_data() -> BoundedVec<u8, MaxQueryDataLength> {
		BoundedVec::truncate_from(vec![
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 83, 112, 111, 116, 80, 114, 105, 99, 101, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 3, 116, 114, 98, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 103, 98, 112, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
		])
	}

	fn get_staking_to_local_token_price_query_data() -> BoundedVec<u8, MaxQueryDataLength> {
		BoundedVec::truncate_from(vec![
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 83, 112, 111, 116, 80, 114, 105, 99, 101, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 3, 116, 114, 98, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 111, 99, 112, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
		])
	}
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}

/// Starts a new block, executing the supplied closure thereafter.
pub(crate) fn with_block<R>(execute: impl FnOnce() -> R) -> R {
	with_block_after(0, execute)
}

/// Starts a new block after some time, executing the supplied closure thereafter.
pub(crate) fn with_block_after<R>(time_in_secs: u64, execute: impl FnOnce() -> R) -> R {
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
			assert_ok!(Timestamp::set(
				RuntimeOrigin::none(),
				(<Timestamp as UnixTime>::now() + Duration::from_secs(1 + time_in_secs)).as_millis()
					as u64
			));
		},
	}
	let result = execute();
	// Reset events after block executed, ensuring we only receive events for current block
	System::reset_events();
	SENT_XCM.with(|q| q.borrow_mut().clear());
	result
}

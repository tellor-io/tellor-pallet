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
	constants::DECIMALS,
	contracts::{gas_limits, registry},
	mock,
	mock::*,
	types::{
		AccountIdOf, Address, BalanceOf, DisputeId, QueryDataOf, QueryId, Timestamp, Tributes,
		ValueOf,
	},
	xcm::{ethereum_xcm, gas_to_weight, weigh, DbWeight, XcmConfig},
	Event, Origin, StakeAmount,
};
use ethabi::{Bytes, Token, Uint};
use frame_support::{
	assert_noop, assert_ok,
	traits::{Get, PalletInfoAccess, UnixTime},
};
use sp_core::{bytes::to_hex, keccak_256, H256, U256};
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin},
	ArithmeticError,
};
use std::convert::Into;
use xcm::{latest::prelude::*, DoubleEncoded};

mod autopay;
mod governance;
mod oracle;

type Balance = <Test as crate::Config>::Balance;
type Config = crate::types::Configuration;
type Configuration = crate::pallet::Configuration<Test>;
type Error = crate::Error<Test>;
type U256ToBalance = crate::types::U256ToBalance<Test>;

const MINIMUM_STAKE_AMOUNT: u128 = 100 * TRB;
const TRB: u128 = 10u128.pow(DECIMALS);

fn trb(amount: impl Into<f64>) -> Tributes {
	// TRB amount has 18 decimals
	Tributes::from((amount.into() * TRB as f64) as u128)
}

fn dispute_id(para_id: u32, query_id: QueryId, timestamp: Timestamp) -> DisputeId {
	keccak_256(&ethabi::encode(&[
		Token::Uint(para_id.into()),
		Token::FixedBytes(query_id.0.to_vec()),
		Token::Uint(timestamp.into()),
	]))
	.into()
}

// Returns the timestamp for the current block.
fn now() -> crate::types::Timestamp {
	<mock::Timestamp as UnixTime>::now().as_secs()
}

fn submit_value_and_begin_dispute(
	reporter: AccountIdOf<Test>,
	query_id: QueryId,
	query_data: QueryDataOf<Test>,
) -> DisputeId {
	assert_ok!(Tellor::submit_value(
		RuntimeOrigin::signed(reporter),
		query_id,
		uint_value(10),
		0,
		query_data
	));
	assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now(), None));

	match System::events().last().unwrap().event {
		RuntimeEvent::Tellor(Event::<Test>::NewDispute { dispute_id, .. }) => dispute_id,
		_ => panic!(),
	}
}

fn deposit_stake(reporter: AccountIdOf<Test>, amount: impl Into<Tributes>, address: Address) {
	assert_ok!(Tellor::report_stake_deposited(
		Origin::Staking.into(),
		reporter,
		amount.into(),
		address
	));
}

// Configures the parachain for remote transact calls to controller contracts
fn configure() {
	let self_reserve = MultiLocation { parents: 0, interior: X1(PalletInstance(3)) };
	assert_ok!(Tellor::configure(
		RuntimeOrigin::root(),
		Box::new(MultiAsset { id: Concrete(self_reserve), fun: Fungible(300_000_000_000_000u128) }),
		WeightLimit::Unlimited,
		u64::MAX,
		u128::MAX
	));
}

fn spot_price(asset: impl Into<String>, currency: impl Into<String>) -> Bytes {
	ethabi::encode(&[
		Token::String("SpotPrice".to_string()),
		Token::Bytes(ethabi::encode(&[
			Token::String(asset.into()),
			Token::String(currency.into()),
		])),
	])
}

fn token(amount: impl Into<f64>) -> Balance {
	// test parachain token
	(amount.into() * unit() as f64) as u64
}

fn uint_value(value: impl Into<Uint>) -> ValueOf<Test> {
	ethabi::encode(&[Token::Uint(value.into())]).try_into().unwrap()
}

// A unit of the token configured on the pallet, with corresponding decimal places.
fn unit() -> u128 {
	let decimals: u8 = <Test as crate::Config>::Decimals::get();
	10u128.pow(decimals.into())
}

fn xcm_transact(call: DoubleEncoded<RuntimeCall>, gas_limit: u64) -> Vec<(MultiLocation, Xcm<()>)> {
	// Calculate weights and fees to construct xcm
	let xt_weight = gas_to_weight(gas_limit) + DbWeight::get().reads(1);
	let total_weight = weigh() + xt_weight;
	let fees = MultiAsset {
		id: Concrete(MultiLocation { parents: 0, interior: X1(PalletInstance(3)) }), // Balances pallet for simplicity
		fun: Fungible(total_weight.ref_time() as u128 * 50_000),
	};
	vec![(
		MultiLocation { parents: 1, interior: X1(Parachain(EVM_PARA_ID)) },
		Xcm(vec![
			DescendOrigin(X1(PalletInstance(PALLET_INDEX))), // interior
			WithdrawAsset(fees.clone().into()),
			BuyExecution { fees, weight_limit: Limited(total_weight.ref_time()) },
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: xt_weight.ref_time(),
				call: call.into(),
			},
		]),
	)]
}

#[test]
fn configures() {
	let fees = Box::new(MultiAsset {
		id: Concrete(MultiLocation { parents: 0, interior: X1(PalletInstance(3)) }),
		fun: Fungible(300_000_000_000_000u128),
	});
	let weight_limit = WeightLimit::Limited(123456);
	let require_weight_at_most = u64::MAX;
	let gas_limit = u128::MAX;

	new_test_ext().execute_with(|| {
		with_block(|| {
			for origin in
				vec![RuntimeOrigin::signed(0), Origin::Governance.into(), Origin::Staking.into()]
			{
				assert_noop!(
					Tellor::configure(origin, fees.clone(), WeightLimit::Unlimited, 0, 0),
					BadOrigin
				);
			}

			assert_ok!(Tellor::configure(
				RuntimeOrigin::root(),
				fees.clone(),
				weight_limit.clone(),
				require_weight_at_most,
				gas_limit
			));
			assert_eq!(
				Configuration::get().unwrap(),
				Config {
					xcm_config: XcmConfig {
						fees: *fees.clone(),
						weight_limit: weight_limit.clone(),
						require_weight_at_most
					},
					gas_limit
				}
			);
			System::assert_last_event(Event::Configured {}.into())
		});
	});
}

#[test]
fn converts_token() {
	assert_eq!(token(2.97), 2_970_000_000_000)
}

#[test]
fn converts() {
	assert_eq!(Tellor::convert(trb(100)).unwrap(), token(100).into())
}

#[test]
fn converts_to_decimals() {
	assert_eq!(
		Tellor::convert_to_decimals((100 * 10u128.pow(18)).into(), 12),
		Ok((100 * 10u64.pow(12)).into())
	);
	assert_eq!(
		Tellor::convert_to_decimals((100 * 10u128.pow(18)).into(), 20),
		Ok((100 * 10u128.pow(20)).into())
	);
	assert_eq!(
		Tellor::convert_to_decimals((100 * 10u128.pow(18)).into(), 18),
		Ok((100 * 10u128.pow(18)).into())
	);
	assert_eq!(Tellor::convert_to_decimals((100 * 10u128.pow(18)).into(), 0), Ok(100.into()));
	assert_eq!(Tellor::convert_to_decimals(U256::zero(), u32::MAX), Ok(0.into()));
	assert_eq!(
		Tellor::convert_to_decimals((100 * 10u128.pow(18)).into(), u8::MAX.into()),
		Err(ArithmeticError::Overflow.into())
	);
	assert_eq!(
		Tellor::convert_to_decimals(U256::MAX, u32::MAX),
		Err(ArithmeticError::Overflow.into())
	);
}

#[test]
fn dispute_fees() {
	assert_eq!(
		Tellor::dispute_fees(),
		<Test as crate::Config>::PalletId::get().into_sub_account_truncating(b"dispute")
	)
}

#[test]
fn encodes_spot_price() {
	assert_eq!(
		"0xa6f013ee236804827b77696d350e9f0ac3e879328f2a3021d473a0b778ad78ac",
		to_hex(&keccak_256(&spot_price("btc", "usd")), false)
	)
}

#[test]
fn registers() {
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| configure()));

	ext.execute_with(|| {
		with_block(|| {
			for origin in
				vec![RuntimeOrigin::signed(0), Origin::Governance.into(), Origin::Staking.into()]
			{
				assert_noop!(Tellor::register(origin), BadOrigin);
			}

			assert_ok!(Tellor::register(RuntimeOrigin::root()));
			assert_eq!(
				sent_xcm(),
				xcm_transact(
					ethereum_xcm::transact(
						*REGISTRY,
						registry::register(PARA_ID, PALLET_INDEX).try_into().unwrap(),
						gas_limits::REGISTER
					)
					.into(),
					gas_limits::REGISTER
				)
			);
			System::assert_last_event(
				Event::RegistrationAttempted {
					para_id: EVM_PARA_ID,
					contract_address: (*REGISTRY).into(),
				}
				.into(),
			)
		});
	});
}

#[test]
fn staking_rewards() {
	assert_eq!(
		Tellor::staking_rewards(),
		<Test as crate::Config>::PalletId::get().into_sub_account_truncating(b"staking")
	)
}

#[test]
fn tips() {
	assert_eq!(
		Tellor::tips(),
		<Test as crate::Config>::PalletId::get().into_sub_account_truncating(b"tips")
	)
}

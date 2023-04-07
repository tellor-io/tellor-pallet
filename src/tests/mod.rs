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
	contracts::registry,
	mock,
	mock::*,
	types::{
		AccountIdOf, Address, Amount, AmountOf, DisputeId, QueryDataOf, QueryId, Timestamp, ValueOf,
	},
	xcm::{ethereum_xcm, XcmConfig},
	Event, Origin, StakeAmount,
};
use ethabi::{Bytes, Token, Uint};
use frame_support::{
	assert_noop, assert_ok,
	traits::{PalletInfoAccess, UnixTime},
};
use sp_core::{bytes::to_hex, keccak_256, H256};
use sp_runtime::traits::BadOrigin;
use xcm::{latest::prelude::*, DoubleEncoded};

mod autopay;
mod governance;
mod oracle;

type Config = crate::types::Configuration;
type Configuration = crate::pallet::Configuration<Test>;
type Error = crate::Error<Test>;

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
	assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));

	match System::events().last().unwrap().event {
		RuntimeEvent::Tellor(Event::<Test>::NewDispute { dispute_id, .. }) => dispute_id,
		_ => panic!(),
	}
}

fn deposit_stake(reporter: AccountIdOf<Test>, amount: impl Into<Amount>, address: Address) {
	assert_ok!(Tellor::report_stake_deposited(
		Origin::Staking.into(),
		reporter,
		amount.into(),
		address
	));
}

const STAKE_AMOUNT: AmountOf<Test> = 100 * UNIT;
fn register_parachain(stake_amount: AmountOf<Test>) {
	let self_reserve = MultiLocation { parents: 0, interior: X1(PalletInstance(3)) };
	assert_ok!(Tellor::register(
		RuntimeOrigin::root(),
		stake_amount,
		Box::new(MultiAsset { id: Concrete(self_reserve), fun: Fungible(300_000_000_000_000u128) }),
		WeightLimit::Unlimited,
		1000,
		1000
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

fn token(amount: impl Into<f64>) -> AmountOf<Test> {
	(amount.into() * UNIT as f64) as u64
}

fn uint_value(value: impl Into<Uint>) -> ValueOf<Test> {
	ethabi::encode(&[Token::Uint(value.into())]).try_into().unwrap()
}

fn xcm_transact(
	call: DoubleEncoded<RuntimeCall>,
	fees: Box<MultiAsset>,
	weight_limit: WeightLimit,
	require_weight_at_most: u64,
) -> Vec<(MultiLocation, Xcm<()>)> {
	vec![(
		MultiLocation { parents: 1, interior: X1(Parachain(EVM_PARA_ID)) },
		Xcm(vec![
			DescendOrigin(X1(PalletInstance(PALLET_INDEX))), // interior
			WithdrawAsset((*fees.clone()).into()),
			BuyExecution { fees: *fees, weight_limit },
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most,
				call: call.into(),
			},
		]),
	)]
}

#[test]
fn converts_token() {
	assert_eq!(token(2.97), 2_970_000_000_000)
}

#[test]
fn encodes_spot_price() {
	assert_eq!(
		"0xa6f013ee236804827b77696d350e9f0ac3e879328f2a3021d473a0b778ad78ac",
		to_hex(&keccak_256(&spot_price("btc", "usd")), false)
	)
}

#[test]
fn register() {
	let fees = Box::new(MultiAsset {
		id: Concrete(MultiLocation { parents: 0, interior: X1(PalletInstance(3)) }),
		fun: Fungible(300_000_000_000_000u128),
	});
	let weight_limit = WeightLimit::Limited(123456);
	let require_weight_at_most = u64::MAX;
	let gas_limit = u128::MAX;
	let mut ext = new_test_ext();

	ext.execute_with(|| {
		with_block(|| {
			for origin in
				vec![RuntimeOrigin::signed(0), Origin::Governance.into(), Origin::Staking.into()]
			{
				assert_noop!(Tellor::register(origin, 0, fees.clone(), Unlimited, 0, 0), BadOrigin);
			}

			assert_ok!(Tellor::register(
				RuntimeOrigin::root(),
				STAKE_AMOUNT,
				fees.clone(),
				weight_limit.clone(),
				require_weight_at_most,
				gas_limit
			));
			assert_eq!(StakeAmount::<Test>::get().unwrap(), STAKE_AMOUNT);
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
			System::assert_has_event(Event::Configured { stake_amount: STAKE_AMOUNT }.into());

			assert_eq!(
				sent_xcm(),
				xcm_transact(
					ethereum_xcm::transact(
						*REGISTRY,
						registry::register(PARA_ID, PALLET_INDEX).try_into().unwrap(),
						gas_limit,
						None,
					)
					.into(),
					fees,
					weight_limit,
					require_weight_at_most,
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

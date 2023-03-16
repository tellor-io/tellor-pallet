use crate::{
	mock::*,
	types::{AccountIdOf, Address, Amount, AmountOf, DisputeIdOf, QueryDataOf, QueryIdOf, ValueOf},
	Event, Origin,
};
use ethabi::{Bytes, Token, Uint};
use frame_support::{assert_ok, traits::PalletInfoAccess};
use sp_core::{bytes::to_hex, keccak_256, H256};
use xcm::latest::prelude::*;

mod autopay;
mod governance;
mod oracle;

type Error = crate::Error<Test>;

fn submit_value_and_begin_dispute(
	reporter: AccountIdOf<Test>,
	query_id: QueryIdOf<Test>,
	query_data: QueryDataOf<Test>,
) -> DisputeIdOf<Test> {
	assert_ok!(Tellor::submit_value(
		RuntimeOrigin::signed(reporter),
		query_id,
		uint_value(10),
		0,
		query_data
	));
	assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, Timestamp::get()));

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

#[test]
fn encodes_spot_price() {
	assert_eq!(
		"0xa6f013ee236804827b77696d350e9f0ac3e879328f2a3021d473a0b778ad78ac",
		to_hex(&keccak_256(&spot_price("btc", "usd")), false)
	)
}

#[test]
fn converts_token() {
	assert_eq!(token(2.97), 2_970_000_000_000)
}

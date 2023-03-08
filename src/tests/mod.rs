use crate::{
	mock::*,
	types::{AccountIdOf, Address, Amount, AmountOf, QueryDataOf, ValueOf},
	Event, Origin,
};
use ethabi::{Bytes, Token, Uint};
use frame_support::{assert_ok, traits::PalletInfoAccess};
use sp_core::{bytes::to_hex, keccak_256, H256};
use xcm::prelude::{DescendOrigin, PalletInstance, X1};

mod autopay;

type Error = crate::Error<Test>;

#[test]
fn reports_stake_deposited() {
	new_test_ext().execute_with(|| {
		with_block(|| {
			let reporter = 1;
			let amount: Amount = 42.into();
			let address = Address::random();
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				amount,
				address
			));

			System::assert_last_event(
				Event::NewStakerReported { staker: reporter, amount: amount.low_u64(), address }
					.into(),
			);
		});
	});
}

#[test]
fn begins_dispute() {
	new_test_ext().execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);

			let reporter = 1;
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());

			let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
			let query_id = keccak_256(query_data.as_ref()).into();
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(123),
				0,
				query_data
			));

			let timestamp = Timestamp::now();
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, timestamp));

			let sent_messages = sent_xcm();
			let (_, sent_message) = sent_messages.first().unwrap();
			assert!(sent_message
				.0
				.contains(&DescendOrigin(X1(PalletInstance(Tellor::index() as u8)))));
			// todo: check remaining instructions

			System::assert_last_event(
				Event::NewDispute { dispute_id: 1, query_id, timestamp, reporter }.into(),
			);
		});
	});
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
	assert_ok!(Tellor::register(RuntimeOrigin::root(), stake_amount, 1000, 1000));
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

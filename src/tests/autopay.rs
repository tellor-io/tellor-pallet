use super::*;
use crate::types::QueryDataOf;
use frame_support::{
	assert_noop, assert_ok,
	traits::{fungible::Inspect, Currency},
};
use sp_core::{bounded_vec, keccak_256};
use sp_runtime::traits::BadOrigin;

#[test]
fn tip() {
	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L199
	new_test_ext().execute_with(|| {
		register_parachain(STAKE_AMOUNT);

		let reporter = 2;
		deposit_stake(reporter, STAKE_AMOUNT, Address::random());

		let tipper = 1;
		let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let amount = 100;

		assert_noop!(
			Tellor::tip(RuntimeOrigin::root(), H256::random(), amount, query_data.clone()),
			BadOrigin
		);
		assert_noop!(
			Tellor::tip(RuntimeOrigin::signed(tipper), H256::random(), amount, query_data.clone()),
			Error::InvalidQueryId
		);
		assert_noop!(
			Tellor::tip(RuntimeOrigin::signed(tipper), query_id, 0, query_data.clone()),
			Error::InvalidAmount
		);
		assert_noop!(
			Tellor::tip(RuntimeOrigin::signed(tipper), query_id, amount, query_data.clone()),
			<pallet_balances::Error<Test>>::InsufficientBalance
		);

		Balances::make_free_balance_be(&tipper, 1000);
		assert_ok!(Tellor::tip(
			RuntimeOrigin::signed(tipper),
			query_id,
			amount,
			query_data.clone()
		));
		assert_eq!(Tellor::get_current_tip(query_id), amount, "tip 1 should be correct");
		assert_eq!(Balances::balance(&tipper), 900);
		assert_ok!(Tellor::submit_value(
			RuntimeOrigin::signed(reporter),
			query_id,
			uint_value(3550),
			0,
			query_data.clone()
		));

		next_block(); // required for a timestamp that is newer than last submitted value

		assert_ok!(Tellor::tip(RuntimeOrigin::signed(tipper), query_id, 200, query_data.clone()));
		assert_eq!(Tellor::get_current_tip(query_id), 200, "tip 2 should be correct");
		assert_eq!(Balances::balance(&tipper), 700);
		assert_ok!(Tellor::tip(RuntimeOrigin::signed(tipper), query_id, 300, query_data.clone()));
		assert_eq!(Tellor::get_current_tip(query_id), 500, "tip 3 should be correct");
		assert_eq!(Balances::balance(&tipper), 400);

		// test query data storage
		assert_eq!(
			Tellor::get_query_data(query_id).unwrap(),
			query_data,
			"query data not stored correctly"
		);
		let query_data: QueryDataOf<Test> = spot_price("btc", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		assert_ok!(Tellor::tip(RuntimeOrigin::signed(tipper), query_id, 10, query_data.clone()));
		assert_eq!(
			Tellor::get_query_data(query_id).unwrap(),
			query_data,
			"query data not stored correctly"
		);
		assert_ok!(Tellor::submit_value(
			RuntimeOrigin::signed(reporter),
			query_id,
			uint_value(3550),
			0,
			query_data.clone()
		));
		assert_ok!(Tellor::tip(RuntimeOrigin::signed(tipper), query_id, 10, query_data.clone()));
		assert_eq!(
			Tellor::get_query_data(query_id).unwrap(),
			query_data,
			"query data not stored correctly"
		);
	});
}

#[test]
fn claim_onetime_tip() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();

	new_test_ext()
		// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L229
		.execute_with(|| {
			register_parachain(STAKE_AMOUNT);

			let tipper = 1;
			let reporter = 2;
			let another_reporter = 4;

			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
			deposit_stake(another_reporter, STAKE_AMOUNT, Address::random());

			let start_balance = Balances::balance(&reporter);
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				0,
				query_data.clone()
			));

			let timestamp = Timestamp::get();
			assert_noop!(
				Tellor::claim_onetime_tip(RuntimeOrigin::root(), query_id, bounded_vec![timestamp]),
				BadOrigin
			);
			assert_noop!(
				Tellor::claim_onetime_tip(
					RuntimeOrigin::signed(reporter),
					query_id,
					bounded_vec![timestamp]
				),
				Error::NoTipsSubmitted
			);

			Balances::make_free_balance_be(&tipper, 1000);
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				100,
				query_data.clone()
			));

			assert_noop!(
				Tellor::claim_onetime_tip(
					RuntimeOrigin::signed(reporter),
					query_id,
					bounded_vec![timestamp]
				),
				Error::ClaimBufferNotPassed
			);

			assert_noop!(
				Tellor::claim_onetime_tip(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					bounded_vec![timestamp]
				),
				Error::TimestampIneligibleForTip
			);

			// todo: complete test once reference tests updated to check returned errors
		})
}

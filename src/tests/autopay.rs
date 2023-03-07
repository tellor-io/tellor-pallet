use super::*;
use crate::types::{FeedIdOf, QueryDataOf, QueryIdOf, TimestampOf};
use frame_support::{
	assert_noop, assert_ok,
	traits::{fungible::Inspect, Currency},
};
use sp_core::{bounded::BoundedVec, bounded_vec, keccak_256};
use sp_runtime::traits::{AccountIdConversion, BadOrigin};

#[test]
fn claim_tip() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;
	let feed_creator = 10;
	let mut feed_id = H256::zero();
	let mut timestamps = BoundedVec::default();
	let mut bad_timestamps = BoundedVec::default();
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		register_parachain(STAKE_AMOUNT);
		deposit_stake(reporter, STAKE_AMOUNT, Address::random());
		deposit_stake(another_reporter, STAKE_AMOUNT, Address::random());

		Balances::make_free_balance_be(&feed_creator, token(1_000) + 1);
		feed_id = create_feed(
			feed_creator,
			query_id,
			token(1),
			Timestamp::get(),
			3600,
			600,
			0,
			0,
			query_data.clone(),
			0,
		);

		assert_ok!(Tellor::submit_value(
			RuntimeOrigin::signed(reporter),
			query_id,
			uint_value(3500),
			0,
			query_data.clone(),
		));
		timestamps.try_push(Timestamp::get()).unwrap();
		next_block();

		assert_ok!(Tellor::submit_value(
			RuntimeOrigin::signed(another_reporter),
			query_id,
			uint_value(3525),
			1,
			query_data.clone(),
		));
		bad_timestamps.try_push(Timestamp::get()).unwrap();
		next_block();

		assert_ok!(Tellor::submit_value(
			RuntimeOrigin::signed(reporter),
			query_id,
			uint_value(3550),
			2,
			query_data.clone(),
		));
		// Note: timestamp not added as per reference test
		next_block_with_timestamp(Timestamp::get() + { 3600 * 1000 });

		assert_ok!(Tellor::submit_value(
			RuntimeOrigin::signed(reporter),
			query_id,
			uint_value(3550),
			3,
			query_data.clone(),
		));
		timestamps.try_push(Timestamp::get()).unwrap();
		bad_timestamps.try_push(Timestamp::get()).unwrap();
		next_block_with_timestamp(Timestamp::get() + { 3600 * 1000 });

		assert_ok!(Tellor::submit_value(
			RuntimeOrigin::signed(reporter),
			query_id,
			uint_value(3575),
			4,
			query_data,
		));
		timestamps.try_push(Timestamp::get()).unwrap();
		next_block();
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L74
	ext.execute_with(|| {
		// Require Checks
		assert_noop!(
			Tellor::claim_tip(RuntimeOrigin::root(), feed_id, query_id, bounded_vec![]),
			BadOrigin
		);
		assert_noop!(
			Tellor::claim_tip(
				RuntimeOrigin::signed(reporter),
				H256::random(),
				query_id,
				bounded_vec![]
			),
			Error::InvalidFeed
		);
		assert_noop!(
			Tellor::claim_tip(RuntimeOrigin::signed(reporter), feed_id, query_id, bounded_vec![]),
			Error::InsufficientFeedBalance
		);
		assert_ok!(Tellor::fund_feed(
			RuntimeOrigin::signed(feed_creator),
			feed_id,
			query_id,
			token(1_000)
		));
		assert_noop!(
			Tellor::claim_tip(
				RuntimeOrigin::signed(reporter),
				feed_id,
				query_id,
				timestamps.clone()
			),
			Error::ClaimBufferNotPassed
		);
		// Advancing time 12 hours to satisfy hardcoded buffer time.
		next_block_with_timestamp(Timestamp::get() + (12 * HOUR_IN_MILLISECONDS) + 1);
		// Expect throw cause of bad timestamp values.
		assert_noop!(
			Tellor::claim_tip(RuntimeOrigin::signed(reporter), feed_id, query_id, bad_timestamps),
			Error::InvalidClaimer
		);
		// Testing Events emitted and claiming tips for later checks.
		assert_noop!(
			Tellor::claim_tip(
				RuntimeOrigin::signed(another_reporter),
				feed_id,
				query_id,
				timestamps.clone()
			),
			Error::InvalidClaimer
		);
		let payer_before = Tellor::get_data_feed(feed_id).unwrap();
		assert_ok!(Tellor::claim_tip(
			RuntimeOrigin::signed(reporter),
			feed_id,
			query_id,
			timestamps
		));
		System::assert_last_event(
			Event::TipClaimed { feed_id, query_id, amount: token(3), reporter }.into(),
		);
		let payer_after = Tellor::get_data_feed(feed_id).unwrap();
		assert!(payer_before.balance != payer_after.balance);
		assert_eq!(payer_after.balance, token(997));
		// Updating Balance Checks
		// 1% of each tip being shaved for Tellor ~= .01 token/tip claimed
		// That's why tellor balance is .03 lower than originally expected.
		assert_eq!(Balances::balance(&reporter), token(2.97));
		// Checking if owner (Tellor) account was updated by fee amount (0.03)
		let pallet_id = <Test as crate::Config>::PalletId::get();
		assert_eq!(
			Balances::balance(&pallet_id.into_sub_account_truncating(b"staking")),
			token(0.03)
		);
		assert_eq!(Balances::balance(&pallet_id.into_account_truncating()), token(997));

		// // Require Checks
		// // Advancing time 12 hours to satisfy hardcoded buffer time.
		// await h.expectThrow(autopay.connect(accounts[1]).claimTip(bytesId, ETH_QUERY_ID, array));//bufferTime not passed
		// await h.advanceTime(43200);
		// // Expect throw cause of bad timestamp values.
		// await h.expectThrow(autopay.connect(accounts[1]).claimTip(bytesId, ETH_QUERY_ID, badArray));
		// // Testing Events emitted and claiming tips for later checks.
		// await h.expectThrow(autopay.connect(accounts[2]).claimTip(bytesId, ETH_QUERY_ID, array));//not reporter
		// await expect(autopay.connect(accounts[1]).claimTip(bytesId, ETH_QUERY_ID, array)).to.emit(autopay, "TipClaimed").withArgs(bytesId, ETH_QUERY_ID, (h.toWei("3")), accounts[1].address);
		// let payerAfter = await autopay.getDataFeed(bytesId);
		// expect(payerBefore.balance).to.not.equal(payerAfter.balance);
		// expect(payerAfter.balance).to.equal(h.toWei("997"));
		// // Updating Balance Checks
		// // 1% of each tip being shaved for Tellor ~= .01 token/tip claimed
		// // That's why tellor balance is .03 lower than originally expected.
		// expect(await tellor.balanceOf(accounts[1].address)).to.equal(h.toWei("2.97"));
		// // Checking if owner (Tellor) account was updated by fee amount (0.03)
		// expect(await tellor.balanceOf(await tellor.address)).to.equal(h.toWei("0.03"));
		// expect(await tellor.balanceOf(autopay.address)).to.equal(h.toWei("997"));
	});
}

#[test]
#[ignore]
fn _get_reward_amount() {
	todo!()
}

#[test]
fn fund_feed() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let feed_creator = 1;
	let feed_funder = 2;
	let mut feed_id = H256::zero();
	let amount = token(1_000_000);
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		feed_id = create_feed(
			feed_creator,
			query_id,
			token(1),
			Timestamp::get(),
			3600,
			600,
			1,
			3,
			query_data.clone(),
			0,
		);
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L134
	ext.execute_with(|| {
		// Require checks
		assert_noop!(
			Tellor::fund_feed(RuntimeOrigin::none(), H256::random(), H256::random(), amount),
			BadOrigin
		);
		assert_noop!(
			Tellor::fund_feed(
				RuntimeOrigin::signed(feed_funder),
				H256::random(),
				H256::random(),
				amount
			),
			Error::InvalidFeed
		);
		assert_noop!(
			Tellor::fund_feed(RuntimeOrigin::signed(feed_funder), feed_id, query_id, amount),
			pallet_balances::Error::<Test>::InsufficientBalance
		);

		// Variable updates
		Balances::make_free_balance_be(&feed_funder, amount * 3);
		assert_ok!(Tellor::fund_feed(
			RuntimeOrigin::signed(feed_funder),
			feed_id,
			query_id,
			amount
		));
		let feed = Tellor::get_data_feed(feed_id).unwrap();
		assert_eq!(amount, feed.balance);

		// Event details
		let pallet_account = <Test as crate::Config>::PalletId::get().into_account_truncating();
		let initial_balance = Balances::balance(&pallet_account);
		assert_ok!(Tellor::fund_feed(
			RuntimeOrigin::signed(feed_funder),
			feed_id,
			query_id,
			amount
		));
		let feed_details = Tellor::get_data_feed(feed_id).unwrap();
		System::assert_last_event(
			Event::DataFeedFunded { query_id, feed_id, amount, feed_funder, feed_details }.into(),
		);
		assert_eq!(
			Balances::balance(&pallet_account) - initial_balance,
			amount,
			"balance should change"
		);
	});
}

#[test]
fn setup_data_feed() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let feed_creator = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		let timestamp = Timestamp::get();
		create_feed(
			feed_creator,
			query_id,
			token(1),
			timestamp,
			600,
			60,
			0,
			0,
			query_data.clone(),
			0,
		);
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L155
	ext.execute_with(|| {
		let timestamp = Timestamp::get();
		assert_noop!(
			Tellor::setup_data_feed(
				RuntimeOrigin::none(),
				query_id,
				token(1),
				timestamp,
				3600,
				600,
				0,
				0,
				query_data.clone(),
				0
			),
			BadOrigin
		);
		assert_noop!(
			Tellor::setup_data_feed(
				RuntimeOrigin::signed(feed_creator),
				H256::random(),
				token(1),
				timestamp,
				3600,
				600,
				0,
				0,
				query_data.clone(),
				0
			),
			Error::InvalidQueryId
		);
		assert_noop!(
			Tellor::setup_data_feed(
				RuntimeOrigin::signed(feed_creator),
				query_id,
				token(1),
				timestamp,
				600,
				60,
				0,
				0,
				query_data.clone(),
				0
			),
			Error::FeedAlreadyExists
		);
		assert_noop!(
			Tellor::setup_data_feed(
				RuntimeOrigin::signed(feed_creator),
				query_id,
				0,
				timestamp,
				3600,
				600,
				0,
				0,
				query_data.clone(),
				0
			),
			Error::InvalidReward
		);
		assert_noop!(
			Tellor::setup_data_feed(
				RuntimeOrigin::signed(feed_creator),
				query_id,
				token(1),
				timestamp,
				0,
				600,
				0,
				0,
				query_data.clone(),
				0
			),
			Error::InvalidInterval
		);
		assert_noop!(
			Tellor::setup_data_feed(
				RuntimeOrigin::signed(feed_creator),
				query_id,
				token(1),
				timestamp,
				0,
				600,
				0,
				0,
				query_data.clone(),
				0
			),
			Error::InvalidInterval
		);
		assert_noop!(
			Tellor::setup_data_feed(
				RuntimeOrigin::signed(feed_creator),
				query_id,
				token(1),
				timestamp,
				600,
				3600,
				0,
				0,
				query_data.clone(),
				0
			),
			Error::InvalidWindow
		);

		let feed_id = create_feed(
			feed_creator,
			query_id,
			token(1),
			timestamp,
			3600,
			600,
			1,
			3,
			query_data.clone(),
			0,
		);
		System::assert_last_event(
			Event::NewDataFeed { query_id, feed_id, query_data: query_data.clone(), feed_creator }
				.into(),
		);
		let result = Tellor::get_data_feed(feed_id).unwrap();
		assert_eq!(result.reward, token(1));
		assert_eq!(result.balance, 0);
		assert_eq!(result.start_time, timestamp);
		assert_eq!(result.interval, 3600);
		assert_eq!(result.window, 600);
		assert_eq!(result.price_threshold, 1);
		assert_eq!(result.reward_increase_per_second, 3);
		assert_eq!(result.feeds_with_funding_index, 0);

		Balances::make_free_balance_be(&feed_creator, token(100));
		create_feed(
			feed_creator,
			query_id,
			token(1),
			timestamp,
			7600,
			600,
			2,
			4,
			query_data.clone(),
			token(10),
		);

		let query_data: QueryDataOf<Test> = spot_price("btc", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();

		create_feed(
			feed_creator,
			query_id,
			token(1),
			timestamp,
			3600,
			600,
			1,
			3,
			query_data.clone(),
			0,
		);
		assert_eq!(
			query_data,
			Tellor::get_query_data(query_id).unwrap(),
			"query data not stored correctly"
		);

		// setup second feed for same query id
		create_feed(
			feed_creator,
			query_id,
			token(1),
			timestamp,
			3600,
			1200,
			1,
			3,
			query_data.clone(),
			0,
		);
		assert_eq!(
			query_data,
			Tellor::get_query_data(query_id).unwrap(),
			"query data not stored correctly"
		);
	});
}

#[test]
#[ignore]
fn get_reward_claimed_status() {
	todo!()
}

#[test]
fn tip() {
	let reporter = 2;
	let tipper = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let amount = token(100);
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		register_parachain(STAKE_AMOUNT);
		deposit_stake(reporter, STAKE_AMOUNT, Address::random());
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L199
	ext.execute_with(|| {
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

		Balances::make_free_balance_be(&tipper, token(1000));
		assert_ok!(Tellor::tip(
			RuntimeOrigin::signed(tipper),
			query_id,
			amount,
			query_data.clone()
		));
		assert_eq!(Tellor::get_current_tip(query_id), amount, "tip 1 should be correct");
		assert_eq!(Balances::balance(&tipper), token(900));
		assert_ok!(Tellor::submit_value(
			RuntimeOrigin::signed(reporter),
			query_id,
			uint_value(3550),
			0,
			query_data.clone()
		));

		next_block(); // required for a timestamp that is newer than last submitted value

		assert_ok!(Tellor::tip(
			RuntimeOrigin::signed(tipper),
			query_id,
			token(200),
			query_data.clone()
		));
		assert_eq!(Tellor::get_current_tip(query_id), token(200), "tip 2 should be correct");
		assert_eq!(Balances::balance(&tipper), token(700));
		assert_ok!(Tellor::tip(
			RuntimeOrigin::signed(tipper),
			query_id,
			token(300),
			query_data.clone()
		));
		assert_eq!(Tellor::get_current_tip(query_id), token(500), "tip 3 should be correct");
		assert_eq!(Balances::balance(&tipper), token(400));

		// test query data storage
		assert_eq!(
			Tellor::get_query_data(query_id).unwrap(),
			query_data,
			"query data not stored correctly"
		);
		let query_data: QueryDataOf<Test> = spot_price("btc", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		assert_ok!(Tellor::tip(
			RuntimeOrigin::signed(tipper),
			query_id,
			token(10),
			query_data.clone()
		));
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
		assert_ok!(Tellor::tip(
			RuntimeOrigin::signed(tipper),
			query_id,
			token(10),
			query_data.clone()
		));
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
	let tipper = 1;
	let reporter = 2;
	let another_reporter = 4;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		register_parachain(STAKE_AMOUNT);
		deposit_stake(reporter, STAKE_AMOUNT, Address::random());
		deposit_stake(another_reporter, STAKE_AMOUNT, Address::random());
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L229
	ext.execute_with(|| {
		let _start_balance = Balances::balance(&reporter);
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

		Balances::make_free_balance_be(&tipper, token(1000));
		assert_ok!(Tellor::tip(
			RuntimeOrigin::signed(tipper),
			query_id,
			token(100),
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

#[test]
#[ignore]
fn get_data_feed() {
	todo!()
}

#[test]
#[ignore]
fn get_current_tip() {
	todo!()
}

#[test]
#[ignore]
fn get_past_tips() {
	todo!()
}

#[test]
#[ignore]
fn get_past_tip_by_index() {
	todo!()
}

#[test]
#[ignore]
fn get_past_tip_count() {
	todo!()
}

#[test]
#[ignore]
fn get_funded_feeds() {
	todo!()
}

#[test]
#[ignore]
fn get_query_id_from_feed_id() {
	todo!()
}

#[test]
#[ignore]
fn get_funded_query_ids() {
	todo!()
}

#[test]
#[ignore]
fn get_tips_by_address() {
	todo!()
}

#[test]
#[ignore]
fn get_reward_amount() {
	todo!()
}

#[test]
#[ignore]
fn value_to_amount() {
	todo!()
}

#[test]
#[ignore]
fn get_funded_single_tips_info() {
	todo!()
}

#[test]
#[ignore]
fn get_funded_feed_details() {
	todo!()
}

#[test]
#[ignore]
fn get_reward_claim_status_list() {
	todo!()
}

// Helper function for creating feeds
fn create_feed(
	feed_creator: AccountIdOf<Test>,
	query_id: QueryIdOf<Test>,
	reward: AmountOf<Test>,
	start_time: TimestampOf<Test>,
	interval: TimestampOf<Test>,
	window: TimestampOf<Test>,
	price_threshold: u16,
	reward_increase_per_second: AmountOf<Test>,
	query_data: QueryDataOf<Test>,
	amount: AmountOf<Test>,
) -> FeedIdOf<Test> {
	assert_ok!(Tellor::setup_data_feed(
		RuntimeOrigin::signed(feed_creator),
		query_id,
		reward,
		start_time,
		interval,
		window,
		price_threshold,
		reward_increase_per_second,
		query_data.clone(),
		amount
	));
	let feed_id = keccak_256(&ethabi::encode(&vec![
		Token::FixedBytes(query_id.0.into()),
		Token::Uint(reward.into()),
		Token::Uint(start_time.into()),
		Token::Uint(interval.into()),
		Token::Uint(window.into()),
		Token::Uint(price_threshold.into()),
		Token::Uint(reward_increase_per_second.into()),
	]))
	.into();
	if amount == 0 {
		System::assert_last_event(
			Event::NewDataFeed { query_id, feed_id, query_data, feed_creator }.into(),
		);
	}
	feed_id
}

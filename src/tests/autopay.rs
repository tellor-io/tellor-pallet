use super::*;
use crate::{
	types::{FeedDetailsOf, FeedIdOf, QueryDataOf, QueryIdOf, TimestampOf, TipOf},
	Config, WEEK_IN_MILLISECONDS,
};
use frame_support::{
	assert_noop, assert_ok,
	traits::{fungible::Inspect, Currency, Get},
};
use sp_core::{bounded::BoundedVec, bounded_vec, keccak_256};
use sp_runtime::traits::{AccountIdConversion, BadOrigin};

type ClaimBuffer = <Test as Config>::ClaimBuffer;
type Fee = <Test as Config>::Fee;
type ReportingLock = <Test as Config>::ReportingLock;
type Pallet = crate::Pallet<Test>;
type Price = <Test as Config>::Price;

const SECONDS: u64 = 1_000;

#[test]
fn claim_tip_ensures() {
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
	let claimed = ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
			deposit_stake(another_reporter, STAKE_AMOUNT, Address::random());

			Balances::make_free_balance_be(&feed_creator, token(1_010) + 1);
			feed_id = create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				0,
				0,
				query_data.clone(),
				0,
			);
		});

		let (claimed_timestamp, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3575),
				0,
				query_data.clone(),
			));
		});

		with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3500),
				1,
				query_data.clone(),
			));
			timestamps.try_push(Timestamp::get()).unwrap();
		});

		with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3525),
				2,
				query_data.clone(),
			));
			bad_timestamps.try_push(Timestamp::get()).unwrap();
		});

		with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3550),
				3,
				query_data.clone(),
			));
			// Note: timestamp not added to vector as per reference test
		});

		with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3550),
				4,
				query_data.clone(),
			));
			timestamps.try_push(Timestamp::get()).unwrap();
			bad_timestamps.try_push(Timestamp::get()).unwrap();
		});

		with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3575),
				5,
				query_data.clone(),
			));
			timestamps.try_push(Timestamp::get()).unwrap();
		});

		claimed_timestamp
	});

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L74
	ext.execute_with(|| {
		with_block(|| {
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
			// no tips submitted for this queryId
			assert_noop!(
				Tellor::claim_tip(
					RuntimeOrigin::signed(reporter),
					feed_id,
					query_id,
					bounded_vec![12345]
				),
				Error::InsufficientFeedBalance
			);
			assert_ok!(Tellor::fund_feed(
				RuntimeOrigin::signed(feed_creator),
				feed_id,
				query_id,
				token(1_000)
			));
			// buffer time has not passed
			assert_noop!(
				Tellor::claim_tip(
					RuntimeOrigin::signed(reporter),
					feed_id,
					query_id,
					timestamps.clone()
				),
				Error::ClaimBufferNotPassed
			);
		});
		// Advancing time 12 hours to satisfy hardcoded buffer time.
		with_block_after(ClaimBuffer::get(), || {
			// message sender not reporter for given queryId and timestamp
			assert_noop!(
				Tellor::claim_tip(
					RuntimeOrigin::signed(reporter),
					feed_id,
					query_id,
					bad_timestamps
				),
				Error::InvalidClaimer
			);
			assert_noop!(
				Tellor::claim_tip(
					RuntimeOrigin::signed(another_reporter),
					feed_id,
					query_id,
					timestamps.clone()
				),
				Error::InvalidClaimer
			);
			// reward already claimed
			assert_ok!(Tellor::claim_tip(
				RuntimeOrigin::signed(reporter),
				feed_id,
				query_id,
				bounded_vec![claimed]
			));
			assert_noop!(
				Tellor::claim_tip(
					RuntimeOrigin::signed(reporter),
					feed_id,
					query_id,
					bounded_vec![claimed]
				),
				Error::TipAlreadyClaimed
			);
		});
		// no value exists at timestamp
		let (timestamp, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3575),
				6,
				query_data.clone(),
			));
		});
		with_block(|| {
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				timestamp
			));
		});
		with_block_after(ClaimBuffer::get(), || {
			assert_noop!(
				Tellor::claim_tip(
					RuntimeOrigin::signed(another_reporter),
					feed_id,
					query_id,
					bounded_vec![timestamp]
				),
				Error::InvalidTimestamp
			);
		});
		// price threshold not met
		let (_, feed_id) = with_block(|| {
			create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				3600000 * SECONDS,
				2 * SECONDS,
				10_000,
				0,
				query_data.clone(),
				token(1),
			)
		});
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3500),
				7,
				query_data.clone(),
			));
		});
		let (timestamp, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3501),
				8,
				query_data.clone(),
			));
		});
		with_block_after(ClaimBuffer::get(), || {
			assert_noop!(
				Tellor::claim_tip(
					RuntimeOrigin::signed(reporter),
					feed_id,
					query_id,
					bounded_vec![timestamp]
				),
				Error::PriceThresholdNotMet
			);
		});
		// insufficient balance for all submitted timestamps
		let (timestamp_1, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(35000),
				9,
				query_data.clone(),
			));
		});
		let (timestamp_2, _) = with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(350000),
				10,
				query_data.clone(),
			));
		});
		with_block_after(ClaimBuffer::get(), || {
			assert_noop!(
				Tellor::claim_tip(
					RuntimeOrigin::signed(reporter),
					feed_id,
					query_id,
					bounded_vec![timestamp_1, timestamp_2]
				),
				Error::InsufficientFeedBalance
			);
		});
		// timestamp too old to claim tip
		with_block_after(4 * WEEK_IN_MILLISECONDS, || {
			assert_noop!(
				Tellor::claim_tip(
					RuntimeOrigin::signed(reporter),
					feed_id,
					query_id,
					bounded_vec![timestamp_2]
				),
				Error::ClaimPeriodExpired
			);
		});
	});
}

#[test]
fn claim_tip() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let feed_creator = 10;
	let mut feed_id = H256::zero();
	let mut timestamps = Vec::default();
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());

			Balances::make_free_balance_be(&feed_creator, token(1_000) + 1);
			feed_id = create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				0,
				0,
				query_data.clone(),
				token(1_000),
			);
		});
		with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3500),
				0,
				query_data.clone(),
			));
			timestamps.push(Timestamp::get());
		});
		with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3550),
				1,
				query_data.clone(),
			));
			timestamps.push(Timestamp::get());
		});
		with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3575),
				2,
				query_data.clone(),
			));
			timestamps.push(Timestamp::get());
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L120
	ext.execute_with(|| {
		// Advancing time 12 hours to satisfy hardcoded buffer time.
		with_block_after(ClaimBuffer::get(), || {
			let payer_before = Tellor::get_data_feed(feed_id).unwrap();
			assert_ok!(Tellor::claim_tip(
				RuntimeOrigin::signed(reporter),
				feed_id,
				query_id,
				timestamps.try_into().unwrap()
			));
			System::assert_last_event(
				Event::TipClaimed { feed_id, query_id, amount: token(3), reporter }.into(),
			);

			let payer_after = Tellor::get_data_feed(feed_id).unwrap();
			assert_ne!(payer_before.balance, payer_after.balance);
			assert_eq!(payer_after.balance, token(997));
			// Updating Balance Checks
			// 1% of each tip being shaved for Tellor ~= .01 token/tip claimed
			// That's why tellor balance is .03 lower than originally expected.
			assert_eq!(Balances::balance(&reporter), token(2.97));
			// Checking if owner (Tellor) account was updated by fee amount (0.03)
			let pallet_id = <Test as Config>::PalletId::get();
			assert_eq!(
				Balances::balance(&pallet_id.into_sub_account_truncating(b"staking")),
				token(0.03)
			);
			assert_eq!(Balances::balance(&pallet_id.into_account_truncating()), token(997));
		});
	});
}

#[test]
fn _get_reward_amount() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let feed_creator = 2;
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L136
	ext.execute_with(|| {
		let (timestamp, feed_id) = with_block(|| {
			Balances::make_free_balance_be(&feed_creator, token(100) + 1);
			let feed_id = create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				0,
				0,
				query_data.clone(),
				token(100),
			);

			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3550),
				0,
				query_data.clone(),
			));

			feed_id
		});

		with_block_after(ClaimBuffer::get(), || {
			// Variable updates
			assert_ok!(Tellor::claim_tip(
				RuntimeOrigin::signed(reporter),
				feed_id,
				query_id,
				bounded_vec![timestamp]
			));
			assert_eq!(Tellor::get_data_feed(feed_id).unwrap().balance, token(99));
			assert!(Tellor::get_reward_claimed_status(feed_id, query_id, timestamp).unwrap())
		});
	});
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
		with_block(|| {
			feed_id = create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				1,
				3,
				query_data.clone(),
				0,
			);
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L134
	ext.execute_with(|| {
		with_block(|| {
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
			let pallet_account = <Test as Config>::PalletId::get().into_account_truncating();
			let initial_balance = Balances::balance(&pallet_account);
			assert_ok!(Tellor::fund_feed(
				RuntimeOrigin::signed(feed_funder),
				feed_id,
				query_id,
				amount
			));
			let feed_details = Tellor::get_data_feed(feed_id).unwrap();
			System::assert_last_event(
				Event::DataFeedFunded { query_id, feed_id, amount, feed_funder, feed_details }
					.into(),
			);
			assert_eq!(
				Balances::balance(&pallet_account) - initial_balance,
				amount,
				"balance should change"
			);
		});
	});
}

#[test]
fn setup_data_feed() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let feed_creator = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	let timestamp = ext.execute_with(|| {
		with_block(|| {
			create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				600 * SECONDS,
				60 * SECONDS,
				0,
				0,
				query_data.clone(),
				0,
			);
		})
		.0
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L155
	ext.execute_with(|| {
		with_block(|| {
			assert_noop!(
				Tellor::setup_data_feed(
					RuntimeOrigin::none(),
					query_id,
					token(1),
					timestamp,
					3600 * SECONDS,
					600 * SECONDS,
					0,
					0,
					query_data.clone(),
					0
				),
				BadOrigin
			);
			// id must be hash of bytes data
			assert_noop!(
				Tellor::setup_data_feed(
					RuntimeOrigin::signed(feed_creator),
					H256::random(),
					token(1),
					timestamp,
					3600 * SECONDS,
					600 * SECONDS,
					0,
					0,
					query_data.clone(),
					0
				),
				Error::InvalidQueryId
			);
			// reward must be greater than zero
			assert_noop!(
				Tellor::setup_data_feed(
					RuntimeOrigin::signed(feed_creator),
					query_id,
					0,
					timestamp,
					3600 * SECONDS,
					600 * SECONDS,
					0,
					0,
					query_data.clone(),
					0
				),
				Error::InvalidReward
			);
			// feed must not be set up already
			assert_noop!(
				Tellor::setup_data_feed(
					RuntimeOrigin::signed(feed_creator),
					query_id,
					token(1),
					timestamp,
					600 * SECONDS,
					60 * SECONDS,
					0,
					0,
					query_data.clone(),
					0
				),
				Error::FeedAlreadyExists
			);
			// window must be less than interval length
			assert_noop!(
				Tellor::setup_data_feed(
					RuntimeOrigin::signed(feed_creator),
					query_id,
					token(1),
					timestamp,
					600 * SECONDS,
					3600 * SECONDS,
					0,
					0,
					query_data.clone(),
					0
				),
				Error::InvalidWindow
			);
			// interval must be greater than zero
			assert_noop!(
				Tellor::setup_data_feed(
					RuntimeOrigin::signed(feed_creator),
					query_id,
					token(1),
					timestamp,
					0 * SECONDS,
					600 * SECONDS,
					0,
					0,
					query_data.clone(),
					0
				),
				Error::InvalidInterval
			);

			let feed_id = create_feed(
				feed_creator,
				query_id,
				token(1),
				timestamp,
				3600 * SECONDS,
				600 * SECONDS,
				1,
				3,
				query_data.clone(),
				0,
			);
			System::assert_last_event(
				Event::NewDataFeed {
					query_id,
					feed_id,
					query_data: query_data.clone(),
					feed_creator,
				}
				.into(),
			);
			let result = Tellor::get_data_feed(feed_id).unwrap();
			assert_eq!(result.reward, token(1));
			assert_eq!(result.balance, 0);
			assert_eq!(result.start_time, timestamp);
			assert_eq!(result.interval, 3600 * SECONDS);
			assert_eq!(result.window, 600 * SECONDS);
			assert_eq!(result.price_threshold, 1);
			assert_eq!(result.reward_increase_per_second, 3);
			assert_eq!(result.feeds_with_funding_index, 0);

			Balances::make_free_balance_be(&feed_creator, token(100));
			create_feed(
				feed_creator,
				query_id,
				token(1),
				timestamp,
				7600 * SECONDS,
				600 * SECONDS,
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
				3600 * SECONDS,
				600 * SECONDS,
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
				3600 * SECONDS,
				1200 * SECONDS,
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
	});
}

#[test]
fn get_reward_claimed_status() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let feed_creator = 10;
	let mut feed_id = H256::zero();
	let mut timestamp = 0;
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
			timestamp = Timestamp::get();
			Balances::make_free_balance_be(&feed_creator, token(3));
			feed_id = create_feed(
				feed_creator,
				query_id,
				token(1),
				timestamp,
				3600 * SECONDS,
				600 * SECONDS,
				0,
				0,
				query_data.clone(),
				token(2),
			);
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3500),
				0,
				query_data.clone(),
			));
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L190
	ext.execute_with(|| {
		assert_eq!(Tellor::get_reward_claimed_status(feed_id, query_id, timestamp).unwrap(), false);
		with_block_after(86_400 * 1_000, || {
			assert_ok!(Tellor::claim_tip(
				RuntimeOrigin::signed(reporter),
				feed_id,
				query_id,
				bounded_vec![timestamp]
			));
			assert_eq!(
				Tellor::get_reward_claimed_status(feed_id, query_id, timestamp).unwrap(),
				true
			);
		});
	});
}

#[test]
fn tip() {
	let reporter = 1;
	let another_reporter = 2;
	let tipper = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let amount = token(100);
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
			deposit_stake(another_reporter, STAKE_AMOUNT, Address::random());

			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3550),
				0,
				query_data.clone()
			));
		});
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

		with_block(|| {
			Balances::make_free_balance_be(&tipper, token(1_000));
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				amount,
				query_data.clone()
			));
			assert_eq!(Tellor::get_current_tip(query_id), amount, "tip 1 should be correct");
			assert_eq!(Balances::balance(&tipper), token(900));
		});

		// next block required for a timestamp that is newer than last tip timestamp
		// i.e. next submitted value gets paired with previous tip, leaving following tip added as the current tip
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				1,
				query_data.clone()
			));

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
		});

		with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				2,
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
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
			deposit_stake(another_reporter, STAKE_AMOUNT, Address::random());
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L267
	ext.execute_with(|| {
		assert_noop!(
			Tellor::claim_onetime_tip(RuntimeOrigin::root(), query_id, bounded_vec![]),
			BadOrigin
		);
		// no tips submitted for this queryId
		assert_noop!(
			Tellor::claim_onetime_tip(RuntimeOrigin::signed(reporter), query_id, bounded_vec![]),
			Error::NoTipsSubmitted
		);

		// buffer time has not passed
		with_block(|| {
			Balances::make_free_balance_be(&tipper, token(100));
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(1),
				query_data.clone()
			));
		});
		let (timestamp, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				0,
				query_data.clone()
			));
		});
		with_block(|| {
			assert_noop!(
				Tellor::claim_onetime_tip(
					RuntimeOrigin::signed(reporter),
					query_id,
					bounded_vec![timestamp]
				),
				Error::ClaimBufferNotPassed
			);
		});
		with_block_after(ClaimBuffer::get(), || {
			assert_ok!(Tellor::claim_onetime_tip(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				bounded_vec![timestamp]
			));
		});

		// Value disputed
		let (timestamp, _) = with_block(|| {
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(1),
				query_data.clone()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				1,
				query_data.clone()
			));
		});
		let claim_buffer: u64 = ClaimBuffer::get();
		with_block_after(claim_buffer / 2, || {
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, timestamp));
		});
		with_block_after(claim_buffer / 2, || {
			assert_noop!(
				Tellor::claim_onetime_tip(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					bounded_vec![timestamp]
				),
				Error::ValueDisputed
			);
		});

		// msg sender must be reporter address
		let (timestamp, _) = with_block(|| {
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(1),
				query_data.clone()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				2,
				query_data.clone()
			));
		});
		with_block_after(ClaimBuffer::get(), || {
			assert_noop!(
				Tellor::claim_onetime_tip(
					RuntimeOrigin::signed(reporter),
					query_id,
					bounded_vec![timestamp]
				),
				Error::InvalidClaimer
			);
		});

		// tip earned by previous submission
		with_block(|| {
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(1),
				query_data.clone()
			));
		});
		let (timestamp_1, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				3,
				query_data.clone()
			));
		});
		let (timestamp_2, _) = with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				4,
				query_data.clone()
			));
		});
		with_block_after(ClaimBuffer::get(), || {
			assert_noop!(
				Tellor::claim_onetime_tip(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					bounded_vec![timestamp_2]
				),
				Error::TipAlreadyEarned
			);
		});
		with_block(|| {
			assert_ok!(Tellor::claim_onetime_tip(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				bounded_vec![timestamp_1]
			));
		});

		// timestamp not eligible for tip
		let query_data: QueryDataOf<Test> = spot_price("ksm", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let (timestamp_1, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				0,
				query_data.clone()
			));
		});
		with_block(|| {
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(1),
				query_data.clone()
			));
		});
		let (timestamp_2, _) = with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(3550),
				1,
				query_data.clone()
			));
		});
		with_block_after(ClaimBuffer::get(), || {
			assert_noop!(
				Tellor::claim_onetime_tip(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					bounded_vec![timestamp_1]
				),
				Error::TimestampIneligibleForTip
			);
			assert_ok!(Tellor::claim_onetime_tip(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				bounded_vec![timestamp_2]
			));
		});

		// tip already claimed
		with_block_after(ClaimBuffer::get(), || {
			assert_noop!(
				Tellor::claim_onetime_tip(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					bounded_vec![timestamp_2]
				),
				Error::TipAlreadyClaimed
			);
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L323
	ext.execute_with(|| {
		let start_balance = Balances::balance(&reporter);
		with_block(|| {
			Balances::make_free_balance_be(&tipper, token(100) + 1);
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(100),
				query_data.clone(),
			));
		});
		let (timestamp, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3550),
				5,
				query_data.clone(),
			));
		});
		with_block_after(ClaimBuffer::get(), || {
			assert_ok!(Tellor::claim_onetime_tip(
				RuntimeOrigin::signed(reporter),
				query_id,
				bounded_vec![timestamp]
			));
			assert_eq!(Tellor::get_current_tip(query_id), 0, "tip should be correct");
			let final_balance = Balances::balance(&reporter);
			assert_eq!(final_balance - start_balance, token(99), "balance should change correctly")
		});
	});
}

#[test]
fn get_data_feed() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let feed_creator = 10;
	let mut feed_id = H256::zero();
	let mut ext = new_test_ext();

	// Prerequisites
	let (timestamp, _) = ext.execute_with(|| {
		with_block(|| {
			Balances::make_free_balance_be(&feed_creator, token(1_000) + 1);
			feed_id = create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				0,
				0,
				query_data.clone(),
				token(1_000),
			);
		})
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L252
	ext.execute_with(|| {
		assert_eq!(
			Tellor::get_data_feed(feed_id).unwrap(),
			FeedDetailsOf::<Test> {
				reward: token(1),
				balance: token(1_000),
				start_time: timestamp,
				interval: 3600 * SECONDS,
				window: 600 * SECONDS,
				price_threshold: 0,
				reward_increase_per_second: 0,
				feeds_with_funding_index: 1,
			}
		);
	});
}

#[test]
fn get_current_tip() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let tipper = 10;

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L262
	new_test_ext().execute_with(|| {
		with_block(|| {
			assert_eq!(Tellor::get_current_tip(query_id), 0, "tip amount should be zero");
			assert_noop!(
				Tellor::tip(
					RuntimeOrigin::signed(tipper),
					query_id,
					token(100),
					query_data.clone()
				),
				pallet_balances::Error::<Test>::InsufficientBalance
			);
			Balances::make_free_balance_be(&tipper, token(100) + 1);
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(100),
				query_data
			));
			assert_eq!(Tellor::get_current_tip(query_id), token(100), "tip should be correct");
		});
	});
}

#[test]
fn get_past_tips() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let tipper = 10;
	let reporter = 10;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L271
	ext.execute_with(|| {
		let (timestamp_1, _) = with_block(|| {
			assert_eq!(Tellor::get_past_tips(query_id), vec![], "should be no tips");

			Balances::make_free_balance_be(&tipper, token(800) + 1);
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(100),
				query_data.clone()
			));
		});

		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3550),
				0,
				query_data.clone(),
			));
		});

		let (timestamp_2, _) = with_block(|| {
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(200),
				query_data.clone()
			));
		});

		assert_eq!(
			Tellor::get_past_tips(query_id),
			vec![
				TipOf::<Test> {
					amount: token(100),
					timestamp: timestamp_1 + 1,
					cumulative_tips: token(100)
				},
				TipOf::<Test> {
					amount: token(200),
					timestamp: timestamp_2 + 1,
					cumulative_tips: token(300)
				}
			],
			"past tips should be correct"
		);

		let (timestamp_3, _) = with_block(|| {
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(300),
				query_data.clone()
			));
		});

		assert_eq!(
			Tellor::get_past_tips(query_id),
			vec![
				TipOf::<Test> {
					amount: token(100),
					timestamp: timestamp_1 + 1,
					cumulative_tips: token(100)
				},
				TipOf::<Test> {
					amount: token(500),
					timestamp: timestamp_3 + 1,
					cumulative_tips: token(600)
				}
			],
			"past tips should be correct"
		);
	});
}

#[test]
fn get_past_tip_by_index() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let tipper = 10;
	let reporter = 10;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L297
	ext.execute_with(|| {
		let (timestamp_1, _) = with_block(|| {
			Balances::make_free_balance_be(&tipper, token(800) + 1);
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(100),
				query_data.clone()
			));
		});

		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3550),
				0,
				query_data.clone(),
			));
		});

		let (timestamp_2, _) = with_block(|| {
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(200),
				query_data.clone()
			));
		});

		assert_eq!(
			Tellor::get_past_tip_by_index(query_id, 0).unwrap(),
			TipOf::<Test> {
				amount: token(100),
				timestamp: timestamp_1 + 1,
				cumulative_tips: token(100),
			},
			"past tip should be correct"
		);
		assert_eq!(
			Tellor::get_past_tip_by_index(query_id, 1).unwrap(),
			TipOf::<Test> {
				amount: token(200),
				timestamp: timestamp_2 + 1,
				cumulative_tips: token(300),
			},
			"past tip should be correct"
		);

		let (timestamp_3, _) = with_block(|| {
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(300),
				query_data.clone()
			));
		});

		assert_eq!(
			Tellor::get_past_tip_by_index(query_id, 0).unwrap(),
			TipOf::<Test> {
				amount: token(100),
				timestamp: timestamp_1 + 1,
				cumulative_tips: token(100),
			},
			"past tip should be correct"
		);
		assert_eq!(
			Tellor::get_past_tip_by_index(query_id, 1).unwrap(),
			TipOf::<Test> {
				amount: token(500),
				timestamp: timestamp_3 + 1,
				cumulative_tips: token(600),
			},
			"past tip should be correct"
		);
	});
}

#[test]
fn get_past_tip_count() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let tipper = 10;
	let reporter = 10;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L322
	ext.execute_with(|| {
		with_block(|| {
			assert_eq!(Tellor::get_past_tip_count(query_id), 0, "past tip count should be correct");
			Balances::make_free_balance_be(&tipper, token(300) + 1);
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(100),
				query_data.clone()
			));
		});

		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(3550),
				0,
				query_data.clone(),
			));
		});

		with_block(|| {
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(100),
				query_data.clone()
			));
			assert_eq!(Tellor::get_past_tip_count(query_id), 2, "past tip count should be correct");
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(100),
				query_data.clone()
			));
			assert_eq!(Tellor::get_past_tip_count(query_id), 2, "past tip count should be correct");
		});
	});
}

#[test]
fn get_funded_feeds() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let query_data_2: QueryDataOf<Test> = spot_price("ksm", "usd").try_into().unwrap();
	let query_id_2: H256 = keccak_256(query_data_2.as_ref()).into();
	let query_data_3: QueryDataOf<Test> = spot_price("glmr", "usd").try_into().unwrap();
	let query_id_3: H256 = keccak_256(query_data_3.as_ref()).into();
	let feed_creator = 10;
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
			Balances::make_free_balance_be(&feed_creator, token(3) + 1);
			create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				0,
				0,
				query_data,
				token(1),
			);
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L338
	ext.execute_with(|| {
		let (_, (feed_1, feed_2, feed_3)) = with_block(|| {
			// Check one existing funded feed
			let feeds = Tellor::get_funded_feeds();
			assert_eq!(feeds.len(), 1, "should be one funded feed from previous test");
			let feed_1 = feeds[0];
			assert_eq!(
				Tellor::get_query_id_from_feed_id(feed_1).unwrap(),
				query_id,
				"incorrect query ID"
			);

			// Check adding two funded feeds
			let feed_2 = create_feed(
				feed_creator,
				query_id_2,
				token(1),
				Timestamp::get(),
				600 * SECONDS,
				400 * SECONDS,
				0,
				0,
				query_data_2.clone(),
				token(1),
			);
			let feed_3 = create_feed(
				feed_creator,
				query_id_3,
				token(1),
				Timestamp::get(),
				600 * SECONDS,
				400 * SECONDS,
				0,
				0,
				query_data_3,
				token(1),
			);
			assert_eq!(
				Tellor::get_funded_feeds(),
				vec![feed_1, feed_2, feed_3],
				"should be three funded feeds"
			);
			(feed_1, feed_2, feed_3)
		});

		let (timestamp, _) = with_block(|| {
			// Check remove funded feed
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id_2,
				uint_value(3500),
				0,
				query_data_2,
			));
		});

		// Check feed details
		for (index, feed) in vec![feed_1, feed_2, feed_3]
			.iter()
			.map(|feed| Tellor::get_data_feed(*feed).unwrap())
			.enumerate()
		{
			let item = index as u32 + 1;
			assert_eq!(
				feed.feeds_with_funding_index, item,
				"queryId {0} feedsWithFundingIndex should be {0}",
				item
			)
		}

		with_block_after(ClaimBuffer::get(), || {
			assert_ok!(Tellor::claim_tip(
				RuntimeOrigin::signed(reporter),
				feed_2,
				query_id_2,
				bounded_vec![timestamp]
			));
			assert_eq!(Tellor::get_funded_feeds(), vec![feed_1, feed_3], "incorrect funded feeds");
			for (index, (feed, expected)) in vec![(feed_1, 1), (feed_2, 0), (feed_3, 2)]
				.iter()
				.map(|(feed, expected)| (Tellor::get_data_feed(*feed).unwrap(), *expected))
				.enumerate()
			{
				assert_eq!(
					feed.feeds_with_funding_index,
					expected,
					"queryId {} feedsWithFundingIndex should be {}",
					index + 1,
					expected
				)
			}
		});
	});
}

#[test]
fn get_query_id_from_feed_id() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let feed_creator = 10;

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L386
	new_test_ext().execute_with(|| {
		with_block(|| {
			let feed_id = create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				600 * SECONDS,
				400 * SECONDS,
				0,
				0,
				query_data,
				0,
			);
			assert_eq!(Tellor::get_query_id_from_feed_id(feed_id).unwrap(), query_id);
		});
	});
}

#[test]
fn get_funded_query_ids() {
	let query_data_1: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id_1: H256 = keccak_256(query_data_1.as_ref()).into();
	let query_data_2: QueryDataOf<Test> = spot_price("ksm", "usd").try_into().unwrap();
	let query_id_2: H256 = keccak_256(query_data_2.as_ref()).into();
	let query_data_3: QueryDataOf<Test> = spot_price("glmr", "usd").try_into().unwrap();
	let query_id_3: H256 = keccak_256(query_data_3.as_ref()).into();
	let query_data_4: QueryDataOf<Test> = spot_price("dev", "usd").try_into().unwrap();
	let query_id_4: H256 = keccak_256(query_data_4.as_ref()).into();
	let tipper = 10;
	let reporter = 1;
	let another_reporter = 2;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
			deposit_stake(another_reporter, STAKE_AMOUNT, Address::random());

			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id_1,
				uint_value(3500),
				0,
				query_data_1.clone(),
			));
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/b0eca105f536d7fd6046cf1f53125928839a3bb0/test/functionTests-TellorAutopay.js#L403
	ext.execute_with(|| {
		with_block(|| {
			Balances::make_free_balance_be(&tipper, token(1_000) + 1);
			assert_eq!(Tellor::get_funded_query_ids(), vec![]);
			// Tip queryId 1
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_1,
				token(1),
				query_data_1.clone()
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_1]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1).unwrap(), 1);
			// Tip queryId 1 again
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_1,
				token(1),
				query_data_1.clone()
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_1]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1).unwrap(), 1);
			// Tip queryId 2
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_2,
				token(1),
				query_data_2.clone()
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_1, query_id_2]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1).unwrap(), 1);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_2).unwrap(), 2);
			// Tip queryId 2 again
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_2,
				token(1),
				query_data_2.clone()
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_1, query_id_2]);
			// Tip queryId 3
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_3,
				token(1),
				query_data_3.clone()
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_1, query_id_2, query_id_3]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1).unwrap(), 1);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_2).unwrap(), 2);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_3).unwrap(), 3);
			// Tip queryId 4
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_4,
				token(1),
				query_data_4.clone()
			));
			assert_eq!(
				Tellor::get_funded_query_ids(),
				vec![query_id_1, query_id_2, query_id_3, query_id_4]
			);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1).unwrap(), 1);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_2).unwrap(), 2);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_3).unwrap(), 3);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_4).unwrap(), 4);
		});

		let (timestamp_1, _) = with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id_1,
				uint_value(3550),
				1,
				query_data_1.clone(),
			));
		});

		let (timestamp_2, _) = with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id_2,
				uint_value(3550),
				0,
				query_data_2.clone(),
			));
		});

		let (timestamp_3, _) = with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id_3,
				uint_value(3550),
				0,
				query_data_3.clone(),
			));
		});

		let (timestamp_4, _) = with_block_after(ReportingLock::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id_4,
				uint_value(3550),
				0,
				query_data_4.clone(),
			));
		});

		with_block_after(ClaimBuffer::get(), || {
			assert_ok!(Tellor::claim_onetime_tip(
				RuntimeOrigin::signed(reporter),
				query_id_1,
				bounded_vec![timestamp_1]
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_4, query_id_2, query_id_3]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_2).unwrap(), 2);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_3).unwrap(), 3);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_4).unwrap(), 1);

			// Tip queryId 2
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_2,
				token(1),
				query_data_2.clone()
			));

			assert_ok!(Tellor::claim_onetime_tip(
				RuntimeOrigin::signed(reporter),
				query_id_2,
				bounded_vec![timestamp_2]
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_4, query_id_2, query_id_3]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_2).unwrap(), 2);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_3).unwrap(), 3);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_4).unwrap(), 1);

			assert_ok!(Tellor::claim_onetime_tip(
				RuntimeOrigin::signed(reporter),
				query_id_3,
				bounded_vec![timestamp_3]
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_4, query_id_2]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_2).unwrap(), 2);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_3), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_4).unwrap(), 1);

			assert_ok!(Tellor::claim_onetime_tip(
				RuntimeOrigin::signed(reporter),
				query_id_4,
				bounded_vec![timestamp_4]
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_2]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_2).unwrap(), 1);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_3), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_4), None);
		});

		let (timestamp_2, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id_2,
				uint_value(3550),
				1,
				query_data_2.clone(),
			));
		});

		with_block_after(ClaimBuffer::get(), || {
			assert_ok!(Tellor::claim_onetime_tip(
				RuntimeOrigin::signed(reporter),
				query_id_2,
				bounded_vec![timestamp_2]
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_2), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_3), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_4), None);

			// Tip queryId 4
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_4,
				token(1),
				query_data_4.clone()
			));
			assert_eq!(Tellor::get_funded_query_ids(), vec![query_id_4]);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_1), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_2), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_3), None);
			assert_eq!(Tellor::query_ids_with_funding_index(query_id_4).unwrap(), 1);
		});
	});
}

#[test]
fn get_tips_by_address() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let tipper = 2;

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L621
	new_test_ext().execute_with(|| {
		with_block(|| {
			Balances::make_free_balance_be(&tipper, token(1_000));
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id,
				token(10),
				query_data.clone()
			));
			assert_eq!(Tellor::get_tips_by_address(tipper), token(10));

			create_feed(
				tipper,
				query_id,
				token(1),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				0,
				0,
				query_data.clone(),
				token(99),
			);
			assert_eq!(Tellor::get_tips_by_address(tipper), token(109));
		});
	});
}

#[test]
fn get_reward_amount() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let feed_creator = 1;
	// Multiple reporters required due to reporting lock vs feed interval
	let reporter_1 = 2;
	let reporter_2 = 3;
	let reporter_3 = 4;
	let mut ext = new_test_ext();

	const INTERVAL: u64 = 3600 * SECONDS;

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter_1, STAKE_AMOUNT, Address::random());
			deposit_stake(reporter_2, STAKE_AMOUNT, Address::random());
			deposit_stake(reporter_3, STAKE_AMOUNT, Address::random());
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L632
	ext.execute_with(|| {
		let (timestamp_0, feed_id) = with_block(|| {
			// setup data feed with time based rewards
			Balances::make_free_balance_be(&feed_creator, token(1_000) + 1);
			let feed_id = create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				0,
				token(1),
				query_data.clone(),
				token(1_000),
			);

			feed_id
		});

		// advance some time within window
		let (timestamp_1, _) = with_block_after(10 * SECONDS, || {
			// submit value within window
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_1),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
		});

		// advance some time to next window
		let (timestamp_2, _) = with_block_after(INTERVAL + 10 * SECONDS, || {
			// submit value inside next window
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_2),
				query_id,
				uint_value(100),
				1,
				query_data.clone(),
			));
		});

		// advance some time to next window
		let (timestamp_3, _) = with_block_after(INTERVAL + 10 * SECONDS, || {
			// submit value inside next window
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_3),
				query_id,
				uint_value(100),
				2,
				query_data.clone(),
			));
		});

		// query non-existent rewards
		assert_eq!(Tellor::get_reward_amount(feed_id, query_id, vec![12345]), 0);

		// query rewards
		let fee: u16 = Fee::get();
		let mut expected_reward = token(1) + token(1) * (timestamp_1 - timestamp_0) / SECONDS; // reward/sec vs timestamp in ms
		expected_reward = expected_reward - (expected_reward * fee as u64 / (1_000)); // fee
		let mut reward_sum = expected_reward;
		assert_eq!(
			Tellor::get_reward_amount(feed_id, query_id, vec![timestamp_1]),
			expected_reward
		);

		expected_reward =
			token(1) + token(1) * (timestamp_2 - (timestamp_0 + INTERVAL * 1)) / SECONDS; // reward/sec vs timestamp in ms
		expected_reward = expected_reward - (expected_reward * fee as u64 / (1_000)); // fee
		reward_sum += expected_reward;
		assert_eq!(
			Tellor::get_reward_amount(feed_id, query_id, vec![timestamp_2]),
			expected_reward
		);

		expected_reward =
			token(1) + token(1) * (timestamp_3 - (timestamp_0 + INTERVAL * 2)) / SECONDS; // reward/sec vs timestamp in ms
		expected_reward = expected_reward - (expected_reward * fee as u64 / (1_000)); // fee
		reward_sum += expected_reward;
		assert_eq!(
			Tellor::get_reward_amount(feed_id, query_id, vec![timestamp_3]),
			expected_reward
		);

		// query rewards for multiple queries
		assert_eq!(
			Tellor::get_reward_amount(
				feed_id,
				query_id,
				vec![timestamp_1, timestamp_2, timestamp_3]
			),
			reward_sum
		);

		// query rewards 1 week later
		with_block_after(1 * WEEK_IN_MILLISECONDS, || {
			assert_eq!(
				Tellor::get_reward_amount(
					feed_id,
					query_id,
					vec![timestamp_1, timestamp_2, timestamp_3]
				),
				reward_sum
			);
		});

		// query after 12 weeks
		with_block_after(12 * WEEK_IN_MILLISECONDS, || {
			assert_eq!(
				Tellor::get_reward_amount(
					feed_id,
					query_id,
					vec![timestamp_1, timestamp_2, timestamp_3]
				),
				0 // Note: zero, rewards lost
			);
		});
	});
}

#[test]
fn bytes_to_price() {
	fn uint_to_bytes32(value: impl Into<Uint>) -> Bytes {
		ethabi::encode(&vec![Token::Uint(value.into())])
	}

	let x: Vec<(Bytes, Price)> = vec![
		(uint_to_bytes32(1), 1),
		(uint_to_bytes32(2), 2),
		(uint_to_bytes32(300000000000000u64), 300000000000000),
		(uint_to_bytes32(300000000000001u64), 300000000000001),
		(uint_to_bytes32(1u128), 1),
		(uint_to_bytes32(u128::MAX), u128::MAX),
	];
	for (source, expected) in x {
		println!("{:?}", source);
		let source: ValueOf<Test> = source.try_into().unwrap();
		let amount = Pallet::bytes_to_price(source.try_into().unwrap()).unwrap();
		assert_eq!(amount, expected);
	}
}

#[test]
fn get_funded_single_tips_info() {
	let query_data_1: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id_1: H256 = keccak_256(query_data_1.as_ref()).into();
	let query_data_2: QueryDataOf<Test> = spot_price("ksm", "usd").try_into().unwrap();
	let query_id_2: H256 = keccak_256(query_data_2.as_ref()).into();
	let tipper = 1;

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L713
	new_test_ext().execute_with(|| {
		with_block(|| {
			Balances::make_free_balance_be(&tipper, token(1_000));
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_1,
				token(100),
				query_data_1.clone()
			));
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(tipper),
				query_id_2,
				token(100),
				query_data_2.clone()
			));
			assert_eq!(
				Tellor::get_funded_single_tips_info(),
				vec![(query_data_1, token(100)), (query_data_2, token(100))]
			)
		});
	});
}

#[test]
fn get_funded_feed_details() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let feed_creator = 1;

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L724
	new_test_ext().execute_with(|| {
		with_block(|| {
			Balances::make_free_balance_be(&feed_creator, token(1_000) + 1);
			create_feed(
				feed_creator,
				query_id,
				token(1),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				0,
				0,
				query_data.clone(),
				token(1_000),
			);
			assert_eq!(
				&Tellor::get_funded_feed_details()[0].0,
				&FeedDetailsOf::<Test> {
					reward: token(1),
					balance: token(1_000),
					start_time: Timestamp::get(),
					interval: 3600 * SECONDS,
					window: 600 * SECONDS,
					price_threshold: 0,
					reward_increase_per_second: 0,
					feeds_with_funding_index: 1,
				}
			);
		});
	});
}

#[test]
fn get_reward_claim_status_list() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id: H256 = keccak_256(query_data.as_ref()).into();
	let feed_creator = 1;
	let reporter_1 = 2;
	let reporter_2 = 3;
	let reporter_3 = 4;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter_1, STAKE_AMOUNT, Address::random());
			deposit_stake(reporter_2, STAKE_AMOUNT, Address::random());
			deposit_stake(reporter_3, STAKE_AMOUNT, Address::random());
		});
	});

	// Based on https://github.com/tellor-io/autoPay/blob/ffff033170db06e231fba90213db59b4dc42b982/test/functionTests-TellorAutopay.js#L738
	ext.execute_with(|| {
		// setup feeds with funding
		let (_, feed_id) = with_block(|| {
			Balances::make_free_balance_be(&feed_creator, token(1_000) + 1);
			create_feed(
				feed_creator,
				query_id,
				token(10),
				Timestamp::get(),
				3600 * SECONDS,
				600 * SECONDS,
				0,
				0,
				query_data.clone(),
				token(1_000),
			)
		});

		// submit to feeds
		let (timestamp_1, _) = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_1),
				query_id,
				uint_value(3500),
				0,
				query_data.clone(),
			));
		});
		let (timestamp_2, _) = with_block_after(ClaimBuffer::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_2),
				query_id,
				uint_value(3525),
				1,
				query_data.clone(),
			));
		});
		let (timestamp_3, _) = with_block_after(ClaimBuffer::get(), || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_3),
				query_id,
				uint_value(3550),
				2,
				query_data.clone(),
			));
		});

		// check timestamps
		assert_eq!(
			Tellor::get_reward_claim_status_list(
				feed_id,
				query_id,
				vec![timestamp_1, timestamp_2, timestamp_3]
			),
			vec![false, false, false]
		);

		// claim tip and check status
		with_block_after(ClaimBuffer::get(), || {
			assert_ok!(Tellor::claim_tip(
				RuntimeOrigin::signed(reporter_1),
				feed_id,
				query_id,
				bounded_vec![timestamp_1]
			));
			assert_eq!(
				Tellor::get_reward_claim_status_list(
					feed_id,
					query_id,
					vec![timestamp_1, timestamp_2, timestamp_3]
				),
				vec![true, false, false]
			);
			assert_ok!(Tellor::claim_tip(
				RuntimeOrigin::signed(reporter_2),
				feed_id,
				query_id,
				bounded_vec![timestamp_2]
			));
			assert_eq!(
				Tellor::get_reward_claim_status_list(
					feed_id,
					query_id,
					vec![timestamp_1, timestamp_2, timestamp_3]
				),
				vec![true, true, false]
			);
			assert_ok!(Tellor::claim_tip(
				RuntimeOrigin::signed(reporter_3),
				feed_id,
				query_id,
				bounded_vec![timestamp_3]
			));
			assert_eq!(
				Tellor::get_reward_claim_status_list(
					feed_id,
					query_id,
					vec![timestamp_1, timestamp_2, timestamp_3]
				),
				vec![true, true, true]
			);
		})
	});
}

#[test]
fn get_current_feeds() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let feed_creator = 1;

	new_test_ext().execute_with(|| {
		with_block(|| {
			// create multiple feeds for the same query id
			let feeds: Vec<FeedIdOf<Test>> = (1..=5u8)
				.map(|i| {
					create_feed(
						feed_creator,
						query_id,
						token(i),
						Timestamp::get(),
						600 * SECONDS,
						60 * SECONDS,
						0,
						0,
						query_data.clone(),
						0,
					)
				})
				.collect();
			assert_eq!(feeds.len(), 5);
			assert_eq!(Tellor::get_current_feeds(query_id), feeds);
		});
	});
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

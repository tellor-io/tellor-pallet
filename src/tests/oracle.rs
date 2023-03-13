use super::*;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BadOrigin;

#[test]
fn deposit_stake() {
	let reporter = 1;
	let address = Address::random();
	let amount = token(100);
	let another_reporter = 2;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| register_parachain(STAKE_AMOUNT));
	});

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L86
	ext.execute_with(|| {
		with_block(|| {
			assert_noop!(
				Tellor::report_stake_deposited(
					RuntimeOrigin::signed(another_reporter),
					reporter,
					amount.into(),
					address
				),
				BadOrigin
			);
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				amount.into(),
				address
			));
			System::assert_last_event(
				Event::NewStakerReported { staker: reporter, amount, address }.into(),
			);

			assert_eq!(Tellor::get_total_stakers(), 1);
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.address, address);
			assert_eq!(staker_details.start_date, Timestamp::get());
			assert_eq!(staker_details.staked_balance, amount);
			assert_eq!(staker_details.locked_balance, 0);
			assert_eq!(staker_details.reward_debt, 0);
			assert_eq!(staker_details.reporter_last_timestamp, 0);
			assert_eq!(staker_details.reports_submitted, 0);
			assert_eq!(staker_details.start_vote_count, 0);
			assert_eq!(staker_details.start_vote_tally, 0);
			assert_eq!(staker_details.staked, true);
			assert!(staker_details.reports_submitted_by_query_id.is_empty());
			//assert_eq!(Tellor::total_reward_debt(), 0); // todo: total reward debt?
			assert_eq!(Tellor::get_total_stake_amount(), amount);

			// Test min value for amount argument
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				another_reporter,
				0.into(),
				Address::random()
			));
			assert_eq!(Tellor::get_total_stakers(), 1);

			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				token(5).into(),
				address
			));
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				token(10).into(),
				address
			));
			assert_eq!(Tellor::get_total_stakers(), 1); // Ensure only unique addresses add to total stakers
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, token(105));
			assert_eq!(staker_details.locked_balance, token(0));
			assert_eq!(Tellor::get_total_stake_amount(), token(105));
		})
	});
}

#[test]
#[ignore]
fn remove_value() {
	todo!()
}

#[test]
#[ignore]
fn request_stake_withdraw() {
	todo!()
}

#[test]
#[ignore]
fn slash_reporter() {
	todo!()
}

#[test]
#[ignore]
fn submit_value() {
	todo!()
}

#[test]
#[ignore]
fn withdraw_stake() {
	todo!()
}

#[test]
#[ignore]
fn get_block_number_by_timestamp() {
	todo!()
}

#[test]
#[ignore]
fn get_current_value() {
	todo!()
}

#[test]
#[ignore]
fn get_new_value_count_by_query_id() {
	todo!()
}

#[test]
#[ignore]
fn get_report_details() {
	todo!()
}

#[test]
#[ignore]
fn get_reporting_lock() {
	todo!()
}

#[test]
#[ignore]
fn get_reporter_by_timestamp() {
	todo!()
}

#[test]
#[ignore]
fn get_reporter_last_timestamp() {
	todo!()
}

#[test]
#[ignore]
fn get_reports_submitted_by_address() {
	todo!()
}

#[test]
#[ignore]
fn get_reports_submitted_by_address_and_query_id() {
	todo!()
}

#[test]
#[ignore]
fn get_stake_amount() {
	todo!()
}

#[test]
#[ignore]
fn get_staker_info() {
	todo!()
}

#[test]
#[ignore]
fn get_time_of_last_new_value() {
	todo!()
}

#[test]
#[ignore]
fn get_timestamp_by_query_and_index() {
	todo!()
}

#[test]
#[ignore]
fn get_timestamp_index_by_timestamp() {
	todo!()
}

#[test]
#[ignore]
fn get_total_stake_amount() {
	todo!()
}

#[test]
#[ignore]
fn get_total_stakers() {
	todo!()
}

#[test]
#[ignore]
fn retrieve_data() {
	todo!()
}

#[test]
#[ignore]
fn get_total_time_based_rewards_balance() {
	todo!()
}

#[test]
#[ignore]
fn add_staking_rewards() {
	todo!()
}

#[test]
#[ignore]
fn get_pending_reward_by_staker() {
	todo!()
}

#[test]
#[ignore]
fn get_index_for_data_before() {
	todo!()
}

#[test]
#[ignore]
fn get_data_before() {
	todo!()
}

#[test]
#[ignore]
fn update_stake_amount() {
	todo!()
}

#[test]
#[ignore]
fn update_rewards() {
	todo!()
}

#[test]
#[ignore]
fn update_stake_and_pay_rewards() {
	todo!()
}

#[test]
#[ignore]
fn get_real_staking_rewards_balance() {
	todo!()
}

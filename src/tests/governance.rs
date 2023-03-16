use super::*;
use crate::Config;
use frame_support::{assert_noop, assert_ok};
use sp_core::Get;
use sp_runtime::traits::BadOrigin;

type DisputeRoundReportingPeriod = <Test as Config>::DisputeRoundReportingPeriod;
type VoteTallyDisputePeriod = <Test as Config>::VoteTallyDisputePeriod;

#[test]
fn begin_dispute() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(reporter, STAKE_AMOUNT, Address::random());
		})
	});

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L43
	ext.execute_with(|| {
		let (timestamp, _) = with_block(|| {
			assert_noop!(Tellor::begin_dispute(RuntimeOrigin::root(), query_id, 0), BadOrigin);
			assert_noop!(
				Tellor::begin_dispute(RuntimeOrigin::signed(another_reporter), query_id, 0),
				Error::NotReporter
			);

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				another_reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_noop!(
				Tellor::begin_dispute(RuntimeOrigin::signed(another_reporter), query_id, 0),
				Error::NoValueExists
			);
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
		});

		let (_, dispute_id) = with_block(|| {
			// todo:
			// await h.expectThrow(gov.connect(accounts[4]).beginDispute(ETH_QUERY_ID, blocky.timestamp)) // must have tokens to pay/begin dispute
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				timestamp
			));
			let dispute_id = 1;
			let dispute_info = Tellor::get_dispute_info(dispute_id).unwrap();
			let vote_info = Tellor::get_vote_info(dispute_id).unwrap();
			assert_eq!(Tellor::get_vote_count(), 1, "vote count should be correct");
			assert_eq!(
				dispute_info,
				(query_id, timestamp, uint_value(100), reporter),
				"dispute info should be correct"
			);
			assert_eq!(vote_info.initiator, another_reporter, "initiator should be correct");
			assert_eq!(
				Tellor::get_open_disputes_on_id(query_id),
				1,
				"open disputes on ID should be correct"
			);
			assert_eq!(
				Tellor::get_vote_rounds(vote_info.identifier),
				vec![1],
				"number of vote rounds should be correct"
			);
			// todo
			// assert(balance1 - balance2 - (await flex.getStakeAmount()/10) == 0, "dispute fee paid should be correct")
			dispute_id
		});

		let dispute_period = DisputeRoundReportingPeriod::get();
		with_block_after(dispute_period, || {
			assert_ok!(Tellor::tally_votes(dispute_id));
		});

		with_block_after(VoteTallyDisputePeriod::get(), || {
			assert_ok!(Tellor::report_slash(
				Origin::Governance.into(),
				dispute_id,
				reporter,
				another_reporter,
				STAKE_AMOUNT.into()
			));
		});

		let (timestamp, _) = with_block_after(dispute_period * 2, || {
			assert_noop!(
				Tellor::begin_dispute(RuntimeOrigin::signed(another_reporter), query_id, timestamp),
				Error::DisputeRoundReportingPeriodExpired
			); //assert second dispute started within a day

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				3,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(3),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
		});

		with_block_after(DisputeRoundReportingPeriod::get(), || {
			assert_noop!(
				Tellor::begin_dispute(RuntimeOrigin::signed(another_reporter), query_id, timestamp),
				Error::DisputeReportingPeriodExpired
			); //dispute must be started within timeframe
		})
	});
}

#[test]
fn begins_dispute_xcm() {
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

#[test]
#[ignore]
fn execute_vote() {
	todo!()
}

#[test]
#[ignore]
fn tally_votes() {
	todo!()
}

#[test]
#[ignore]
fn vote() {
	todo!()
}

#[test]
#[ignore]
fn vote_on_multiple_disputes() {
	todo!()
}

#[test]
#[ignore]
fn did_vote() {
	todo!()
}

#[test]
#[ignore]
fn get_dispute_info() {
	todo!()
}

#[test]
#[ignore]
fn get_open_disputes_on_id() {
	todo!()
}

#[test]
#[ignore]
fn get_vote_count() {
	todo!()
}

#[test]
#[ignore]
fn get_vote_info() {
	todo!()
}

#[test]
#[ignore]
fn get_vote_rounds() {
	todo!()
}

#[test]
#[ignore]
fn get_vote_tally_by_address() {
	todo!()
}

#[test]
#[ignore]
fn get_user_tips() {
	todo!()
}

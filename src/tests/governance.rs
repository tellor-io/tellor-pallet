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

use super::*;
use crate::{constants::REPORTING_LOCK, mock::AccountId, types::Tally, Config, VoteResult};
use frame_support::{assert_noop, assert_ok, traits::Currency};
use sp_core::{bounded::BoundedBTreeMap, bounded_btree_map};
use sp_runtime::traits::BadOrigin;

type BoundedVotes = BoundedBTreeMap<AccountId, bool, <Test as Config>::MaxVotes>;
type ParachainId = <Test as Config>::ParachainId;

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
		let timestamp = with_block(|| {
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
			now()
		});

		let dispute_id = with_block(|| {
			// todo:
			// await h.expectThrow(gov.connect(accounts[4]).beginDispute(ETH_QUERY_ID, blocky.timestamp)) // must have tokens to pay/begin dispute
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				timestamp
			));
			let dispute_id = dispute_id(PARA_ID, query_id, timestamp);
			let dispute_info = Tellor::get_dispute_info(dispute_id).unwrap();
			let vote_info = Tellor::get_vote_info(dispute_id, 1).unwrap();
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
				1,
				"number of vote rounds should be correct"
			);
			// todo
			// assert(balance1 - balance2 - (await flex.getStakeAmount()/10) == 0, "dispute fee paid should be correct")
			dispute_id
		});

		// Tally votes after vote duration
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::tally_votes(dispute_id, 1));
		});

		// Report slash after tally dispute period
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_slash(
				Origin::Governance.into(),
				dispute_id,
				reporter,
				another_reporter,
				STAKE_AMOUNT.into()
			));
		});

		let timestamp = with_block_after(86_400 * 2, || {
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
			now()
		});

		with_block_after(86_400 + 10, || {
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

			let timestamp = now();
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, timestamp));

			let sent_messages = sent_xcm();
			let (_, sent_message) = sent_messages.first().unwrap();
			assert!(sent_message
				.0
				.contains(&DescendOrigin(X1(PalletInstance(Tellor::index() as u8)))));
			// todo: check remaining instructions

			System::assert_last_event(
				Event::NewDispute {
					dispute_id: dispute_id(PARA_ID, query_id, timestamp),
					query_id,
					timestamp,
					reporter,
				}
				.into(),
			);
		});
	});
}

#[test]
fn execute_vote() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter_1 = 1;
	let dispute_reporter = 2;
	let reporter_3 = 3;
	let result = VoteResult::Passed;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(dispute_reporter, STAKE_AMOUNT, Address::random())
		})
	});

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L85
	ext.execute_with(|| {
		let (timestamp_1, dispute_1) = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter_1,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			//let balance_1 = Balances::balance(&dispute_reporter);
			assert_noop!(Tellor::execute_vote(H256::random(), 1, result), Error::InvalidDispute); // dispute id must be valid
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_1),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			let timestamp_1 = now();
			// todo
			// assert_noop!(
			// 	Tellor::begin_dispute(RuntimeOrigin::signed(4), query_id, now()),
			// 	pallet_balances::Error::<Test>::InsufficientBalance
			// ); // must have tokens to pay for dispute
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(dispute_reporter),
				query_id,
				timestamp_1
			));
			//let balance_2 = Balances::balance(&dispute_reporter);
			let dispute_1 = dispute_id(PARA_ID, query_id, timestamp_1);
			assert_eq!(
				Tellor::get_dispute_info(dispute_1).unwrap(),
				(query_id, timestamp_1, uint_value(100), reporter_1)
			);
			assert_eq!(
				Tellor::get_open_disputes_on_id(query_id),
				1,
				"open disputes on id should be correct"
			);
			assert_eq!(
				Tellor::get_vote_rounds(dispute_1),
				1,
				"number of vote rounds should be correct"
			);
			// todo: assert_eq!(balance_1 - balance_2, token(10), "dispute fee paid should be correct");

			assert_noop!(Tellor::execute_vote(H256::random(), 1, result), Error::InvalidDispute); // dispute id must exist
			assert_noop!(Tellor::execute_vote(dispute_1, 10, result), Error::InvalidVote); // vote round must exist
			assert_noop!(Tellor::execute_vote(dispute_1, 1, result), Error::VoteNotTallied); // vote must be tallied
			(timestamp_1, dispute_1)
		});

		// Tally votes after vote duration
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::tally_votes(dispute_1, 1));
			assert_noop!(
				Tellor::execute_vote(dispute_1, 1, result),
				Error::TallyDisputePeriodActive
			); // a day must pass before execution
		});

		// Execute after tally dispute period
		let (timestamp_2, dispute_2) = with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::execute_vote(dispute_1, 1, result));
			assert_noop!(Tellor::execute_vote(dispute_1, 1, result), Error::VoteAlreadyExecuted); // vote already executed
			assert_noop!(
				Tellor::begin_dispute(
					RuntimeOrigin::signed(dispute_reporter),
					query_id,
					timestamp_1
				),
				Error::DisputeRoundReportingPeriodExpired
			); // assert second dispute started within a day

			let vote = Tellor::get_vote_info(dispute_1, 1).unwrap();
			assert_eq!(vote.identifier, dispute_1, "identifier should be correct");
			assert_eq!(vote.vote_round, 1, "vote round should be correct");
			assert_eq!(vote.executed, true, "vote should be executed");
			assert_eq!(vote.result, Some(result), "vote should pass");

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter_3,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_3),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			let timestamp_2 = now();
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(dispute_reporter),
				query_id,
				timestamp_2
			));
			(timestamp_2, dispute_id(PARA_ID, query_id, timestamp_2))
		});

		let dispute_3 = with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::tally_votes(dispute_2, 1));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(dispute_reporter),
				query_id,
				timestamp_2
			));
			dispute_id(PARA_ID, query_id, timestamp_2)
		});

		with_block_after(86_400 * 2, || {
			assert_noop!(Tellor::execute_vote(dispute_2, 1, result), Error::VoteNotFinal);
			// vote must be the final vote
		});

		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::tally_votes(dispute_3, 2));
			assert_noop!(
				Tellor::execute_vote(dispute_3, 2, result),
				Error::TallyDisputePeriodActive
			); // must wait longer
		});

		with_block_after(86_400, || {
			assert_ok!(Tellor::execute_vote(dispute_3, 2, result));
		});
	});
}

#[test]
fn tally_votes() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L143
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			// 1) dispute could not have been tallied,
			// 2) dispute does not exist,
			// 3) cannot tally before the voting time has ended
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_noop!(Tellor::tally_votes(H256::random(), 1), Error::InvalidDispute); // Cannot tally a dispute that does not exist

			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			let dispute_id = super::dispute_id(PARA_ID, query_id, now());
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id, Some(false)));
			assert_noop!(Tellor::tally_votes(dispute_id, 1), Error::VotingPeriodActive); // Time for voting has not elapsed
			dispute_id
		});

		with_block_after(86_400, || {
			assert_ok!(Tellor::tally_votes(dispute_id, 1));
			assert_noop!(Tellor::tally_votes(dispute_id, 1), Error::VoteAlreadyTallied); // cannot re-tally a dispute

			let vote_info = Tellor::get_vote_info(dispute_id, 1).unwrap();
			assert_eq!(vote_info.tally_date, now(), "Tally date should be correct");
		});
	});
}

#[test]
fn vote() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter_1 = 1;
	let reporter_2 = 2;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L170
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			// 1 dispute must exist
			// 2) cannot have been tallied
			// 3) sender has already voted
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter_2,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_2),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter_2), query_id, now()));
			assert_noop!(
				Tellor::vote(RuntimeOrigin::signed(reporter_2), H256::random(), Some(false)),
				Error::InvalidVote
			); // Can't vote on dispute does not exist

			let dispute_id = super::dispute_id(PARA_ID, query_id, now());
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter_1), dispute_id, Some(true)));
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter_2), dispute_id, Some(false)));
			assert_noop!(
				Tellor::vote(RuntimeOrigin::signed(reporter_2), dispute_id, Some(true)),
				Error::AlreadyVoted
			); // Sender has already voted
			dispute_id
		});

		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::tally_votes(dispute_id, 1));
			assert_noop!(
				Tellor::vote(RuntimeOrigin::signed(reporter_2), dispute_id, Some(true)),
				Error::VoteAlreadyTallied
			); // Vote has already been tallied

			let vote_info = Tellor::get_vote_info(dispute_id, 1).unwrap();
			assert_eq!(vote_info.users, Tally::default(), "users tally should be correct");
			assert_eq!(
				vote_info.reporters.does_support, 0,
				"reporters does_support tally should be correct"
			);
			assert_eq!(vote_info.reporters.against, 1, "reporters against tally should be correct");
			assert_eq!(
				vote_info.reporters.invalid_query, 0,
				"reporters invalid tally should be correct"
			);

			assert!(
				Tellor::did_vote(dispute_id, 1, reporter_2),
				"voter's voted status should be correct"
			);
			assert!(
				Tellor::did_vote(dispute_id, 1, reporter_1),
				"voter's voted status should be correct"
			);
			assert!(!Tellor::did_vote(dispute_id, 1, 3), "voter's voted status should be correct");

			assert_eq!(
				Tellor::get_vote_tally_by_address(reporter_2),
				1,
				"vote tally by address should be correct"
			);
			assert_eq!(
				Tellor::get_vote_tally_by_address(reporter_1),
				1,
				"vote tally by address should be correct"
			);
		})
	});
}

#[test]
#[ignore]
fn vote_on_multiple_disputes() {
	todo!()
}

#[test]
fn did_vote() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L248
	ext.execute_with(|| {
		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			let dispute_id = super::dispute_id(PARA_ID, query_id, now());
			assert!(
				!Tellor::did_vote(dispute_id, 1, reporter),
				"voter's voted status should be correct"
			);
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id, Some(true)));
			assert!(
				Tellor::did_vote(dispute_id, 1, reporter),
				"voter's voted status should be correct"
			);
		});
	});
}

#[test]
fn get_dispute_info() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L260
	ext.execute_with(|| {
		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			let dispute_info =
				Tellor::get_dispute_info(dispute_id(PARA_ID, query_id, now())).unwrap();
			assert_eq!(dispute_info.0, query_id, "disputed query id should be correct");
			assert_eq!(dispute_info.1, now(), "disputed timestamp should be correct");
			assert_eq!(dispute_info.2, uint_value(100), "disputed value should be correct");
			assert_eq!(dispute_info.3, reporter, "disputed reporter should be correct");
		});
	});
}

#[test]
fn get_disputes_by_reporter() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let dispute_initiator = 2;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			register_parachain(STAKE_AMOUNT);
			deposit_stake(dispute_initiator, STAKE_AMOUNT, Address::random())
		})
	});

	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_disputes_by_reporter(reporter), Vec::<DisputeId>::new());
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(dispute_initiator),
				query_id,
				now()
			));
			dispute_id(PARA_ID, query_id, now())
		});

		let dispute_id_2 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));

			assert_eq!(Tellor::get_disputes_by_reporter(reporter), vec![dispute_id]);
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(dispute_initiator),
				query_id,
				now()
			));
			let dispute_id_2 = super::dispute_id(PARA_ID, query_id, now());
			assert_eq!(
				sort(Tellor::get_disputes_by_reporter(reporter)),
				sort(vec![dispute_id, dispute_id_2])
			);
			dispute_id_2
		});

		// Tally votes after vote duration
		with_block_after(86_400, || {
			assert_ok!(Tellor::tally_votes(dispute_id, 1));
		});
		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::execute_vote(dispute_id, 1, VoteResult::Passed));
		});

		assert_eq!(
			sort(Tellor::get_disputes_by_reporter(reporter)),
			sort(vec![dispute_id, dispute_id_2])
		);
	});
}

#[test]
fn get_open_disputes_on_id() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L274
	ext.execute_with(|| {
		let timestamp = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			now()
		});
		let dispute_id = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				another_reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));

			assert_eq!(Tellor::get_open_disputes_on_id(query_id), 0);
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, timestamp));
			assert_eq!(Tellor::get_open_disputes_on_id(query_id), 1);
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			assert_eq!(Tellor::get_open_disputes_on_id(query_id), 2);
			dispute_id(PARA_ID, query_id, now())
		});

		// Tally votes after vote duration
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::tally_votes(dispute_id, 1));
		});
		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::execute_vote(dispute_id, 1, VoteResult::Passed));
		});

		assert_eq!(Tellor::get_open_disputes_on_id(query_id), 1);
	});
}

#[test]
fn get_vote_count() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L298
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			assert_eq!(Tellor::get_vote_count(), 0, "vote count should start at 0");
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			assert_eq!(Tellor::get_vote_count(), 1, "vote count should increment correctly");
			dispute_id(PARA_ID, query_id, now())
		});

		// Tally votes after vote duration
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::tally_votes(dispute_id, 1));
		});
		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::execute_vote(dispute_id, 1, VoteResult::Passed));
			assert_eq!(
				Tellor::get_vote_count(),
				1,
				"vote count should not change after vote execution"
			);
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			assert_eq!(Tellor::get_vote_count(), 2, "vote count should increment correctly");
		})
	});
}

#[test]
fn get_vote_info() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L322
	ext.execute_with(|| {
		let (disputed_time, disputed_block, dispute_id) = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id, Some(true)));
			(now(), System::block_number(), dispute_id)
		});

		// Tally votes after vote duration
		let tallied = with_block_after(86_400 * 7, || {
			assert_ok!(Tellor::tally_votes(dispute_id, 1));
			now()
		});
		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::execute_vote(dispute_id, 1, VoteResult::Passed));
			let vote = Tellor::get_vote_info(dispute_id, 1).unwrap();
			let parachain_id: u32 = ParachainId::get();
			assert_eq!(
				vote.identifier,
				keccak_256(&ethabi::encode(&vec![
					Token::Uint(parachain_id.into()),
					Token::FixedBytes(query_id.0.to_vec()),
					Token::Uint(disputed_time.into())
				]))
				.into(),
				"vote identifier should be correct"
			);
			assert_eq!(vote.vote_round, 1, "vote round should be correct");
			assert_eq!(vote.start_date, disputed_time, "vote start date should be correct");
			assert_eq!(vote.block_number, disputed_block, "vote block number should be correct");
			assert_eq!(vote.fee, token(10), "vote fee should be correct");
			assert_eq!(vote.tally_date, tallied, "vote tally date should be correct");
			assert_eq!(vote.users, Tally::default(), "vote users should be correct");
			assert_eq!(
				vote.reporters,
				Tally { does_support: 1, against: 0, invalid_query: 0 },
				"vote reporters should be correct"
			);
			assert_eq!(vote.executed, true, "vote executed should be correct");
			assert_eq!(vote.result, Some(VoteResult::Passed), "vote result should be Passed");
			assert_eq!(vote.initiator, reporter, "vote initiator should be correct");
			let voted: BoundedVotes = bounded_btree_map!(reporter => true);
			assert_eq!(vote.voted, voted, "vote account vote status should be correct");
		})
	});
}

#[test]
fn get_vote_rounds() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L361
	ext.execute_with(|| {
		let (timestamp, dispute_id) = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_eq!(Tellor::get_vote_rounds(dispute_id), 1);
			(now(), dispute_id)
		});

		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::tally_votes(dispute_id, 1));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, timestamp));
			assert_eq!(Tellor::get_vote_rounds(dispute_id), 2);
		});
	});
}

#[test]
fn get_vote_tally_by_address() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L383
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			dispute_id(PARA_ID, query_id, now())
		});

		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				another_reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			let dispute_id_2 = super::dispute_id(PARA_ID, query_id, now());

			assert_eq!(
				Tellor::get_vote_tally_by_address(reporter),
				0,
				"vote tally should be correct"
			);
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id, Some(false)));
			assert_eq!(
				Tellor::get_vote_tally_by_address(reporter),
				1,
				"vote tally should be correct"
			);
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id_2, Some(false)));
			assert_eq!(
				Tellor::get_vote_tally_by_address(reporter),
				2,
				"vote tally should be correct"
			);
		});
	});
}

#[test]
fn get_tips_by_address() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let user = 1;
	let reporter = 2;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L404
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				STAKE_AMOUNT.into(),
				Address::random()
			));
			Balances::make_free_balance_be(&user, token(1_000) + 1);
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(user),
				query_id,
				token(20),
				query_data.clone()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data,
			));

			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, now()));
			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(user), dispute_id, Some(true)));
			dispute_id
		});

		// Tally votes after vote duration
		with_block_after(86_400, || {
			assert_ok!(Tellor::tally_votes(dispute_id, 1));
			now()
		});

		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::execute_vote(dispute_id, 1, VoteResult::Passed));
			assert_eq!(
				Tellor::get_vote_info(dispute_id, 1).unwrap().users,
				Tally { does_support: token(20), against: 0, invalid_query: 0 },
				"vote users does_support weight should be based on tip total"
			)
		});
	});
}

fn sort(mut disputes: Vec<DisputeId>) -> Vec<DisputeId> {
	disputes.sort();
	disputes
}

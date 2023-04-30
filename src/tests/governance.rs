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
use crate::{
	constants::REPORTING_LOCK, contracts, mock::AccountId, types::Tally, Config, VoteResult, HOURS,
};
use frame_support::{
	assert_noop, assert_ok,
	traits::{Currency, Hooks},
};
use sp_core::{bounded::BoundedBTreeMap, bounded_btree_map};
use sp_runtime::traits::BadOrigin;
use std::collections::VecDeque;

type BoundedVotes = BoundedBTreeMap<AccountId, bool, <Test as Config>::MaxVotes>;
type ParachainId = <Test as Config>::ParachainId;
type PendingVotes = crate::pallet::PendingVotes<Test>;
type VoteInfo = crate::pallet::VoteInfo<Test>;
type VoteRounds = crate::pallet::VoteRounds<Test>;

#[test]
fn begin_dispute() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random()))
	});

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L43
	ext.execute_with(|| {
		let timestamp = with_block(|| {
			assert_noop!(
				Tellor::begin_dispute(RuntimeOrigin::root(), query_id, 0, None),
				BadOrigin
			);
			assert_noop!(
				Tellor::begin_dispute(RuntimeOrigin::signed(another_reporter), query_id, 0, None),
				Error::NotReporter
			);

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				another_reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_noop!(
				Tellor::begin_dispute(RuntimeOrigin::signed(another_reporter), query_id, 0, None),
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
			assert_noop!(
				Tellor::begin_dispute(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					timestamp,
					None
				),
				pallet_balances::Error::<Test>::InsufficientBalance
			);
			Balances::make_free_balance_be(&another_reporter, token(1_000));
			let balance_before_begin_dispute = Balances::free_balance(&another_reporter);
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				timestamp,
				None,
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

			let balance_after_begin_dispute = Balances::free_balance(another_reporter);
			assert_eq!(
				balance_before_begin_dispute - balance_after_begin_dispute - dispute_fee(),
				0,
				"dispute fee paid should be correct"
			);

			let dispute_initialization_fee = vote_info.fee;
			assert_eq!(
				Balances::free_balance(another_reporter),
				balance_before_begin_dispute - dispute_initialization_fee
			);
			dispute_id
		});

		// Tally votes after vote duration
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
		});

		// Report slash after tally dispute period
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_slash(
				Origin::Governance.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into()
			));
		});

		let timestamp = with_block_after(86_400 * 2, || {
			assert_noop!(
				Tellor::begin_dispute(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					timestamp,
					None
				),
				Error::DisputeRoundReportingPeriodExpired
			); //assert second dispute started within a day

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				3,
				MINIMUM_STAKE_AMOUNT.into(),
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
				Tellor::begin_dispute(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					timestamp,
					None
				),
				Error::DisputeReportingPeriodExpired
			); //dispute must be started within timeframe
		})
	});
}

#[test]
fn begin_dispute_by_non_reporter() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;
	let oracle_user = Address::random();
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random()))
	});

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L43
	ext.execute_with(|| {
		let timestamp = with_block(|| {
			assert_noop!(
				Tellor::begin_dispute(RuntimeOrigin::root(), query_id, 0, Some(oracle_user)),
				BadOrigin
			);

			assert_noop!(
				Tellor::begin_dispute(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					0,
					Some(oracle_user)
				),
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
			// await h.expectThrow(gov.connect(accounts[4]).beginDispute(ETH_QUERY_ID, blocky.timestamp)) // must have tokens to pay/begin dispute
			assert_noop!(
				Tellor::begin_dispute(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					timestamp,
					Some(oracle_user)
				),
				pallet_balances::Error::<Test>::InsufficientBalance
			);
			Balances::make_free_balance_be(&another_reporter, token(1_000));
			let balance_before_begin_dispute = Balances::free_balance(&another_reporter);
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				timestamp,
				Some(oracle_user),
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

			let balance_after_begin_dispute = Balances::free_balance(another_reporter);
			assert_eq!(
				balance_before_begin_dispute - balance_after_begin_dispute - dispute_fee(),
				0,
				"dispute fee paid should be correct"
			);

			let dispute_initialization_fee = vote_info.fee;
			assert_eq!(
				Balances::free_balance(another_reporter),
				balance_before_begin_dispute - dispute_initialization_fee
			);
			dispute_id
		});

		// Tally votes after vote duration
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
		});

		// Report slash after tally dispute period
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_slash(
				Origin::Governance.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into()
			));
		});

		let timestamp = with_block_after(86_400 * 2, || {
			assert_noop!(
				Tellor::begin_dispute(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					timestamp,
					Some(oracle_user)
				),
				Error::DisputeRoundReportingPeriodExpired
			); //assert second dispute started within a day

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				3,
				MINIMUM_STAKE_AMOUNT.into(),
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
				Tellor::begin_dispute(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					timestamp,
					Some(oracle_user)
				),
				Error::DisputeReportingPeriodExpired
			); //dispute must be started within timeframe
		})
	});
}

#[test]
fn begins_dispute_xcm() {
	new_test_ext().execute_with(|| {
		with_block(|| {
			let reporter = 1;
			let reporter_address = Address::random();
			deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, reporter_address);

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
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				timestamp,
				None
			));

			assert_eq!(
				sent_xcm(),
				vec![xcm_transact(
					ethereum_xcm::transact(
						*GOVERNANCE,
						contracts::governance::begin_parachain_dispute(
							query_id.as_ref(),
							timestamp,
							uint_value(123).as_ref(),
							reporter_address,
							reporter_address,
							MINIMUM_STAKE_AMOUNT
						)
						.try_into()
						.unwrap(),
						gas_limits::BEGIN_PARACHAIN_DISPUTE
					)
					.into(),
					gas_limits::BEGIN_PARACHAIN_DISPUTE
				)]
			);
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
fn begin_dispute_checks_max_vote_rounds() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random());
		})
	});

	ext.execute_with(|| {
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));

			let timestamp = now();
			let dispute_id = dispute_id(PARA_ID, query_id, timestamp);
			VoteRounds::set(dispute_id, u8::MAX);

			Balances::make_free_balance_be(&reporter, token(10));
			assert_noop!(
				Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, timestamp, None),
				Error::MaxVoteRoundsReached
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
		with_block(|| deposit_stake(dispute_reporter, MINIMUM_STAKE_AMOUNT, Address::random()))
	});

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L85
	ext.execute_with(|| {
		let (timestamp_1, dispute_1) = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter_1,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_noop!(
				Tellor::report_vote_executed(Origin::Governance.into(), H256::random()),
				Error::InvalidDispute
			);
			// dispute id must be valid
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_1),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			let timestamp_1 = now();
			assert_noop!(
				Tellor::begin_dispute(
					RuntimeOrigin::signed(dispute_reporter),
					query_id,
					now(),
					None
				),
				pallet_balances::Error::<Test>::InsufficientBalance
			); // must have tokens to pay for dispute
			Balances::make_free_balance_be(&dispute_reporter, token(1_000));
			let balance_1 = Balances::free_balance(&dispute_reporter);
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(dispute_reporter),
				query_id,
				timestamp_1,
				None
			));
			let balance_2 = Balances::free_balance(&dispute_reporter);
			let dispute_id = dispute_id(PARA_ID, query_id, timestamp_1);
			assert_eq!(
				Tellor::get_dispute_info(dispute_id).unwrap(),
				(query_id, timestamp_1, uint_value(100), reporter_1)
			);
			assert_eq!(
				Tellor::get_open_disputes_on_id(query_id),
				1,
				"open disputes on id should be correct"
			);
			assert_eq!(
				Tellor::get_vote_rounds(dispute_id),
				1,
				"number of vote rounds should be correct"
			);
			assert_eq!(balance_1 - balance_2, dispute_fee(), "dispute fee paid should be correct");

			assert_noop!(
				Tellor::report_vote_executed(Origin::Governance.into(), H256::random()),
				Error::InvalidDispute
			); // dispute id must exist
			assert_noop!(
				Tellor::report_vote_executed(Origin::Governance.into(), dispute_id),
				Error::VoteNotTallied
			); // vote must be tallied
			(timestamp_1, dispute_id)
		});

		// Tally votes after vote duration
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_vote_tallied(Origin::Governance.into(), dispute_1, result));
			assert_noop!(
				Tellor::report_vote_executed(Origin::Governance.into(), dispute_1),
				Error::TallyDisputePeriodActive
			);
			// a day must pass before execution
		});

		// Execute after tally dispute period
		let (timestamp_2, dispute_2) = with_block_after(86_400 * 2, || {
			let previous_balance = Balances::free_balance(&dispute_reporter);
			assert_ok!(Tellor::report_vote_executed(Origin::Governance.into(), dispute_1));
			let dispute_fee = Tellor::get_vote_info(dispute_1, 1).unwrap().fee;

			assert_eq!(previous_balance + dispute_fee, Balances::free_balance(&dispute_reporter));

			assert_noop!(
				Tellor::report_vote_executed(Origin::Governance.into(), dispute_1),
				Error::VoteAlreadyExecuted
			);
			// vote already executed
			assert_noop!(
				Tellor::begin_dispute(
					RuntimeOrigin::signed(dispute_reporter),
					query_id,
					timestamp_1,
					None
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
				MINIMUM_STAKE_AMOUNT.into(),
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
				timestamp_2,
				None
			));
			(timestamp_2, dispute_id(PARA_ID, query_id, timestamp_2))
		});

		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_vote_tallied(Origin::Governance.into(), dispute_2, result));
			// Start another round
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(dispute_reporter),
				query_id,
				timestamp_2,
				None
			));
		});

		with_block_after(86_400 * 2, || {
			assert_eq!(Tellor::get_vote_rounds(dispute_2), 2);
			assert_noop!(
				Tellor::report_vote_executed(Origin::Governance.into(), dispute_2),
				Error::VoteNotTallied
			);
			// vote round must be tallied
		});

		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_vote_tallied(Origin::Governance.into(), dispute_2, result));
			assert_noop!(
				Tellor::report_vote_executed(Origin::Governance.into(), dispute_2),
				Error::TallyDisputePeriodActive
			);
			// must wait longer
		});

		with_block_after(86_400, || {
			let previous_balance = Balances::free_balance(&dispute_reporter);
			assert_ok!(Tellor::report_vote_executed(Origin::Governance.into(), dispute_2));
			let first_round_fee = Tellor::get_vote_info(dispute_2, 1).unwrap().fee;
			let second_round_fee = Tellor::get_vote_info(dispute_2, 2).unwrap().fee;
			assert_eq!(
				previous_balance + first_round_fee + second_round_fee,
				Balances::free_balance(&dispute_reporter)
			);
		});
	});
}

#[test]
fn tally_votes() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L143
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			// 1) dispute could not have been tallied,
			// 2) dispute does not exist,
			// 3) cannot tally before the voting time has ended
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_noop!(
				Tellor::report_vote_tallied(
					Origin::Governance.into(),
					H256::random(),
					VoteResult::Invalid
				),
				Error::InvalidDispute
			); // Cannot tally a dispute that does not exist

			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id, Some(false)));
			assert_noop!(
				Tellor::report_vote_tallied(
					Origin::Governance.into(),
					dispute_id,
					VoteResult::Failed
				),
				Error::VotingPeriodActive
			); // Time for voting has not elapsed
			dispute_id
		});

		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
			assert_noop!(
				Tellor::report_vote_tallied(
					Origin::Governance.into(),
					dispute_id,
					VoteResult::Passed
				),
				Error::VoteAlreadyTallied
			); // cannot re-tally a dispute

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

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L170
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			// 1 dispute must exist
			// 2) cannot have been tallied
			// 3) sender has already voted
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter_2,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter_2),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			Balances::make_free_balance_be(&reporter_2, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter_2),
				query_id,
				now(),
				None
			));
			assert_noop!(
				Tellor::vote(RuntimeOrigin::signed(reporter_2), H256::random(), Some(false)),
				Error::InvalidVote
			); // Can't vote on dispute does not exist

			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter_1), dispute_id, Some(true)));
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter_2), dispute_id, Some(false)));
			assert_noop!(
				Tellor::vote(RuntimeOrigin::signed(reporter_2), dispute_id, Some(true)),
				Error::AlreadyVoted
			); // Sender has already voted
			dispute_id
		});

		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
			assert_noop!(
				Tellor::vote(RuntimeOrigin::signed(reporter_2), dispute_id, Some(true)),
				Error::VoteAlreadyTallied
			); // Vote has already been tallied

			let vote_info = Tellor::get_vote_info(dispute_id, 1).unwrap();
			assert_eq!(
				vote_info.users,
				Tally::<BalanceOf<Test>>::default(),
				"users tally should be correct"
			);
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
				Tellor::get_vote_tally_by_address(&reporter_2),
				1,
				"vote tally by address should be correct"
			);
			assert_eq!(
				Tellor::get_vote_tally_by_address(&reporter_1),
				1,
				"vote tally by address should be correct"
			);
		})
	});
}

#[test]
fn send_votes() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let user = 1;
	let reporter = 2;
	let disputer = 3;
	let mut ext = new_test_ext();

	const DISPUTES: u8 = 6;

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random());
			Balances::make_free_balance_be(&user, token(10));
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(user),
				query_id,
				token(1),
				query_data.clone()
			));
			Balances::make_free_balance_be(&disputer, token(1_000));
		})
	});

	ext.execute_with(|| {
		// Create a series of disputes
		let mut expected_pending_votes = VecDeque::new();
		for _ in 1..=DISPUTES {
			with_block_after(12 * HOURS, || {
				assert_ok!(Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					query_id,
					uint_value(100),
					0,
					query_data.clone(),
				));
				let timestamp = now();
				assert_ok!(Tellor::begin_dispute(
					RuntimeOrigin::signed(disputer),
					query_id,
					timestamp,
					Some(Address::random())
				));

				let dispute_id = dispute_id(PARA_ID, query_id, timestamp);

				assert_ok!(Tellor::vote(RuntimeOrigin::signed(user), dispute_id, Some(true)));
				assert_ok!(Tellor::vote(RuntimeOrigin::signed(disputer), dispute_id, None)); // No effect as disputer is neither user or reporter
				assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id, Some(false)));

				let vote_round = VoteRounds::get(dispute_id);
				let vote_info = VoteInfo::get(dispute_id, vote_round).unwrap();
				assert!(!vote_info.sent); // Ensure new vote round not sent
				assert_eq!(
					PendingVotes::get(dispute_id).unwrap(),
					(vote_round, timestamp + 11 * HOURS)
				); // Ensure vote scheduled
				expected_pending_votes.push_back((
					dispute_id,
					vote_round,
					(vote_info.users, vote_info.reporters),
				));
			});
		}
		assert_eq!(PendingVotes::iter().count(), DISPUTES as usize);

		// Process pending votes in batches
		for _ in 1..DISPUTES {
			with_block_after(1 * HOURS, || {
				assert_ok!(Tellor::send_votes(RuntimeOrigin::signed(reporter), 5));
				let mut sent_xcm: VecDeque<_> = sent_xcm().into();
				for e in System::events() {
					let (dispute_id, vote_round, (tips, reports)) =
						expected_pending_votes.pop_front().unwrap();
					// Ensure votes sent to governance contract
					assert_eq!(
						sent_xcm.pop_front().unwrap(),
						xcm_transact(
							ethereum_xcm::transact(
								*GOVERNANCE,
								contracts::governance::vote(
									dispute_id.as_ref(),
									tips.does_support,
									tips.against,
									tips.invalid_query,
									reports.does_support,
									reports.against,
									reports.invalid_query
								)
								.try_into()
								.unwrap(),
								gas_limits::VOTE
							)
							.into(),
							gas_limits::VOTE
						)
					);
					assert!(VoteInfo::get(dispute_id, vote_round).unwrap().sent); // Ensure sent
					assert!(!PendingVotes::contains_key(dispute_id)); // Ensure 'dequeued'
					assert_eq!(e.event, Event::VoteSent { dispute_id, vote_round }.into()); // Ensure event emitted
					assert_noop!(
						Tellor::vote(RuntimeOrigin::signed(0), dispute_id, None),
						Error::VoteAlreadySent
					); // Ensure no more votes can be cast
				}
			});
		}

		assert_eq!(PendingVotes::iter().count(), 1);
		assert_eq!(expected_pending_votes.len(), 1);
	});
}

#[test]
fn send_votes_via_hook() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let user = 1;
	let reporter = 2;
	let disputer = 3;
	let mut ext = new_test_ext();

	const DISPUTES: u8 = 10;

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random());
			Balances::make_free_balance_be(&user, token(10));
			assert_ok!(Tellor::tip(
				RuntimeOrigin::signed(user),
				query_id,
				token(1),
				query_data.clone()
			));
			Balances::make_free_balance_be(&disputer, token(1_000));
		})
	});

	ext.execute_with(|| {
		// Create a series of disputes
		let mut expected_pending_votes = VecDeque::new();
		for _ in 1..=DISPUTES {
			with_block_after(12 * HOURS, || {
				assert_ok!(Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					query_id,
					uint_value(100),
					0,
					query_data.clone(),
				));
				let timestamp = now();
				assert_ok!(Tellor::begin_dispute(
					RuntimeOrigin::signed(disputer),
					query_id,
					timestamp,
					Some(Address::random())
				));

				let dispute_id = dispute_id(PARA_ID, query_id, timestamp);

				assert_ok!(Tellor::vote(RuntimeOrigin::signed(user), dispute_id, Some(true)));
				assert_ok!(Tellor::vote(RuntimeOrigin::signed(disputer), dispute_id, None)); // No effect as disputer is neither user or reporter
				assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id, Some(false)));

				let vote_round = VoteRounds::get(dispute_id);
				let vote_info = VoteInfo::get(dispute_id, vote_round).unwrap();
				assert!(!vote_info.sent); // Ensure new vote round not sent
				assert_eq!(
					PendingVotes::get(dispute_id).unwrap(),
					(vote_round, timestamp + 11 * HOURS)
				); // Ensure vote scheduled
				expected_pending_votes.push_back((
					dispute_id,
					vote_round,
					(vote_info.users, vote_info.reporters),
				));
			});
		}

		// Process pending votes in batches
		while PendingVotes::iter().count() > 0 {
			with_block_after(12 * HOURS, || {
				Tellor::on_initialize(System::block_number());
				let mut sent_xcm: VecDeque<_> = sent_xcm().into();
				for e in System::events() {
					let (dispute_id, vote_round, (tips, reports)) =
						expected_pending_votes.pop_front().unwrap();
					// Ensure votes sent to governance contract
					assert_eq!(
						sent_xcm.pop_front().unwrap(),
						xcm_transact(
							ethereum_xcm::transact(
								*GOVERNANCE,
								contracts::governance::vote(
									dispute_id.as_ref(),
									tips.does_support,
									tips.against,
									tips.invalid_query,
									reports.does_support,
									reports.against,
									reports.invalid_query
								)
								.try_into()
								.unwrap(),
								gas_limits::VOTE
							)
							.into(),
							gas_limits::VOTE
						)
					);
					assert!(VoteInfo::get(dispute_id, vote_round).unwrap().sent); // Ensure sent
					assert!(!PendingVotes::contains_key(dispute_id)); // Ensure 'dequeued'
					assert_eq!(e.event, Event::VoteSent { dispute_id, vote_round }.into()); // Ensure event emitted
					assert_noop!(
						Tellor::vote(RuntimeOrigin::signed(0), dispute_id, None),
						Error::VoteAlreadySent
					); // Ensure no more votes can be cast
				}
			});
		}
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

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L248
	ext.execute_with(|| {
		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
			let dispute_id = dispute_id(PARA_ID, query_id, now());
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

fn dispute_fee() -> Balance {
	token(100 / 10 * 5) // 10% of 100 TRB, * initial price of 5
}

#[test]
fn get_dispute_fee() {
	new_test_ext().execute_with(|| {
		with_block(|| {
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee());
		})
	});
}

#[test]
fn get_dispute_info() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L260
	ext.execute_with(|| {
		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
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
		with_block(|| deposit_stake(dispute_initiator, MINIMUM_STAKE_AMOUNT, Address::random()))
	});

	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
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
			Balances::make_free_balance_be(&dispute_initiator, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(dispute_initiator),
				query_id,
				now(),
				None
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
				now(),
				None
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
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
		});
		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_executed(Origin::Governance.into(), dispute_id));
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

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L274
	ext.execute_with(|| {
		let timestamp = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
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
				MINIMUM_STAKE_AMOUNT.into(),
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
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				timestamp,
				None
			));
			assert_eq!(Tellor::get_open_disputes_on_id(query_id), 1);
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
			assert_eq!(Tellor::get_open_disputes_on_id(query_id), 2);
			dispute_id(PARA_ID, query_id, now())
		});

		// Tally votes after vote duration
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
		});
		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_executed(Origin::Governance.into(), dispute_id));
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

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L298
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			assert_eq!(Tellor::get_vote_count(), 0, "vote count should start at 0");
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
			assert_eq!(Tellor::get_vote_count(), 1, "vote count should increment correctly");
			dispute_id(PARA_ID, query_id, now())
		});

		// Tally votes after vote duration
		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
		});
		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_executed(Origin::Governance.into(), dispute_id));
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
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
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

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L322
	ext.execute_with(|| {
		let (disputed_time, disputed_block, dispute_id) = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id, Some(true)));
			(now(), System::block_number(), dispute_id)
		});

		// Tally votes after vote duration
		let tallied = with_block_after(86_400 * 7, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
			now()
		});
		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_executed(Origin::Governance.into(), dispute_id));
			let vote = Tellor::get_vote_info(dispute_id, 1).unwrap();
			let parachain_id: u32 = ParachainId::get();
			assert_eq!(
				vote.identifier,
				keccak_256(&ethabi::encode(&vec![
					Token::Uint(parachain_id.into()),
					Token::FixedBytes(query_id.0.to_vec()),
					Token::Uint(disputed_time.into()),
				]))
				.into(),
				"vote identifier should be correct"
			);
			assert_eq!(vote.vote_round, 1, "vote round should be correct");
			assert_eq!(vote.start_date, disputed_time, "vote start date should be correct");
			assert_eq!(vote.block_number, disputed_block, "vote block number should be correct");
			assert_eq!(vote.fee, dispute_fee(), "vote fee should be correct");
			assert_eq!(vote.tally_date, tallied, "vote tally date should be correct");
			assert_eq!(
				vote.users,
				Tally::<BalanceOf<Test>>::default(),
				"vote users should be correct"
			);
			assert_eq!(
				vote.reporters,
				Tally::<u128> { does_support: 1, against: 0, invalid_query: 0 },
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

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L361
	ext.execute_with(|| {
		let (timestamp, dispute_id) = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_eq!(Tellor::get_vote_rounds(dispute_id), 1);
			(now(), dispute_id)
		});

		with_block_after(86_400 * 2, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				timestamp,
				None
			));
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

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L383
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
			dispute_id(PARA_ID, query_id, now())
		});

		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				another_reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
			let dispute_id_2 = super::dispute_id(PARA_ID, query_id, now());

			assert_eq!(
				Tellor::get_vote_tally_by_address(&reporter),
				0,
				"vote tally should be correct"
			);
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id, Some(false)));
			assert_eq!(
				Tellor::get_vote_tally_by_address(&reporter),
				1,
				"vote tally should be correct"
			);
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), dispute_id_2, Some(false)));
			assert_eq!(
				Tellor::get_vote_tally_by_address(&reporter),
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

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L404
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
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
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				now(),
				None
			));
			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(user), dispute_id, Some(true)));
			dispute_id
		});

		// Tally votes after vote duration
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
			now()
		});

		// Execute vote after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_executed(Origin::Governance.into(), dispute_id));
			assert_eq!(
				Tellor::get_vote_info(dispute_id, 1).unwrap().users,
				Tally::<BalanceOf<Test>> { does_support: token(20), against: 0, invalid_query: 0 },
				"vote users does_support weight should be based on tip total"
			)
		});
	});
}

fn sort(mut disputes: Vec<DisputeId>) -> Vec<DisputeId> {
	disputes.sort();
	disputes
}

#[test]
fn invalid_dispute() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random()))
	});

	ext.execute_with(|| {
		Balances::make_free_balance_be(&reporter, token(1_000));
		let balance_before_begin_dispute = Balances::free_balance(&reporter);
		let dispute_id = with_block(|| {
			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_noop!(
				Tellor::report_vote_executed(RuntimeOrigin::signed(reporter), dispute_id),
				BadOrigin
			);

			submit_value_and_begin_dispute(reporter, query_id, query_data.clone())
		});

		// Tally votes after vote duration
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Invalid
			));
		});

		// Report invalid dispute executed after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_executed(Origin::Governance.into(), dispute_id));

			// validate updated balance of dispute initiator
			assert_eq!(Balances::free_balance(reporter), balance_before_begin_dispute);
		});
	});
}

#[test]
fn slash_dispute_initiator() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;
	let mut ext = new_test_ext();

	ext.execute_with(|| {
		Balances::make_free_balance_be(&another_reporter, token(1_000));
		let balance_before_begin_dispute = Balances::free_balance(&another_reporter);
		let dispute_id = with_block(|| {
			let dispute_id = dispute_id(PARA_ID, query_id, now());
			assert_noop!(
				Tellor::report_vote_executed(RuntimeOrigin::signed(reporter), dispute_id),
				BadOrigin
			);

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));

			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data,
			));

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				another_reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));

			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				now(),
				None
			));

			match System::events().last().unwrap().event {
				RuntimeEvent::Tellor(Event::<Test>::NewDispute { dispute_id, .. }) => dispute_id,
				_ => panic!(),
			}
		});

		// Tally votes after vote duration
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Failed
			));
		});

		// Report failed dispute executed after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_executed(Origin::Governance.into(), dispute_id));

			// validate slashed balance of dispute initiator
			let vote_info = Tellor::get_vote_info(dispute_id, 1).unwrap();
			assert_eq!(
				Balances::free_balance(another_reporter),
				balance_before_begin_dispute - vote_info.fee
			);
		});
	});
}

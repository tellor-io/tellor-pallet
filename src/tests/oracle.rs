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
	constants::REPORTING_LOCK,
	types::{Nonce, QueryId, Timestamp},
	Config, VoteResult,
};
use frame_support::{
	assert_err, assert_noop, assert_ok,
	dispatch::DispatchResult,
	traits::{Currency, Hooks},
};
use sp_core::{bounded_vec, Get, U256};
use sp_runtime::{
	traits::{BadOrigin, Convert},
	Saturating,
};
use sp_std::num::NonZeroU32;
use std::time::Instant;

type InitialDisputeFee = <Test as Config>::InitialDisputeFee;
type LastReportedTimestamp = crate::LastReportedTimestamp<Test>;
type MaxDisputedTimeSeries = <Test as Config>::MaxDisputedTimeSeries;
type Reports = crate::Reports<Test>;
type ReportedTimestampCount = crate::ReportedTimestampCount<Test>;
type ReportedTimestampsByIndex = crate::ReportedTimestampsByIndex<Test>;
type StakeAmountCurrencyTarget = <Test as Config>::StakeAmountCurrencyTarget;
type StakerReportsSubmittedByQueryId = crate::StakerReportsSubmittedByQueryId<Test>;

const PRICE_TRB: u128 = 50 * 10u128.pow(18); // £50
const PRICE_TRB_LOCAL: u128 = 6 * 10u128.pow(18); // TRB 1:6 OCP

#[test]
fn deposit_stake() {
	let reporter = 1;
	let address = Address::random();
	let amount = trb(100);
	let another_reporter = 2;
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L86
	ext.execute_with(|| {
		with_block(|| {
			assert_noop!(
				Tellor::report_stake_deposited(
					RuntimeOrigin::signed(another_reporter),
					reporter,
					amount,
					address
				),
				BadOrigin
			);
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				amount,
				address
			));
			System::assert_last_event(
				Event::NewStakerReported { staker: reporter, amount, address }.into(),
			);

			assert_eq!(Tellor::get_total_stakers(), 1);
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.address, address);
			assert_eq!(staker_details.start_date, now());
			assert_eq!(staker_details.staked_balance, amount);
			assert_eq!(staker_details.locked_balance, trb(0));
			assert_eq!(staker_details.reward_debt, 0);
			assert_eq!(staker_details.reporter_last_timestamp, 0);
			assert_eq!(staker_details.reports_submitted, 0);
			assert_eq!(staker_details.start_vote_count, 0);
			assert_eq!(staker_details.start_vote_tally, 0);
			assert_eq!(staker_details.staked, true);
			assert_eq!(StakerReportsSubmittedByQueryId::iter_key_prefix(reporter).count(), 0);
			assert_eq!(Tellor::total_reward_debt(), 0);
			assert_eq!(Tellor::get_total_stake_amount(), amount);

			// Test min value for amount argument
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				another_reporter,
				trb(0),
				Address::random()
			));
			assert_eq!(Tellor::get_total_stakers(), 1);

			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(5),
				address
			));
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(10),
				address
			));
			assert_eq!(Tellor::get_total_stakers(), 1); // Ensure only unique addresses add to total stakers
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(105));
			assert_eq!(staker_details.locked_balance, trb(0));
			assert_eq!(Tellor::get_total_stake_amount(), trb(105));
		})
	});
}

#[test]
fn remove_value() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;
	let address = Address::random();
	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			super::deposit_stake(another_reporter, MINIMUM_STAKE_AMOUNT, Address::random());
		})
	});

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L127
	ext.execute_with(|| {
		with_block(|| {
			let timestamp = now();

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				address
			));
			assert_eq!(LastReportedTimestamp::get(query_id), None);
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			assert_eq!(LastReportedTimestamp::get(query_id), Some(timestamp));

			assert_eq!(Tellor::get_new_value_count_by_query_id(query_id), 1);
			assert_noop!(Tellor::remove_value(query_id, 500), Error::InvalidTimestamp);
			assert_eq!(Tellor::retrieve_data(query_id, timestamp).unwrap(), uint_value(100));
			assert!(!Tellor::is_in_dispute(query_id, timestamp));

			Balances::make_free_balance_be(&another_reporter, token(1_000));
			// Value can only be removed via dispute
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				timestamp,
				None
			));
			assert_eq!(Tellor::get_new_value_count_by_query_id(query_id), 1);
			assert_eq!(Tellor::retrieve_data(query_id, timestamp), None);
			assert!(Reports::get(query_id, timestamp).unwrap().is_disputed);
			assert_eq!(LastReportedTimestamp::get(query_id), None);
			assert!(Tellor::is_in_dispute(query_id, timestamp));
			assert_noop!(Tellor::remove_value(query_id, timestamp), Error::ValueDisputed);

			// Test min/max values for timestamp argument
			assert_noop!(Tellor::remove_value(query_id, 0), Error::InvalidTimestamp);
			assert_noop!(Tellor::remove_value(query_id, u64::MAX), Error::InvalidTimestamp);
		});

		with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data,
			));

			// Remove index to ensure verified upon value removal
			let timestamp = now();
			ReportedTimestampsByIndex::remove(
				query_id,
				Tellor::get_timestamp_index_by_timestamp(query_id, timestamp).unwrap(),
			);
			assert_noop!(Tellor::remove_value(query_id, timestamp), Error::InvalidTimestamp);
		});
	});
}

#[test]
fn remove_values() {
	assert_ok!(remove_from_time_series(0, vec![]));
	assert_ok!(remove_from_time_series(2, vec![0]));
	assert_ok!(remove_from_time_series(3, vec![0, 2]));
	assert_ok!(remove_from_time_series(3, vec![1]));
	assert_ok!(remove_from_time_series(5, vec![1, 0, 3]));
	assert_ok!(remove_from_time_series(5, vec![1, 3]));
	assert_ok!(remove_from_time_series(5, vec![1, 2, 3]));
	assert_ok!(remove_from_time_series(5, 0..5));
	assert_ok!(remove_from_time_series(5, (0..5).rev()));
	assert_ok!(remove_from_time_series(10, (0..10).step_by(3)));

	use rand::seq::{IteratorRandom, SliceRandom};
	let mut rng = rand::thread_rng();
	assert_ok!(remove_from_time_series(20, (0..20).rev().choose_multiple(&mut rng, 10)));
	assert_ok!(remove_from_time_series(50, (0..50).choose_multiple(&mut rng, 25)));

	let mut disputes = (0..100).choose_multiple(&mut rng, 50);
	disputes.shuffle(&mut rng);
	assert_ok!(remove_from_time_series(100, disputes));

	let max: u32 = MaxDisputedTimeSeries::get();
	assert_err!(
		remove_from_time_series(max + 50, (25..(max as usize + 25 + 1)).rev()),
		Error::MaxDisputedTimeSeriesReached
	);
}

fn remove_from_time_series(size: u32, disputes: impl IntoIterator<Item = usize>) -> DispatchResult {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;

	new_test_ext().execute_with(|| -> DispatchResult {
		with_block(|| super::deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random()));

		assert_eq!(LastReportedTimestamp::get(query_id), None);

		fn print(timestamps: &Vec<Option<Timestamp>>) {
			println!(
				"{:?}",
				timestamps
					.iter()
					.map(|t| match t {
						Some(t) => t.to_string(),
						None => format!("{:_^10}", ""),
					})
					.collect::<Vec<_>>()
			);
		}

		// Add series of timestamps of specified size
		let mut timestamps = Vec::new();
		for i in 0..size {
			with_block_after(REPORTING_LOCK, || {
				assert_ok!(Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					query_id,
					uint_value(100),
					i,
					query_data.clone(),
				));
				timestamps.push(Some(now()));
				assert_eq!(LastReportedTimestamp::get(query_id), *timestamps.last().unwrap());
			});
		}
		print(&timestamps);

		// Remove disputed items, in specified order
		for i in disputes {
			let timestamp = timestamps[i].unwrap();
			Tellor::remove_value(query_id, timestamp)?;
			let report = Reports::get(query_id, timestamp).unwrap();
			assert!(report.is_disputed);
			timestamps[i] = None;
		}
		print(&timestamps);

		// Form new time series of undisputed values and verify in order
		let timestamps: Vec<_> = timestamps.into_iter().filter_map(|t| t).collect();
		for (i, timestamp) in timestamps.iter().enumerate() {
			let report = Reports::get(query_id, timestamp).unwrap();
			assert!(!report.is_disputed);
			match i {
				0 => assert!(report.previous == None),
				_ => assert_eq!(report.previous, Some(timestamps[i - 1])),
			}
		}
		println!("{:?}", timestamps.iter().map(|t| t.to_string()).collect::<Vec<_>>());

		// Follow linked timestamps using Report.previous, starting from last reported
		if timestamps.len() > 0 {
			let mut i = timestamps.len() - 1;
			let mut current = LastReportedTimestamp::get(query_id);
			while let Some(timestamp) = current {
				assert_eq!(timestamp, timestamps[i]);
				let report = Reports::get(query_id, timestamp).unwrap();
				assert!(!report.is_disputed);
				current = report.previous;
				i.saturating_dec();
			}
		}

		// Verify every item's previous value remains valid
		print!("[");
		for i in 0..size {
			let timestamp = ReportedTimestampsByIndex::get(query_id, i).unwrap();
			let report = Reports::get(query_id, timestamp).unwrap();
			if let Some(previous) = report.previous {
				assert_eq!(Reports::get(query_id, previous).unwrap().is_disputed, false)
			} else if i != 0 {
				let timestamp = ReportedTimestampsByIndex::get(query_id, i - 1).unwrap();
				assert_eq!(Reports::get(query_id, timestamp).unwrap().is_disputed, true);
			}
			print!(
				"\"{timestamp}{} -> {:?}\", ",
				if report.is_disputed { "(❌)" } else { "" },
				report.previous
			)
		}
		println!("]");

		assert_eq!(LastReportedTimestamp::get(query_id), timestamps.last().copied());
		println!();
		Ok(())
	})
}

#[test]
fn request_stake_withdraw() {
	let reporter = 1;
	let amount = trb(1_000);
	let address = Address::random();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L151
	ext.execute_with(|| {
		with_block(|| {
			assert_noop!(
				Tellor::report_staking_withdraw_request(
					RuntimeOrigin::signed(reporter),
					reporter,
					trb(10),
					address
				),
				BadOrigin
			);
			assert_noop!(
				Tellor::report_staking_withdraw_request(
					Origin::Staking.into(),
					reporter,
					trb(5),
					address
				),
				Error::InsufficientStake
			);
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				amount,
				address
			));

			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.start_date, now());
			assert_eq!(staker_details.staked_balance, amount);
			assert_eq!(staker_details.locked_balance, trb(0));
			assert_eq!(staker_details.staked, true);
			assert_eq!(Tellor::get_total_stake_amount(), amount);
			assert_eq!(Tellor::total_reward_debt(), 0);
			assert_noop!(
				Tellor::report_staking_withdraw_request(
					Origin::Staking.into(),
					reporter,
					(amount + 1).into(),
					address
				),
				Error::InsufficientStake
			);

			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(10),
				address
			));
			System::assert_has_event(
				Event::StakeWithdrawRequestReported { reporter, amount: trb(10), address }.into(),
			);
			System::assert_last_event(
				Event::StakeWithdrawRequestConfirmationSent {
					para_id: EVM_PARA_ID,
					contract_address: (*STAKING).into(),
				}
				.into(),
			);
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.start_date, now());
			assert_eq!(staker_details.reward_debt, 0);
			assert_eq!(staker_details.staked_balance, trb(990));
			assert_eq!(staker_details.locked_balance, trb(10));
			assert_eq!(staker_details.staked, true);
			assert_eq!(Tellor::get_total_stake_amount(), trb(990));
			assert_eq!(Tellor::total_reward_debt(), 0);

			// Test max/min for amount arg
			assert_noop!(
				Tellor::report_staking_withdraw_request(
					Origin::Staking.into(),
					reporter,
					U256::max_value(),
					address
				),
				Error::InsufficientStake
			);
			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				U256::zero(),
				address
			));
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.start_date, now());
			assert_eq!(staker_details.reward_debt, 0);
			assert_eq!(staker_details.staked_balance, trb(990));
			assert_eq!(staker_details.locked_balance, trb(10));
			assert_eq!(staker_details.staked, true);
			assert_eq!(Tellor::get_total_stake_amount(), trb(990));
			assert_eq!(Tellor::total_reward_debt(), 0);

			assert_eq!(Tellor::get_total_stakers(), 1);
			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(990),
				address
			));
			assert_eq!(Tellor::get_total_stakers(), 0);
		});
	});
}

#[test]
fn slash_reporter() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let amount = trb(1_000);
	let address = Address::random();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L195
	ext.execute_with(|| {
		let dispute_id = with_block(|| {
			Balances::make_free_balance_be(&reporter, token(1_000));
			assert_noop!(
				Tellor::report_slash(
					RuntimeOrigin::signed(reporter),
					0,
					MINIMUM_STAKE_AMOUNT.into()
				),
				BadOrigin
			);

			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				amount,
				address
			));

			submit_value_and_begin_dispute(reporter, query_id, query_data.clone()) // start dispute, required for slashing
		});

		// Tally votes after vote duration
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
		});

		// Report slash after tally dispute period
		let dispute_id = with_block_after(86_400, || {
			// Slash when locked balance = 0
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, amount);
			assert_eq!(staker_details.locked_balance, trb(0));
			assert_eq!(Tellor::get_total_stake_amount(), amount);
			assert_noop!(
				Tellor::report_slash(
					Origin::Governance.into(),
					0,
					(MINIMUM_STAKE_AMOUNT + 1).into()
				),
				Error::InsufficientStake
			);
			assert_ok!(Tellor::report_slash(
				Origin::Governance.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into()
			));

			assert_eq!(Tellor::time_of_last_allocation(), now());
			assert_eq!(Tellor::accumulated_reward_per_share(), 0);
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(900));
			assert_eq!(staker_details.locked_balance, trb(0));
			assert!(staker_details.staked);
			assert_eq!(Tellor::get_total_stakers(), 1); // Still one staker as reporter has 900 staked & stake amount is 100
			assert_eq!(Tellor::get_total_stake_amount(), trb(900));

			submit_value_and_begin_dispute(reporter, query_id, query_data.clone()) // start dispute, required for slashing
		});

		// Tally votes after vote duration
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
		});

		// Report slash after tally dispute period
		let dispute_id = with_block_after(86_400, || {
			// Slash when lockedBalance >= stakeAmount
			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(100),
				address
			));
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(800));
			assert_eq!(staker_details.locked_balance, trb(100));
			assert!(staker_details.staked);
			assert_ok!(Tellor::report_slash(
				Origin::Governance.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into()
			));
			assert_eq!(Tellor::time_of_last_allocation(), now());
			assert_eq!(Tellor::accumulated_reward_per_share(), 0);
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(800));
			assert_eq!(staker_details.locked_balance, trb(0));
			assert!(staker_details.staked);
			assert_eq!(Tellor::get_total_stake_amount(), trb(800));

			submit_value_and_begin_dispute(reporter, query_id, query_data.clone()) // start dispute, required for slashing
		});

		// Tally votes after vote duration
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
		});

		// Report slash after tally dispute period
		with_block_after(86_400, || {
			// Slash when 0 < locked balance < stake amount
			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(5),
				address
			));
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(795));
			assert_eq!(staker_details.locked_balance, trb(5));
			assert_eq!(Tellor::get_total_stake_amount(), trb(795));
			assert_ok!(Tellor::report_slash(
				Origin::Governance.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into()
			));
			assert_eq!(Tellor::time_of_last_allocation(), now());
			assert_eq!(Tellor::accumulated_reward_per_share(), 0);
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(700));
			assert_eq!(staker_details.locked_balance, trb(0));
			assert_eq!(Tellor::get_total_stake_amount(), trb(700));

			// Slash when locked balance + staked balance < stake amount
			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(625),
				address
			));
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(75));
			assert_eq!(staker_details.locked_balance, trb(625));
			assert_eq!(Tellor::get_total_stake_amount(), trb(75));
		});

		let dispute_id = with_block_after(604_800, || {
			assert_ok!(Tellor::report_stake_withdrawn(Origin::Staking.into(), reporter, trb(625),));
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(75));
			assert_eq!(staker_details.locked_balance, trb(0));

			// reporter now has insufficient stake for another submission, so top up stake before final dispute/slash
			super::deposit_stake(reporter, Tributes::from(MINIMUM_STAKE_AMOUNT) - trb(75), address);
			submit_value_and_begin_dispute(reporter, query_id, query_data) // start dispute, required for slashing
		});

		// Tally votes after vote duration
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_vote_tallied(
				Origin::Governance.into(),
				dispute_id,
				VoteResult::Passed
			));
		});

		// Report slash after tally dispute period
		with_block_after(86_400, || {
			assert_ok!(Tellor::report_slash(
				Origin::Governance.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into()
			));
			assert_eq!(Tellor::time_of_last_allocation(), now());
			assert_eq!(Tellor::accumulated_reward_per_share(), 0);
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(0));
			assert_eq!(staker_details.locked_balance, trb(0));
			assert_eq!(Tellor::get_total_stakers(), 0);
			assert_eq!(Tellor::get_total_stake_amount(), trb(0));
		})
	});
}

#[test]
fn submit_value() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;
	let address = Address::random();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L277
	ext.execute_with(|| {
		let timestamp = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(1_200),
				address
			));
			assert_noop!(
				Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					query_id,
					bounded_vec![],
					0,
					query_data.clone()
				),
				Error::InvalidValue
			);
			assert_noop!(
				Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					query_id,
					uint_value(4_000),
					1,
					query_data.clone()
				),
				Error::InvalidNonce
			);
			assert_noop!(
				Tellor::submit_value(
					RuntimeOrigin::signed(another_reporter),
					query_id,
					uint_value(4_000),
					0,
					query_data.clone()
				),
				Error::InsufficientStake
			);
			assert_noop!(
				Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					H256::random(),
					uint_value(4_000),
					0,
					query_data.clone()
				),
				Error::InvalidQueryId
			);
			assert_eq!(LastReportedTimestamp::get(query_id), None);
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4_000),
				0,
				query_data.clone()
			));
			let timestamp = now();
			assert_eq!(LastReportedTimestamp::get(query_id).unwrap(), timestamp);
			assert_noop!(
				Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					query_id,
					uint_value(4_000),
					1,
					query_data.clone()
				),
				Error::ReporterTimeLocked
			);
			timestamp
		});

		with_block_after(3_600 /* 1 hour */, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4_001),
				1,
				query_data.clone()
			));
			let previous = timestamp;
			let timestamp = now();
			assert_eq!(Tellor::get_timestamp_index_by_timestamp(query_id, timestamp).unwrap(), 1);
			assert_eq!(
				Tellor::get_timestamp_by_query_id_and_index(query_id, 1).unwrap(),
				timestamp
			);
			assert_eq!(
				Tellor::get_block_number_by_timestamp(query_id, timestamp).unwrap(),
				System::block_number()
			);
			assert_eq!(Tellor::retrieve_data(query_id, timestamp).unwrap(), uint_value(4_001));
			assert_eq!(Tellor::get_reporter_by_timestamp(query_id, timestamp).unwrap(), reporter);
			assert_eq!(Tellor::time_of_last_new_value().unwrap(), timestamp);
			assert_eq!(Reports::get(query_id, timestamp).unwrap().previous, Some(previous));
			assert_eq!(Tellor::get_reports_submitted_by_address(&reporter), 2);
			assert_eq!(
				Tellor::get_reports_submitted_by_address_and_query_id(reporter, query_id),
				2
			);
		});

		// Test submit multiple identical values w/ min nonce
		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				another_reporter,
				trb(120),
				address
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(4_001),
				0,
				query_data.clone()
			));
		});
		with_block_after(REPORTING_LOCK, || {
			let timestamp = now();
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4_001),
				0,
				query_data.clone()
			));

			assert_eq!(Tellor::get_timestamp_index_by_timestamp(query_id, timestamp).unwrap(), 3);
			assert_eq!(
				Tellor::get_timestamp_by_query_id_and_index(query_id, 3).unwrap(),
				timestamp
			);
			assert_eq!(
				Tellor::get_block_number_by_timestamp(query_id, timestamp).unwrap(),
				System::block_number()
			);
			assert_eq!(Tellor::retrieve_data(query_id, timestamp).unwrap(), uint_value(4001));
			assert_eq!(Tellor::get_reporter_by_timestamp(query_id, timestamp).unwrap(), reporter);
			assert_eq!(Tellor::time_of_last_new_value().unwrap(), timestamp);
			assert_eq!(Tellor::get_reports_submitted_by_address(&reporter), 3);
			assert_eq!(
				Tellor::get_reports_submitted_by_address_and_query_id(reporter, query_id),
				3
			);

			// Test max val for nonce
			assert_noop!(
				Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					query_id,
					uint_value(4_001),
					Nonce::MAX,
					query_data
				),
				Error::InvalidNonce
			);
		})
	});
}

#[test]
fn withdraw_stake() {
	let reporter = 1;
	let address = Address::random();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L323
	ext.execute_with(|| {
		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				address
			));
			assert_eq!(Tellor::get_total_stakers(), 1);
			assert_noop!(
				Tellor::report_stake_withdrawn(
					Origin::Staking.into(),
					reporter,
					MINIMUM_STAKE_AMOUNT.into(),
				),
				Error::NoWithdrawalRequested
			);
			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(10),
				address
			));
			assert_noop!(
				Tellor::report_stake_withdrawn(
					Origin::Staking.into(),
					reporter,
					MINIMUM_STAKE_AMOUNT.into(),
				),
				Error::WithdrawalPeriodPending
			);
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(90));
			assert_eq!(staker_details.locked_balance, trb(10));
		});

		with_block_after(60 * 60 * 24 * 7, || {
			assert_ok!(Tellor::report_stake_withdrawn(Origin::Staking.into(), reporter, trb(10),));
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.staked_balance, trb(90));
			assert_eq!(staker_details.locked_balance, trb(0));
			assert_noop!(
				Tellor::report_stake_withdrawn(Origin::Staking.into(), reporter, trb(10)),
				Error::NoWithdrawalRequested
			);
		});
	});
}

#[test]
fn get_block_number_by_timestamp() {
	let reporter = 1;
	let address = Address::random();
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L345
	ext.execute_with(|| {
		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				address
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(
				Tellor::get_block_number_by_timestamp(query_id, now()).unwrap(),
				System::block_number()
			)
		});
	});
}

#[test]
fn get_current_value() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L352
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_current_value(query_id).unwrap(), uint_value(4000))
		})
	});
}

#[test]
fn get_new_value_count_by_query_id() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L363
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
		});

		with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_new_value_count_by_query_id(query_id), 2)
		});
	});
}

#[test]
fn get_report_details() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L372
	ext.execute_with(|| {
		let timestamp_1 = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4000),
				0,
				query_data.clone(),
			));
			now()
		});

		let timestamp_2 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4001),
				0,
				query_data.clone(),
			));
			now()
		});

		let timestamp_3 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4002),
				0,
				query_data.clone(),
			));
			assert_ok!(Tellor::remove_value(query_id, now()));
			now()
		});

		assert_eq!(Tellor::get_report_details(query_id, timestamp_1).unwrap(), (reporter, false));
		assert_eq!(Tellor::get_report_details(query_id, timestamp_2).unwrap(), (reporter, false));
		assert_eq!(Tellor::get_report_details(query_id, timestamp_3).unwrap(), (reporter, true));
		assert_eq!(Tellor::get_report_details(H256::zero(), timestamp_1), None);
	});
}

#[test]
fn get_reporting_lock() {
	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L398
	let reporting_lock: Timestamp = REPORTING_LOCK;
	assert_eq!(Tellor::get_reporting_lock(), reporting_lock)
}

#[test]
fn get_reporter_by_timestamp() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L402
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_reporter_by_timestamp(query_id, now()).unwrap(), reporter)
		});
	});
}

#[test]
fn get_reporter_last_timestamp() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L409
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_reporter_last_timestamp(reporter).unwrap(), now())
		});
	});
}

#[test]
fn get_reports_submitted_by_address() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L419
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
		});

		with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_reports_submitted_by_address(&reporter), 2)
		})
	});
}

#[test]
fn get_reports_submitted_by_address_and_query_id() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L429
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
		});

		with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_reports_submitted_by_address_and_query_id(reporter, query_id), 2)
		})
	});
}

#[test]
fn get_stake_amount() {
	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L439
	new_test_ext().execute_with(|| {
		with_block(|| assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into()))
	});
}

#[test]
fn get_staker_info() {
	let reporter = 1;
	let address = Address::random();
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L443
	ext.execute_with(|| {
		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(1_000),
				address
			));
			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(100),
				address
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4000),
				0,
				query_data.clone(),
			));
			let staker_details = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_details.address, address);
			assert_eq!(staker_details.start_date, now());
			assert_eq!(staker_details.staked_balance, trb(900));
			assert_eq!(staker_details.locked_balance, trb(100));
			assert_eq!(staker_details.reward_debt, 0);
			assert_eq!(staker_details.reporter_last_timestamp, now());
			assert_eq!(staker_details.reports_submitted, 1);
			assert_eq!(staker_details.start_vote_count, 0);
			assert_eq!(staker_details.start_vote_tally, 0);
			assert_eq!(staker_details.staked, true);
			assert_eq!(StakerReportsSubmittedByQueryId::get(reporter, query_id), 1);
		});
	});
}

#[test]
fn get_time_of_last_new_value() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L461
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_time_of_last_new_value().unwrap(), now())
		});
	});
}

#[test]
fn get_timestamp_by_query_and_index() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L471
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
		});

		with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_timestamp_by_query_id_and_index(query_id, 1).unwrap(), now())
		})
	});
}

#[test]
fn get_timestamp_index_by_timestamp() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L481
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
		});

		with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4000),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_timestamp_index_by_timestamp(query_id, now()).unwrap(), 1)
		})
	});
}

#[test]
fn get_total_stake_amount() {
	let reporter = 1;
	let address = Address::random();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L491
	ext.execute_with(|| {
		with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				address
			));
			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(10),
				address
			));
			assert_eq!(Tellor::get_total_stake_amount(), trb(90))
		});
	});
}

#[test]
fn get_total_stakers() {
	let reporter = 1;
	let address = Address::random();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L502
	ext.execute_with(|| {
		with_block(|| {
			// Only count unique stakers
			assert_eq!(Tellor::get_total_stakers(), 0);
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				address
			));
			assert_eq!(Tellor::get_total_stakers(), 1);
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				address
			));
			assert_eq!(Tellor::get_total_stakers(), 1);

			// Unstake, restake
			assert_ok!(Tellor::report_staking_withdraw_request(
				Origin::Staking.into(),
				reporter,
				trb(200),
				address
			));
			assert_eq!(Tellor::get_total_stakers(), 0);
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				MINIMUM_STAKE_AMOUNT.into(),
				address
			));
			assert_eq!(Tellor::get_total_stakers(), 1);
		});
	});
}

#[test]
fn is_in_dispute() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

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

			let timestamp = now();
			assert!(!Tellor::is_in_dispute(query_id, timestamp));
			Balances::make_free_balance_be(&reporter, token(1_000));
			// Value can only be removed via dispute
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				timestamp,
				None
			));
			assert!(Tellor::is_in_dispute(query_id, timestamp));
		});
	});
}

#[test]
fn retrieve_data() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L519
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
				uint_value(4000),
				0,
				query_data.clone(),
			));
		});

		with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(4001),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::retrieve_data(query_id, now()).unwrap(), uint_value(4001));

			// Test max/min values for _timestamp arg
			assert_eq!(Tellor::retrieve_data(query_id, 0), None);
			assert_eq!(Tellor::retrieve_data(query_id, Timestamp::MAX), None);
		})
	});
}

#[test]
#[ignore]
fn get_total_time_based_rewards_balance() {
	// https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L533
	unimplemented!("currently in backlog")
}

const REWARD_RATE_TARGET: Balance = 60 * 60 * 24 * 30; // 30 days

#[test]
fn add_staking_rewards() {
	let funder = 1;
	let staking_rewards = &Tellor::staking_rewards();

	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L539
	ext.execute_with(|| {
		Balances::make_free_balance_be(&funder, token(1_000));
		assert_eq!(Balances::free_balance(funder), token(1_000));

		assert_ok!(Tellor::add_staking_rewards(RuntimeOrigin::signed(funder), token(1_000)));
		assert_eq!(Balances::free_balance(staking_rewards), token(1_000));
		assert_eq!(Balances::free_balance(funder), 0);
		assert_eq!(Tellor::reward_rate(), token(1_000) / REWARD_RATE_TARGET);

		// Test min value
		assert_ok!(Tellor::add_staking_rewards(RuntimeOrigin::signed(funder), 0));
		assert_eq!(Balances::free_balance(staking_rewards), token(1_000));
		assert_eq!(Balances::free_balance(funder), 0);
		assert_eq!(Tellor::reward_rate(), token(1_000) / REWARD_RATE_TARGET);

		// Test max value
		Balances::make_free_balance_be(&funder, Balance::MAX);
		Balances::make_free_balance_be(&staking_rewards, 0);
		assert_ok!(Tellor::add_staking_rewards(RuntimeOrigin::signed(funder), Balance::MAX));
		assert_eq!(Balances::free_balance(staking_rewards), Balance::MAX);
		assert_eq!(Balances::free_balance(funder), 0);
		assert_eq!(Tellor::reward_rate(), Balance::MAX / REWARD_RATE_TARGET);
	});
}

#[test]
fn get_index_for_data_before() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L519
	ext.execute_with(|| {
		let timestamp_0 = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(1_000),
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
		let timestamp_1 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				1,
				query_data.clone(),
			));
			now()
		});
		let timestamp_2 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				2,
				query_data.clone(),
			));
			now()
		});

		assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_2), Some(1));
		assert_eq!(
			Tellor::get_index_for_data_before_with_start(query_id, timestamp_2, 0).0,
			Some(1)
		);

		// advance time and test
		for year in 1..2 {
			with_block_after(year * 365 * 86_400, || {
				assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_2), Some(1));
				assert_eq!(
					Tellor::get_index_for_data_before_with_start(query_id, timestamp_2, 0).0,
					Some(1)
				);
			});
		}

		for i in 0..50 {
			with_block_after(REPORTING_LOCK, || {
				assert_ok!(Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					query_id,
					uint_value(100 + i),
					0,
					query_data.clone(),
				));
			});
		}
		let timestamp_52 = now();

		// test last value disputed
		with_block(|| {
			assert_ok!(Tellor::remove_value(query_id, timestamp_52));
			assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_52), Some(51));
			assert_eq!(
				Tellor::get_index_for_data_before_with_start(query_id, timestamp_52, 0).0,
				Some(51)
			);
			assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_2), Some(1));
			assert_eq!(
				Tellor::get_index_for_data_before_with_start(query_id, timestamp_2, 0).0,
				Some(1)
			);
			assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_2 + 1), Some(2));
			assert_eq!(
				Tellor::get_index_for_data_before_with_start(query_id, timestamp_2 + 1, 0).0,
				Some(2)
			);

			// remove value at index 2
			assert_ok!(Tellor::remove_value(query_id, timestamp_2));
			assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_2), Some(1));
			assert_eq!(
				Tellor::get_index_for_data_before_with_start(query_id, timestamp_2, 0).0,
				Some(1)
			);
			assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_2 + 1), Some(1));
			assert_eq!(
				Tellor::get_index_for_data_before_with_start(query_id, timestamp_2 + 1, 0).0,
				Some(1)
			);
			assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_1 + 1), Some(1));
			assert_eq!(
				Tellor::get_index_for_data_before_with_start(query_id, timestamp_1 + 1, 0).0,
				Some(1)
			);

			assert_ok!(Tellor::remove_value(query_id, timestamp_1));
			assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_2 - 1), Some(0));
			assert_eq!(
				Tellor::get_index_for_data_before_with_start(query_id, timestamp_2 - 1, 0).0,
				Some(0)
			);

			assert_ok!(Tellor::remove_value(query_id, timestamp_0));
			assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_2 - 1), None);
			assert_eq!(
				Tellor::get_index_for_data_before_with_start(query_id, timestamp_2 - 1, 0).0,
				None
			);
		});

		let query_data: QueryDataOf<Test> = spot_price("ksm", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();

		let timestamp_0 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			now()
		});
		let timestamp_1 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));

			assert_ok!(Tellor::remove_value(query_id, timestamp_0));
			assert_ok!(Tellor::remove_value(query_id, now()));
			now()
		});

		assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_1 + 1), None);
		assert_eq!(
			Tellor::get_index_for_data_before_with_start(query_id, timestamp_1 + 1, 0).0,
			None
		);
		assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_0 + 1), None);
		assert_eq!(
			Tellor::get_index_for_data_before_with_start(query_id, timestamp_0 + 1, 0).0,
			None
		);

		let timestamp_2 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));
			now()
		});

		with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(100),
				0,
				query_data.clone(),
			));

			assert_ok!(Tellor::remove_value(query_id, timestamp_2));
			assert_eq!(Tellor::get_index_for_data_before(query_id, timestamp_2 + 1), None);
			assert_eq!(
				Tellor::get_index_for_data_before_with_start(query_id, timestamp_2 + 1, 0).0,
				None
			);
		});
	});
}

#[test]
#[ignore]
// cargo test --release get_index_for_data_before_with_start -- --ignored --nocapture
fn get_index_for_data_before_with_start() {
	let reports = 2u32.saturating_pow(22);
	let query_id = QueryId::zero();
	let block_number = 0u8.into();
	let mut timestamp = 1_685_196_686;
	new_test_ext().execute_with(|| {
		println!("Creating {:?} reports, disputing all but first and last...", reports);
		let now = Instant::now();
		for i in 1..=reports {
			timestamp.saturating_inc();
			let index = i - 1;
			Reports::insert(
				query_id,
				timestamp,
				crate::types::ReportOf::<Test> {
					index,
					block_number,
					reporter: 0,
					is_disputed: false,
					previous: LastReportedTimestamp::get(query_id),
				},
			);
			ReportedTimestampsByIndex::insert(query_id, index, timestamp);
			LastReportedTimestamp::insert(query_id, timestamp);

			if index > 0 && i < reports {
				assert_ok!(Tellor::remove_value(query_id, timestamp));
			}
		}
		ReportedTimestampCount::insert(query_id, reports);
		println!("Reports created in {:?}\n", now.elapsed());

		let timestamp = LastReportedTimestamp::get(query_id).unwrap();
		let expected = Some(0);

		println!("Using `get_index_for_data_before`, based on last timestamp");
		let now = Instant::now();
		let index_before = Tellor::get_index_for_data_before(query_id, timestamp);
		println!(
			"get_index_for_data_before: result={:?} elapsed={:?}\n",
			index_before,
			now.elapsed()
		);
		assert_eq!(index_before, expected);

		println!("Using `get_index_for_data_before_with_start`, based on last timestamp");
		let now = Instant::now();
		let (index_before, iterations) =
			Tellor::get_index_for_data_before_with_start(query_id, timestamp, 0);
		let expected_iterations = NonZeroU32::new(reports).unwrap().ilog2() - 1;
		println!(
			"get_index_for_data_before_with_start: result={:?} elapsed: {:?} expected_iterations={} actual_iterations={}\n",
			index_before,
			now.elapsed(),
			expected_iterations,
			iterations
		);
		assert_eq!(index_before, expected);
		assert_eq!(iterations, expected_iterations);
	});
}

#[test]
fn get_data_before() {
	let reporter = 1;
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L697
	ext.execute_with(|| {
		let timestamp_1 = with_block(|| {
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(1_000),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(150),
				0,
				query_data.clone(),
			));
			now()
		});
		let timestamp_2 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(160),
				1,
				query_data.clone(),
			));
			now()
		});
		let timestamp_3 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(170),
				2,
				query_data.clone(),
			));
			now()
		});

		assert_eq!(
			Tellor::get_data_before(query_id, timestamp_3 + 1),
			Some((uint_value(170), timestamp_3))
		);
		assert_eq!(
			Tellor::get_data_before(query_id, timestamp_2),
			Some((uint_value(150), timestamp_1))
		);

		// advance time one year and test
		for year in 1..2 {
			with_block_after(year * 365 * 86_400, || {
				assert_eq!(
					Tellor::get_data_before(query_id, timestamp_3 + 1),
					Some((uint_value(170), timestamp_3))
				);
				assert_eq!(
					Tellor::get_data_before(query_id, timestamp_2),
					Some((uint_value(150), timestamp_1))
				);
			});
		}

		// submit 50 values and test
		for i in 0..50 {
			with_block_after(REPORTING_LOCK, || {
				assert_ok!(Tellor::submit_value(
					RuntimeOrigin::signed(reporter),
					query_id,
					uint_value(100 + i),
					0,
					query_data.clone(),
				));
			});
		}

		assert_eq!(
			Tellor::get_data_before(query_id, timestamp_3 + 1),
			Some((uint_value(170), timestamp_3))
		);
		assert_eq!(
			Tellor::get_data_before(query_id, timestamp_2),
			Some((uint_value(150), timestamp_1))
		);
	});
}

#[test]
fn update_stake_amount() {
	let staking_token_price_query_data: QueryDataOf<Test> =
		spot_price("trb", "gbp").try_into().unwrap();
	let staking_token_price_query_id = keccak_256(staking_token_price_query_data.as_ref()).into();
	let staking_to_local_token_query_data: QueryDataOf<Test> =
		spot_price("trb", "ocp").try_into().unwrap();
	let staking_to_local_token_query_id: QueryId =
		keccak_256(staking_to_local_token_query_data.as_ref()).into();
	let reporter = 1;
	let initial_dispute_fee = token(10.0 * (PRICE_TRB_LOCAL / 10u128.pow(18)) as f64); // 10% of 100 TRB * PRICE
	let mut ext = new_test_ext();

	let stake_amount_currency_target: u128 = StakeAmountCurrencyTarget::get();
	let required_stake: u128 = stake_amount_currency_target / PRICE_TRB;

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L762
	ext.execute_with(|| {
		with_block(|| {
			// Setup
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(10_000),
				Address::random()
			));

			// Test no reported TRB price
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidStakingTokenPrice
			);
			println!("REQUIRED_STAKE: {}", required_stake);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());

			// Test updating when 12 hrs have NOT passed
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 2),
				0,
				staking_token_price_query_data.clone()
			));
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidStakingTokenPrice
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
		});

		// Test updating when 12 hrs have passed
		with_block_after(60 * 60 * 12, || {
			// No trb:token price reported yet, so dispute fee cannot be calculated as part of stake amount update
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidPrice
			);
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(PRICE_TRB_LOCAL),
				0,
				staking_to_local_token_query_data.clone()
			));
			// Requires waiting another 12 hours until reported price clears dispute period
		});

		// Test updating when 12 hrs have passed
		with_block_after(60 * 60 * 12, || {
			assert_ok!(Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)));
			System::assert_has_event(
				Event::NewStakeAmount { amount: MINIMUM_STAKE_AMOUNT.into() }.into(),
			);
			System::assert_last_event(
				Event::NewDisputeFee { dispute_fee: initial_dispute_fee }.into(),
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
		});

		// Test updating when multiple prices have been reported
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value((PRICE_TRB as f64 * 1.5) as u64),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 2),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 3),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 12, || {
			assert_ok!(Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)));
			System::assert_last_event(
				Event::NewStakeAmount { amount: MINIMUM_STAKE_AMOUNT.into() }.into(),
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
		});

		// Test bad TRB price encoding
		let bad_price = b"Where's the beef?";
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				bad_price.to_vec().try_into().unwrap(),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(86_400 / 2, || {
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidStakingTokenPrice
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
		});

		// Test reported TRB price outside limits - high
		let high_price = trb(1_000_001);
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(high_price),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(86_400 / 2, || {
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidStakingTokenPrice
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
		});

		// Test reported TRB price outside limits - low
		let low_price = trb(0.009);
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(low_price),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(86_400 / 2, || {
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidStakingTokenPrice
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
		});

		// Test updating when multiple prices have been reported
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 7),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 8),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 9),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 12, || {
			assert_ok!(Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)));
			System::assert_last_event(
				Event::NewStakeAmount { amount: MINIMUM_STAKE_AMOUNT.into() }.into(),
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
		});

		// Test with price that updates stake amount
		let price = PRICE_TRB / 11;
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(price),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 12, || {
			assert_ok!(Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)));
			let expected_stake_amount = (U256::from(stake_amount_currency_target) *
				U256::from(10u128.pow(18))) /
				U256::from(price);
			System::assert_has_event(
				Event::NewStakeAmount { amount: expected_stake_amount }.into(),
			);
			// Dispute fee changes with stake amount
			let expected_dispute_fee =
				U256ToBalance::convert(Tellor::convert(expected_stake_amount / 10).unwrap()) * 6; // TRB 1:6 OCP
			System::assert_last_event(
				Event::NewDisputeFee { dispute_fee: expected_dispute_fee }.into(),
			);
			assert_eq!(Tellor::get_stake_amount(), expected_stake_amount);
		});
	});
}

#[test]
fn update_dispute_fee() {
	let staking_token_price_query_data: QueryDataOf<Test> =
		spot_price("trb", "gbp").try_into().unwrap();
	let staking_token_price_query_id = keccak_256(staking_token_price_query_data.as_ref()).into();
	let staking_to_local_token_query_data: QueryDataOf<Test> =
		spot_price("trb", "ocp").try_into().unwrap();
	let staking_to_local_token_query_id: QueryId =
		keccak_256(staking_to_local_token_query_data.as_ref()).into();
	let reporter = 1;
	let mut ext = new_test_ext();

	let stake_amount_currency_target: u128 = StakeAmountCurrencyTarget::get();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L762
	ext.execute_with(|| {
		with_block(|| {
			// Setup
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(10_000),
				Address::random()
			));
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 2),
				0,
				staking_token_price_query_data.clone()
			));
		});

		// Wait until staking token price clears dispute period
		with_block_after(60 * 60 * 12, || {
			// Test no reported TRB:OCP price
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidPrice
			);
			assert_eq!(Tellor::get_dispute_fee(), InitialDisputeFee::get());

			// Test updating when 12 hrs have NOT passed
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(PRICE_TRB_LOCAL * 2),
				0,
				staking_to_local_token_query_data.clone()
			));
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidPrice
			);
			assert_eq!(Tellor::get_dispute_fee(), InitialDisputeFee::get());
		});

		// Test updating when 12 hrs have passed
		with_block_after(60 * 60 * 12, || {
			assert_ok!(Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)));
			System::assert_has_event(
				Event::NewStakeAmount { amount: MINIMUM_STAKE_AMOUNT.into() }.into(),
			);
			System::assert_last_event(
				Event::NewDisputeFee {
					dispute_fee: token(10 * 6 * 2), // 10% of 100 * (PRICE * 2)
				}
				.into(),
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
		});

		// Test updating when multiple prices have been reported
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value((PRICE_TRB_LOCAL as f64 * 1.5) as u64),
				0,
				staking_to_local_token_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(PRICE_TRB_LOCAL * 2),
				0,
				staking_to_local_token_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(PRICE_TRB_LOCAL * 3),
				0,
				staking_to_local_token_query_data.clone()
			));
		});
		let dispute_fee = with_block_after(60 * 60 * 12, || {
			assert_ok!(Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)));
			System::assert_has_event(
				Event::NewStakeAmount { amount: MINIMUM_STAKE_AMOUNT.into() }.into(),
			);
			let expected_dispute_fee = token(10 * 6 * 3); // 10% of 100 * (PRICE * 3)
			System::assert_last_event(
				Event::NewDisputeFee { dispute_fee: expected_dispute_fee }.into(),
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), expected_dispute_fee);
			expected_dispute_fee
		});

		// Test bad price encoding
		let bad_price = b"Where's the beef?";
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				bad_price.to_vec().try_into().unwrap(),
				0,
				staking_to_local_token_query_data.clone()
			));
		});
		with_block_after(86_400 / 2, || {
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidPrice
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});

		// Test reported price outside limits - high
		let high_price = trb(1_000_001);
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(high_price),
				0,
				staking_to_local_token_query_data.clone()
			));
		});
		with_block_after(86_400 / 2, || {
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidPrice
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});

		// Test reported price outside limits - low
		let low_price = trb(0.009);
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(low_price),
				0,
				staking_to_local_token_query_data.clone()
			));
		});
		with_block_after(86_400 / 2, || {
			assert_noop!(
				Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)),
				Error::InvalidPrice
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});

		// Test updating when multiple prices have been reported
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(PRICE_TRB_LOCAL * 7),
				0,
				staking_to_local_token_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(PRICE_TRB_LOCAL * 8),
				0,
				staking_to_local_token_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(PRICE_TRB_LOCAL * 9),
				0,
				staking_to_local_token_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 12, || {
			assert_ok!(Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)));
			System::assert_has_event(
				Event::NewStakeAmount { amount: MINIMUM_STAKE_AMOUNT.into() }.into(),
			);
			let expected_dispute_fee = token(10 * 6 * 9); // 10% of 100 * (PRICE * 9)
			System::assert_last_event(
				Event::NewDisputeFee { dispute_fee: expected_dispute_fee }.into(),
			);
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), expected_dispute_fee)
		});

		// Test with price that updates stake amount
		let price = PRICE_TRB / 11;
		with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(price),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 12, || {
			assert_ok!(Tellor::update_stake_amount(RuntimeOrigin::signed(reporter)));
			let expected_stake_amount = (U256::from(stake_amount_currency_target) *
				U256::from(10u128.pow(18))) /
				U256::from(price);
			System::assert_has_event(
				Event::NewStakeAmount { amount: expected_stake_amount }.into(),
			);
			// Dispute fee changes with stake amount
			let expected_dispute_fee =
				U256ToBalance::convert(Tellor::convert(expected_stake_amount / 10).unwrap()) *
					6 * 9; // TRB 1:(6*9) OCP
			System::assert_last_event(
				Event::NewDisputeFee { dispute_fee: expected_dispute_fee }.into(),
			);
			assert_eq!(Tellor::get_stake_amount(), expected_stake_amount);
			assert_eq!(Tellor::get_dispute_fee(), expected_dispute_fee);
		});
	});
}

#[test]
fn update_stake_amount_and_dispute_fee_via_hook() {
	let staking_token_price_query_data: QueryDataOf<Test> =
		spot_price("trb", "gbp").try_into().unwrap();
	let staking_token_price_query_id = keccak_256(staking_token_price_query_data.as_ref()).into();
	let staking_to_local_token_query_data: QueryDataOf<Test> =
		spot_price("trb", "ocp").try_into().unwrap();
	let staking_to_local_token_query_id: QueryId =
		keccak_256(staking_to_local_token_query_data.as_ref()).into();
	let reporter = 1;
	let dispute_fee = <Test as Config>::InitialDisputeFee::get();
	let mut ext = new_test_ext();

	ext.execute_with(|| {
		with_block(|| {
			Tellor::on_initialize(System::block_number());
			// Setup
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(10_000),
				Address::random()
			));

			// Test updating when 12 hrs have NOT passed
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 2),
				0,
				staking_token_price_query_data.clone()
			));
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});

		// Test updating when 12 hrs have passed
		with_block_after(60 * 60 * 12, || {
			Tellor::on_initialize(System::block_number());
			// No trb:token price reported yet, so dispute fee cannot be calculated as part of stake amount update
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_to_local_token_query_id,
				uint_value(PRICE_TRB_LOCAL),
				0,
				staking_to_local_token_query_data.clone()
			));
			// Requires waiting another 12 hours until reported price clears dispute period
		});

		// Test updating when 12 hrs have passed
		let dispute_fee = with_block_after(60 * 60 * 12, || {
			Tellor::on_initialize(System::block_number());
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			let dispute_fee = token(10 * 6); // 10% of 100 * PRICE_TRB_LOCAL
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
			dispute_fee
		});

		// Test updating when multiple prices have been reported
		with_block_after(60 * 60 * 1, || {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value((PRICE_TRB as f64 * 1.5) as u64),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 2),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 3),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 12, || {
			Tellor::on_initialize(System::block_number());
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});

		// Test bad TRB price encoding
		let bad_price = b"Where's the beef?";
		with_block(|| {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				bad_price.to_vec().try_into().unwrap(),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(86_400 / 2, || {
			Tellor::on_initialize(System::block_number());
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});

		// Test reported TRB price outside limits - high
		let high_price = trb(1_000_001);
		with_block(|| {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(high_price),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(86_400 / 2, || {
			Tellor::on_initialize(System::block_number());
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});

		// Test reported TRB price outside limits - low
		let low_price = trb(0.009);
		with_block(|| {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(low_price),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(86_400 / 2, || {
			Tellor::on_initialize(System::block_number());
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});

		// Test hook when multiple prices have been reported
		with_block_after(60 * 60 * 1, || {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 7),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 8),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 1, || {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(PRICE_TRB * 9),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 12, || {
			Tellor::on_initialize(System::block_number());
			assert_eq!(Tellor::get_stake_amount(), MINIMUM_STAKE_AMOUNT.into());
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});

		// Test with price that updates stake amount
		let price = PRICE_TRB / 11;
		with_block(|| {
			Tellor::on_initialize(System::block_number());
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				staking_token_price_query_id,
				uint_value(price),
				0,
				staking_token_price_query_data.clone()
			));
		});
		with_block_after(60 * 60 * 12, || {
			Tellor::on_initialize(System::block_number());
			let stake_amount_currency_target: u128 = StakeAmountCurrencyTarget::get();
			let expected_stake_amount = (U256::from(stake_amount_currency_target) *
				U256::from(10u128.pow(18))) /
				U256::from(price);
			assert_eq!(Tellor::get_stake_amount(), expected_stake_amount);
			// Dispute fee changes with stake amount
			let dispute_fee =
				U256ToBalance::convert(Tellor::convert(expected_stake_amount / 10).unwrap()) * 6; // TRB 1:6 OCP
			assert_eq!(Tellor::get_dispute_fee(), dispute_fee);
		});
	});
}

#[test]
fn update_rewards() {
	let reporter = 1;
	let funder = 2;
	let address = Address::random();
	let staking_rewards_account = &Tellor::staking_rewards();
	let unit = unit();
	let mut ext = new_test_ext();

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L827
	ext.execute_with(|| {
		let timestamp_0 = with_block(|| {
			// test totalStakeAmount equals 0
			assert_ok!(Tellor::update_rewards());

			let timestamp = now();
			assert_eq!(Tellor::time_of_last_allocation(), timestamp);
			assert_eq!(Tellor::accumulated_reward_per_share(), 0);
			assert_eq!(Tellor::reward_rate(), 0);
			timestamp
		});

		let timestamp_0 = with_block(|| {
			// deposit a stake
			Balances::make_free_balance_be(&Tellor::tips(), token(1_000));
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(50),
				address
			));

			let timestamp = now();
			assert_eq!(timestamp, timestamp_0 + 1);
			assert_eq!(Tellor::time_of_last_allocation(), timestamp);
			assert_eq!(Tellor::accumulated_reward_per_share(), 0);
			assert_eq!(Tellor::reward_rate(), 0);
			timestamp
		});

		let timestamp_0 = with_block(|| {
			// deposit another stake
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(50),
				address
			));

			let timestamp = now();
			assert_eq!(timestamp, timestamp_0 + 1);
			assert_eq!(Tellor::time_of_last_allocation(), timestamp);
			assert_eq!(Tellor::accumulated_reward_per_share(), 0);
			assert_eq!(Tellor::reward_rate(), 0);
			timestamp
		});

		let staking_rewards = token(1_000);
		let expected_reward_rate = staking_rewards / (86_400 * 30);
		let timestamp_1 = with_block(|| {
			// add staking rewards
			Balances::make_free_balance_be(&funder, staking_rewards);
			assert_eq!(Balances::free_balance(staking_rewards_account), 0);
			assert_ok!(Tellor::add_staking_rewards(RuntimeOrigin::signed(funder), staking_rewards));

			let timestamp = now();
			assert_eq!(timestamp, timestamp_0 + 1);
			assert_eq!(Tellor::time_of_last_allocation(), timestamp);
			assert_eq!(Tellor::accumulated_reward_per_share(), 0);
			assert_eq!(Balances::free_balance(staking_rewards_account), staking_rewards);
			assert_eq!(Tellor::total_reward_debt(), 0);
			assert_eq!(Tellor::reward_rate(), expected_reward_rate);
			timestamp
		});

		// advance time 1 day
		let (timestamp_2, expected_accumulated_reward_per_share) = with_block_after(86_400, || {
			// update rewards
			assert_ok!(Tellor::update_rewards());
			let timestamp = now();

			assert_eq!(timestamp, timestamp_1 + 86_400 + 1);
			assert_eq!(Tellor::time_of_last_allocation(), timestamp);
			assert_eq!(Balances::free_balance(staking_rewards_account), staking_rewards);
			assert_eq!(Tellor::total_reward_debt(), 0);
			assert_eq!(Tellor::reward_rate(), expected_reward_rate);
			// expAccumRewPerShare = BigInt(blocky2.timestamp - blocky1.timestamp) * BigInt(expectedRewardRate) * BigInt(1e18) / BigInt(100e18)
			let expected_accumulated_reward_per_share = u128::from(timestamp - timestamp_1) *
				u128::from(expected_reward_rate) *
				unit / (100 * unit);
			assert_eq!(
				Tellor::accumulated_reward_per_share(),
				expected_accumulated_reward_per_share
			);
			(timestamp, expected_accumulated_reward_per_share)
		});

		let timestamp_3 = with_block(|| {
			// deposit another stake
			// todo:
			// assert_ok!(Tellor::report_stake_deposited(
			// 	Origin::Staking.into(),
			// 	reporter,
			// 	trb(50),
			// 	address
			// ));
			assert_ok!(Tellor::update_rewards());

			let timestamp = now();
			assert_eq!(Tellor::time_of_last_allocation(), timestamp);
			assert_eq!(Tellor::reward_rate(), expected_reward_rate);
			let expected_accumulated_reward_per_share = expected_accumulated_reward_per_share +
				(u128::from(timestamp - timestamp_2) * u128::from(expected_reward_rate) * unit /
					(100 * unit));
			assert_eq!(
				Tellor::accumulated_reward_per_share(),
				expected_accumulated_reward_per_share
			);
			// todo:
			// let expected_staking_rewards_balance = U256ToBalance::convert(
			// 	Amount::from(staking_rewards) -
			// 		(Amount::from(expected_accumulated_reward_per_share) *
			// 			Amount::from(token(100))),
			// );
			//assert_eq!(Tellor::staking_rewards_balance(), expected_staking_rewards_balance);
			timestamp
		});

		// advance time 30 days
		let (timestamp_4, expected_accumulated_reward_per_share) =
			with_block_after(86_400 * 30, || {
				// update rewards
				assert_ok!(Tellor::update_rewards());
				let timestamp = now();

				assert_eq!(timestamp, timestamp_3 + 86_400 * 30 + 1);
				assert_eq!(Tellor::time_of_last_allocation(), timestamp);
				assert_eq!(Tellor::reward_rate(), 0); // rewards ran out, reward rate should be 0
				let expected_accumulated_reward_per_share = token(1000) / 100;
				assert_eq!(
					Tellor::accumulated_reward_per_share(),
					expected_accumulated_reward_per_share
				);
				(timestamp, expected_accumulated_reward_per_share)
			});

		// advance time 1 day
		with_block_after(86_400, || {
			// update rewards
			assert_ok!(Tellor::update_rewards());
			let timestamp = now();

			// checks, should be no change
			assert_eq!(timestamp, timestamp_4 + 86_400 + 1);
			assert_eq!(Tellor::time_of_last_allocation(), timestamp); // should update to latest updateRewards ts
			assert_eq!(Tellor::reward_rate(), 0); // should still be zero

			assert_eq!(Balances::free_balance(staking_rewards_account), staking_rewards);
			assert_eq!(Tellor::total_reward_debt(), 0);
			assert_eq!(
				Tellor::accumulated_reward_per_share(),
				expected_accumulated_reward_per_share
			); // shouldn't change
		});
	});
}

#[test]
fn update_stake_and_pay_rewards() {
	let reporter = 1;
	let funder = 1;
	let address = Address::random();
	let staking_rewards = &Tellor::staking_rewards();
	let mut ext = new_test_ext();

	fn begin_dispute_mock() {
		// https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/contracts/testing/GovernanceMock.sol#L16
		crate::pallet::VoteCount::<Test>::mutate(|c| c.saturating_inc());
	}

	fn vote_mock(account: AccountId) {
		// https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/contracts/testing/GovernanceMock.sol#L16
		crate::pallet::VoteTallyByAddress::<Test>::mutate(account, |c| c.saturating_inc());
	}

	// Based on https://github.com/tellor-io/tellorFlex/blob/3b3820f2111ec2813cb51455ef68cf0955c51674/test/functionTests-TellorFlex.js#L919
	ext.execute_with(|| {
		let (timestamp_0, expected_reward_rate) = with_block(|| {
			Balances::make_free_balance_be(&funder, token(1_000));
			// check initial conditions
			assert_eq!(Balances::free_balance(staking_rewards), 0);
			assert_eq!(Tellor::reward_rate(), 0);
			// add staking rewards
			assert_ok!(Tellor::add_staking_rewards(RuntimeOrigin::signed(funder), token(1_000)));
			// check conditions after adding rewards
			assert_eq!(Balances::free_balance(staking_rewards), token(1_000));
			assert_eq!(Tellor::total_reward_debt(), 0);
			let expected_reward_rate = token(1_000) / REWARD_RATE_TARGET;
			assert_eq!(Tellor::reward_rate(), expected_reward_rate);
			// create 2 mock disputes, vote once
			begin_dispute_mock();
			begin_dispute_mock();
			vote_mock(reporter);
			// deposit stake
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(10),
				address
			));
			let timestamp = now();
			// check conditions after depositing stake
			assert_eq!(Balances::free_balance(staking_rewards), token(1_000));
			assert_eq!(Tellor::get_total_stake_amount(), trb(10));
			assert_eq!(Tellor::total_reward_debt(), 0);
			assert_eq!(Tellor::accumulated_reward_per_share(), 0);
			assert_eq!(Tellor::time_of_last_allocation(), timestamp);
			let staker_info = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_info.staked_balance, trb(10)); // staked balance
			assert_eq!(staker_info.reward_debt, 0); // reward debt
			assert_eq!(staker_info.start_vote_count, 2); // start vote count
			assert_eq!(staker_info.start_vote_tally, 1); // start vote tally
			(timestamp, expected_reward_rate)
		});

		// advance time
		let (timestamp_1, expected_accumulated_reward_per_share, expected_balance) =
			with_block_after(86_400 * 10, || {
				// expect(await token.balanceOf(accounts[1].address)).to.equal(h.toWei("990"))
				// deposit 0 stake, update rewards
				assert_ok!(Tellor::report_stake_deposited(
					Origin::Staking.into(),
					reporter,
					trb(0),
					address
				));
				let timestamp = now();
				// check conditions after updating rewards
				assert_eq!(Tellor::time_of_last_allocation(), timestamp);
				assert_eq!(Tellor::reward_rate(), expected_reward_rate);
				let expected_accumulated_reward_per_share =
					(timestamp - timestamp_0) as Balance * expected_reward_rate / 10;
				let expected_balance = U256ToBalance::convert(
					U256::from(token(10)) * U256::from(expected_accumulated_reward_per_share) /
						U256::from(token(1)),
				);
				assert_eq!(Balances::free_balance(1), expected_balance);
				assert_eq!(
					Tellor::accumulated_reward_per_share(),
					expected_accumulated_reward_per_share
				);
				assert_eq!(Tellor::total_reward_debt(), expected_balance);
				let staker_info = Tellor::get_staker_info(reporter).unwrap();
				assert_eq!(staker_info.staked_balance, trb(10)); // staked balance
				assert_eq!(staker_info.reward_debt, expected_balance); // reward debt
				assert_eq!(staker_info.start_vote_count, 2); // start vote count
				assert_eq!(staker_info.start_vote_tally, 1); // start vote tally

				// start a dispute
				begin_dispute_mock();
				(timestamp, expected_accumulated_reward_per_share, expected_balance)
			});

		// advance time
		let (timestamp_2, expected_accumulated_reward_per_share, expected_reward_debt) =
			with_block_after(86400 * 10, || {
				// deposit 0 stake, update rewards
				assert_ok!(Tellor::report_stake_deposited(
					Origin::Staking.into(),
					reporter,
					trb(0),
					address
				));
				let timestamp = now();
				// check conditions after updating rewards
				assert_eq!(Tellor::time_of_last_allocation(), timestamp);
				assert_eq!(Tellor::reward_rate(), expected_reward_rate);
				let expected_accumulated_reward_per_share = ((timestamp - timestamp_1) as Balance *
					expected_reward_rate / 10) +
					expected_accumulated_reward_per_share;
				assert_eq!(Balances::free_balance(1), expected_balance);
				assert_eq!(
					Tellor::accumulated_reward_per_share(),
					expected_accumulated_reward_per_share
				);
				let expected_reward_debt = expected_accumulated_reward_per_share * 10;
				assert_eq!(Tellor::total_reward_debt(), expected_reward_debt);
				let staker_info = Tellor::get_staker_info(reporter).unwrap();
				assert_eq!(staker_info.staked_balance, trb(10)); // staked balance
				assert_eq!(staker_info.reward_debt, expected_reward_debt); // reward debt
				assert_eq!(staker_info.start_vote_count, 2); // start vote count
				assert_eq!(staker_info.start_vote_tally, 1); // start vote tally

				// start a dispute and vote
				begin_dispute_mock();
				vote_mock(reporter);
				(timestamp, expected_accumulated_reward_per_share, expected_reward_debt)
			});

		// advance time
		with_block_after(86_400 * 5, || {
			// deposit 0 stake, update rewards
			assert_ok!(Tellor::report_stake_deposited(
				Origin::Staking.into(),
				reporter,
				trb(0),
				address
			));
			let timestamp = now();
			// check conditions after updating rewards
			assert_eq!(Tellor::time_of_last_allocation(), timestamp);
			assert_eq!(Tellor::reward_rate(), expected_reward_rate);
			let expected_accumulated_reward_per_share = ((timestamp - timestamp_2) as Balance *
				expected_reward_rate /
				10) + expected_accumulated_reward_per_share;
			let expected_balance = expected_balance +
				((expected_accumulated_reward_per_share * 10 - expected_reward_debt) / 2);
			assert_eq!(Balances::free_balance(1), expected_balance);
			assert_eq!(
				Tellor::accumulated_reward_per_share(),
				expected_accumulated_reward_per_share
			);
			let expected_reward_debt = expected_accumulated_reward_per_share * 10;
			assert_eq!(Tellor::total_reward_debt(), expected_reward_debt);
			let staker_info = Tellor::get_staker_info(reporter).unwrap();
			assert_eq!(staker_info.staked_balance, trb(10)); // staked balance
			assert_eq!(staker_info.reward_debt, expected_reward_debt); // reward debt
			assert_eq!(staker_info.start_vote_count, 2); // start vote count
			assert_eq!(staker_info.start_vote_tally, 1); // start vote tally
			assert_eq!(Balances::free_balance(staking_rewards), token(1_000) - expected_balance);
		});
	});
}

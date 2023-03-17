use super::*;
use crate::{mock::AccountId, types::Tally, Config, VoteResult};
use frame_support::{assert_noop, assert_ok};
use sp_core::{bounded::BoundedBTreeMap, bounded_btree_map, Get};
use sp_runtime::traits::BadOrigin;

type BoundedVotes = BoundedBTreeMap<AccountId, bool, <Test as Config>::MaxVotes>;
type ParachainId = <Test as Config>::ParachainId;
type VoteRoundPeriod = <Test as Config>::VoteRoundPeriod;
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
			Timestamp::get()
		});

		let dispute_id = with_block(|| {
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

		let dispute_period = VoteRoundPeriod::get();
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

		let timestamp = with_block_after(dispute_period * 2, || {
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
			Timestamp::get()
		});

		with_block_after(VoteRoundPeriod::get(), || {
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
	// assert(voteInfo[3] == 1, "Vote result should change")
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
		with_block(|| {
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
			assert_noop!(Tellor::tally_votes(1), Error::InvalidDispute); // Cannot tally a dispute that does not exist

			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				Timestamp::get()
			));
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), 1, Some(false)));
			assert_noop!(Tellor::tally_votes(1), Error::VotingPeriodActive); // Time for voting has not elapsed
		});

		with_block_after(VoteRoundPeriod::get(), || {
			assert_ok!(Tellor::tally_votes(1));
			assert_noop!(Tellor::tally_votes(1), Error::VoteAlreadyTallied); // cannot re-tally a dispute

			let vote_info = Tellor::get_vote_info(1).unwrap();
			assert_eq!(vote_info.tally_date, Timestamp::get(), "Tally date should be correct");
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
		with_block(|| {
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
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter_2),
				query_id,
				Timestamp::get()
			));
			assert_noop!(
				Tellor::vote(RuntimeOrigin::signed(reporter_2), 2, Some(false)),
				Error::InvalidVote
			); // Can't vote on dispute does not exist

			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter_1), 1, Some(true)));
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter_2), 1, Some(false)));
			assert_noop!(
				Tellor::vote(RuntimeOrigin::signed(reporter_2), 1, Some(true)),
				Error::AlreadyVoted
			); // Sender has already voted
		});

		with_block_after(VoteRoundPeriod::get(), || {
			assert_ok!(Tellor::tally_votes(1));
			assert_noop!(
				Tellor::vote(RuntimeOrigin::signed(reporter_2), 1, Some(true)),
				Error::VoteAlreadyTallied
			); // Vote has already been tallied

			let vote_info = Tellor::get_vote_info(1).unwrap();
			assert_eq!(
				vote_info.users,
				Tally::<AmountOf<Test>>::default(),
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

			assert!(Tellor::did_vote(1, reporter_2), "voter's voted status should be correct");
			assert!(Tellor::did_vote(1, reporter_1), "voter's voted status should be correct");
			assert!(!Tellor::did_vote(1, 3), "voter's voted status should be correct");

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
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				Timestamp::get()
			));
			assert!(!Tellor::did_vote(1, reporter), "voter's voted status should be correct");
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), 1, Some(true)));
			assert!(Tellor::did_vote(1, reporter), "voter's voted status should be correct");
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
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				Timestamp::get()
			));
			let dispute_info = Tellor::get_dispute_info(1).unwrap();
			assert_eq!(dispute_info.0, query_id, "disputed query id should be correct");
			assert_eq!(dispute_info.1, Timestamp::get(), "disputed timestamp should be correct");
			assert_eq!(dispute_info.2, uint_value(100), "disputed value should be correct");
			assert_eq!(dispute_info.3, reporter, "disputed reporter should be correct");
		});
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
			Timestamp::get()
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

			assert_eq!(Tellor::get_open_disputes_on_id(query_id), 0);
			assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, timestamp));
			assert_eq!(Tellor::get_open_disputes_on_id(query_id), 1);
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				Timestamp::get()
			));
			assert_eq!(Tellor::get_open_disputes_on_id(query_id), 2);
		});

		with_block_after(VoteRoundPeriod::get(), || {
			assert_ok!(Tellor::tally_votes(1));
		});
		with_block_after(VoteTallyDisputePeriod::get(), || {
			assert_ok!(Tellor::execute_vote(1, VoteResult::Passed));
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
		with_block(|| {
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
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				Timestamp::get()
			));
			assert_eq!(Tellor::get_vote_count(), 1, "vote count should increment correctly");
		});

		with_block_after(VoteRoundPeriod::get(), || {
			assert_ok!(Tellor::tally_votes(1));
		});
		with_block_after(VoteTallyDisputePeriod::get(), || {
			assert_ok!(Tellor::execute_vote(1, VoteResult::Passed));
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
				Timestamp::get()
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

	// Prerequisites
	ext.execute_with(|| with_block(|| register_parachain(STAKE_AMOUNT)));

	// Based on https://github.com/tellor-io/governance/blob/0dcc2ad501b1e51383a99a22c60eeb8c36d61bc3/test/functionTests.js#L322
	ext.execute_with(|| {
		let (disputed_time, disputed_block) = with_block(|| {
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
			assert_ok!(Tellor::begin_dispute(
				RuntimeOrigin::signed(reporter),
				query_id,
				Timestamp::get()
			));
			assert_ok!(Tellor::vote(RuntimeOrigin::signed(reporter), 1, Some(true)));
			(Timestamp::get(), System::block_number())
		});

		let tallied = with_block_after(VoteRoundPeriod::get(), || {
			assert_ok!(Tellor::tally_votes(1));
			Timestamp::get()
		});
		with_block_after(VoteTallyDisputePeriod::get(), || {
			assert_ok!(Tellor::execute_vote(1, VoteResult::Passed));
			let vote = Tellor::get_vote_info(1).unwrap();
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
			assert_eq!(
				vote.users,
				Tally::<AmountOf<Test>>::default(),
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

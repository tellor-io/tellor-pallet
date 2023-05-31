use crate::{
	constants::{MAX_ITERATIONS, MAX_VOTES_SENT_PER_BLOCK},
	mock::Test,
	Config, WeightInfo,
};
use frame_support::{
	pallet_prelude::Get,
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
};

type MaxClaimTimestamps = <Test as Config>::MaxClaimTimestamps;
type MaxDisputedTimeSeries = <Test as Config>::MaxDisputedTimeSeries;
type MaxQueryDataLength = <Test as Config>::MaxQueryDataLength;
type MaxValueLength = <Test as Config>::MaxValueLength;
type MaxVotes = <Test as Config>::MaxVotes;
type Tellor = crate::SubstrateWeight<Test>;

const MAX_REF_TIME: u64 = WEIGHT_REF_TIME_PER_SECOND.saturating_div(2); // https://github.com/paritytech/cumulus/blob/98e68bd54257b4039a5d5b734816f4a1b7c83a9d/parachain-template/runtime/src/lib.rs#L221
const MAX_POV_SIZE: u64 = 5 * 1024 * 1024; // https://github.com/paritytech/polkadot/blob/ba1f65493d91d4ab1787af2fd6fe880f1da90586/primitives/src/v4/mod.rs#L384
const MAX_WEIGHT: Weight = Weight::from_parts(MAX_REF_TIME, MAX_POV_SIZE);

#[test]
fn verify() {
	for (function, weight) in vec![
		("register", Tellor::register()),
		("claim_onetime_tip", Tellor::claim_onetime_tip(MaxClaimTimestamps::get())),
		("claim_tip", Tellor::claim_tip(MaxClaimTimestamps::get())),
		("fund_feed", Tellor::fund_feed()),
		("setup_data_feed", Tellor::setup_data_feed(MaxQueryDataLength::get())),
		("tip", Tellor::tip(MaxQueryDataLength::get())),
		("add_staking_rewards", Tellor::add_staking_rewards()),
		("submit_value", Tellor::submit_value(MaxQueryDataLength::get(), MaxValueLength::get())),
		("update_stake_amount", Tellor::update_stake_amount(MAX_ITERATIONS, MAX_ITERATIONS)),
		("begin_dispute", Tellor::begin_dispute(MaxDisputedTimeSeries::get())),
		("vote", Tellor::vote()),
		("vote_on_multiple_disputes", Tellor::vote_on_multiple_disputes(MaxVotes::get())),
		("send_votes", Tellor::send_votes(u8::MAX.into())),
		("report_stake_deposited", Tellor::report_stake_deposited()),
		("report_staking_withdraw_request", Tellor::report_staking_withdraw_request()),
		("report_stake_withdrawn", Tellor::report_stake_withdrawn()),
		("report_slash", Tellor::report_slash()),
		("report_vote_tallied", Tellor::report_vote_tallied()),
		("report_vote_executed", Tellor::report_vote_executed(u8::MAX.into())),
		(
			"on_initialize",
			Tellor::on_initialize(MAX_ITERATIONS, MAX_ITERATIONS, MAX_VOTES_SENT_PER_BLOCK.into()),
		),
	] {
		println!(
			"{function}: max {weight:?}\t{:.2}% max ref_time, {:.2}% max proof_size",
			(weight.ref_time() as f64 / MAX_WEIGHT.ref_time() as f64) * 100.0,
			(weight.proof_size() as f64 / MAX_WEIGHT.proof_size() as f64) * 100.0
		);
		assert!(weight.all_lt(MAX_WEIGHT));
	}
}

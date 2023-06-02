use crate::{
	constants::{MAX_AGGREGATE_VOTES_SENT_PER_BLOCK, MAX_ITERATIONS},
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
type Weights = <Test as Config>::WeightInfo;

const MAX_REF_TIME: u64 = WEIGHT_REF_TIME_PER_SECOND.saturating_div(2); // https://github.com/paritytech/cumulus/blob/98e68bd54257b4039a5d5b734816f4a1b7c83a9d/parachain-template/runtime/src/lib.rs#L221
const MAX_POV_SIZE: u64 = 5 * 1024 * 1024; // https://github.com/paritytech/polkadot/blob/ba1f65493d91d4ab1787af2fd6fe880f1da90586/primitives/src/v4/mod.rs#L384
const MAX_WEIGHT: Weight = Weight::from_parts(MAX_REF_TIME, MAX_POV_SIZE);

#[test]
fn verify() {
	println!("max block weight: {MAX_WEIGHT}\n");
	println!("max weights:");
	for (function, weight) in vec![
		("register", Weights::register()),
		("claim_onetime_tip", Weights::claim_onetime_tip(MaxClaimTimestamps::get())),
		("claim_tip", Weights::claim_tip(MaxClaimTimestamps::get())),
		("fund_feed", Weights::fund_feed()),
		("setup_data_feed", Weights::setup_data_feed(MaxQueryDataLength::get())),
		("tip", Weights::tip(MaxQueryDataLength::get())),
		("add_staking_rewards", Weights::add_staking_rewards()),
		("submit_value", Weights::submit_value(MaxQueryDataLength::get(), MaxValueLength::get())),
		("update_stake_amount", Weights::update_stake_amount(MAX_ITERATIONS, MAX_ITERATIONS)),
		("begin_dispute", Weights::begin_dispute(MaxDisputedTimeSeries::get())),
		("vote", Weights::vote()),
		("vote_on_multiple_disputes", Weights::vote_on_multiple_disputes(MaxVotes::get())),
		("send_votes", Weights::send_votes(u8::MAX.into())),
		("report_stake_deposited", Weights::report_stake_deposited()),
		("report_staking_withdraw_request", Weights::report_staking_withdraw_request()),
		("report_stake_withdrawn", Weights::report_stake_withdrawn()),
		("report_slash", Weights::report_slash()),
		("report_vote_tallied", Weights::report_vote_tallied()),
		("report_vote_executed", Weights::report_vote_executed(u8::MAX.into())),
		(
			"on_initialize",
			Weights::on_initialize(
				MAX_ITERATIONS,
				MAX_ITERATIONS,
				MAX_AGGREGATE_VOTES_SENT_PER_BLOCK.into(),
			),
		),
	] {
		println!(
			"{function}: {weight:?}\tpercentage of block={:.2}% (ref_time), {:.2}% (proof_size), max tx per block={} (ref_time), {} (proof_size)",
			(weight.ref_time() as f64 / MAX_WEIGHT.ref_time() as f64) * 100.0,
			(weight.proof_size() as f64 / MAX_WEIGHT.proof_size() as f64) * 100.0,
			MAX_WEIGHT.ref_time() / weight.ref_time(),
			MAX_WEIGHT.proof_size() / weight.proof_size()
		);
		assert!(weight.all_lt(MAX_WEIGHT));
	}
}

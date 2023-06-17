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

use crate::{
	constants::{MAX_AGGREGATE_VOTES_SENT_PER_BLOCK, MAX_ITERATIONS, MAX_VOTE_ROUNDS},
	mock::Test,
	Config, WeightInfo,
};
use frame_support::{
	pallet_prelude::Get,
	weights::{
		constants::{WEIGHT_REF_TIME_PER_NANOS, WEIGHT_REF_TIME_PER_SECOND},
		Weight,
	},
};
use sp_runtime::Perbill;

type MaxClaimTimestamps = <Test as Config>::MaxClaimTimestamps;
type MaxDisputedTimeSeries = <Test as Config>::MaxDisputedTimeSeries;
type MaxQueryDataLength = <Test as Config>::MaxQueryDataLength;
type MaxValueLength = <Test as Config>::MaxValueLength;
type MaxVotes = <Test as Config>::MaxVotes;
type Weights = <Test as Config>::WeightInfo;

// max block: 0.5s compute with 12s average block time
const MAX_BLOCK_REF_TIME: u64 = WEIGHT_REF_TIME_PER_SECOND.saturating_div(2); // https://github.com/paritytech/cumulus/blob/98e68bd54257b4039a5d5b734816f4a1b7c83a9d/parachain-template/runtime/src/lib.rs#L221
const MAX_BLOCK_POV_SIZE: u64 = 5 * 1024 * 1024; // https://github.com/paritytech/polkadot/blob/ba1f65493d91d4ab1787af2fd6fe880f1da90586/primitives/src/v4/mod.rs#L384
const MAX_BLOCK_WEIGHT: Weight = Weight::from_parts(MAX_BLOCK_REF_TIME, MAX_BLOCK_POV_SIZE);
// max extrinsics: 75% of block
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75); // https://github.com/paritytech/cumulus/blob/d20c4283fe85df0c1ef8cb7c9eb7c09abbcbfa31/parachain-template/runtime/src/lib.rs#L218

// xcm-transactor limit
const DEFAULT_PROOF_SIZE: u64 = 256 * 1024; // https://github.com/PureStake/moonbeam/blob/a51b9570daedeb28853a8f730379a37fd977a487/primitives/xcm/src/ethereum_xcm.rs#L34
const TRANSACT_REQUIRED_WEIGHT_AT_MOST_PROOF_SIZE: u64 = DEFAULT_PROOF_SIZE / 2; // https://github.com/PureStake/moonbeam/blob/a51b9570daedeb28853a8f730379a37fd977a487/precompiles/xcm-transactor/src/functions.rs#L392

#[test]
fn verify() {
	let max_total_extrinsics = MAX_BLOCK_WEIGHT * NORMAL_DISPATCH_RATIO;
	// max extrinsic: max total extrinsics less average on_initialize ratio and less base extrinsic weight
	const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5); // https://github.com/paritytech/cumulus/blob/d20c4283fe85df0c1ef8cb7c9eb7c09abbcbfa31/parachain-template/runtime/src/lib.rs#L214
	const BASE_EXTRINSIC: Weight =
		Weight::from_parts(WEIGHT_REF_TIME_PER_NANOS.saturating_mul(125_000), 0); // https://github.com/paritytech/cumulus/blob/d20c4283fe85df0c1ef8cb7c9eb7c09abbcbfa31/parachain-template/runtime/src/weights/extrinsic_weights.rs#L26
	let max_extrinsic_weight = max_total_extrinsics
		.saturating_sub(MAX_BLOCK_WEIGHT * AVERAGE_ON_INITIALIZE_RATIO)
		.saturating_sub(BASE_EXTRINSIC);
	assert_eq!(max_extrinsic_weight, Weight::from_parts(349_875_000_000, 3_670_016));

	println!("max block weight: {MAX_BLOCK_WEIGHT}");
	println!("max total extrinsics weight: {max_total_extrinsics}");
	println!("max extrinsic weight: {max_extrinsic_weight}\n");

	for (function, weight, check_proof_size_limit) in vec![
		("register", Weights::register(), false),
		("claim_onetime_tip", Weights::claim_onetime_tip(MaxClaimTimestamps::get()), false),
		("claim_tip", Weights::claim_tip(MaxClaimTimestamps::get()), false),
		("fund_feed", Weights::fund_feed(), false),
		("setup_data_feed", Weights::setup_data_feed(MaxQueryDataLength::get()), false),
		("tip", Weights::tip(MaxQueryDataLength::get()), false),
		("add_staking_rewards", Weights::add_staking_rewards(), false),
		(
			"submit_value",
			Weights::submit_value(MaxQueryDataLength::get(), MaxValueLength::get()),
			false,
		),
		(
			"update_stake_amount",
			Weights::update_stake_amount(MAX_ITERATIONS, MAX_ITERATIONS),
			false,
		),
		("begin_dispute", Weights::begin_dispute(MaxDisputedTimeSeries::get()), false),
		("vote", Weights::vote(), false),
		("vote_on_multiple_disputes", Weights::vote_on_multiple_disputes(MaxVotes::get()), false),
		("send_votes", Weights::send_votes(u8::MAX.into()), false),
		("report_stake_deposited", Weights::report_stake_deposited(), true),
		("report_staking_withdraw_request", Weights::report_staking_withdraw_request(), true),
		("report_stake_withdrawn", Weights::report_stake_withdrawn(), true),
		("report_slash", Weights::report_slash(), true),
		("report_vote_tallied", Weights::report_vote_tallied(), true),
		("report_vote_executed", Weights::report_vote_executed(MAX_VOTE_ROUNDS.into()), true),
		(
			"on_initialize",
			Weights::on_initialize(
				MAX_ITERATIONS,
				MAX_ITERATIONS,
				MAX_AGGREGATE_VOTES_SENT_PER_BLOCK.into(),
			),
			false,
		),
	] {
		println!("{function}: {weight:?}",);
		println!(
			"\tpercentage of max extrinsic weight: {:.2}% (ref_time), {:.2}% (proof_size)",
			(weight.ref_time() as f64 / max_extrinsic_weight.ref_time() as f64) * 100.0,
			(weight.proof_size() as f64 / max_extrinsic_weight.proof_size() as f64) * 100.0,
		);
		println!(
			"\tmax tx per block: {} (ref_time), {} (proof_size)",
			max_extrinsic_weight.ref_time() / weight.ref_time(),
			max_extrinsic_weight.proof_size() / weight.proof_size()
		);
		assert!(weight.all_lt(max_extrinsic_weight));

		// ensure max proof size within xcm-transactor limit
		if check_proof_size_limit {
			assert!(
				weight.proof_size() <= TRANSACT_REQUIRED_WEIGHT_AT_MOST_PROOF_SIZE,
				"{function} weight proof_size of {} is greater than the xcm-transactor transact proof limit of {}",
				weight.proof_size(),
				TRANSACT_REQUIRED_WEIGHT_AT_MOST_PROOF_SIZE
			);
		}
	}
}

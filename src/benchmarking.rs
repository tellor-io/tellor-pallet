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

//! Benchmarking setup for tellor

use ethabi::{Bytes, Token, Uint};
use super::*;

#[allow(unused)]
use crate::Pallet as Tellor;
use frame_benchmarking::{benchmarks, account, BenchmarkError};
use frame_system::{RawOrigin};
use types::Address;
use crate::constants::DECIMALS;
use crate::constants::WEEKS;
use crate::types::QueryDataOf;
use sp_core::{bounded::BoundedVec, bounded_vec, keccak_256};
use frame_support::{traits::Currency};
use sp_runtime::traits::Bounded;

type RuntimeOrigin<T> = <T as frame_system::Config>::RuntimeOrigin;
//type Balance = <T as pallet::Config>::Balance;
const TRB: u128 = 10u128.pow(DECIMALS);
const PARA_ID: u32 = 3000;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn trb(amount: impl Into<f64>) -> Tributes {
	// TRB amount has 18 decimals
	Tributes::from((amount.into() * TRB as f64) as u128)
}

fn token<T: Config>(amount: impl Into<u64>) -> BalanceOf<T> {
	// test parachain token
	(amount.into() * unit::<T>() as u64).into()
}

fn unit<T: Config>() -> u128 {
	let decimals: u8 = T::Decimals::get();
	10u128.pow(decimals.into())
}

fn uint_value<T: Config>(value: impl Into<Uint>) -> ValueOf<T> {
	ethabi::encode(&[Token::Uint(value.into())]).try_into().unwrap()
}

fn spot_price(asset: impl Into<String>, currency: impl Into<String>) -> Bytes {
	ethabi::encode(&[
		Token::String("SpotPrice".to_string()),
		Token::Bytes(ethabi::encode(&[
			Token::String(asset.into()),
			Token::String(currency.into()),
		])),
	])
}

fn deposit_stake<T: Config>(reporter: AccountIdOf<T>, amount: Tributes, address: Address) {
	let origin = T::StakingOrigin::try_successful_origin().unwrap();
	Tellor::<T>::report_stake_deposited(origin, reporter, amount, address).unwrap();
}

// Helper function for creating feeds
fn create_feed<T: Config>(
	feed_creator: AccountIdOf<T>,
	query_id: QueryId,
	reward: BalanceOf<T>,
	start_time: Timestamp,
	interval: Timestamp,
	window: Timestamp,
	price_threshold: u16,
	reward_increase_per_second: BalanceOf<T>,
	query_data: QueryDataOf<T>,
	amount: BalanceOf<T>,
) -> FeedId {
	Tellor::<T>::setup_data_feed(
		RawOrigin::Signed(feed_creator).into(),
		query_id,
		reward,
		start_time,
		interval,
		window,
		price_threshold,
		reward_increase_per_second,
		query_data.clone(),
		amount
	).unwrap();
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
	feed_id
}

fn dispute_id(para_id: u32, query_id: QueryId, timestamp: Timestamp) -> DisputeId {
	keccak_256(&ethabi::encode(&[
		Token::Uint(para_id.into()),
		Token::FixedBytes(query_id.0.to_vec()),
		Token::Uint(timestamp.into()),
	]))
		.into()
}


benchmarks! {

	register {

	}: _(RawOrigin::Root)
	verify {
		assert_last_event::<T>(
				Event::RegistrationAttempted { para_id: 2000, contract_address: T::Registry::get().address.into() }.into(),
			);
	}

	report_stake_deposited {

		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		let amount = trb(100);
		let caller = T::StakingOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let _ = deposit_stake::<T>(reporter.clone(), amount, address);
	}: _<RuntimeOrigin<T>>(caller, reporter.clone(), amount, address)
	verify {
		assert_last_event::<T>(
				Event::NewStakerReported { staker: reporter, amount, address }.into(),
			);
	}

	report_staking_withdraw_request {
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		let amount = trb(100);
		let caller = T::StakingOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let _ = deposit_stake::<T>(reporter.clone(), amount, address);
	}: _<RuntimeOrigin<T>>(caller, reporter.clone(), amount, address)
	verify {
		assert_last_event::<T>(
				Event::StakeWithdrawRequestReported { reporter, amount, address }.into(),
			);
	}

	report_stake_withdrawn {
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		let amount = trb(100);
		let caller = T::StakingOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let _ = deposit_stake::<T>(reporter.clone(), amount, address);
		// request stake withdraw
		let _ = Tellor::<T>::report_staking_withdraw_request(caller.clone(), reporter.clone(), amount, address);
		// todo! increase time
		<NowOffset<T>>::put(7 * DAYS);
	}: _<RuntimeOrigin<T>>(caller, reporter.clone(), amount)
	verify {
		assert_last_event::<T>(
				Event::StakeWithdrawnReported { staker: reporter }.into(),
			);
	}

	deregister {
		Tellor::<T>::register(RawOrigin::Root.into());
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		let amount = trb(100);
		let caller = T::StakingOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let _ = deposit_stake::<T>(reporter.clone(), amount, address);
		// request stake withdraw
		let _ = Tellor::<T>::report_staking_withdraw_request(caller.clone(), reporter.clone(), amount, address);
		let _ = Tellor::<T>::report_stake_withdrawn(caller.clone(), reporter.clone(), amount);
	}: _(RawOrigin::Root)
	verify {
		assert_last_event::<T>(
				Event::DeregistrationAttempted { para_id: 2000, contract_address: T::Registry::get().address.into() }.into(),
			);
	}

	setup_data_feed {
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let feed_creator = account::<AccountIdOf<T>>("account", 1, 1);

		// todo! prefund account
		//T::Token::make_free_balance_be(&feed_creator, token(1_000_000));

		// create feed
		let _ = create_feed::<T>(feed_creator.clone(),
				query_id,
				token::<T>(10u64),
				T::Time::now().as_secs(),
				700,
				60,
				0,
				token::<T>(0u64),
				query_data.clone(),
				token::<T>(1000u64)
		);

	}: _(RawOrigin::Signed(feed_creator), query_id, token::<T>(10u64), T::Time::now().as_secs(), 600, 60, 0, token::<T>(0u64), query_data.clone(), token::<T>(1000u64))


	fund_feed{
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let feed_creator = account::<AccountIdOf<T>>("account", 1, 1);

		// todo! prefund account
		//T::Token::make_free_balance_be(&feed_creator, token(1_000_000));
		let feed_id = create_feed::<T>(feed_creator.clone(),
				query_id,
				token::<T>(10u64),
				T::Time::now().as_secs(),
				700,
				60,
				0,
				token::<T>(0u64),
				query_data.clone(),
				token::<T>(1000u64)
		);

	}: _(RawOrigin::Signed(feed_creator), feed_id, query_id, token::<T>(10u64))

	submit_value {
		let s in 0..98;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		// report deposit stake
		let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);

		// submitting multiple reports
		for i in 0..s {

			let _ = Tellor::<T>::submit_value(
				RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(i * 1_000), 0, query_data.clone()
			);
		<NowOffset<T>>::put(12 * HOURS);

		}

		<NowOffset<T>>::put(12 * HOURS);

	}: _(RawOrigin::Signed(reporter), query_id, uint_value::<T>(4_000), 0, query_data.clone())


	add_staking_rewards {
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);

	}: _(RawOrigin::Signed(reporter), token::<T>(100u64))

	update_stake_amount {
		let staking_to_local_token_query_data: QueryDataOf<T> =
			spot_price("trb", "ocp").try_into().unwrap();
		let staking_to_local_token_query_id: QueryId =
			keccak_256(staking_to_local_token_query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		// report deposit stake
		let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);
		// submit value
		let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), staking_to_local_token_query_id, uint_value::<T>(6 * 10u128.pow(18)), 0, staking_to_local_token_query_data);
		<NowOffset<T>>::put(43400);
	}: _(RawOrigin::Signed(reporter))

	tip {
		// max report submissions
		let s in 0..8;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		// report deposit stake
		let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);

		// submitting multiple reports as latest valid report is being extracted
		for i in 0..s {
			let _ = Tellor::<T>::submit_value(
				RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(i * 1_000), 0, query_data.clone()
			);
		// todo! increase time to submit another report
		// todo! raise dispute for few submissions
		}

		// todo! prefund reporter

	}: _(RawOrigin::Signed(reporter), query_id, token::<T>(100u64), query_data)

	claim_onetime_tip {
		// max tippers
		let t in 0..8;
		// max report submissions
		let s in 0..98;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();

		let reporter = account::<AccountIdOf<T>>("account", 20, 1);
		let mut tippers = vec![];
		for i in 0..t {
			tippers.push(account::<AccountIdOf<T>>("account", i, 1));
		}
		let address = Address::random();
		// report deposit stake
		let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);
		for i in tippers {
			//
			let _ = Tellor::<T>::tip(RawOrigin::Signed(reporter.clone()).into(), query_id, token::<T>(10u64), query_data.clone());

		}
		for j in 0..s {
			let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(j * 1_000), 0, query_data.clone());
		}

		let report_timestamps = <Reports<T>>::get(query_id)
		.map( |r| r.timestamps).unwrap();

		let mut timestamps: BoundedVec<Timestamp, T::MaxClaimTimestamps> = BoundedVec::default();

		for timestamp in report_timestamps {
			timestamps.try_push(timestamp);
		}


	}: _(RawOrigin::Signed(reporter), query_id, timestamps)

	claim_tip {
		// max tippers
		let t in 0..8;
		// max value submissions
		let s in 0..98;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 20, 1);
		let mut tippers = vec![];
		let feed_creator = account::<AccountIdOf<T>>("account", 101, 1);
		let address = Address::random();

		// todo! prefund account
		let feed_id = create_feed::<T>(feed_creator.clone(),
				query_id,
				token::<T>(10u64),
				T::Time::now().as_secs(),
				700,
				60,
				0,
				token::<T>(0u64),
				query_data.clone(),
				token::<T>(1000u64)
		);

		for i in 0..t {
			tippers.push(account::<AccountIdOf<T>>("account", i, 1));
		}

		// report deposit stake
		let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);
		for i in tippers {
			//
			let _ = Tellor::<T>::tip(RawOrigin::Signed(reporter.clone()).into(), query_id, token::<T>(10u64), query_data.clone());

		}
		for j in 0..s {
			let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(j * 1_000), 0, query_data.clone());
			<NowOffset<T>>::put(12 * HOURS);
		}

		let report_timestamps = <Reports<T>>::get(query_id)
		.map( |r| r.timestamps).unwrap();

		let mut timestamps: BoundedVec<Timestamp, T::MaxClaimTimestamps> = BoundedVec::default();

		for timestamp in report_timestamps {
			timestamps.try_push(timestamp);
		}
	}: _(RawOrigin::Signed(reporter), feed_id, query_id, timestamps)

	begin_dispute {
		// max report submissions
		let s in 0..90;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
        let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);

		for j in 0..s {
			let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(j * 1_000), 0, query_data.clone());
			<NowOffset<T>>::put(12 * HOURS);
		}

		let report_timestamps = <Reports<T>>::get(query_id)
		.map( |r| r.timestamps).unwrap();

		let mut timestamps: BoundedVec<Timestamp, T::MaxClaimTimestamps> = BoundedVec::default();

		for timestamp in report_timestamps {
			timestamps.try_push(timestamp);
		}
	}: _(RawOrigin::Signed(reporter), query_id, *timestamps.last().unwrap(), None)

	vote {
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);
		let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(4_000), 0, query_data.clone());
		let timestamps = <Reports<T>>::get(query_id).map( |r| r.timestamps).unwrap();

		let disputed_timestamp = timestamps[0];

		let _ = Tellor::<T>::begin_dispute(RawOrigin::Signed(reporter.clone()).into(), query_id, disputed_timestamp, None);

		let dispute_id = dispute_id(PARA_ID, query_id, disputed_timestamp);

	}: _(RawOrigin::Signed(reporter), dispute_id, Some(true))

	report_vote_tallied {
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let caller = T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let address = Address::random();
		let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);
		let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(4_000), 0, query_data.clone());
		let timestamps = <Reports<T>>::get(query_id).map( |r| r.timestamps).unwrap();

		let disputed_timestamp = timestamps[0];

		let _ = Tellor::<T>::begin_dispute(RawOrigin::Signed(reporter.clone()).into(), query_id, disputed_timestamp, None);

		let dispute_id = dispute_id(PARA_ID, query_id, disputed_timestamp);
		let _ = Tellor::<T>::vote(RawOrigin::Signed(reporter).into(), dispute_id, Some(true));

	}: _<RuntimeOrigin<T>>(caller, dispute_id, VoteResult::Passed)

	report_vote_executed {
		// max vote rounds
		let r in 0..20;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let caller = T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let address = Address::random();
		let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);
		let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(4_000), 0, query_data.clone());
		let timestamps = <Reports<T>>::get(query_id).map( |r| r.timestamps).unwrap();

		let disputed_timestamp = timestamps[0];

		for i in 0..r {
			let _ = Tellor::<T>::begin_dispute(RawOrigin::Signed(reporter.clone()).into(), query_id, disputed_timestamp, None);
		}

		let dispute_id = dispute_id(PARA_ID, query_id, disputed_timestamp);
		let _ = Tellor::<T>::vote(RawOrigin::Signed(reporter).into(), dispute_id, Some(true));
		let _ = Tellor::<T>::report_vote_tallied(caller.clone(), dispute_id, VoteResult::Passed);

	}: _<RuntimeOrigin<T>>(caller, dispute_id)

	report_slash {
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let caller = T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let address = Address::random();
		let _ = deposit_stake::<T>(reporter.clone(), trb(100), address);
		let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(4_000), 0, query_data.clone());
		let timestamps = <Reports<T>>::get(query_id).map( |r| r.timestamps).unwrap();

		let disputed_timestamp = timestamps[0];

		let _ = Tellor::<T>::begin_dispute(RawOrigin::Signed(reporter.clone()).into(), query_id, disputed_timestamp, None);
		let dispute_id = dispute_id(PARA_ID, query_id, disputed_timestamp);
		let _ = Tellor::<T>::vote(RawOrigin::Signed(reporter.clone()).into(), dispute_id, Some(true));
		let _ = Tellor::<T>::report_vote_tallied(caller.clone(), dispute_id, VoteResult::Passed);

	}: _<RuntimeOrigin<T>>(caller, reporter, trb(100))

	impl_benchmark_test_suite!(Tellor, crate::mock::new_test_ext(), crate::mock::Test);
}

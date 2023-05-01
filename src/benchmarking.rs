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
use crate::types::QueryDataOf;
use sp_core::{bounded::BoundedVec, keccak_256};
use crate::traits::BenchmarkHelper;

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

fn deposit_stake<T: Config>(reporter: AccountIdOf<T>, amount: Tributes, address: Address) -> Result<RuntimeOrigin<T>, BenchmarkError>{
	match T::StakingOrigin::try_successful_origin() {
		Ok(origin) => {
			Tellor::<T>::report_stake_deposited(origin.clone(), reporter, amount, address)
				.map_err(|_| BenchmarkError::Weightless)?;
			Ok(origin)
		} Err(_) => Err(BenchmarkError::Weightless)
	}
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
		let caller = deposit_stake::<T>(reporter.clone(), amount, address)?;
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
		let caller = deposit_stake::<T>(reporter.clone(), amount, address)?;
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
		let caller = deposit_stake::<T>(reporter.clone(), amount, address)?;
		// request stake withdraw
		Tellor::<T>::report_staking_withdraw_request(caller.clone(), reporter.clone(), amount, address)?;
		T::BenchmarkHelper::set_time(WEEKS);
	}: _<RuntimeOrigin<T>>(caller, reporter.clone(), amount)
	verify {
		assert_last_event::<T>(
				Event::StakeWithdrawnReported { staker: reporter }.into(),
			);
	}

	deregister {
		Tellor::<T>::register(RawOrigin::Root.into())?;
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		let amount = trb(1200);
		let caller = deposit_stake::<T>(reporter.clone(), amount, address)?;
		// request stake withdraw
		Tellor::<T>::report_staking_withdraw_request(caller.clone(), reporter.clone(), amount, address)?;
		T::BenchmarkHelper::set_time(WEEKS);
		Tellor::<T>::report_stake_withdrawn(caller.clone(), reporter.clone(), amount)?;
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

		T::BenchmarkHelper::set_balance(feed_creator.clone(), 1000);
		// create feed
		let _ = create_feed::<T>(
				feed_creator.clone(),
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

		T::BenchmarkHelper::set_balance(feed_creator.clone(), 1000);
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
		let s in 1..98;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		// report deposit stake
		deposit_stake::<T>(reporter.clone(), trb(1200), address)?;

		// submitting multiple reports
		for i in 1..=s {
			T::BenchmarkHelper::set_time(HOURS);
			Tellor::<T>::submit_value(
				RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(i * 1_000), 0, query_data.clone()
			)?;
		}
        T::BenchmarkHelper::set_time(HOURS);

	}: _(RawOrigin::Signed(reporter), query_id, uint_value::<T>(4_000), 0, query_data.clone())


	add_staking_rewards {
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		T::BenchmarkHelper::set_balance(reporter.clone(), 1000);
	}: _(RawOrigin::Signed(reporter), token::<T>(100u64))

	update_stake_amount {
		let staking_token_price_query_data: QueryDataOf<T> =
			spot_price("trb", "gbp").try_into().unwrap();
		let staking_token_price_query_id = keccak_256(staking_token_price_query_data.as_ref()).into();
		let staking_to_local_token_query_data: QueryDataOf<T> =
			spot_price("trb", "ocp").try_into().unwrap();
		let staking_to_local_token_query_id: QueryId =
			keccak_256(staking_to_local_token_query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		// report deposit stake
		deposit_stake::<T>(reporter.clone(), trb(10_000), address)?;
		T::BenchmarkHelper::set_time(HOURS);
		// submit value
		Tellor::<T>::submit_value(
			RawOrigin::Signed(reporter.clone()).into(),
			staking_token_price_query_id,
			uint_value::<T>(50 * 10u128.pow(18)),
			0,
			staking_token_price_query_data.clone())?;
		T::BenchmarkHelper::set_time(HOURS);

		Tellor::<T>::submit_value(
			RawOrigin::Signed(reporter.clone()).into(),
			staking_to_local_token_query_id,
			uint_value::<T>(6 * 10u128.pow(18)),
			0,
			staking_to_local_token_query_data)?;

		T::BenchmarkHelper::set_time(12 * HOURS);
	}: _(RawOrigin::Signed(reporter))

	tip {
		// max report submissions
		let s in 1..10;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		// report deposit stake
		deposit_stake::<T>(reporter.clone(), trb(1200), address)?;

		// submitting multiple reports
		for i in 1..=s {
			T::BenchmarkHelper::set_time(HOURS);
			Tellor::<T>::submit_value(
				RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(i * 1_000), 0, query_data.clone()
			)?;
		}
		// todo! raise dispute for few submissions

		T::BenchmarkHelper::set_balance(reporter.clone(), 1000);

	}: _(RawOrigin::Signed(reporter), query_id, token::<T>(100u64), query_data)

	claim_onetime_tip {
		let s in 1..10;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 20, 1);
		T::BenchmarkHelper::set_balance(reporter.clone(), 1000);

		let address = Address::random();
		// report deposit stake
		deposit_stake::<T>(reporter.clone(), trb(1200), address)?;

		for i in 1..=s {
			T::BenchmarkHelper::set_time(HOURS);
			let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(i * 1_000), 0, query_data.clone());
			Tellor::<T>::tip(RawOrigin::Signed(reporter.clone()).into(), query_id, token::<T>(10u64), query_data.clone())?;

		}
		let mut report_timestamps = <Reports<T>>::get(query_id)
		.map( |r| r.timestamps).unwrap();
		let mut timestamps: BoundedVec<Timestamp, T::MaxClaimTimestamps> = BoundedVec::default();
		report_timestamps.remove(0);
		for timestamp in report_timestamps{
			timestamps.try_push(timestamp).unwrap();
		}

		T::BenchmarkHelper::set_time(12 * HOURS);

	}: _(RawOrigin::Signed(reporter), query_id, timestamps)

	claim_tip {
		// max tippers
		let s in 2..10;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 20, 1);
		//let mut tippers = vec![];
		let feed_creator = account::<AccountIdOf<T>>("account", 101, 1);
		let address = Address::random();
		T::BenchmarkHelper::set_balance(reporter.clone(), 1000);

		T::BenchmarkHelper::set_balance(feed_creator.clone(), 1000);
		let feed_id = create_feed::<T>(feed_creator.clone(),
				query_id,
				token::<T>(10u64),
				T::Time::now().as_secs(),
				3600,
				600,
				0,
				token::<T>(0u64),
				query_data.clone(),
				token::<T>(100u64)
		);

		// report deposit stake
		deposit_stake::<T>(reporter.clone(), trb(1200), address)?;

		for i in 1..=s {
			T::BenchmarkHelper::set_time(HOURS);
			Tellor::<T>::submit_value(
				RawOrigin::Signed(reporter.clone()).into(),
				query_id,
				uint_value::<T>(i * 1_000),
				0,
				query_data.clone())?;
			Tellor::<T>::tip(RawOrigin::Signed(reporter.clone()).into(), query_id, token::<T>(10u64), query_data.clone())?;
		}

		let report_timestamps = <Reports<T>>::get(query_id)
		.map( |r| r.timestamps).unwrap();

		let mut timestamps: BoundedVec<Timestamp, T::MaxClaimTimestamps> = BoundedVec::default();

		for timestamp in report_timestamps {
			timestamps.try_push(timestamp).unwrap();
		}

		T::BenchmarkHelper::set_time(WEEKS);
	}: _(RawOrigin::Signed(reporter), feed_id, query_id, timestamps)

	begin_dispute {
		// max report submissions
		let s in 1..99;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let another_reporter = account::<AccountIdOf<T>>("account", 2, 1);
		let address = Address::random();
        deposit_stake::<T>(reporter.clone(), trb(1200), address)?;
        deposit_stake::<T>(another_reporter.clone(), trb(1200), address)?;
		T::BenchmarkHelper::set_balance(another_reporter.clone(), 1000);

		for i in 1..=s {
			T::BenchmarkHelper::set_time(HOURS);
			Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(i * 1_000), 0, query_data.clone())?;
		}

		let timestamps = <Reports<T>>::get(query_id)
		.map( |r| r.timestamps).unwrap();

	}: _(RawOrigin::Signed(another_reporter), query_id, *timestamps.last().unwrap(), None)

	vote {
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let address = Address::random();
		deposit_stake::<T>(reporter.clone(), trb(1200), address)?;
		T::BenchmarkHelper::set_time(HOURS);
		Tellor::<T>::submit_value(
			RawOrigin::Signed(reporter.clone()).into(),
			query_id,
			uint_value::<T>(4_000),
			0,
			query_data.clone())?;
		let timestamps = <Reports<T>>::get(query_id).map( |r| r.timestamps).unwrap();

		let disputed_timestamp = timestamps[0];

		T::BenchmarkHelper::set_balance(reporter.clone(), 1000);
		Tellor::<T>::begin_dispute(RawOrigin::Signed(reporter.clone()).into(), query_id, disputed_timestamp, None)?;

		let dispute_id = dispute_id(PARA_ID, query_id, disputed_timestamp);

	}: _(RawOrigin::Signed(reporter), dispute_id, Some(true))

	report_vote_tallied {
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let caller = T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let address = Address::random();
		deposit_stake::<T>(reporter.clone(), trb(1200), address)?;
		T::BenchmarkHelper::set_time(HOURS);
		Tellor::<T>::submit_value(
			RawOrigin::Signed(reporter.clone()).into(),
			query_id,
			uint_value::<T>(4_000),
			0,
			query_data.clone())?;
		let timestamps = <Reports<T>>::get(query_id).map( |r| r.timestamps).unwrap();

		let disputed_timestamp = timestamps[0];

		T::BenchmarkHelper::set_balance(reporter.clone(), 1000);
		Tellor::<T>::begin_dispute(RawOrigin::Signed(reporter.clone()).into(), query_id, disputed_timestamp, None)?;

		let dispute_id = dispute_id(PARA_ID, query_id, disputed_timestamp);
		Tellor::<T>::vote(RawOrigin::Signed(reporter).into(), dispute_id, Some(true))?;
		T::BenchmarkHelper::set_time(DAYS);
	}: _<RuntimeOrigin<T>>(caller, dispute_id, VoteResult::Passed)

	report_vote_executed {
		// max vote rounds
		let r in 3..20;
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let caller = T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let address = Address::random();
		deposit_stake::<T>(reporter.clone(), trb(1200), address)?;
		T::BenchmarkHelper::set_time(HOURS);
		Tellor::<T>::submit_value(
			RawOrigin::Signed(reporter.clone()).into(),
			query_id,
			uint_value::<T>(4_000),
			0,
			query_data.clone())?;
		let timestamps = <Reports<T>>::get(query_id).map( |r| r.timestamps).unwrap();
		let mut dispute_initiators: BoundedVec<AccountIdOf<T>, T::MaxRewardClaims> = BoundedVec::default();
		let disputed_timestamp = timestamps[0];
		//assert_eq!(disputed_timestamp, 223455);
		for i in 2..r {
			let another_reporter = account::<AccountIdOf<T>>("account", i, 1);
			deposit_stake::<T>(another_reporter.clone(), trb(1200), address)?;
			T::BenchmarkHelper::set_balance(another_reporter.clone(), 1000);
			let _ = dispute_initiators.try_push(another_reporter);
		}
		for dispute_initiator in dispute_initiators{
			Tellor::<T>::begin_dispute(RawOrigin::Signed(dispute_initiator.clone()).into(), query_id, disputed_timestamp, None)?;
		}
		let dispute_id = dispute_id(PARA_ID, query_id, disputed_timestamp);
		Tellor::<T>::vote(RawOrigin::Signed(reporter).into(), dispute_id, Some(true))?;
		T::BenchmarkHelper::set_time(WEEKS);
		Tellor::<T>::report_vote_tallied(caller.clone(), dispute_id, VoteResult::Passed)?;
		T::BenchmarkHelper::set_time(DAYS);
	}: _<RuntimeOrigin<T>>(caller, dispute_id)

	report_slash {
		let query_data: QueryDataOf<T> = spot_price("dot", "usd").try_into().unwrap();
		let query_id = keccak_256(query_data.as_ref()).into();
		let reporter = account::<AccountIdOf<T>>("account", 1, 1);
		let caller = T::GovernanceOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let address = Address::random();
		let _ = deposit_stake::<T>(reporter.clone(), trb(1200), address);
		T::BenchmarkHelper::set_time(HOURS);
		let _ = Tellor::<T>::submit_value(RawOrigin::Signed(reporter.clone()).into(), query_id, uint_value::<T>(4_000), 0, query_data.clone());
		let timestamps = <Reports<T>>::get(query_id).map( |r| r.timestamps).unwrap();

		let disputed_timestamp = timestamps[0];
		T::BenchmarkHelper::set_balance(reporter.clone(), 1000);
		Tellor::<T>::begin_dispute(RawOrigin::Signed(reporter.clone()).into(), query_id, disputed_timestamp, None)?;
		let dispute_id = dispute_id(PARA_ID, query_id, disputed_timestamp);
		Tellor::<T>::vote(RawOrigin::Signed(reporter.clone()).into(), dispute_id, Some(true))?;
		T::BenchmarkHelper::set_time(DAYS);
		Tellor::<T>::report_vote_tallied(caller.clone(), dispute_id, VoteResult::Passed)?;
	}: _<RuntimeOrigin<T>>(caller, reporter, trb(100))

	impl_benchmark_test_suite!(Tellor, crate::mock::new_test_ext(), crate::mock::Test);
}

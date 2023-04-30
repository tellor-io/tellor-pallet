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
use crate::{constants::REPORTING_LOCK, UsingTellor};
use sp_core::bytes::from_hex;

#[test]
#[ignore]
fn retrieve_data() {
	todo!()
}

#[test]
#[ignore]
fn get_new_value_count_by_query_id() {
	todo!()
}

#[test]
#[ignore]
fn get_timestamp_by_query_id_and_index() {
	todo!()
}

#[test]
#[ignore]
fn get_index_for_data_before() {
	todo!()
}

#[test]
#[ignore]
fn get_data_before() {
	todo!()
}

#[test]
#[ignore]
fn is_in_dispute() {
	todo!()
}

#[test]
fn get_index_for_data_after() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;

	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random()))
	});

	// Based on https://github.com/tellor-io/usingtellor/blob/cfc56240e0f753f452d2f376b5ab126fa95222ad/test/functionTests-UsingTellor.js#L132
	ext.execute_with(|| {
		let timestamp_0 = with_block(|| {
			let timestamp = now();
			assert_eq!(Tellor::get_index_for_data_after(query_id, timestamp), None);
			timestamp
		});

		let timestamp_1 = with_block(|| {
			let timestamp = now();
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(150),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_index_for_data_after(query_id, timestamp_0), Some(0));
			assert_eq!(Tellor::get_index_for_data_after(query_id, timestamp), None);
			timestamp
		});

		with_block_after(REPORTING_LOCK, || {
			let timestamp = now();
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(160),
				1,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_index_for_data_after(query_id, timestamp_0), Some(0));
			assert_eq!(Tellor::get_index_for_data_after(query_id, timestamp_1), Some(1));
			assert_eq!(Tellor::get_index_for_data_after(query_id, timestamp), None);
		});
	});
}

#[test]
#[ignore]
fn get_data_after() {
	todo!()
}

#[test]
fn get_multiple_values_before() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;

	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random()))
	});

	// Based on https://github.com/tellor-io/usingtellor/blob/cfc56240e0f753f452d2f376b5ab126fa95222ad/test/functionTests-UsingTellor.js#L192
	ext.execute_with(|| {
		// submit 2 values
		let timestamp_1 = with_block(|| {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(150),
				0,
				query_data.clone(),
			));
			Tellor::get_timestamp_by_query_id_and_index(query_id, 0).unwrap()
		});
		let timestamp_2 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(160),
				1,
				query_data.clone(),
			));
			Tellor::get_timestamp_by_query_id_and_index(query_id, 1).unwrap()
		});

		let ten_secs_after_submission = with_block_after(10, || now());

		// 1 hour before 1st submission
		assert_eq!(
			Tellor::get_multiple_values_before(query_id, timestamp_1 - 3_600, 3_600, 4),
			vec![]
		);

		// maxCount = 4
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_submission,
				3_600 + REPORTING_LOCK,
				4
			),
			vec![
				(uint_value(150).into_inner(), timestamp_1),
				(uint_value(160).into_inner(), timestamp_2),
			]
		);

		// maxCount = 3
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_submission,
				3_600 + REPORTING_LOCK,
				3
			),
			vec![
				(uint_value(150).into_inner(), timestamp_1),
				(uint_value(160).into_inner(), timestamp_2),
			]
		);

		// maxCount = 2
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_submission,
				3_600 + REPORTING_LOCK,
				2
			),
			vec![
				(uint_value(150).into_inner(), timestamp_1),
				(uint_value(160).into_inner(), timestamp_2),
			]
		);

		// maxCount = 1
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_submission,
				3_600 + REPORTING_LOCK,
				1
			),
			vec![(uint_value(160).into_inner(), timestamp_2)]
		);

		// maxAge = 5
		assert_eq!(
			Tellor::get_multiple_values_before(query_id, ten_secs_after_submission, 5, 4),
			vec![]
		);

		// submit another 2 values
		let timestamp_3 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(170),
				2,
				query_data.clone(),
			));
			Tellor::get_timestamp_by_query_id_and_index(query_id, 2).unwrap()
		});
		let timestamp_4 = with_block_after(REPORTING_LOCK, || {
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(180),
				3,
				query_data.clone(),
			));
			Tellor::get_timestamp_by_query_id_and_index(query_id, 3).unwrap()
		});

		let ten_secs_after_final_submission = with_block_after(10, || now());

		// maxCount = 6, don't update timestamp
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_submission,
				3_600 + (REPORTING_LOCK * 3),
				6
			),
			vec![
				(uint_value(150).into_inner(), timestamp_1),
				(uint_value(160).into_inner(), timestamp_2),
			]
		);

		// maxCount = 6, update timestamp
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_final_submission,
				3_600 + (REPORTING_LOCK * 3),
				6
			),
			vec![
				(uint_value(150).into_inner(), timestamp_1),
				(uint_value(160).into_inner(), timestamp_2),
				(uint_value(170).into_inner(), timestamp_3),
				(uint_value(180).into_inner(), timestamp_4),
			]
		);

		// maxCount = 5
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_final_submission,
				3_600 + (REPORTING_LOCK * 3),
				5
			),
			vec![
				(uint_value(150).into_inner(), timestamp_1),
				(uint_value(160).into_inner(), timestamp_2),
				(uint_value(170).into_inner(), timestamp_3),
				(uint_value(180).into_inner(), timestamp_4),
			]
		);

		// maxCount = 4
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_final_submission,
				3_600 + (REPORTING_LOCK * 3),
				4
			),
			vec![
				(uint_value(150).into_inner(), timestamp_1),
				(uint_value(160).into_inner(), timestamp_2),
				(uint_value(170).into_inner(), timestamp_3),
				(uint_value(180).into_inner(), timestamp_4),
			]
		);

		// maxCount = 3
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_final_submission,
				3_600 + (REPORTING_LOCK * 3),
				3
			),
			vec![
				(uint_value(160).into_inner(), timestamp_2),
				(uint_value(170).into_inner(), timestamp_3),
				(uint_value(180).into_inner(), timestamp_4),
			]
		);

		// maxCount = 2
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_final_submission,
				3_600 + (REPORTING_LOCK * 3),
				2
			),
			vec![
				(uint_value(170).into_inner(), timestamp_3),
				(uint_value(180).into_inner(), timestamp_4),
			]
		);

		// maxCount = 1
		assert_eq!(
			Tellor::get_multiple_values_before(
				query_id,
				ten_secs_after_final_submission,
				3_600 + (REPORTING_LOCK * 3),
				1
			),
			vec![(uint_value(180).into_inner(), timestamp_4)]
		);
	});
}

#[test]
fn bytes_to_uint() {
	// Based on https://github.com/tellor-io/usingtellor/blob/cfc56240e0f753f452d2f376b5ab126fa95222ad/test/functionTests-UsingTellor.js#L332
	assert_eq!(Tellor::bytes_to_uint(1u8.to_be_bytes().to_vec()).unwrap(), 1.into());
	assert_eq!(Tellor::bytes_to_uint(2u32.to_be_bytes().to_vec()).unwrap(), 2.into());
	assert_eq!(
		Tellor::bytes_to_uint(300000000000000u64.to_be_bytes().to_vec()).unwrap(),
		300000000000000u64.into()
	);
	assert_eq!(
		Tellor::bytes_to_uint(300000000000001u64.to_be_bytes().to_vec()).unwrap(),
		300000000000001u64.into()
	);
	assert_eq!(
		Tellor::bytes_to_uint(ethabi::encode(&[Token::Uint(1.into())]).try_into().unwrap())
			.unwrap(),
		1.into()
	);
	assert_eq!(
		Tellor::bytes_to_uint(
			ethabi::encode(&[Token::Uint(21010191828172717718232237237237128u128.into())])
				.try_into()
				.unwrap()
		)
		.unwrap(),
		21010191828172717718232237237237128u128.into()
	);
	assert_eq!(Tellor::bytes_to_uint(from_hex("0x01").unwrap()).unwrap(), 1.into());
	assert_eq!(Tellor::bytes_to_uint(from_hex("0x10").unwrap()).unwrap(), 16.into());
}

#[test]
fn get_reporter_by_timestamp() {
	let query_data: QueryDataOf<Test> = spot_price("dot", "usd").try_into().unwrap();
	let query_id = keccak_256(query_data.as_ref()).into();
	let reporter = 1;
	let another_reporter = 2;

	let mut ext = new_test_ext();

	// Prerequisites
	ext.execute_with(|| {
		with_block(|| {
			deposit_stake(reporter, MINIMUM_STAKE_AMOUNT, Address::random());
			deposit_stake(another_reporter, MINIMUM_STAKE_AMOUNT, Address::random());
		})
	});

	// Based on https://github.com/tellor-io/usingtellor/blob/cfc56240e0f753f452d2f376b5ab126fa95222ad/test/functionTests-UsingTellor.js#L352
	ext.execute_with(|| {
		with_block(|| {
			let timestamp = now();
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(reporter),
				query_id,
				uint_value(150),
				0,
				query_data.clone(),
			));
			assert_eq!(Tellor::get_reporter_by_timestamp(query_id, timestamp).unwrap(), reporter)
		});
		with_block(|| {
			let timestamp = now();
			assert_ok!(Tellor::submit_value(
				RuntimeOrigin::signed(another_reporter),
				query_id,
				uint_value(160),
				0,
				query_data.clone(),
			));
			assert_eq!(
				Tellor::get_reporter_by_timestamp(query_id, timestamp).unwrap(),
				another_reporter
			)
		});
	});
}

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

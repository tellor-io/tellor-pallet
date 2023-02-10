use crate::{
	mock::*,
	types::{Address, Amount},
	Error, Event, Origin,
};
use frame_support::{assert_noop, assert_ok, traits::PalletInfoAccess};
use sp_core::{bounded_vec, H256};
use xcm::prelude::{DescendOrigin, PalletInstance, Parachain, X2};

#[test]
fn reports_stake_deposited() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		let reporter = 1;
		let amount: Amount = 42.into();
		let address = Address::random();
		assert_ok!(Tellor::report_stake_deposited(
			Origin::Staking.into(),
			reporter,
			amount,
			address
		));

		// // Read pallet storage and assert an expected result.
		// assert_eq!(TemplateModule::something(), Some(42));

		System::assert_last_event(
			Event::NewStakerReported { staker: reporter, amount: amount.low_u64(), address }.into(),
		);
	});
}

#[test]
fn begins_dispute() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		let reporter = 1;
		assert_ok!(Tellor::report_stake_deposited(
			Origin::Staking.into(),
			reporter,
			42.into(),
			Address::random()
		));

		let query_id = H256::random();
		assert_ok!(Tellor::submit_value(
			RuntimeOrigin::signed(reporter),
			query_id,
			bounded_vec![],
			1,
			bounded_vec![]
		));

		let timestamp = Timestamp::now();
		assert_ok!(Tellor::begin_dispute(RuntimeOrigin::signed(reporter), query_id, timestamp));

		let sent_messages = sent_xcm();
		let (_, sent_message) = sent_messages.first().unwrap();
		assert!(sent_message
			.0
			.contains(&DescendOrigin(X2(Parachain(0), PalletInstance(Tellor::index() as u8)))));
		// todo: check remaining instructions

		// // Read pallet storage and assert an expected result.
		// assert_eq!(TemplateModule::something(), Some(42));

		System::assert_last_event(
			Event::NewDispute { dispute_id: 0, query_id, timestamp, reporter }.into(),
		);
	});
}

// #[test]
// fn correct_error_for_none_value() {
// 	new_test_ext().execute_with(|| {
// 		// Ensure the expected error is thrown when no value is present.
// 		assert_noop!(
// 			TemplateModule::cause_error(RuntimeOrigin::signed(1)),
// 			Error::<Test>::NoneValue
// 		);
// 	});
// }

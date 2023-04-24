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

use super::{traits, Config, Error, Pallet};
use crate::types::ParaId;
use ::xcm::latest::prelude::*;
use core::marker::PhantomData;
use frame_support::{
	log,
	pallet_prelude::*,
	traits::{OriginTrait, PalletInfoAccess},
	weights::constants::WEIGHT_REF_TIME_PER_SECOND,
};
use sp_core::Get;
use sp_std::{fmt::Debug, vec, vec::Vec};
use traits::SendXcm;
use xcm_executor::traits::{Convert, ConvertOrigin};

pub(crate) type DbWeight = frame_support::weights::constants::RocksDbWeight;

pub(crate) mod ethereum_xcm;

impl<T: Config> Pallet<T> {
	pub(super) fn send_xcm(para_id: ParaId, message: Xcm<()>) -> Result<(), Error<T>> {
		let interior = X1(PalletInstance(Pallet::<T>::index() as u8));
		let dest = MultiLocation { parents: 1, interior: X1(Parachain(para_id)) };
		T::Xcm::send_xcm(interior, dest, message).map_err(|e| match e {
			SendError::CannotReachDestination(..) => Error::<T>::Unreachable,
			_ => Error::<T>::SendFailure,
		})
	}
}

pub struct LocationToAccount<Location, Account, AccountId>(
	PhantomData<(Location, Account, AccountId)>,
);
impl<Location: Get<ContractLocation>, Account: Get<AccountId>, AccountId: Clone + Debug>
	Convert<MultiLocation, AccountId> for LocationToAccount<Location, Account, AccountId>
{
	fn convert(location: MultiLocation) -> Result<AccountId, MultiLocation> {
		if location == Location::get().into() {
			Ok(Account::get())
		} else {
			Err(location)
		}
	}
}

pub struct LocationToOrigin<Location, PalletOrigin, RuntimeOrigin>(
	PhantomData<(Location, PalletOrigin, RuntimeOrigin)>,
);
impl<
		Location: Get<ContractLocation>,
		PalletOrigin: Get<RuntimeOrigin>,
		RuntimeOrigin: OriginTrait,
	> ConvertOrigin<RuntimeOrigin> for LocationToOrigin<Location, PalletOrigin, RuntimeOrigin>
where
	RuntimeOrigin: Debug,
	RuntimeOrigin::AccountId: Clone + Debug,
{
	fn convert_origin(
		origin: impl Into<MultiLocation>,
		kind: OriginKind,
	) -> Result<RuntimeOrigin, MultiLocation> {
		let origin = origin.into();
		log::trace!(
			target: "xcm::origin_conversion",
			"LocationToPalletOrigin origin: {:?}, kind: {:?}",
			origin, kind,
		);
		match kind {
			OriginKind::SovereignAccount if origin == Location::get().into() =>
				Ok(PalletOrigin::get()),
			_ => Err(origin),
		}
	}
}

#[derive(Clone, Default, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ContractLocation {
	pub(crate) para_id: ParaId,
	pub(crate) address: [u8; 20],
}
impl From<(ParaId, [u8; 20])> for ContractLocation {
	fn from(value: (ParaId, [u8; 20])) -> Self {
		ContractLocation { para_id: value.0, address: value.1 }
	}
}

impl Into<MultiLocation> for ContractLocation {
	fn into(self) -> MultiLocation {
		MultiLocation {
			parents: 1,
			interior: X2(Parachain(self.para_id), AccountKey20 { network: Any, key: self.address }),
		}
	}
}

/// Constructs XCM message for remote transact of the supplied call.
/// # Arguments
/// * `call` - The encoded transaction to be applied.
/// * `gas_limit` - The gas limit used to calculate the weight and corresponding fees required.
/// # Returns
/// A XCM message for remote transact.
pub(crate) fn transact<T: Config>(call: Vec<u8>, gas_limit: u64) -> Xcm<()> {
	// Calculate weight for executing smart contract call via ethereum_xcm::transact(): https://github.com/PureStake/moonbeam/blob/056f67494ccf8f815e33cf350fe0575734b89ec5/pallets/ethereum-xcm/src/lib.rs#L138-L147
	let transact_extrinsic_weight = gas_to_weight(gas_limit) + DbWeight::get().reads(1);
	// Calculate total weight based on xcm message weight and transact execution
	let total_weight = weigh() + transact_extrinsic_weight;
	// Convert to fee amount
	let amount = weight_to_fee::<T>(total_weight);
	let asset: MultiAsset = (T::XcmFeesAsset::get(), Fungible(amount)).into();
	// Construct xcm message
	Xcm(vec![
		WithdrawAsset(asset.clone().into()),
		BuyExecution { fees: asset, weight_limit: Limited(total_weight.ref_time()) },
		Transact {
			origin_type: OriginKind::SovereignAccount,
			require_weight_at_most: transact_extrinsic_weight.ref_time(),
			call: call.into(),
		},
	])
}

pub(crate) fn gas_to_weight(gas_limit: u64) -> Weight {
	// https://github.com/PureStake/moonbeam/blob/master/runtime/moonbase/src/lib.rs#L371-L375
	const GAS_PER_SECOND: u64 = 40_000_000;
	const WEIGHT_PER_GAS: u64 = WEIGHT_REF_TIME_PER_SECOND / GAS_PER_SECOND;
	Weight::from_ref_time(gas_limit.saturating_mul(WEIGHT_PER_GAS))
}

/// The weight of a XCM message.
pub(crate) fn weigh() -> Weight {
	// Standard database weight
	let db_weight = frame_support::weights::constants::RocksDbWeight::get();
	// Moonbase Alpha benchmarked instruction weights
	const DESCEND_ORIGIN: Weight = Weight::from_ref_time(10_084_000); // https://github.com/PureStake/moonbeam/blob/056f67494ccf8f815e33cf350fe0575734b89ec5/pallets/moonbeam-xcm-benchmarks/src/weights/moonbeam_xcm_benchmarks_generic.rs#L169
	const WITHDRAW_ASSET: Weight = Weight::from_ref_time(200_000_000); // https://github.com/PureStake/moonbeam/blob/056f67494ccf8f815e33cf350fe0575734b89ec5/pallets/moonbeam-xcm-benchmarks/src/weights/moonbeam_xcm_benchmarks_fungible.rs#L28
	let buy_execution: Weight =
		Weight::from_ref_time(158_702_000).saturating_add(db_weight.reads(4)); // https://github.com/PureStake/moonbeam/blob/056f67494ccf8f815e33cf350fe0575734b89ec5/pallets/moonbeam-xcm-benchmarks/src/weights/moonbeam_xcm_benchmarks_generic.rs#L136
	let transact: Weight = Weight::from_ref_time(34_785_000).saturating_add(db_weight.reads(1)); // https://github.com/PureStake/moonbeam/blob/056f67494ccf8f815e33cf350fe0575734b89ec5/pallets/moonbeam-xcm-benchmarks/src/weights/moonbeam_xcm_benchmarks_generic.rs#L148

	// Calculate combined weight of xcm instructions
	DESCEND_ORIGIN
		.saturating_add(WITHDRAW_ASSET)
		.saturating_add(buy_execution)
		.saturating_add(transact)
}

pub(crate) fn weight_to_fee<T: Config>(weight: Weight) -> u128 {
	(weight.ref_time() as u128).saturating_mul(T::XcmWeightToAsset::get())
}

pub(super) struct FeeLocation<T>(PhantomData<T>);
impl<T: Config> FeeLocation<T> {
	pub(super) fn get() -> Result<MultiLocation, DispatchError> {
		// Convert interior fee location to multilocation as used by registry contract on controller chain
		let dest = MultiLocation::new(1, X1(Parachain(T::Registry::get().para_id)));
		Self::convert(T::FeeLocation::get(), &dest, T::ParachainId::get())
	}

	fn convert(
		interior: Junctions,
		dest: &MultiLocation,
		para_id: ParaId,
	) -> Result<MultiLocation, DispatchError> {
		let mut interior = interior.into();
		interior
			.reanchor(dest, &MultiLocation::new(1, X1(Parachain(para_id))))
			.map_err(|_| Error::<T>::JunctionOverflow)?;
		Ok(interior)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		mock::{Test, EVM_PARA_ID},
		types::Address,
	};
	use codec::Encode;
	use sp_core::blake2_256;
	use std::borrow::Borrow;

	const PARA_ID: u32 = 12_345;

	#[test]
	fn contract_location_from_tuple() {
		let address = Address::random().0;
		let location: ContractLocation = (PARA_ID, address).into();
		assert_eq!(location, ContractLocation { para_id: PARA_ID, address });
	}

	#[test]
	fn calculate_multilocation_derivative_account() {
		const PARA_ID: u32 = 3000;
		const PALLET_INSTANCE: u8 = 40;

		// https://docs.moonbeam.network/builders/interoperability/xcm/remote-evm-calls/#calculate-multilocation-derivative
		let location = MultiLocation {
			parents: 1,
			interior: X2(Parachain(PARA_ID), PalletInstance(PALLET_INSTANCE)),
		};

		// From: https://github.com/PureStake/moonbeam/blob/master/primitives/xcm/src/location_conversion.rs#L31
		let hash: [u8; 32] = ("multiloc", location.borrow()).borrow().using_encoded(blake2_256);
		let mut account_id = [0u8; 20];
		account_id.copy_from_slice(&hash[0..20]);
		println!("{:?}", account_id)
	}

	#[test]
	fn converts_fee_location() {
		let dest = MultiLocation::new(1, X1(Parachain(EVM_PARA_ID)));

		assert_eq!(
			FeeLocation::<Test>::convert(Here, &dest, PARA_ID).unwrap(),
			MultiLocation::new(1, X1(Parachain(PARA_ID)))
		);
		assert_eq!(
			FeeLocation::<Test>::convert(X1(PalletInstance(3)), &dest, PARA_ID).unwrap(),
			MultiLocation::new(1, X2(Parachain(PARA_ID), PalletInstance(3)))
		);
		assert_eq!(
			FeeLocation::<Test>::convert(X2(PalletInstance(50), GeneralIndex(7)), &dest, PARA_ID)
				.unwrap(),
			MultiLocation::new(1, X3(Parachain(PARA_ID), PalletInstance(50), GeneralIndex(7)))
		);
	}

	#[test]
	fn fee_location() {
		use crate::mock::PARA_ID;
		assert_eq!(
			FeeLocation::<Test>::get().unwrap(),
			MultiLocation::new(1, X1(Parachain(PARA_ID)))
		);
	}

	#[test]
	fn gas_to_weight() {
		// https://docs.moonbeam.network/builders/interoperability/xcm/remote-evm-calls/#differences-regular-remote-evm
		const WEIGHT_PER_GAS: u64 = 25_000;
		assert_eq!(super::gas_to_weight(1), Weight::from_ref_time(WEIGHT_PER_GAS));
		const MAX_GAS_LIMIT: u64 = 720_000;
		assert_eq!(super::gas_to_weight(MAX_GAS_LIMIT), Weight::from_ref_time(18_000_000_000));
	}

	#[test]
	fn transact() {
		const GAS_LIMIT: u64 = 100_000;
		let xt_weight = super::gas_to_weight(GAS_LIMIT) + DbWeight::get().reads(1);
		let total_weight = super::weigh() + xt_weight;
		let fees = MultiAsset {
			id: Concrete(MultiLocation { parents: 0, interior: X1(PalletInstance(3)) }),
			fun: Fungible(total_weight.ref_time() as u128 * 50_000),
		};

		assert_eq!(
			super::transact::<Test>(vec![], GAS_LIMIT),
			Xcm(vec![
				WithdrawAsset(fees.clone().into()),
				BuyExecution { fees, weight_limit: Limited(total_weight.ref_time()) },
				Transact {
					origin_type: OriginKind::SovereignAccount,
					require_weight_at_most: xt_weight.ref_time(),
					call: vec![].into(),
				},
			]),
		);
	}

	#[test]
	fn weigh() {
		let read = frame_support::weights::constants::RocksDbWeight::get().read;
		assert_eq!(
			super::weigh(),
			Weight::from_ref_time(
				10_084_000 + // DescendOrigin
					200_000_000 + // WithdrawAsset
					(158_702_000 + read * 4) + // BuyExecution
					(34_785_000 + 1 * read) // Transact
			)
		);
	}

	#[test]
	fn weight_to_fee() {
		const WEIGHT_FEE: u128 = 50_000;
		assert_eq!(super::weight_to_fee::<Test>(Weight::from_ref_time(1)), WEIGHT_FEE);
	}
}

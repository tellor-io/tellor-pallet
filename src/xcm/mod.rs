use super::{traits, Config, Error, Pallet};
use crate::types::ParaId;
use ::xcm::latest::prelude::*;
use core::marker::PhantomData;
use frame_support::{
	log,
	pallet_prelude::*,
	traits::{OriginTrait, PalletInfoAccess},
};
use sp_core::Get;
use sp_std::{fmt::Debug, vec, vec::Vec};
use traits::SendXcm;
use xcm_executor::traits::{Convert, ConvertOrigin};

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

pub(crate) fn transact(
	fees: MultiAsset,
	weight_limit: WeightLimit,
	require_weight_at_most: u64,
	call: Vec<u8>,
) -> Xcm<()> {
	let withdrawal_assets =
		MultiAssets::from_sorted_and_deduplicated_skip_checks(vec![fees.clone()]);

	// Construct xcm message
	Xcm(vec![
		WithdrawAsset(withdrawal_assets),
		BuyExecution { fees, weight_limit },
		Transact {
			origin_type: OriginKind::SovereignAccount,
			require_weight_at_most,
			call: call.into(),
		},
	])
}

pub(crate) fn transact_with_config(call: Vec<u8>, config: XcmConfig) -> Xcm<()> {
	transact(config.fees, config.weight_limit, config.require_weight_at_most, call)
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct XcmConfig {
	pub(crate) fees: MultiAsset,
	pub(crate) weight_limit: WeightLimit,
	pub(crate) require_weight_at_most: u64,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::Address;
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
}

use ::xcm::latest::{prelude::*, MultiLocation};
use core::marker::PhantomData;
use frame_support::{log, traits::OriginTrait};
use sp_core::Get;
use sp_std::{fmt::Debug, vec, vec::Vec};
use xcm_executor::traits::{Convert, ConvertOrigin};

pub(crate) mod ethereum_xcm;

pub struct LocationToPalletAccount<Location, Account, AccountId>(
    PhantomData<(Location, Account, AccountId)>,
);
impl<Location: Get<MultiLocation>, Account: Get<AccountId>, AccountId: Clone + Debug>
    Convert<MultiLocation, AccountId> for LocationToPalletAccount<Location, Account, AccountId>
{
    fn convert(location: MultiLocation) -> Result<AccountId, MultiLocation> {
        if location == Location::get() {
            Ok(Account::get())
        } else {
            Err(location)
        }
    }
}

pub struct LocationToPalletOrigin<Location, PalletOrigin, RuntimeOrigin>(
    PhantomData<(Location, PalletOrigin, RuntimeOrigin)>,
);
impl<
        Location: Get<MultiLocation>,
        PalletOrigin: Get<RuntimeOrigin>,
        RuntimeOrigin: OriginTrait,
    > ConvertOrigin<RuntimeOrigin> for LocationToPalletOrigin<Location, PalletOrigin, RuntimeOrigin>
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
            OriginKind::SovereignAccount if origin == Location::get() => Ok(PalletOrigin::get()),
            _ => Err(origin),
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

pub(crate) fn contract_address(location: &MultiLocation) -> Option<&[u8; 20]> {
    match location {
        MultiLocation {
            parents: _parents,
            interior:
                X2(
                    Parachain(_para_id),
                    AccountKey20 {
                        key,
                        network: _network,
                    },
                ),
        } => Some(key),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Address;

    #[test]
    fn contract_address_matches() {
        let address = Address::random().0;
        let location: MultiLocation = MultiLocation {
            parents: 1,
            interior: Junctions::X2(
                Parachain(2000),
                AccountKey20 {
                    network: Any,
                    key: address,
                },
            ),
        };

        assert_eq!(&address, contract_address(&location).unwrap())
    }
}

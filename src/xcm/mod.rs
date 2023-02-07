use ::xcm::latest::{prelude::*, MultiLocation};
use core::marker::PhantomData;
use frame_support::{log, traits::OriginTrait};
use sp_core::Get;
use sp_std::fmt::Debug;
use xcm_executor::traits::{Convert, ConvertOrigin};

mod ethereum_xcm;

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

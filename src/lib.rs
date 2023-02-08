#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod api;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod contracts;
mod types;
pub mod xcm;

#[frame_support::pallet(dev_mode)]
pub mod pallet {
    use super::{
        contracts::governance,
        types::*,
        xcm::{self, ethereum_xcm},
    };
    use ::xcm::latest::prelude::*;
    use frame_support::traits::fungible::{Inspect, Transfer};
    use frame_support::{
        pallet_prelude::*,
        sp_runtime::traits::{
            AtLeast32BitUnsigned, BadOrigin, CheckEqual, Hash, MaybeDisplay,
            MaybeSerializeDeserialize, Member, SimpleBitOps,
        },
        traits::Time,
        PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_core::U256;
    use sp_runtime::{traits::AccountIdConversion, Saturating};
    use sp_std::{fmt::Debug, prelude::*, result};

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The runtime origin type.
        type RuntimeOrigin: From<<Self as frame_system::Config>::RuntimeOrigin>
            + Into<result::Result<Origin, <Self as Config>::RuntimeOrigin>>;

        /// The units in which we record amounts.
        type Amount: Member
            + Parameter
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + MaxEncodedLen
            + TypeInfo;

        /// The identifier used for disputes.
        type DisputeId: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + MaybeDisplay
            + Ord
            + MaxEncodedLen
            + Default
            + Into<U256>;

        /// Percentage, 1000 is 100%, 50 is 5%, etc
        #[pallet::constant]
        type Fee: Get<u8>;

        #[pallet::constant]
        type Governance: Get<MultiLocation>;

        /// The output of the `Hasher` function.
        type Hash: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + MaybeDisplay
            + SimpleBitOps
            + Ord
            + Default
            + Copy
            + CheckEqual
            + sp_std::hash::Hash
            + AsRef<[u8]>
            + AsMut<[u8]>
            + MaxEncodedLen;

        ///The hashing system (algorithm) to be used (e.g. keccak256).
        type Hasher: Hash<Output = <Self as Config>::Hash> + TypeInfo;

        /// The maximum number of timestamps per claim.
        #[pallet::constant]
        type MaxClaimTimestamps: Get<u32>;

        /// The maximum number of feeds per query.
        #[pallet::constant]
        type MaxFeedsPerQuery: Get<u32>;

        /// The maximum number of funded feeds.
        #[pallet::constant]
        type MaxFundedFeeds: Get<u32>;

        /// The maximum number of queries (data feeds) per reporter.
        #[pallet::constant]
        type MaxQueriesPerReporter: Get<u32> + TypeInfo;

        /// The maximum length of query data.
        #[pallet::constant]
        type MaxQueryDataLength: Get<u32>;

        /// The maximum number of timestamps per data feed.
        #[pallet::constant]
        type MaxTimestamps: Get<u32> + TypeInfo;

        /// The maximum number of tips per query.
        #[pallet::constant]
        type MaxTipsPerQuery: Get<u32>;

        /// The maximum length of an individual value submitted to the oracle.
        #[pallet::constant]
        type MaxValueLength: Get<u32>;

        /// The maximum number of votes.
        #[pallet::constant]
        type MaxVotes: Get<u32>;

        /// The identifier of the pallet within the runtime.
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// The local parachain's own identifier.
        #[pallet::constant]
        type ParachainId: Get<ParaId>;

        /// Base amount of time before a reporter is able to submit a value again.
        #[pallet::constant]
        type ReportingLock: Get<TimestampOf<Self>>;

        /// The on-chain time provider.
        type Time: Time;

        type Token: Inspect<Self::AccountId, Balance = Self::Amount> + Transfer<Self::AccountId>;

        type Xcm: SendXcm;
    }

    // AutoPay
    #[pallet::storage]
    pub type CurrentFeeds<T> = StorageMap<
        _,
        Blake2_128Concat,
        QueryIdOf<T>,
        BoundedVec<FeedIdOf<T>, <T as Config>::MaxFeedsPerQuery>,
    >;
    #[pallet::storage]
    pub type DataFeeds<T> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        QueryIdOf<T>,
        Blake2_128Concat,
        FeedIdOf<T>,
        FeedDetailsOf<T>,
    >;
    #[pallet::storage]
    pub type FeedsWithFunding<T> =
        StorageValue<_, BoundedVec<FeedIdOf<T>, <T as Config>::MaxFundedFeeds>>;
    #[pallet::storage]
    pub type QueryIdFromDataFeedId<T> = StorageMap<_, Blake2_128Concat, FeedIdOf<T>, QueryIdOf<T>>;
    #[pallet::storage]
    pub type QueryIdsWithFunding<T> =
        StorageValue<_, BoundedVec<QueryIdOf<T>, <T as Config>::MaxFundedFeeds>>;
    #[pallet::storage]
    pub type QueryIdsWithFundingIndex<T> = StorageMap<_, Blake2_128Concat, QueryIdOf<T>, u32>;
    #[pallet::storage]
    #[pallet::getter(fn tips)]
    pub type Tips<T> = StorageMap<
        _,
        Blake2_128Concat,
        QueryIdOf<T>,
        BoundedVec<TipOf<T>, <T as Config>::MaxTipsPerQuery>,
    >;
    #[pallet::storage]
    pub type UserTipsTotal<T> =
        StorageMap<_, Blake2_128Concat, AccountIdOf<T>, AmountOf<T>, ValueQuery>;
    // Oracle
    #[pallet::storage]
    pub type Reports<T> = StorageMap<_, Blake2_128Concat, QueryIdOf<T>, ReportOf<T>>;
    #[pallet::storage]
    pub type RewardRate<T> = StorageValue<_, AmountOf<T>>;
    #[pallet::storage]
    pub type StakeAmount<T> = StorageValue<_, AmountOf<T>>;
    #[pallet::storage]
    pub type StakerDetails<T> = StorageMap<_, Blake2_128Concat, AccountIdOf<T>, StakeInfoOf<T>>;
    #[pallet::storage]
    pub type StakerAddresses<T> = StorageMap<_, Blake2_128Concat, Address, AccountIdOf<T>>;
    #[pallet::storage]
    pub type TimeOfLastNewValue<T> = StorageValue<_, TimestampOf<T>>;
    #[pallet::storage]
    pub type TotalStakeAmount<T> = StorageValue<_, AmountOf<T>>;
    #[pallet::storage]
    pub type TotalStakers<T> = StorageValue<_, u128>;
    // Governance
    #[pallet::storage]
    pub type DisputeIdsByReporter<T> =
        StorageDoubleMap<_, Blake2_128Concat, AccountIdOf<T>, Blake2_128Concat, DisputeIdOf<T>, ()>;
    #[pallet::storage]
    pub type VoteCount<T> = StorageValue<_, DisputeIdOf<T>, ValueQuery>;
    // Query Data
    #[pallet::storage]
    pub type QueryData<T> = StorageMap<_, Blake2_128Concat, QueryIdOf<T>, QueryDataOf<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // AutoPay
        DataFeedFunded {
            query_id: QueryIdOf<T>,
            feed_id: FeedIdOf<T>,
            amount: AmountOf<T>,
            feed_funder: AccountIdOf<T>,
            feed_details: FeedDetailsOf<T>,
        },
        NewDataFeed {
            query_id: QueryIdOf<T>,
            feed_id: FeedIdOf<T>,
            query_data: QueryDataOf<T>,
            feed_creator: AccountIdOf<T>,
        },
        OneTimeTipClaimed {
            query_id: QueryIdOf<T>,
            amount: AmountOf<T>,
            reporter: AccountIdOf<T>,
        },
        TipAdded {
            query_id: QueryIdOf<T>,
            amount: AmountOf<T>,
            query_data: QueryDataOf<T>,
            tipper: AccountIdOf<T>,
        },
        TipClaimed {
            feed_id: FeedIdOf<T>,
            query_id: QueryIdOf<T>,
            amount: AmountOf<T>,
            reporter: AccountIdOf<T>,
        },
        // Oracle
        NewReport {
            query_id: QueryIdOf<T>,
            time: TimestampOf<T>,
            value: ValueOf<T>,
            nonce: Nonce,
            query_data: QueryDataOf<T>,
            reporter: AccountIdOf<T>,
        },
        NewStakerReported {
            staker: AccountIdOf<T>,
            amount: AmountOf<T>,
            address: Address,
        },
        SlashReported {
            reporter: AccountIdOf<T>,
            recipient: AccountIdOf<T>,
            amount: AmountOf<T>,
        },
        StakeWithdrawnReported {
            staker: AccountIdOf<T>,
        },
        StakeWithdrawRequestReported {
            reporter: AccountIdOf<T>,
            balance: AmountOf<T>,
            address: Address,
        },
        ValueRemoved {
            query_id: QueryIdOf<T>,
            timestamp: TimestampOf<T>,
        },
        // Governance
        NewDispute {
            dispute_id: DisputeIdOf<T>,
            query_id: QueryIdOf<T>,
            timestamp: TimestampOf<T>,
            reporter: AccountIdOf<T>,
        },
        Voted {
            dispute_id: DisputeIdOf<T>,
            supports: Option<bool>,
            voter: AccountIdOf<T>,
        },
        // Query Data
        QueryDataStored {
            query_id: QueryIdOf<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        // AutoPay
        /// Tip must be greater than zero.
        InvalidAmount,
        /// Query identifier must be a hash of bytes data.
        InvalidQueryId,
        /// No tips submitted for this query identifier.
        NoTipsSubmitted,

        NoValueExists,
        NotStaking,
        // XCM
        InvalidContractAddress,
        MaxEthereumXcmInputSizeExceeded,
        SendFailure,
        Unreachable,
    }

    /// Origin for the Tellor module.
    #[pallet::origin]
    #[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
    pub enum Origin {
        /// It comes from the governance controller contract.
        Governance,
        /// It comes from the staking controller contract.
        Staking,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Function to claim singular tip.
        ///
        /// - `query_id`: Identifier of reported data.
        /// - `timestamps`: Batch of timestamps of reported data eligible for reward.
        pub fn claim_onetime_tip(
            origin: OriginFor<T>,
            query_id: QueryIdOf<T>,
            timestamps: BoundedVec<TimestampOf<T>, T::MaxClaimTimestamps>,
        ) -> DispatchResult {
            let reporter = ensure_signed(origin)?;
            ensure!(
                <Tips<T>>::get(query_id).map_or(false, |t| t.len() > 0),
                Error::<T>::NoTipsSubmitted
            );

            let cumulative_reward = AmountOf::<T>::default();
            for _timestamp in timestamps {}
            Self::deposit_event(Event::OneTimeTipClaimed {
                query_id,
                amount: cumulative_reward,
                reporter,
            });
            Ok(())
        }

        /// Allows Tellor reporters to claim their tips in batches.
        ///
        /// - `feed_id`: Unique feed identifier.
        /// - `query_id`: Identifier of reported data.
        /// - `timestamps`: Batch of timestamps of reported data eligible for reward.
        pub fn claim_tip(
            _origin: OriginFor<T>,
            _feed_id: FeedIdOf<T>,
            _query_id: QueryIdOf<T>,
            _timestamps: BoundedVec<TimestampOf<T>, T::MaxClaimTimestamps>,
        ) -> DispatchResult {
            Ok(())
        }

        /// Allows Tellor reporters to claim their tips in batches.
        ///
        /// - `feed_id`: Unique feed identifier.
        /// - `query_id`: Identifier of reported data.
        /// - `timestamps`: Batch of timestamps of reported data eligible for reward.
        pub fn fund_feed(
            _origin: OriginFor<T>,
            _feed_id: FeedIdOf<T>,
            _query_id: QueryIdOf<T>,
            _amount: AmountOf<T>,
        ) -> DispatchResult {
            Ok(())
        }

        /// Initializes data feed parameters.
        ///
        /// - `query_id`: Unique identifier of desired data feed.
        /// - `reward`: Tip amount per eligible data submission.
        /// - `start_time`: Timestamp of first autopay window.
        /// - `interval`: Amount of time between autopay windows.
        /// - `window`: Amount of time after each new interval when reports are eligible for tips.
        /// - `price_threshold`: Amount price must change to automate update regardless of time (negated if 0, 100 = 1%).
        /// - `reward_increase_per_second`: Amount reward increases per second within a window (0 for flat reward).
        /// - `query_data`: The data used by reporters to fulfil the query.
        /// - `amount`: Optional initial amount to fund it with.
        pub fn setup_data_feed(
            _origin: OriginFor<T>,
            _query_id: QueryIdOf<T>,
            _reward: AmountOf<T>,
            _start_time: TimestampOf<T>,
            _interval: u8,
            _window: u8,
            _price_threshold: AmountOf<T>,
            _reward_increase_per_second: AmountOf<T>,
            _query_data: QueryDataOf<T>,
            _amount: Option<AmountOf<T>>,
        ) -> DispatchResult {
            Ok(())
        }

        /// Function to run a single tip.
        ///
        /// - `query_id`: Identifier of tipped data.
        /// - `amount`: Amount to tip.
        /// - `query_data`: The data used by reporters to fulfil the query.
        pub fn tip(
            origin: OriginFor<T>,
            query_id: QueryIdOf<T>,
            amount: AmountOf<T>,
            query_data: QueryDataOf<T>,
        ) -> DispatchResult {
            let tipper = ensure_signed(origin)?;
            ensure!(
                query_id == HasherOf::<T>::hash_of(&query_data),
                Error::<T>::InvalidQueryId
            );
            ensure!(amount > AmountOf::<T>::default(), Error::<T>::InvalidAmount);

            <Tips<T>>::try_mutate(query_id, |maybe_tips| -> DispatchResult {
                match maybe_tips {
                    None => {
                        *maybe_tips = Some(
                            BoundedVec::try_from(vec![TipOf::<T> {
                                amount,
                                timestamp: T::Time::now(),
                                cumulative_tips: amount,
                            }])
                            .map_err(|_| Error::<T>::InvalidQueryId)?,
                        );
                        Self::store_data(query_id, &query_data);
                        Ok(())
                    }
                    Some(tips) => {
                        todo!()
                    }
                }
            })?;

            T::Token::transfer(
                &tipper,
                &T::PalletId::get().into_account_truncating(),
                amount,
                true,
            )?;
            <UserTipsTotal<T>>::mutate(&tipper, |total| total.saturating_add(amount));
            Self::deposit_event(Event::TipAdded {
                query_id,
                amount,
                query_data,
                tipper,
            });
            Ok(())
        }

        /// Removes a value from the oracle.
        ///
        /// - `query_id`: Identifier of the specific data feed.
        /// - `timestamp`: The timestamp of the value to remove.
        pub fn remove_value(
            origin: OriginFor<T>,
            _query_id: QueryIdOf<T>,
            _timestamp: TimestampOf<T>,
        ) -> DispatchResult {
            // ensure origin is governance controller contract
            ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
            Ok(())
        }

        /// Allows a reporter to submit a value to the oracle.
        ///
        /// - `query_id`: Identifier of the specific data feed.
        /// - `value`: Value the user submits to the oracle.
        /// - `nonce`: The current value count for the query identifier.
        /// - `query_data`: The data used to fulfil the data query.
        pub fn submit_value(
            origin: OriginFor<T>,
            _query_id: QueryIdOf<T>,
            _value: ValueOf<T>,
            _nonce: Nonce,
            _query_data: QueryDataOf<T>,
        ) -> DispatchResult {
            let _reporter = ensure_signed(origin)?;
            Ok(())
        }

        /// Initialises a dispute/vote in the system.
        ///
        /// - `query_id`: Query identifier being disputed.
        /// - `timestamp`: Timestamp being disputed.
        pub fn begin_dispute(
            origin: OriginFor<T>,
            query_id: QueryIdOf<T>,
            timestamp: TimestampOf<T>,
        ) -> DispatchResult {
            let dispute_initiator = ensure_signed(origin)?;
            ensure!(
                StakerDetails::<T>::contains_key(&dispute_initiator),
                Error::<T>::NotStaking
            );
            ensure!(
                Reports::<T>::get(query_id).map_or(false, |r| r.timestamps.contains(&timestamp)),
                Error::<T>::NoValueExists
            );

            let dispute = DisputeOf::<T> {
                query_id,
                timestamp,
                value: <ValueOf<T>>::default(),
                dispute_reporter: dispute_initiator.clone(),
            };

            let dispute_id = <VoteCount<T>>::get();
            let query_id = [0u8; 32];
            let timestamp = 12345;
            let disputed_reporter = Address::default();
            let dispute_initiator = Address::default();

            const GAS_LIMIT: u32 = 71_000;

            let destination = T::Governance::get();
            // Balances pallet on destination chain
            let self_reserve = MultiLocation {
                parents: 0,
                interior: X1(PalletInstance(3)),
            };
            let message = xcm::transact(
                MultiAsset {
                    id: Concrete(self_reserve),
                    fun: Fungible(1_000_000_000_000_000_u128),
                },
                WeightLimit::Unlimited,
                5_000_000_000u64,
                ethereum_xcm::transact(
                    xcm::contract_address(&destination)
                        .ok_or(Error::<T>::InvalidContractAddress)?
                        .into(),
                    governance::begin_parachain_dispute(
                        T::ParachainId::get(),
                        &query_id.into(),
                        timestamp,
                        dispute_id,
                        &dispute.value,
                        disputed_reporter,
                        dispute_initiator,
                    )
                    .try_into()
                    .map_err(|_| Error::<T>::MaxEthereumXcmInputSizeExceeded)?,
                    GAS_LIMIT.into(),
                    None,
                ),
            );
            Self::send_xcm(destination, message)?;

            Ok(())
        }

        /// Enables the caller to cast a vote.
        ///
        /// - `dispute_id`: The identifier of the dispute.
        /// - `supports`: Whether the caller supports or is against the vote. None indicates the caller’s classification of the dispute as invalid.
        pub fn vote(
            origin: OriginFor<T>,
            dispute_id: DisputeIdOf<T>,
            supports: Option<bool>,
        ) -> DispatchResult {
            let voter = ensure_signed(origin)?;
            Ok(())
        }

        /// Reports a stake deposited by a reporter.
        ///
        /// - `reporter`: The reporter who deposited a stake.
        /// - `amount`: The amount staked.
        /// - `address`: The corresponding address on the controlling chain.
        pub fn report_stake_deposited(
            origin: OriginFor<T>,
            reporter: AccountIdOf<T>,
            amount: AmountOf<T>,
            address: Address,
        ) -> DispatchResult {
            // ensure origin is staking controller contract
            ensure_staking(<T as Config>::RuntimeOrigin::from(origin))?;
            Ok(())
        }

        /// Reports a staking withdrawal request by a reporter.
        ///
        /// - `reporter`: The reporter who requested a withdrawal.
        /// - `amount`: The amount requested to withdraw.
        /// - `address`: The corresponding address on the controlling chain.
        pub fn report_staking_withdraw_request(
            origin: OriginFor<T>,
            reporter: AccountIdOf<T>,
            amount: AmountOf<T>,
            address: Address,
        ) -> DispatchResult {
            // ensure origin is staking controller contract
            ensure_staking(<T as Config>::RuntimeOrigin::from(origin))?;
            Ok(())
        }

        /// Reports a stake withdrawal by a reporter.
        ///
        /// - `reporter`: The reporter who withdrew a stake.
        /// - `amount`: The total amount withdrawn.
        /// - `address`: The corresponding address on the controlling chain.
        pub fn report_stake_withdrawal(
            origin: OriginFor<T>,
            reporter: AccountIdOf<T>,
            amount: AmountOf<T>,
            address: Address,
        ) -> DispatchResult {
            // ensure origin is staking controller contract
            ensure_staking(<T as Config>::RuntimeOrigin::from(origin))?;
            Ok(())
        }

        /// Reports a slashing of a reporter.
        ///
        /// - `reporter`: The address of the slashed reporter.
        /// - `recipient`: The address of the recipient.
        /// - `amount`: The slashed amount.
        pub fn report_slash(
            origin: OriginFor<T>,
            reporter: Address,
            recipient: Address,
            amount: AmountOf<T>,
        ) -> DispatchResult {
            // ensure origin is governance controller contract
            ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
            Ok(())
        }

        pub fn report_invalid_dispute(
            origin: OriginFor<T>,
            dispute_id: DisputeIdOf<T>,
        ) -> DispatchResult {
            // ensure origin is governance controller contract
            ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
            Ok(())
        }

        pub fn slash_dispute_initiator(
            origin: OriginFor<T>,
            dispute_id: DisputeIdOf<T>,
        ) -> DispatchResult {
            // ensure origin is governance controller contract
            ensure_governance(<T as Config>::RuntimeOrigin::from(origin))?;
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn send_xcm(
            destination: impl Into<MultiLocation>,
            message: Xcm<()>,
        ) -> Result<(), Error<T>> {
            T::Xcm::send_xcm(destination, message).map_err(|e| match e {
                SendError::CannotReachDestination(..) => Error::<T>::Unreachable,
                _ => Error::<T>::SendFailure,
            })
        }

        fn store_data(query_id: QueryIdOf<T>, query_data: &QueryDataOf<T>) {
            QueryData::<T>::insert(query_id, query_data);
            Self::deposit_event(Event::QueryDataStored { query_id });
        }
    }

    /// Ensure that the origin `o` represents is the governance controller contract.
    /// Returns `Ok` if it does or an `Err` otherwise.
    fn ensure_governance<OuterOrigin>(o: OuterOrigin) -> Result<(), BadOrigin>
    where
        OuterOrigin: Into<Result<Origin, OuterOrigin>>,
    {
        match o.into() {
            Ok(Origin::Governance) => Ok(()),
            _ => Err(BadOrigin),
        }
    }

    /// Ensure that the origin `o` represents is the staking controller contract.
    /// Returns `Ok` if it does or an `Err` otherwise.
    fn ensure_staking<OuterOrigin>(o: OuterOrigin) -> Result<(), BadOrigin>
    where
        OuterOrigin: Into<Result<Origin, OuterOrigin>>,
    {
        match o.into() {
            Ok(Origin::Staking) => Ok(()),
            _ => Err(BadOrigin),
        }
    }
}

use api::autopay::*;
use api::ApiResult;
use codec::Codec;
use sp_std::vec::Vec;
use types::autopay::*;

sp_api::decl_runtime_apis! {
    pub trait AutoPayApi<AccountId: Codec, Amount: Codec, FeedId: Codec, QueryId: Codec, Timestamp: Codec>
    {
        /// Read current data feeds.
        /// # Arguments
        /// * `query_id` - Identifier of reported data.
        /// # Returns
        /// Feed identifiers for query identifier.
        fn get_current_feeds(query_id: QueryId) -> ApiResult<Vec<FeedId>>;

        /// Read current onetime tip by query identifier.
        /// # Arguments
        /// * `query_id` - Identifier of reported data.
        /// # Returns
        /// Amount of tip.
        fn get_current_tip(query_id: QueryId) -> ApiResult<Amount>;

        /// Read a specific data feed.
        /// # Arguments
        /// * `query_id` - Unique feed identifier of parameters.
        /// # Returns
        /// Details of the specified feed.
        fn get_data_feed(feed_id: FeedId) -> ApiResult<Option<FeedDetails<Amount, Timestamp>>>;

        /// Read currently funded feed details.
        /// # Arguments
        /// * `query_id` - Unique feed identifier of parameters.
        /// # Returns
        /// Details of the specified feed.
        fn get_funded_feed_details(feed_id: FeedId) -> ApiResult<Vec<FeedDetailsWithQueryData<Amount, Timestamp>>>;

        /// Read currently funded feeds.
        /// # Returns
        /// The currently funded feeds
        fn get_funded_feeds() -> ApiResult<Vec<FeedId>>;

        /// Read query identifiers with current one-time tips.
        /// # Returns
        /// Query identifiers with current one-time tips.
        fn get_funded_query_ids() -> ApiResult<Vec<QueryId>>;

        /// Read currently funded single tips with query data.
        /// # Returns
        /// The current single tips.
        fn get_funded_single_tips_info() -> ApiResult<Vec<SingleTipWithQueryData<Amount>>>;

        /// Read the number of past tips for a query identifier.
        /// # Arguments
        /// * `query_id` - Identifier of reported data.
        /// # Returns
        /// The number of past tips.
        fn get_past_tip_count(query_id: QueryId) -> ApiResult<u32>;

        /// Read the past tips for a query identifier.
        /// # Arguments
        /// * `query_id` - Identifier of reported data.
        /// # Returns
        /// All past tips.
        fn get_past_tips(query_id: QueryId) -> ApiResult<Vec<Tip<Amount, Timestamp>>>;

        /// Read a past tip for a query identifier and index.
        /// # Arguments
        /// * `query_id` - Identifier of reported data.
        /// * `index` - The index of the tip.
        /// # Returns
        /// The past tip, if found.
        fn get_past_tip_by_index(query_id: QueryId, index: u32) -> ApiResult<Option<Tip<Amount, Timestamp>>>;

        /// Look up a query identifier from a data feed identifier.
        /// # Arguments
        /// * `feed_id` - Data feed unique identifier.
        /// # Returns
        /// Corresponding query identifier, if found.
        fn get_query_id_from_feed_id(feed_id: FeedId) -> ApiResult<Option<QueryId>>;

        /// Read potential reward for a set of oracle submissions.
        /// # Arguments
        /// * `feed_id` - Data feed unique identifier.
        /// * `query_id` - Identifier of reported data.
        /// * `timestamps` - Timestamps of oracle submissions.
        /// # Returns
        /// Potential reward for a set of oracle submissions.
        fn get_reward_amount(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> ApiResult<Amount>;

        /// Read whether a reward has been claimed.
        /// # Arguments
        /// * `feed_id` - Data feed unique identifier.
        /// * `query_id` - Identifier of reported data.
        /// * `timestamp` - Timestamp of reported data.
        /// # Returns
        /// Whether a reward has been claimed, if timestamp exists.
        fn get_reward_claimed_status(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> ApiResult<Option<bool>>;

        /// Read whether rewards have been claimed.
        /// # Arguments
        /// * `feed_id` - Data feed unique identifier.
        /// * `query_id` - Identifier of reported data.
        /// * `timestamps` - Timestamps of oracle submissions.
        /// # Returns
        /// Whether rewards have been claimed.
        fn get_reward_claim_status_list(feed_id: FeedId, query_id: QueryId, timestamps: Vec<Timestamp>) -> ApiResult<Vec<Option<bool>>>;

        /// Read the total amount of tips paid by a user.
        /// # Arguments
        /// * `user` - Address of user to query.
        /// # Returns
        /// Total amount of tips paid by a user.
        fn get_tips_by_address(user: AccountId) -> ApiResult<Amount>;
    }

    pub trait OracleApi<BlockNumber: Codec, QueryId: Codec, Timestamp: Codec> where
    {
        /// Returns the block number at a given timestamp.
        /// # Arguments
        /// * `query_id` - The identifier of the specific data feed.
        /// * `timestamp` - The timestamp to find the corresponding block number for.
        /// # Returns
        /// Block number of the timestamp for the given query identifier and timestamp, if found.
        fn get_block_number_by_timestamp(query_id: QueryId, timestamp: Timestamp) -> ApiResult<Option<BlockNumber>>;

        // todo: add remaining functions
    }

    pub trait GovernanceApi<AccountId: Codec, DisputeId: Codec, QueryId: Codec, Timestamp: Codec> where
    {
        /// Determines if an account voted for a specific dispute.
        /// # Arguments
        /// * `dispute_id` - The identifier of the dispute.
        /// * `voter` - The account of the voter to check.
        /// # Returns
        /// Whether or not the account voted for the specific dispute.
        fn did_vote(dispute_id: DisputeId, voter: AccountId) -> ApiResult<Option<bool>>;

        // todo: add remaining functions
    }
}

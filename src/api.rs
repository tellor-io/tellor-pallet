use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use sp_std::vec::Vec;

#[derive(Eq, PartialEq, Encode, Decode, RuntimeDebug)]
pub enum ApiError {}

pub(crate) type ApiResult<T> = Result<T, ApiError>;

pub(crate) mod autopay {
    use super::*;
    use crate::types::autopay::*;

    #[derive(Encode, Decode)]
    pub struct FeedDetailsWithQueryData<Amount, Timestamp> {
        /// Feed details for feed identifier with funding.
        details: FeedDetails<Amount, Timestamp>,
        /// Query data for requested data
        query_data: Vec<u8>,
    }

    #[derive(Encode, Decode)]
    pub struct SingleTipWithQueryData<Amount> {
        /// Query data with single tip for requested data.
        query_data: Vec<u8>,
        /// Reward amount for request.
        tip: Amount,
    }
}

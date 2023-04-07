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

use codec::{Decode, Encode};
use sp_std::vec::Vec;
use tellor::FeedDetails;

#[derive(Encode, Debug, Decode, Eq, PartialEq)]
pub struct FeedDetailsWithQueryData<Amount> {
	/// Feed details for feed identifier with funding.
	pub details: FeedDetails<Amount>,
	/// Query data for requested data
	pub query_data: Vec<u8>,
}

#[derive(Encode, Debug, Decode, Eq, PartialEq)]
pub struct SingleTipWithQueryData<Amount> {
	/// Query data with single tip for requested data.
	pub query_data: Vec<u8>,
	/// Reward amount for request.
	pub tip: Amount,
}

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
use scale_info::TypeInfo;

#[derive(Encode, Debug, Decode, Eq, PartialEq, TypeInfo)]
pub struct VoteInfo<Balance, BlockNumber, Timestamp> {
	pub vote_round: u8,
	pub start_date: Timestamp,
	pub block_number: BlockNumber,
	pub fee: Balance,
	pub tally_date: Timestamp,
	pub users_does_support: Balance,
	pub users_against: Balance,
	pub users_invalid_query: Balance,
	pub reporters_does_support: u128,
	pub reporters_against: u128,
	pub reporters_invalid_query: u128,
}

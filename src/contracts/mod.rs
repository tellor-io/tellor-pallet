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

use crate::types::ParaId;
pub(crate) use ethabi::{encode, Token};
use sp_core::{H160, U256};
use sp_std::{vec, vec::Vec};

pub(crate) type Abi = ethabi::Token;

pub(crate) mod governance;
pub(crate) mod registry;
pub(crate) mod staking;

fn call(function: &[u8; 4], mut parameters: Vec<u8>) -> Vec<u8> {
	let mut encoded = function.to_vec();
	encoded.append(parameters.as_mut());
	encoded
}

pub(crate) mod gas_limits {
	// Static gas limits, based on max gas from `forge test --gas-report` of contracts
	pub(crate) const BEGIN_PARACHAIN_DISPUTE: u64 = 600_000;
	pub(crate) const CONFIRM_STAKING_WITHDRAW_REQUEST: u64 = 60_000;
	pub(crate) const DEREGISTER: u64 = 30_000;
	pub(crate) const REGISTER: u64 = 95_000;
	pub(crate) const VOTE: u64 = 150_000;
}

#[cfg(test)]
pub(crate) mod tests {
	use ethabi::{Param, ParamType};

	// Helper for creating a parameter
	pub(crate) fn param(name: &str, kind: ParamType) -> Param {
		Param { name: name.to_string(), kind, internal_type: None }
	}
}

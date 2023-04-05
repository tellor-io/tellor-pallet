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

use super::*;

pub(crate) fn confirm_parachain_stake_withdraw_request(
	address: impl Into<Address>,
	amount: impl Into<Amount>,
) -> Vec<u8> {
	const FUNCTION: [u8; 4] = [116, 48, 87, 226];
	Call::new(&FUNCTION).address(address.into()).uint(amount.into()).encode()
}

#[cfg(test)]
mod tests {
	use super::{super::tests::*, Address};
	use ethabi::{Function, ParamType, Token};

	#[allow(deprecated)]
	fn confirm_parachain_stake_withdraw_request() -> Function {
		// confirmParachainStakeWithdrawRequest(address,uint256)
		Function {
			name: "confirmParachainStakeWithdrawRequest".to_string(),
			inputs: vec![
				param("_staker", ParamType::Address),
				param("_amount", ParamType::Uint(256)),
			],
			outputs: vec![],
			constant: None,
			state_mutability: Default::default(),
		}
	}

	#[test]
	#[ignore]
	fn confirm_parachain_stake_withdraw_request_function_selector() {
		// Short signature bytes used for FUNCTION const
		let function = confirm_parachain_stake_withdraw_request();
		println!("{} {:?}", function.signature(), function.short_signature());
	}

	#[test]
	fn encodes_confirm_parachain_stake_withdraw_request_call() {
		let staker = Address::random();
		let amount = 1675711956967u128;

		assert_eq!(
			confirm_parachain_stake_withdraw_request()
				.encode_input(&vec![Token::Address(staker), Token::Uint(amount.into()),])
				.unwrap()[..],
			super::confirm_parachain_stake_withdraw_request(staker, amount)[..]
		)
	}
}

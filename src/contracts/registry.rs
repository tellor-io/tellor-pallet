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

pub(crate) fn register(
	para_id: ParaId,
	pallet_index: u8,
	stake_amount: impl Into<U256>,
) -> Vec<u8> {
	const FUNCTION: [u8; 4] = [40, 162, 149, 29];
	Call::new(&FUNCTION)
		.uint(para_id)
		.uint(pallet_index)
		.uint(stake_amount)
		.encode()
}

pub(crate) fn deregister() -> Vec<u8> {
	const FUNCTION: [u8; 4] = [175, 245, 237, 177];
	FUNCTION.to_vec()
}

#[cfg(test)]
mod tests {
	use super::super::tests::*;
	use ethabi::{Function, ParamType, Token};

	#[allow(deprecated)]
	fn register() -> Function {
		// register(uint32,uint8,uint256)
		Function {
			name: "register".to_string(),
			inputs: vec![
				param("_paraId", ParamType::Uint(32)),
				param("_palletIndex", ParamType::Uint(8)),
				param("_stakeAmount", ParamType::Uint(256)),
			],
			outputs: vec![],
			constant: None,
			state_mutability: Default::default(),
		}
	}

	#[test]
	#[ignore]
	fn register_function_selector() {
		// Short signature bytes used for FUNCTION const
		let function = register();
		println!("{} {:?}", function.signature(), function.short_signature());
	}

	#[test]
	fn encodes_register_call() {
		let para_id = 3000;
		let pallet_index = 3;
		let stake_amount = 1675711956967u128;

		assert_eq!(
			register()
				.encode_input(&vec![
					Token::Uint(para_id.into()),
					Token::Uint(pallet_index.into()),
					Token::Uint(stake_amount.into()),
				])
				.unwrap()[..],
			super::register(para_id, pallet_index, stake_amount)[..]
		)
	}

	#[allow(deprecated)]
	fn deregister() -> Function {
		// deregister()
		Function {
			name: "deregister".to_string(),
			inputs: vec![],
			outputs: vec![],
			constant: None,
			state_mutability: Default::default(),
		}
	}

	#[test]
	#[ignore]
	fn deregister_function_selector() {
		// Short signature bytes used for FUNCTION const
		let function = deregister();
		println!("{} {:?}", function.signature(), function.short_signature());
	}

	#[test]
	fn encodes_deregister_call() {
		assert_eq!(deregister().encode_input(&vec![]).unwrap()[..], super::deregister()[..])
	}
}

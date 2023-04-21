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

pub(crate) fn register(para_id: ParaId, pallet_index: u8) -> Vec<u8> {
	call(
		&[20, 1, 238, 43],
		encode(&vec![Token::Uint(para_id.into()), Token::Uint(pallet_index.into())]),
	)
}

pub(crate) fn deregister() -> Vec<u8> {
	[175, 245, 237, 177].to_vec()
}

#[cfg(test)]
mod tests {
	use super::super::tests::*;
	use ethabi::{Function, ParamType, Token};

	#[allow(deprecated)]
	fn register() -> Function {
		// register(uint32,uint8)
		Function {
			name: "register".to_string(),
			inputs: vec![
				param("_paraId", ParamType::Uint(32)),
				param("_palletIndex", ParamType::Uint(8)),
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

		assert_eq!(
			register()
				.encode_input(&vec![Token::Uint(para_id.into()), Token::Uint(pallet_index.into()),])
				.unwrap()[..],
			super::register(para_id, pallet_index)[..]
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

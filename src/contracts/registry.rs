use super::*;

pub(crate) fn register(
	para_id: ParaId,
	pallet_index: u8,
	stake_amount: impl Into<Amount>,
) -> Vec<u8> {
	const FUNCTION: [u8; 4] = [40, 162, 149, 29];
	Call::new(&FUNCTION)
		.uint(para_id)
		.uint(pallet_index)
		.uint(stake_amount)
		.encode()
}

#[cfg(test)]
mod tests {
	use super::{super::tests::*, Address};
	use ethabi::{Function, ParamType, Token};

	fn register() -> Function {
		// register(uint32,uint8,uint256)
		ethabi::Function {
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
	fn function_selector() {
		// Short signature bytes used for FUNCTION const
		let function = register();
		println!("{} {:?}", function.signature(), function.short_signature());
	}

	#[test]
	fn encodes_register() {
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
}

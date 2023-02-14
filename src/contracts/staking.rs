use super::*;

fn confirm_parachain_staking_withdraw_request(
	para_id: ParaId,
	reporter: Address,
	amount: impl Into<Amount>,
) -> Vec<u8> {
	const FUNCTION: [u8; 4] = [141, 45, 83, 196];
	Call::new(&FUNCTION).uint(para_id).address(reporter).uint(amount).encode()
}

#[cfg(test)]
mod tests {
	use super::{super::tests::*, Address};
	use ethabi::{Function, ParamType, Token};

	fn confirm_parachain_staking_withdraw_request() -> Function {
		// confirmParachainStakingWithdrawRequest(uint32,address,uint256)
		ethabi::Function {
			name: "confirmParachainStakingWithdrawRequest".to_string(),
			inputs: vec![
				param("_paraId", ParamType::Uint(32)),
				param("_reporter", ParamType::Address),
				param("_timestamp", ParamType::Uint(256)),
			],
			outputs: vec![],
			constant: None,
			state_mutability: Default::default(),
		}
	}

	#[test]
	#[ignore]
	fn function_selector() {
		// Short signature bytes used for FUNCTION const
		let function = confirm_parachain_staking_withdraw_request();
		println!("{} {:?}", function.signature(), function.short_signature());
	}

	#[test]
	fn encodes_begin_parachain_dispute() {
		let para_id = 3000;
		let reporter = Address::random();
		let amount = 1675711956967u128;

		assert_eq!(
			confirm_parachain_staking_withdraw_request()
				.encode_input(&vec![
					Token::Uint(para_id.into()),
					Token::Address(reporter),
					Token::Uint(amount.into()),
				])
				.unwrap()[..],
			super::confirm_parachain_staking_withdraw_request(para_id, reporter, amount)[..]
		)
	}
}

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

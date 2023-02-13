use super::*;

pub(crate) fn begin_parachain_dispute(
	para_id: ParaId,
	query_id: &[u8; 32],
	timestamp: impl Into<U256>,
	dispute_id: impl Into<U256>,
	value: &[u8],
	disputed_reporter: Address,
	dispute_initiator: Address,
) -> Vec<u8> {
	const FUNCTION: [u8; 4] = [40, 254, 222, 231];

	Call::new(&FUNCTION)
		.uint(para_id)
		.fixed_bytes(query_id)
		.uint(timestamp)
		.uint(dispute_id)
		.bytes(value)
		.address(disputed_reporter)
		.address(dispute_initiator)
		.encode()
}

#[cfg(test)]
mod tests {
	use super::{super::tests::*, *};
	use ethabi::{Function, ParamType, Token};
	use sp_core::keccak_256;

	fn begin_parachain_dispute() -> Function {
		// beginParachainDispute(uint32,bytes32,uint256,uint256,bytes,address,address)
		ethabi::Function {
			name: "beginParachainDispute".to_string(),
			inputs: vec![
				param("_paraId", ParamType::Uint(32)),
				param("_queryId", ParamType::FixedBytes(32)),
				param("_timestamp", ParamType::Uint(256)),
				param("_disputeId", ParamType::Uint(256)),
				param("_value", ParamType::Bytes),
				param("_disputedReporter", ParamType::Address),
				param("_disputeInitiator", ParamType::Address),
			],
			outputs: vec![],
			constant: None,
			state_mutability: Default::default(),
		}
	}

	#[test]
	fn encodes_begin_parachain_dispute() {
		// Short signature bytes used for FUNCTION const
		let function = begin_parachain_dispute();
		println!("{} {:?}", function.signature(), function.short_signature());
	}

	#[test]
	fn encode_begin_parachain_dispute() {
		let para_id = 3000;
		let query_id = keccak_256("my_query".as_bytes());
		let timestamp = 1675711956967u64;
		let dispute_id = 1;
		let value = [
			0u8, 65, 242, 124, 97, 37, 67, 41, 189, 109, 132, 185, 252, 136, 215, 37, 101, 25, 113,
			126, 143, 68, 226, 21, 52, 30, 20, 190, 109, 250, 166, 10, 71, 121, 118, 208, 186, 68,
			115, 103, 116, 24, 76, 18, 145, 31, 14, 132, 213, 146, 98, 184, 227, 250, 43, 5, 1, 73,
			97, 130, 5,
		];
		let disputed_reporter = Address::random();
		let dispute_initiator = Address::random();

		assert_eq!(
			begin_parachain_dispute()
				.encode_input(&vec![
					Token::Uint(para_id.into()),
					Token::FixedBytes(query_id.into()),
					Token::Uint(timestamp.into()),
					Token::Uint(dispute_id.into()),
					Token::Bytes(value.into()),
					Token::Address(disputed_reporter),
					Token::Address(dispute_initiator)
				])
				.unwrap()[..],
			super::begin_parachain_dispute(
				para_id,
				&query_id,
				timestamp,
				1,
				&value,
				disputed_reporter,
				dispute_initiator
			)[..]
		)
	}
}

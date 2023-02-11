use crate::types::{Address, Amount, ParaId};
use sp_core::U256;
use sp_std::{vec, vec::Vec};

pub(crate) mod governance;
pub(crate) mod registry;
pub(crate) mod staking;

struct Call<'a> {
	function: Vec<u8>,
	parameters: Vec<Parameter<'a>>,
}

impl<'a> Call<'a> {
	fn new(function: &[u8; 4]) -> Self {
		Call { function: function.to_vec(), parameters: Vec::new() }
	}

	fn address(mut self, address: Address) -> Self {
		let mut encoded = [0u8; 32];
		encoded[12..].copy_from_slice(address.as_fixed_bytes());
		self.parameters.push(Parameter::Static(encoded));
		self
	}

	fn bytes(mut self, bytes: &'a [u8]) -> Self {
		self.parameters.push(Parameter::Dynamic(DynamicParameter::Bytes(bytes)));
		self
	}

	fn fixed_bytes(mut self, bytes: &[u8]) -> Self {
		let mut encoded = [0u8; 32];
		encoded.copy_from_slice(bytes);
		self.parameters.push(Parameter::Static(encoded));
		self
	}

	fn uint(mut self, value: impl Into<U256>) -> Self {
		let mut encoded = [0u8; 32];
		value.into().to_big_endian(&mut encoded);
		self.parameters.push(Parameter::Static(encoded));
		self
	}

	fn encode(mut self) -> Vec<u8> {
		let mut buffer = [0u8; 32];

		// Add head parts: https://docs.soliditylang.org/en/latest/abi-spec.html#function-selector-and-argument-encoding
		for parameter in &self.parameters {
			match parameter {
				Parameter::Static(parameter) => self.function.extend(parameter),
				Parameter::Dynamic(parameter) => match parameter {
					// https://docs.soliditylang.org/en/latest/abi-spec.html#use-of-dynamic-types
					DynamicParameter::Bytes(_) => {
						// offset in bytes to start of data area
						U256::from(self.parameters.len() * 32).to_big_endian(&mut buffer);
						self.function.extend(buffer);
					},
				},
			}
		}

		// Add dynamic payloads
		for parameter in self.parameters {
			if let Parameter::Dynamic(parameter) = parameter {
				match parameter {
					DynamicParameter::Bytes(parameter) => {
						// Define length
						U256::from(parameter.len()).to_big_endian(&mut buffer);
						self.function.extend(buffer);

						// Add data, padding to 32 bytes
						self.function.extend(parameter.iter());
						self.function.extend(vec![0; ((parameter.len() + 31) / 32) as usize + 1]);
					},
				}
			}
		}

		self.function
	}
}

enum Parameter<'a> {
	Static([u8; 32]),
	Dynamic(DynamicParameter<'a>),
}

enum DynamicParameter<'a> {
	Bytes(&'a [u8]),
}

#[cfg(test)]
pub(crate) mod tests {
	use super::Call;
	use crate::types::Address;
	use ethabi::{encode, Param, ParamType, Token};
	use sp_core::U256;

	// Helper for creating a parameter
	pub(crate) fn param(name: &str, kind: ParamType) -> Param {
		Param { name: name.to_string(), kind, internal_type: None }
	}

	#[test]
	fn encodes_address() {
		let address = Address::random();
		assert_eq!(
			encode(&[Token::Address(address)])[..],
			Call::new(&[0; 4]).address(address).encode()[4..]
		);
	}

	#[test]
	fn encodes_uint() {
		let value = 12345u128;
		assert_eq!(
			encode(&[Token::Uint(U256::from(value))])[..],
			Call::new(&[0; 4]).uint(value).encode()[4..]
		);
	}
}

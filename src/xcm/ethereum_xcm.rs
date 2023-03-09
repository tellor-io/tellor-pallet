use frame_support::{dispatch::Encode, traits::ConstU32, BoundedVec};
use scale_info::TypeInfo;
use sp_core::{H160, H256, U256};
use sp_std::vec::Vec;

/// Max. allowed size of 65_536 bytes.
pub(crate) const MAX_ETHEREUM_XCM_INPUT_SIZE: u32 = 2u32.pow(16);

// The fixed index of `pallet-ethereum-xcm` within various runtimes.
#[derive(Clone, Eq, PartialEq, Encode)]
#[allow(dead_code)]
pub enum EthereumXcm {
	#[codec(index = 38u8)]
	Moonbase(EthereumXcmCall),
}

// The fixed index of calls available within `pallet-ethereum-xcm`.
#[derive(Clone, Eq, PartialEq, Encode)]
#[allow(dead_code)]
pub enum EthereumXcmCall {
	#[codec(index = 0u8)]
	Transact { xcm_transaction: EthereumXcmTransaction },
	#[codec(index = 1u8)]
	TransactThroughProxy { transact_as: H160, xcm_transaction: EthereumXcmTransaction },
}

// Various helper types from https://github.com/PureStake/moonbeam/tree/master/pallets/ethereum-xcm to ease transact call encoding.
#[derive(Clone, Debug, Eq, PartialEq, Encode, TypeInfo)]
pub struct ManualEthereumXcmFee {
	pub gas_price: Option<U256>,
	pub max_fee_per_gas: Option<U256>,
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, TypeInfo)]
#[allow(dead_code)]
pub enum EthereumXcmFee {
	Manual(ManualEthereumXcmFee),
	Auto,
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, TypeInfo)]
#[allow(dead_code)]
pub enum EthereumXcmTransaction {
	V1(EthereumXcmTransactionV1),
	V2(EthereumXcmTransactionV2),
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, TypeInfo)]
pub struct EthereumXcmTransactionV1 {
	pub gas_limit: U256,
	pub fee_payment: EthereumXcmFee,
	pub action: TransactionAction,
	pub value: U256,
	pub input: BoundedVec<u8, ConstU32<MAX_ETHEREUM_XCM_INPUT_SIZE>>,
	pub access_list: Option<Vec<(H160, Vec<H256>)>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, TypeInfo)]
pub struct EthereumXcmTransactionV2 {
	pub gas_limit: U256,
	pub action: TransactionAction,
	pub value: U256,
	pub input: BoundedVec<u8, ConstU32<MAX_ETHEREUM_XCM_INPUT_SIZE>>,
	pub access_list: Option<Vec<(H160, Vec<H256>)>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, TypeInfo)]
pub enum TransactionAction {
	Call(H160),
}

pub(crate) fn transact(
	contract_address: impl Into<H160>,
	call_data: BoundedVec<u8, ConstU32<MAX_ETHEREUM_XCM_INPUT_SIZE>>,
	gas_limit: U256,
	value: Option<U256>,
) -> Vec<u8> {
	EthereumXcm::Moonbase(EthereumXcmCall::Transact {
		xcm_transaction: EthereumXcmTransaction::V2(EthereumXcmTransactionV2 {
			gas_limit,
			action: TransactionAction::Call(contract_address.into()),
			value: value.unwrap_or(U256::zero()),
			input: call_data,
			access_list: None,
		}),
	})
	.encode()
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;
	use sp_core::bytes::from_hex;

	#[test]
	fn encodes_transact() {
		let contract_address: H160 =
			H160::from_slice(&from_hex("0xa72f549a1a12b9b49f30a7f3aeb1f4e96389c5d8").unwrap());
		let evm_call_data = from_hex("0xd09de08a").unwrap().try_into().unwrap();
		let call = transact(contract_address, evm_call_data, 71_000.into(), None);
		assert_eq!(from_hex("0x260001581501000000000000000000000000000000000000000000000000000000000000a72f549a1a12b9b49f30a7f3aeb1f4e96389c5d8000000000000000000000000000000000000000000000000000000000000000010d09de08a00").unwrap(),
                   call);
	}
}

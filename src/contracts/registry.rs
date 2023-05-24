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
use crate::types::Weights;
use codec::Encode;
use ethabi::Token;
use xcm::latest::{Junction, MultiLocation, NetworkId};

pub(crate) fn register(
	para_id: ParaId,
	pallet_index: u8,
	weight_to_fee: u128,
	fee_location: MultiLocation,
	weights: &Weights,
) -> Vec<u8> {
	call(
		&[96, 114, 55, 127],
		encode(&[
			Token::Uint(para_id.into()),
			Token::Uint(pallet_index.into()),
			Token::Uint(weight_to_fee.into()),
			Token::Tuple(vec![
				Token::Uint(fee_location.parents.into()),
				Token::Array(
					fee_location
						.interior
						.into_iter()
						.map(|j| Token::Bytes(encode_junction(j)))
						.collect(),
				),
			]),
			Token::Tuple(vec![
				Token::Uint(weights.report_stake_deposited.into()),
				Token::Uint(weights.report_staking_withdraw_request.into()),
				Token::Uint(weights.report_stake_withdrawn.into()),
				Token::Uint(weights.report_vote_tallied.into()),
				Token::Uint(weights.report_vote_executed.into()),
				Token::Uint(weights.report_slash.into()),
			]),
		]),
	)
}

fn encode_junction(junction: Junction) -> Vec<u8> {
	// Based on https://github.com/PureStake/moonbeam/blob/v0.31.1/precompiles/utils/src/data/xcm.rs#L262
	match junction {
		Junction::Parachain(para_id) => {
			let mut encoded = vec![0];
			encoded.extend(para_id.to_be_bytes());
			encoded
		},
		Junction::AccountId32 { network, id } => {
			let mut encoded = vec![1];
			encoded.extend(id);
			encoded.extend(encode_network_id(network));
			encoded
		},
		Junction::AccountIndex64 { network, index } => {
			let mut encoded = vec![2];
			encoded.extend(index.to_be_bytes());
			encoded.extend(encode_network_id(network));
			encoded
		},
		Junction::AccountKey20 { network, key } => {
			let mut encoded = vec![3];
			encoded.extend(key);
			encoded.extend(network.encode());
			encoded
		},
		Junction::PalletInstance(i) => {
			let mut encoded = vec![4];
			encoded.extend(i.to_be_bytes());
			encoded
		},
		Junction::GeneralIndex(i) => {
			let mut encoded = vec![5];
			encoded.extend(i.to_be_bytes());
			encoded
		},
		Junction::GeneralKey { length, data } => {
			let mut encoded = vec![6];
			encoded.push(length);
			encoded.extend(data);
			encoded
		},
		Junction::OnlyChild => vec![7],
		Junction::GlobalConsensus(network_id) => {
			let mut encoded = vec![9];
			encoded.extend(encode_network_id(Some(network_id)));
			encoded
		},
		_ => unreachable!("Junction::Plurality not supported yet"),
	}
}

fn encode_network_id(network_id: Option<NetworkId>) -> Vec<u8> {
	// Based on https://github.com/PureStake/moonbeam/blob/b71106f45100eb70bb21404cb48db1e73ed448d7/precompiles/utils/src/data/xcm.rs#L44
	match network_id {
		None => vec![0],
		Some(network_id) => match network_id {
			NetworkId::ByGenesis(id) => {
				let mut encoded = vec![1];
				encoded.extend(id);
				encoded
			},
			NetworkId::Polkadot => vec![2, 2],
			NetworkId::Kusama => vec![3, 3],
			NetworkId::ByFork { block_number, block_hash } => {
				let mut encoded = vec![4, 1];
				encoded.extend(block_number.to_be_bytes());
				encoded.extend(block_hash);
				encoded
			},
			NetworkId::Westend => vec![5, 4],
			NetworkId::Rococo => vec![6, 5],
			NetworkId::Wococo => vec![7, 6],
			NetworkId::Ethereum { chain_id } => {
				let mut encoded = vec![8, 7];
				encoded.extend(chain_id.to_be_bytes());
				encoded
			},
			NetworkId::BitcoinCore => vec![9, 8],
			NetworkId::BitcoinCash => vec![10, 9],
		},
	}
}

#[cfg(test)]
mod tests {
	use super::super::tests::*;
	use crate::{
		contracts::registry::{encode_junction, encode_network_id},
		types::Weights,
	};
	use ethabi::{Function, ParamType, Token};
	use sp_core::{bytes::from_hex, H160, H256};
	use xcm::latest::prelude::*;

	#[allow(deprecated)]
	fn register() -> Function {
		// register(uint32,uint8,uint256,uint8,(uint8,bytes[]))
		Function {
			name: "register".to_string(),
			inputs: vec![
				param("_paraId", ParamType::Uint(32)),
				param("_palletIndex", ParamType::Uint(8)),
				param("_weightToFee", ParamType::Uint(256)),
				param(
					"_feeLocation",
					ParamType::Tuple(vec![
						ParamType::Uint(8),                           // parents
						ParamType::Array(Box::new(ParamType::Bytes)), // interior
					]),
				),
				param(
					"_weights",
					ParamType::Tuple(vec![
						ParamType::Uint(64),
						ParamType::Uint(64),
						ParamType::Uint(64),
						ParamType::Uint(64),
						ParamType::Uint(64),
						ParamType::Uint(64),
					]),
				),
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
		let weight_to_fee = 10_000;
		let fee_location = MultiLocation::new(1, X1(Parachain(3000))); // fee location for execution on this parachain, from context of evm parachain
		let weights = Weights {
			report_stake_deposited: 1200000,
			report_staking_withdraw_request: 1000000,
			report_stake_withdrawn: 1500000,
			report_vote_tallied: 3500000,
			report_vote_executed: 2500000,
			report_slash: 1000000,
		};

		assert_eq!(
			register()
				.encode_input(&vec![
					Token::Uint(para_id.into()),
					Token::Uint(pallet_index.into()),
					Token::Uint(weight_to_fee.into()),
					Token::Tuple(vec![
						Token::Uint(1.into()),
						Token::Array(vec![Token::Bytes(encode_junction(Parachain(3000)))])
					]),
					Token::Tuple(vec![
						Token::Uint(weights.report_stake_deposited.into()),
						Token::Uint(weights.report_staking_withdraw_request.into()),
						Token::Uint(weights.report_stake_withdrawn.into()),
						Token::Uint(weights.report_vote_tallied.into()),
						Token::Uint(weights.report_vote_executed.into()),
						Token::Uint(weights.report_slash.into()),
					])
				])
				.unwrap()[..],
			super::register(para_id, pallet_index, weight_to_fee, fee_location, &weights)[..]
		)
	}

	#[test]
	fn encodes_junctions() {
		let id = H256::random().0;
		let key = H160::random().0;
		let x: Vec<(Junction, Vec<u8>)> = vec![
			(Parachain(2023), from_hex("0x00000007E7").unwrap()),
			(
				AccountId32 { network: None, id },
				from_hex(&format!("0x01{}00", hex::encode(id))).unwrap(),
			),
			(
				AccountIndex64 { network: None, index: u64::MAX },
				from_hex(&format!("0x02{}00", hex::encode(u64::MAX.to_be_bytes()))).unwrap(),
			),
			(
				AccountKey20 { network: None, key },
				from_hex(&format!("0x03{}00", hex::encode(key))).unwrap(),
			),
			(PalletInstance(3), from_hex("0x0403").unwrap()),
			(
				GeneralIndex(u128::MAX),
				from_hex(&format!("0x05{}", hex::encode(u128::MAX.to_be_bytes()))).unwrap(),
			),
			(
				GeneralKey { length: id.len() as u8, data: id },
				from_hex(&format!(
					"0x06{}{}",
					hex::encode((id.len() as u8).to_be_bytes()),
					hex::encode(id)
				))
				.unwrap(),
			),
			(OnlyChild, from_hex("0x07").unwrap()),
			(
				GlobalConsensus(Polkadot),
				from_hex(&format!("0x09{}", hex::encode(encode_network_id(Some(Polkadot)))))
					.unwrap(),
			),
		];

		for (source, expected) in x {
			assert_eq!(encode_junction(source), expected);
		}
	}

	#[test]
	fn encodes_network_id() {
		let x: Vec<(Option<NetworkId>, Vec<u8>)> = vec![(None, from_hex("0x0").unwrap())];

		for (source, expected) in x {
			assert_eq!(encode_network_id(source), expected);
		}
	}
}

# Tellor Pallet

[![Check Set-Up & Build](https://github.com/tellor-io/tellor-pallet/actions/workflows/check.yml/badge.svg?branch=main)](https://github.com/tellor-io/tellor-pallet/actions/workflows/check.yml)
[![Run Tests](https://github.com/tellor-io/tellor-pallet/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/tellor-io/tellor-pallet/actions/workflows/test.yml)
[![Discord Chat](https://img.shields.io/discord/461602746336935936)](https://discord.gg/tellor)
[![Twitter Follow](https://img.shields.io/twitter/follow/wearetellor?style=social)](https://twitter.com/WeAreTellor)

## Overview
This pallet introduces Tellor oracle functionality to parachains, controlled by Tellor staking and governance smart contracts on another EVM smart contract enabled parachain.

A parachain first registers itself with the controller contracts. After that, anyone can request oracle data to be reported to the parachain by either creating a 'tip' for a onetime report, or by creating and funding a feed for recurring reports.
Reporters are required to first stake Tellor Tributes (TRB) into the Tellor parachain staking contract before they can begin reporting directly to the parachain.

Anyone can then dispute a reported value, provided they lock a dispute fee.
Votes are collated and sent to the Tellor parachain governance contract for tallying and execution.
A successful dispute results in a disputed reporter being slashed and the slash amount being awarded to the dispute initiator.
The dispute fee is awarded to the disputed reporter if the dispute is unsuccessful, but given back to initiator if tallied and executed as an invalid dispute.

### Terminology
- Data Feed: a request for recurring reports to the oracle.
- Dispute: a challenge on a reported value.
- Dispute Fee: a fee paid on the parachain in order to dispute a value.
- Origins:
    - Staking: the staking controller contract
    - Governance: the governance controller contract
- Query Data: tells reporters how to fulfil a data query. See https://github.com/tellor-io/dataSpecs/ for examples.
- Query Id: the `keccak256` hash of the query data.
- Reporter (Staker): anyone that has staked the required amount of TRB into the staking contract.
- Slash Amount: amount slashed from a reporter if a dispute is successful.
- Tip: a reward for a onetime request for an oracle report.

## Interface

### Dispatchable Functions

#### For Users
- `add_staking_rewards` - Funds the pallet with staking rewards, which can be used to incentivize oracle usage.
- `begin_dispute` - Initialises a dispute/vote in the system. Requires a dispute fee to be paid.
- `fund_feed` - Allows a data feed to be funded with tokens.
- `send_votes` - Sends any dispute votes to the governance controller contract for tallying, provided the voting period hasn't elapsed.
- `setup_data_feed` - Initializes a data feed for recurring reports.
- `tip` - Adds a tip for a onetime request.
- `update_stake_amount` - Updates the stake amount after retrieving the latest token price from oracle.
- `vote` - Enables the caller to cast a vote. Only votes from oracle users and reporters are counted.
- `vote_on_multiple_disputes` - Enables the caller to cast votes for multiple disputes.

#### For Reporters
- `claim_onetime_tip` - Function to claim tips for onetime requests, in batches.
- `claim_tip` - Allows Tellor reporters to claim their data feed tips in batches.
- `submit_value` - Allows a reporter to submit a value to the oracle.

#### For Controller Contracts
- Staking:
    - `report_stake_deposited` - Reports a stake deposited by a reporter.
    - `report_staking_withdraw_request` - Reports a staking withdrawal request by a reporter.
    - `report_stake_withdrawn` - Reports a stake withdrawal by a reporter.
- Governance:
    - `report_slash` - Reports a slashing of a reporter.
    - `report_vote_executed` - Reports the execution of a vote.
    - `report_vote_tallied` - Reports the tally of a vote.

#### Root Calls
- `register` - Registers the parachain with the controller contracts.


## Setup Environment & Run Tests
### Option 1: Run tests using local environment
- install Rust as per https://docs.substrate.io/install/
- install `cargo-nextest` to local environment:
```shell
cargo install cargo-nextest --locked
```
- run the `pallet` tests using the command:
```shell
cargo nextest run --all --release
```
- run the `runtime-api` tests using the command:
```shell
cd ./runtime-api/ && cargo nextest run --all --release
```
### Option 2: Docker
Run tests in Docker container:
- [Install Docker](https://docs.docker.com/get-docker/)
- Allocate at least 8GB of RAM to Docker, 3GB swap space, or you'll get out of memory errors
- Build and run the `tellor-pallet-tests` image defined in `Dockerfile` using the command:
```shell
docker build -t tellor-pallet-tests . && docker run --rm tellor-pallet-tests
```
- Build and run the `tellor-runtime-api-tests` image defined in `Dockerfile.runtime-api` using the command:
```shell
docker build -f Dockerfile.runtime-api -t tellor-runtime-api-tests . && docker run --rm tellor-runtime-api-tests
```

License: GPL-3.0
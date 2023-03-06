# Tellor

[![Check Set-Up & Build](https://github.com/evilrobot-01/substrate-pallets/actions/workflows/check.yml/badge.svg?branch=tellor)](https://github.com/evilrobot-01/substrate-pallets/actions/workflows/check.yml)
[![Run Tests](https://github.com/evilrobot-01/substrate-pallets/actions/workflows/test.yml/badge.svg?branch=tellor)](https://github.com/evilrobot-01/substrate-pallets/actions/workflows/test.yml)

License: Unlicense

## Porting Progress

### AutoPay

| Dispatchable Function                  |    Functional Test     | Notes                                                          |
|----------------------------------------|:----------------------:|----------------------------------------------------------------|
| :white_check_mark: `claim_onetime_tip` | :white_square_button:  | Waiting on source tests to be updated to check returned errors |
| :white_check_mark: `claim_tip`         |   :white_check_mark:   |                                                                |
| :white_check_mark: `fund_feed`         |   :white_check_mark:   |                                                                |
| :white_check_mark: `setup_data_feed`   |   :white_check_mark:   |                                                                |
| :white_check_mark: `tip`               |   :white_check_mark:   |                                                                |

### Tellor Flex

| Dispatchable Function                |    Functional Test     | Notes                                     |
|--------------------------------------|:----------------------:|-------------------------------------------|
| :white_square_button: `remove_value` | :white_square_button:  |                                           |
| :heavy_check_mark: `submit_value`    | :white_square_button:  | Implemented apart from time-based rewards |

### Governance

| Dispatchable Function                 |    Functional Test     | Notes |
|---------------------------------------|:----------------------:|-------|
| :white_square_button: `begin_dispute` | :white_square_button:  |       |
| :white_square_button: `vote`          | :white_square_button:  |       |


## Controller Contract Integration

| Dispatchable Function                                   |    Functional Test    | Notes |
|---------------------------------------------------------|:---------------------:|-------|
| :white_square_button: `register`                        | :white_square_button: |       |
| :white_square_button: `report_stake_deposited`          | :white_square_button: |       |
| :white_square_button: `report_staking_withdraw_request` | :white_square_button: |       |
| :white_square_button: `report_stake_withdrawn`          | :white_square_button: |       |
| :white_square_button: `report_slash`                    | :white_square_button: |       |
| :white_square_button: `report_invalid_dispute`          | :white_square_button: |       |
| :white_square_button: `slash_dispute_initiator`         | :white_square_button: |       |
| :white_square_button: `deregister`                      | :white_square_button: |       |

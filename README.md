# Tellor

[![Check Set-Up & Build](https://github.com/evilrobot-01/substrate-pallets/actions/workflows/check.yml/badge.svg?branch=tellor)](https://github.com/evilrobot-01/substrate-pallets/actions/workflows/check.yml)
[![Run Tests](https://github.com/evilrobot-01/substrate-pallets/actions/workflows/test.yml/badge.svg?branch=tellor)](https://github.com/evilrobot-01/substrate-pallets/actions/workflows/test.yml)

License: Unlicense

## Porting Progress

### AutoPay

| Dispatchable Function                  |  Functional Test   | Notes |
|----------------------------------------|:------------------:|-------|
| :white_check_mark: `claim_onetime_tip` | :white_check_mark: |       |
| :white_check_mark: `claim_tip`         | :white_check_mark: |       |
| :white_check_mark: `fund_feed`         | :white_check_mark: |       |
| :white_check_mark: `setup_data_feed`   | :white_check_mark: |       |
| :white_check_mark: `tip`               | :white_check_mark: |       |

| Runtime API                                       |    Functional Test    | Notes             |
|---------------------------------------------------|:---------------------:|-------------------|
| :white_check_mark: `get_current_feeds`            | :white_square_button: | No reference test |
| :white_check_mark: `get_current_tip`              |  :white_check_mark:   |                   |
| :white_check_mark: `get_data_feed`                |  :white_check_mark:   |                   |
| :white_check_mark: `get_funded_feed_details `     |  :white_check_mark:   |                   |
| :white_check_mark: `get_funded_feeds`             |  :white_check_mark:   |                   |
| :white_check_mark: `get_funded_query_ids`         |  :white_check_mark:   |                   |
| :white_check_mark: `get_funded_single_tips_info`  |  :white_check_mark:   |                   |
| :white_check_mark: `get_past_tip_count`           |  :white_check_mark:   |                   |
| :white_check_mark: `get_past_tips`                |  :white_check_mark:   |                   |
| :white_check_mark: `get_past_tip_by_index`        |  :white_check_mark:   |                   |
| :white_check_mark: `get_query_id_from_feed_id`    |  :white_check_mark:   |                   |
| :white_check_mark: `get_reward_amount`            |  :white_check_mark:   |                   |
| :white_check_mark: `get_reward_claimed_status`    |  :white_check_mark:   |                   |
| :white_check_mark: `get_reward_claim_status_list` |  :white_check_mark:   |                   |
| :white_check_mark: `get_tips_by_address`          |  :white_check_mark:   |                   |

### Oracle (Tellor Flex)

| Dispatchable Function                |    Functional Test     | Notes                                     |
|--------------------------------------|:----------------------:|-------------------------------------------|
| :white_square_button: `remove_value` | :white_square_button:  | Implemented, needs test                   |
| :heavy_check_mark: `submit_value`    | :white_square_button:  | Implemented apart from time-based rewards |

| Runtime API                                                        |    Functional Test     | Notes |
|--------------------------------------------------------------------|:----------------------:|-------|
| :white_check_mark: `get_block_number_by_timestamp`                 | :white_square_button:  |       |
| :white_check_mark: `get_current_value`                             | :white_square_button:  |       |
| :white_check_mark: `get_data_before`                               | :white_square_button:  |       |
| :white_check_mark: `get_new_value_count_by_query_id`               | :white_square_button:  |       |
| :white_square_button: `get_pending_reward_by_staker`?              | :white_square_button:  |       |
| :white_square_button: `get_real_staking_rewards_balance`?          | :white_square_button:  |       |
| :white_check_mark: `get_report_details`                            | :white_square_button:  |       |
| :white_check_mark: `get_reporter_by_timestamp`                     | :white_square_button:  |       |
| :white_check_mark: `get_reporter_last_timestamp`                   | :white_square_button:  |       |
| :white_check_mark: `get_reporting_lock`                            | :white_square_button:  |       |
| :white_check_mark: `get_reports_submitted_by_address`              | :white_square_button:  |       |
| :white_check_mark: `get_reports_submitted_by_address_and_query_id` | :white_square_button:  |       |
| :white_check_mark: `get_stake_amount`                              | :white_square_button:  |       |
| :white_check_mark: `get_staker_info`                               | :white_square_button:  |       |
| :white_check_mark: `get_time_of_last_new_value`                    | :white_square_button:  |       |
| :white_check_mark: `get_timestamp_by_query_id_and_index`           | :white_square_button:  |       |
| :white_check_mark: `get_index_for_data_before`                     | :white_square_button:  |       |
| :white_check_mark: `get_timestamp_index_by_timestamp`              | :white_square_button:  |       |
| :white_check_mark: `get_total_stake_amount`                        | :white_square_button:  |       |
| :white_check_mark: `get_total_stakers`                             | :white_square_button:  |       |
| :white_square_button: `get_total_time_based_rewards_balance`?      | :white_square_button:  |       |
| :white_check_mark: `is_in_dispute`                                 | :white_square_button:  |       |
| :white_check_mark: `retrieve_data`                                 | :white_square_button:  |       |

### Governance

| Dispatchable Function                 |    Functional Test     | Notes |
|---------------------------------------|:----------------------:|-------|
| :white_square_button: `begin_dispute` | :white_square_button:  |       |
| :white_square_button: `vote`          | :white_square_button:  |       |

| Runtime API                          |    Functional Test    | Notes |
|--------------------------------------|:---------------------:|-------|
| :white_check_mark: `did_vote`        | :white_square_button: |       |
| :white_check_mark: `get_dispute_fee` | :white_square_button: |       |

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

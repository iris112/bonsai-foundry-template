#![no_main]

use std::io::Read;

use ethabi::{ethereum_types::U256, ethereum_types::I256, ParamType, Token};
use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

struct StrategyDataParams {
    cur_timestamp: U256,
    version: U256,
    last_block: U256,
    last_timestamp: U256,
    rate_per_sec: U256,
    full_utilization_rate: U256,
    total_asset: U256,
    total_borrow: U256,
    util_prec: U256,
    min_target_util: U256,
    max_target_util: U256,
    vertex_utilization: U256,
    min_full_util_rate: U256,
    max_full_util_rate: U256,
    zero_util_rate: U256,
    rate_half_life: U256,
    vertex_rate_percent: U256,
    rate_prec: U256,
    is_interest_paused: U256
}

struct Position {
    strategy: ParamType::Address,
    debt: U256,
}

struct StrategyParams {
    activation: U256,
    last_report: U256,
    current_debt: U256,
    max_debt: U256
}

const SECONDS_PER_YEAR: U256 = 31556952;

fn get_full_utilization_interest(delta_time: U256, utilization: U256, full_utilization_interest: u64) -> u64 {
    let mut new_full_utilization_interest: u64;

    if utilization < min_target_util {
        let delta_utilization = ((min_target_util - utilization) * 1e18) / min_target_util;
        let decay_growth = (rate_half_life * 1e36) + (delta_utilization * delta_utilization * delta_time);
        new_full_utilization_interest =
            ((full_utilization_interest * (rate_half_life * 1e36)) / decay_growth) as u64;
    } else if utilization > max_target_util {
        let delta_utilization = ((utilization - max_target_util) * 1e18) / (util_prec - max_target_util);
        let decay_growth = (rate_half_life * 1e36) + (delta_utilization * delta_utilization * delta_time);
        new_full_utilization_interest =
            ((full_utilization_interest * decay_growth) / (rate_half_life * 1e36)) as u64;
    } else {
        new_full_utilization_interest = full_utilization_interest;
    }

    if new_full_utilization_interest > max_full_util_rate {
        new_full_utilization_interest = max_full_util_rate;
    } else if new_full_utilization_interest < min_full_util_rate {
        new_full_utilization_interest = min_full_util_rate;
    }

    new_full_utilization_interest
}

fn get_new_rate(delta_time: U256, utilization: U256, old_full_utilization_interest: u64) -> (u64, u64) {
    let new_full_utilization_interest = get_full_utilization_interest(delta_time, utilization, old_full_utilization_interest);

    let vertex_interest =
        (((new_full_utilization_interest - zero_util_rate) * vertex_rate_percent) / rate_prec) + zero_util_rate;

    let new_rate_per_sec = if utilization < vertex_utilization {
        (zero_util_rate + (utilization * (vertex_interest - zero_util_rate)) / vertex_utilization) as u64
    } else {
        let slope =
            ((new_full_utilization_interest - vertex_interest) * util_prec) / (util_prec - vertex_utilization);
        (vertex_interest + ((utilization - vertex_utilization) * slope) / util_prec) as u64
    };

    (new_rate_per_sec, new_full_utilization_interest)
}

fn apr_after_debt_change(
    strategy_data: StrategyDataParams,
    delta: I256
) -> U256 {
    if delta == 0 {
        return strategy_data.rate_per_sec * SECONDS_PER_YEAR;
    }

    let asset_amount = U256::from(I256::from(strategy_data.total_asset) + delta);

    if strategy_data.is_interest_paused {
        return strategy_data.rate_per_sec * SECONDS_PER_YEAR;
    }

    let delta_time = strategy_data.cur_timestamp - strategy_data.last_timestamp;
    let utilization_rate;
    if asset_amount == 0 {
        utilization_rate = 0;
    } else {
        utilization_rate = (strategy_data.util_prec * strategy_data.total_borrow) / asset_amount
    };

    let (strategy_data.rate_per_sec, _) = get_new_rate(
        delta_time,
        utilization_rate,
        strategy_data.full_utilization_rate,
    );

    strategy_data.rate_per_sec * SECONDS_PER_YEAR
}

fn get_optimal_allocation(
    c: U256,
    total_initial_amount: U256,
    total_available_amount: U256,
    initial_datas: Vec<Position>,
    strategy_datas: Vec<StrategyDataParams>,
    sturdy_datas: Vec<StrategyParams>
) -> Vec<Position> {
    let mut b = initial_datas.clone();
    let deposit_unit = (total_available_amount - total_initial_amount) / c;
    let strategy_count = initial_datas.length;

    // Iterate chunk count
    for i in 0..c {
        // Calculate the correct last remained amount
        if i == c - 1 {
            b[i as usize] += total_available_amount - total_initial_amount - deposit_unit * (c - 1);
        }

        // Find max apr silo when deposit unit amount
        let mut max_apr = 0.0;
        let mut max_index = 0;

        for j in 0..strategy_count {
            // Check silo's max debt
            if b[j] + deposit_unit > sturdy_datas[j].max_debt {
                continue;
            }

            let apr = apr_after_debt_change(strategy_datas[j], b[j] + deposit_unit - sturdy_datas[j].current_debt);

            if max_apr >= apr {
                continue;
            }

            max_apr = apr;
            max_index = j;
        }

        if max_apr == 0.0 {
            println!("There is no max apr");
        }

        b[max_index] += deposit_unit;
    }

    // Make position array - first withdraw positions and next deposit positions.
    let mut deposits = Vec::new();
    let mut withdraws = Vec::new();

    for i in 0..strategy_count {
        let position = Position {
            strategy: initial_datas[i].strategy.clone(),
            debt: b[i],
        };

        if sturdy_datas[i].current_debt > b[i] {
            withdraws.push(position);
        } else {
            deposits.push(position);
        }
    }

    deposits.reverse();
    withdraws.extend(deposits);

    withdraws
}

fn main() {
    // Read data sent from the application contract.
    let mut input_bytes = Vec::<u8>::new();
    env::stdin().read_to_end(&mut input_bytes).unwrap();
    // Type array passed to `ethabi::decode_whole` should match the types encoded in
    // the application contract.
    let input = ethabi::decode_whole(
        &[
            ParamType::Uint(256),      // chunk count
            ParamType::Uint(256),      // total initial amount
            ParamType::Uint(256),      // total available amount
            ParamType::Array(Vec<Position>)                 // initial datas
            ParamType::Array(Vec<StrategyDataParams>)       // strategy datas
            ParamType::Array(Vec<StrategyParams>)       // sturdy datas
        ],
        &input_bytes,
    )
    .unwrap();

    let chunk_count: U256 = input[0].clone().into_uint().unwrap();
    let total_initial_amount: U256 = input[1].clone().into_uint().unwrap();
    let total_available_amount: U256 = input[2].clone().into_uint().unwrap();

    let initial_datas: Vec<Position> = input[3].clone().into_array().unwrap().as_u128();
    let strategy_datas: Vec<StrategyDataParams> = input[4].clone().into_array().unwrap().as_u128();
    let sturdy_datas: Vec<StrategyParams> = input[5].clone().into_array().unwrap().as_u32();

    let result: Vec<Position> = get_optimal_allocation(
        chunk_count, 
        total_initial_amount, 
        total_available_amount, 
        initial_datas, 
        strategy_datas, 
        sturdy_datas
    ).unwrap();

    // Commit the journal that will be received by the application contract.
    // Encoded types should match the args expected by the application callback.
    env::commit_slice(&ethabi::encode(&[
        Token::Array(result),
    ]));
}
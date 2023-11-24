#![no_main]

use std::io::Read;

use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_primitives::{Address, I256, U256};

use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

#[derive(Clone, Copy)]
struct SturdyDataParams {
    cur_timestamp: U256,
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
    is_interest_paused: bool
}

#[derive(Clone)]
struct Position {
    strategy: Address,
    debt: U256,
}

struct StrategyParams {
    activation: U256,
    last_report: U256,
    current_debt: U256,
    max_debt: U256
}

const SECONDS_PER_YEAR: u128 = 31556952 as u128;

fn get_full_utilization_interest(delta_time: U256, utilization: U256, sturdy_data: SturdyDataParams) -> u64 {
    let mut new_full_utilization_interest: u64;

    if utilization < sturdy_data.min_target_util {
        let delta_utilization = ((sturdy_data.min_target_util - utilization) * U256::from(1e18)) / sturdy_data.min_target_util;
        let decay_growth = (sturdy_data.rate_half_life * U256::from(1e36)) + (delta_utilization * delta_utilization * delta_time);
        new_full_utilization_interest =
            ((sturdy_data.full_utilization_rate * (sturdy_data.rate_half_life * U256::from(1e36))) / decay_growth).try_into().unwrap();
    } else if utilization > sturdy_data.max_target_util {
        let delta_utilization = ((utilization - sturdy_data.max_target_util) * U256::from(1e18)) / (sturdy_data.util_prec - sturdy_data.max_target_util);
        let decay_growth = (sturdy_data.rate_half_life * U256::from(1e36)) + (delta_utilization * delta_utilization * delta_time);
        new_full_utilization_interest =
            ((sturdy_data.full_utilization_rate * decay_growth) / (sturdy_data.rate_half_life * U256::from(1e36))).try_into().unwrap();
    } else {
        new_full_utilization_interest = sturdy_data.full_utilization_rate.try_into().unwrap();
    }

    if new_full_utilization_interest > sturdy_data.max_full_util_rate.try_into().unwrap() {
        new_full_utilization_interest = sturdy_data.max_full_util_rate.try_into().unwrap();
    } else if new_full_utilization_interest < sturdy_data.min_full_util_rate.try_into().unwrap(){
        new_full_utilization_interest = sturdy_data.min_full_util_rate.try_into().unwrap();
    }

    new_full_utilization_interest
}

fn get_new_rate(delta_time: U256, utilization: U256, sturdy_data: SturdyDataParams) -> (u64, u64) {
    let new_full_utilization_interest = get_full_utilization_interest(delta_time, utilization, sturdy_data);

    let vertex_interest =
        (((U256::from(new_full_utilization_interest) - sturdy_data.zero_util_rate) * sturdy_data.vertex_rate_percent) / sturdy_data.rate_prec) + sturdy_data.zero_util_rate;

    let new_rate_per_sec = if utilization < sturdy_data.vertex_utilization {
        (sturdy_data.zero_util_rate + (utilization * (vertex_interest - sturdy_data.zero_util_rate)) / sturdy_data.vertex_utilization).try_into().unwrap()
    } else {
        (vertex_interest + ((utilization - sturdy_data.vertex_utilization) * (U256::from(new_full_utilization_interest) - vertex_interest)) / (sturdy_data.util_prec - sturdy_data.vertex_utilization)).try_into().unwrap()
    };

    (new_rate_per_sec, new_full_utilization_interest)
}

fn apr_after_debt_change(
    sturdy_data: SturdyDataParams,
    delta: I256
) -> U256 {
    if delta == I256::try_from(0).unwrap() {
        return sturdy_data.rate_per_sec * U256::from(SECONDS_PER_YEAR) / U256::from(1e13);
    }

    if sturdy_data.is_interest_paused {
        return sturdy_data.rate_per_sec * U256::from(SECONDS_PER_YEAR) / U256::from(1e13);
    }

    let asset_amount = U256::try_from(I256::try_from(sturdy_data.total_asset).unwrap() + delta).unwrap();
    let delta_time = sturdy_data.cur_timestamp - sturdy_data.last_timestamp;
    let utilization_rate = if asset_amount == U256::from(0) {
        U256::from(0)
    } else {
        (sturdy_data.util_prec * sturdy_data.total_borrow) / asset_amount
    };

    let (rate_per_sec, _) = get_new_rate(
        delta_time,
        utilization_rate,
        sturdy_data,
    );

    U256::from(rate_per_sec) * U256::from(SECONDS_PER_YEAR) / U256::from(1e13)
}

fn get_optimal_allocation(
    c: u64,
    total_initial_amount: U256,
    total_available_amount: U256,
    initial_datas: &Vec<Position>,
    sturdy_datas: &Vec<SturdyDataParams>,
    strategy_datas: &Vec<StrategyParams>
) -> Vec<Position> {
    let mut b = initial_datas.clone();
    let mut deposit_unit = (total_available_amount - total_initial_amount) / U256::try_from(c).unwrap();
    let strategy_count = initial_datas.len();
    if deposit_unit == U256::from(0) {
        return vec![];
    }

    // Iterate chunk count
    for i in 0..c {
        // Calculate the correct last remained amount
        if i == c - 1 {
            deposit_unit = total_available_amount - total_initial_amount - deposit_unit * U256::try_from(c - 1).unwrap();
        }

        // Find max apr silo when deposit unit amount
        let mut max_apr = 0;
        let mut max_index = 0;

        for j in 0..strategy_count {
            // Check silo's max debt
            if b[j].debt + deposit_unit > strategy_datas[j].max_debt {
                continue;
            }

            let apr = apr_after_debt_change(sturdy_datas[j], I256::try_from(b[j].debt + deposit_unit).unwrap() - I256::try_from(strategy_datas[j].current_debt).unwrap()).try_into().unwrap();

            if max_apr >= apr {
                continue;
            }

            max_apr = apr;
            max_index = j;
        }

        if max_apr == 0 {
            panic!("There is no max apr");
        }

        b[max_index].debt += deposit_unit;
    }

    // Make position array - first withdraw positions and next deposit positions.
    let mut deposits = Vec::new();
    let mut withdraws = Vec::new();

    for i in 0..strategy_count {
        let position = Position {
            strategy: initial_datas[i].strategy,
            debt: b[i].debt,
        };

        if strategy_datas[i].current_debt > b[i].debt {
            withdraws.push(position);
        } else {
            deposits.push(position);
        }
    }

    deposits.reverse();
    withdraws.extend(deposits);

    withdraws
}

fn get_current_and_new_apr(
    initial_datas: &Vec<Position>,
    sturdy_datas: &Vec<SturdyDataParams>,
    strategy_datas: &Vec<StrategyParams>,
    optimal_datas: &Vec<Position>
) -> (u64, u64) {
    let strategy_count = initial_datas.len();
    let mut total_amount = U256::from(0);
    let mut total_apr = U256::from(0);
    if optimal_datas.len() == 0 {
        return (0, 0);
    }

    // get current apr
    for i in 0..strategy_count {
        let apr = apr_after_debt_change(sturdy_datas[i], I256::try_from(0).unwrap());
        total_apr += apr * strategy_datas[i].current_debt;
        total_amount += strategy_datas[i].current_debt;
    }
    let current_apr = if total_apr == U256::from(0) || total_amount == U256::from(0) {
        0
    } else {
        (total_apr / total_amount).try_into().unwrap()
    };

    total_amount = U256::from(0);
    total_apr = U256::from(0);
    // get new apr
    for i in 0..strategy_count {
        let mut index = strategy_count;
        for j in 0..strategy_count {
            if initial_datas[j].strategy == optimal_datas[i].strategy {
                index = j;
                break;
            }
        }
        
        if index == strategy_count {
            break;
        }

        let apr = apr_after_debt_change(sturdy_datas[index], I256::try_from(optimal_datas[i].debt).unwrap() - I256::try_from(strategy_datas[index].current_debt).unwrap());
        total_apr += apr * optimal_datas[i].debt;
        total_amount += optimal_datas[i].debt;
    }
    let new_apr = if total_apr == U256::from(0) || total_amount == U256::from(0) {
        0
    } else {
        (total_apr / total_amount).try_into().unwrap()
    };

    (current_apr, new_apr)
}

fn main() {
    // Read data sent from the application contract.
    let mut input_bytes = Vec::<u8>::new();
    env::stdin().read_to_end(&mut input_bytes).unwrap();

    let my_type: DynSolType = "(uint256,uint256,uint256,(address,uint256)[],(uint256,uint256,uint256,uint256)[],(uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint256,bool)[])".parse().unwrap();
    let my_data = my_type.abi_decode_params(&input_bytes).unwrap();
    let input = my_data.as_tuple().unwrap();

    let (chunk_count, _) = input[0].clone().as_uint().unwrap();
    let (total_initial_amount, _) = input[1].clone().as_uint().unwrap();
    let (total_available_amount, _) = input[2].clone().as_uint().unwrap();
    let initial_datas: Vec<Position> = input[3].clone().as_array().unwrap().into_iter().map(|item| {
        let fields = item.as_tuple().unwrap();
        Position {
            strategy: fields[0].clone().as_address().unwrap(),
            debt: fields[1].clone().as_uint().unwrap().0,
        }
    }).collect();
    let strategy_datas: Vec<StrategyParams> = input[4].clone().as_array().unwrap().into_iter().map(|item| {
        let fields = item.as_tuple().unwrap();
        StrategyParams {
            activation: fields[0].clone().as_uint().unwrap().0,
            last_report: fields[1].clone().as_uint().unwrap().0,
            current_debt: fields[2].clone().as_uint().unwrap().0,
            max_debt: fields[3].clone().as_uint().unwrap().0
        }
    }).collect();
    let sturdy_datas: Vec<SturdyDataParams> = input[5].clone().as_array().unwrap().into_iter().map(|item| {
        let fields = item.as_tuple().unwrap();
        SturdyDataParams {
            cur_timestamp: fields[0].clone().as_uint().unwrap().0,
            last_timestamp: fields[1].clone().as_uint().unwrap().0,
            rate_per_sec: fields[2].clone().as_uint().unwrap().0,
            full_utilization_rate: fields[3].clone().as_uint().unwrap().0,
            total_asset: fields[4].clone().as_uint().unwrap().0,
            total_borrow: fields[5].clone().as_uint().unwrap().0,
            util_prec: fields[6].clone().as_uint().unwrap().0,
            min_target_util: fields[7].clone().as_uint().unwrap().0,
            max_target_util: fields[8].clone().as_uint().unwrap().0,
            vertex_utilization: fields[9].clone().as_uint().unwrap().0,
            min_full_util_rate: fields[10].clone().as_uint().unwrap().0,
            max_full_util_rate: fields[11].clone().as_uint().unwrap().0,
            zero_util_rate: fields[12].clone().as_uint().unwrap().0,
            rate_half_life: fields[13].clone().as_uint().unwrap().0,
            vertex_rate_percent: fields[14].clone().as_uint().unwrap().0,
            rate_prec: fields[15].clone().as_uint().unwrap().0,
            is_interest_paused: fields[16].clone().as_bool().unwrap()
        }
    }).collect();

    let optimal_allocations: Vec<Position> = get_optimal_allocation(
        chunk_count.try_into().unwrap(), 
        total_initial_amount, 
        total_available_amount, 
        &initial_datas,
        &sturdy_datas, 
        &strategy_datas
    );

    let (current_apr, new_apr) = get_current_and_new_apr(
        &initial_datas, 
        &sturdy_datas,
        &strategy_datas, 
        &optimal_allocations
    );

    // Commit the journal that will be received by the application contract.
    // Encoded types should match the args expected by the application callback.
    if new_apr > current_apr {
        let result = DynSolValue::Tuple(vec![
            DynSolValue::Array(
                optimal_allocations.iter().map(|allocation| {
                    DynSolValue::Tuple(
                        vec![
                            DynSolValue::Address(allocation.strategy),
                            DynSolValue::Uint(allocation.debt, 256),
                        ]
                    )
                }).collect()
            ),
            DynSolValue::Uint(U256::from(new_apr), 256),
            DynSolValue::Uint(U256::from(current_apr), 256),
            DynSolValue::Bool(true)
        ]).abi_encode_params();
        env::commit_slice(&result);
    } else {
        let result = DynSolValue::Tuple(vec![
            DynSolValue::Array(vec![]),
            DynSolValue::Uint(U256::from(new_apr), 256),
            DynSolValue::Uint(U256::from(current_apr), 256),
            DynSolValue::Bool(false)
        ]).abi_encode_params();
        env::commit_slice(&result);
    }
}
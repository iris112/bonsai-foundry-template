// Copyright 2023 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env;

use alloy_primitives::U256;
use alloy_sol_types::SolValue;
use anyhow::Context;
use bonsai_ethereum_relay::sdk::client::{CallbackRequest, Client};
use clap::Parser;
use ethers::{abi::ethabi, types::{Address, I256}, utils::id};
use ethabi::{ethereum_types::U256, Token};
use methods::OPTIMAL_ALLOCATION_ID;
use risc0_zkvm::sha::Digest;

/// Exmaple code for sending a REST API request to the Bonsai relay service to
/// requests, execution, proving, and on-chain callback for a zkVM guest
/// application.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Adress for the BonsaiStarter application contract.
    address: Address,

    /// Input
    chunk_count: U256,
    total_initial_amount: U256,
    total_available_amount: U256,
    initial_datas: Vec<Token>,
    strategy_datas: Vec<Token>,
    sturdy_datas: Vec<Token>,

    /// Bonsai Relay API URL.
    #[arg(long, env, default_value = "http://localhost:8080")]
    bonsai_relay_api_url: String,

    /// Bonsai API key. Used by the relay to send requests to the Bonsai proving
    /// service. Can be set to an empty string when DEV_MODE is enabled.
    #[arg(long, env)]
    bonsai_api_key: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // check for bonsai_api_key
    if args.bonsai_api_key.is_none() && env::var("BONSAI_API_KEY").is_err() {
        eprintln!(
            "Error: the following required arguments were not provided: \
            \n'BONSAI_API_KEY' must be set either as an argument or as an environment variable. \
            \nIf `DEV_MODE` is enabled, you can use an empty string."
        );
        std::process::exit(1);
    }
    // initialize a relay client
    let relay_client = Client::from_parts(
        args.bonsai_relay_api_url.clone(),
        args.bonsai_api_key.unwrap(),
    )
    .context("Failed to initialize the relay client")?;

    // Initialize the input for the OPTIMAL_ALLOCATION guest.
    let input = ethabi::encode(&[
        Token::Uint(args.chunk_count.into()),
        Token::Uint(args.total_initial_amount.into()),
        Token::Uint(args.total_available_amount.into()),
        Token::Array(args.initial_datas.into()),
        Token::Array(args.strategy_datas.into()),
        Token::Array(args.sturdy_datas.into()),
    ]);

    // Set the function selector of the callback function.
    let function_signature = "onResult(IDebtManager.StrategyAllocation[],uint256,uint256,bool)";
    let function_selector = id(function_signature);

    // Create a CallbackRequest for your contract
    // example: (contracts/BonsaiStarter.sol).
    let request = CallbackRequest {
        callback_contract: args.address.into(),
        function_selector,
        gas_limit: 3000000,
        image_id: Digest::from(OPTIMAL_ALLOCATION_ID).into(),
        input,
    };

    // Send the callback request to the Bonsai Relay.
    relay_client
        .callback_request(request)
        .await
        .context("Callback request failed")?;

    Ok(())
}

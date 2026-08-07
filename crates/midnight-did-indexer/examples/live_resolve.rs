// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Live DID resolution against a running Midnight indexer.
//!
//! The issue-#4 acceptance harness: resolves a deployed `did:midnight`
//! through the full read path — indexer GraphQL → `ContractState`
//! deserialize → `DidLedgerSnapshot` decode → W3C DID document — and
//! prints the resolution result as JSON.
//!
//! ```bash
//! cargo run -p midnight-did-indexer --example live_resolve -- \
//!     <contract-address-hex> [indexer-graphql-url] [network]
//! ```
//!
//! Defaults: `http://127.0.0.1:8088/api/v3/graphql`, network `undeployed`.

use midnight_did_api::resolution::resolve;
use midnight_did_indexer::{IndexerBackend, IndexerClient};
use midnight_did_method::midnight_did::{MidnightNetwork, parse_contract_address};
use midnight_did_runtime::Contract;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let address_hex = args
        .next()
        .ok_or("usage: live_resolve <contract-address-hex> [url] [network]")?;
    let url = args
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:8088/api/v3/graphql".to_string());
    let network = match args.next().as_deref() {
        None | Some("undeployed") => MidnightNetwork::Undeployed,
        Some("testnet") => MidnightNetwork::Testnet,
        Some("mainnet") => MidnightNetwork::Mainnet,
        Some(other) => return Err(format!("unknown network {other}").into()),
    };

    let address = parse_contract_address(&address_hex)?;
    let backend = IndexerBackend::new(IndexerClient::new(url)?, address_hex);
    let contract = Contract::new(backend, address, network);

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let resolved = rt.block_on(resolve(&contract))?;

    match resolved {
        Some(r) => {
            let out = serde_json::json!({
                "didDocument": r.did_document,
                "didDocumentMetadata": r.did_document_metadata,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
            Ok(())
        }
        None => Err("no live DID state (notFound)".into()),
    }
}

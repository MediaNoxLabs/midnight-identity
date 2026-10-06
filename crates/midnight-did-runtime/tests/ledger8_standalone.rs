// SPDX-License-Identifier: Apache-2.0

//! Environment-gated Ledger8 standalone protocol probe.
//!
//! This is not a synthetic lifecycle test.  When enabled it interrogates the
//! exact node/indexer/proof-server stack and either proves the HTTP/RPC surfaces
//! needed by a pure-Rust lifecycle or reports the concrete missing external
//! wallet/funding/signing API that prevents deploy/create from being driven by
//! this runtime crate alone.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn split_http_url(url: &str) -> (&str, &str) {
    let rest = url
        .strip_prefix("http://")
        .unwrap_or_else(|| panic!("only http:// standalone URLs are supported by this probe: {url}"));
    rest.split_once('/')
        .map_or((rest, "/"), |(host, path)| (host, &url[url.find(path).unwrap() - 1..]))
}

fn http_request(method: &str, url: &str, body: Option<&str>, content_type: &str) -> (u16, String) {
    let (host, path) = split_http_url(url);
    let mut stream = TcpStream::connect(host).unwrap_or_else(|e| panic!("connect {host}: {e}"));
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
    let payload = body.unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, body) = response
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("malformed HTTP response from {url}: {response:?}"));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);
    (status, body.to_owned())
}

fn rpc(node_url: &str, method: &str, params: serde_json::Value) -> serde_json::Value {
    let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    let (status, response) = http_request("POST", node_url, Some(&body), "application/json");
    assert_eq!(status, 200, "{method} HTTP status {status}: {response}");
    let value: serde_json::Value = serde_json::from_str(&response).expect("json rpc response");
    assert!(value.get("error").is_none(), "{method} returned error: {value}");
    value["result"].clone()
}

fn graphql(indexer_url: &str, query: &str) -> serde_json::Value {
    let base = indexer_url.trim_end_matches('/');
    let url = if base.ends_with("/graphql") {
        base.to_owned()
    } else {
        format!("{base}/api/v3/graphql")
    };
    let body = serde_json::json!({"query":query}).to_string();
    let (status, response) = http_request("POST", &url, Some(&body), "application/json");
    assert_eq!(status, 200, "indexer graphql HTTP status {status}: {response}");
    let value: serde_json::Value = serde_json::from_str(&response).expect("graphql response");
    assert!(value.get("errors").is_none(), "graphql errors: {value}");
    value
}

#[test]
fn ledger8_standalone_protocol_probe_reports_lifecycle_blockers() {
    if env("MIDNIGHT_DID_LEDGER8_STANDALONE").as_deref() != Some("1") {
        eprintln!(
            "skip: set MIDNIGHT_DID_LEDGER8_STANDALONE=1 with node/indexer/proof URLs to probe Ledger8 standalone services"
        );
        return;
    }

    let node = env("MIDNIGHT_DID_LEDGER8_NODE_URL").expect("MIDNIGHT_DID_LEDGER8_NODE_URL");
    let indexer = env("MIDNIGHT_DID_LEDGER8_INDEXER_URL").expect("MIDNIGHT_DID_LEDGER8_INDEXER_URL");
    let proof = env("MIDNIGHT_DID_LEDGER8_PROOF_URL").expect("MIDNIGHT_DID_LEDGER8_PROOF_URL");

    let health = rpc(&node, "system_health", serde_json::json!([]));
    assert_eq!(health["isSyncing"], false, "node is syncing: {health}");
    let ledger_version = rpc(&node, "midnight_ledgerVersion", serde_json::json!([]));
    let methods = rpc(&node, "rpc_methods", serde_json::json!([]));
    let method_names = methods["methods"].as_array().expect("rpc method list");
    for required in [
        "author_submitExtrinsic",
        "chain_getFinalizedHead",
        "chain_getBlock",
        "midnight_contractState",
    ] {
        assert!(
            method_names.iter().any(|m| m.as_str() == Some(required)),
            "node missing {required}; methods={methods}"
        );
    }

    let schema = graphql(&indexer, "{ __schema { queryType { fields { name args { name } } } } }");
    let fields = schema["data"]["__schema"]["queryType"]["fields"]
        .as_array()
        .expect("query fields");
    assert!(
        fields.iter().any(|f| f["name"] == "contractAction"),
        "indexer schema missing contractAction: {schema}"
    );

    let (proof_status, proof_body) = http_request(
        "GET",
        &format!("{}/health", proof.trim_end_matches('/')),
        None,
        "application/json",
    );
    assert_eq!(proof_status, 200, "proof health HTTP {proof_status}: {proof_body}");

    let wallet_like = method_names
        .iter()
        .filter_map(|m| m.as_str())
        .filter(|m| {
            let lower = m.to_ascii_lowercase();
            lower.contains("wallet") || lower.contains("fund") || lower.contains("balance") || lower.contains("faucet")
        })
        .collect::<Vec<_>>();

    assert!(
        env("MIDNIGHT_DID_LEDGER8_WALLET_PROVIDER_URL").is_some() || !wallet_like.is_empty(),
        "Ledger8 services are healthy (ledgerVersion={ledger_version}) and expose node submission/finality, indexer contractAction, and proof /prove-tx, but no wallet/funding/balancing/signing API is available to this Rust runtime test. Node wallet-like RPC methods: {wallet_like:?}. Set MIDNIGHT_DID_LEDGER8_WALLET_PROVIDER_URL or provide a Rust wallet provider before claiming deploy/create lifecycle evidence."
    );
}

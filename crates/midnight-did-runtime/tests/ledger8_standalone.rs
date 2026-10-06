// SPDX-License-Identifier: Apache-2.0

//! Environment-gated Ledger8 standalone smoke harness.
//!
//! Set all of the following to exercise externally-managed services:
//! - `MIDNIGHT_DID_LEDGER8_STANDALONE=1`
//! - `MIDNIGHT_DID_LEDGER8_NODE_URL`
//! - `MIDNIGHT_DID_LEDGER8_INDEXER_URL`
//! - `MIDNIGHT_DID_LEDGER8_PROOF_URL`
//!
//! This harness deliberately skips when services are absent; it must not be
//! interpreted as a hosted node/indexer/proof-server lifecycle pass unless the
//! environment variables are present and the test output says so.

#[test]
fn ledger8_standalone_services_are_explicitly_gated() {
    if std::env::var("MIDNIGHT_DID_LEDGER8_STANDALONE").as_deref() != Ok("1") {
        eprintln!(
            "skip: set MIDNIGHT_DID_LEDGER8_STANDALONE=1 with node/indexer/proof URLs to run Ledger8 standalone harness"
        );
        return;
    }

    let required = [
        "MIDNIGHT_DID_LEDGER8_NODE_URL",
        "MIDNIGHT_DID_LEDGER8_INDEXER_URL",
        "MIDNIGHT_DID_LEDGER8_PROOF_URL",
    ];
    let missing = required
        .into_iter()
        .filter(|name| std::env::var(name).ok().filter(|v| !v.is_empty()).is_none())
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "Ledger8 standalone harness requested but required environment variables are missing: {missing:?}"
    );

    eprintln!(
        "Ledger8 standalone harness requested for node={}, indexer={}, proof={}; full lifecycle driver is intentionally provider-supplied",
        std::env::var("MIDNIGHT_DID_LEDGER8_NODE_URL").unwrap(),
        std::env::var("MIDNIGHT_DID_LEDGER8_INDEXER_URL").unwrap(),
        std::env::var("MIDNIGHT_DID_LEDGER8_PROOF_URL").unwrap(),
    );
}

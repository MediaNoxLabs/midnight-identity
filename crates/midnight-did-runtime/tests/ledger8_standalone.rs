// SPDX-License-Identifier: Apache-2.0

//! Environment-gated Ledger8 standalone protocol probe.
//!
//! This is not a synthetic lifecycle test.  When enabled it interrogates the
//! exact node/indexer/proof-server stack and proves the protocol surfaces used
//! by the ignored pure-Rust lifecycle harness below. The lifecycle harness also
//! requires an operator-supplied standalone funding seed and is never run by
//! default.

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

    eprintln!(
        "Ledger8 protocol surfaces are healthy for ledgerVersion={ledger_version}; run the ignored lifecycle with a local funding seed to submit transactions"
    );
}

fn standalone_funding_seed() -> Option<[u8; 32]> {
    let value = env("MIDNIGHT_DID_LEDGER8_FUNDER_SEED_HEX").or_else(|| env("OXID_STANDALONE_FUNDER_SEED_HEX"))?;
    let value = value.trim_start_matches("0x");
    if value.len() != 64 {
        panic!("standalone funding seed must be 32 bytes of hex");
    }
    let bytes = hex::decode(value).expect("standalone funding seed hex decodes");
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&bytes);
    Some(seed)
}

#[cfg(all(feature = "http", feature = "node-subxt"))]
mod live_lifecycle {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use compact_runtime::WitnessContext;
    use midnight_compact_runtime as compact_runtime;
    use midnight_did_domain::did_document::{CurveType, KeyType, VerificationMethodType};
    use midnight_did_jubjub_schnorr::{
        JubjubSchnorrSignature, derive_public_key_from_seed, encode_signature, field_from_scalar, scalar_from_field,
        seed_to_secret_scalar,
    };
    use midnight_did_runtime::backend::*;
    use midnight_did_runtime::contract::{Ledger, Witnesses};
    use midnight_did_runtime::contract_call::{
        DidContractCall, LedgerPublicKeyJwk, LedgerVerificationMethod, LedgerVerificationMethodRelation, MapMutation,
        SetMutation,
    };
    use midnight_serialize::{tagged_deserialize, tagged_serialize};
    use midnight_storage::DefaultDB;
    use midnight_transient_crypto::curve::{EmbeddedFr, EmbeddedGroupAffine};
    use midnight_transient_crypto::hash::transient_hash;
    use midnight_transient_crypto::proofs::{KeyLocation, ProvingKeyMaterial, VerifierKey};
    use sha2::{Digest, Sha256};
    use zeroize::Zeroize;

    const EXERCISED: &[(&str, &str)] = &[
        ("set_verification_method", "setVerificationMethod"),
        ("set_verification_method_relation", "setVerificationMethodRelation"),
        ("rotate_controller_key", "rotateControllerKey"),
        ("deactivate", "deactivate"),
    ];

    #[derive(Clone)]
    struct DidPrivateState {
        controller_seed: [u8; 32],
        recovery_seed: [u8; 32],
    }

    impl Drop for DidPrivateState {
        fn drop(&mut self) {
            self.controller_seed.zeroize();
            self.recovery_seed.zeroize();
        }
    }

    #[derive(Clone)]
    struct LifecycleWitnesses;

    impl Witnesses<DidPrivateState> for LifecycleWitnesses {
        fn get_schnorr_reduction<'a>(
            &self,
            _ctx: &WitnessContext<Ledger<'a>, DidPrivateState>,
            _challenge_hash: compact_runtime::Fr,
        ) -> (DidPrivateState, (u8, u128)) {
            // Compact currently lowers `schnorrVerify` directly to
            // `midnight_compact_runtime::schnorr_verify_jubjub`; the generated
            // Rust contains this trait method but has no call site for it.  The
            // ABI cannot represent the real `Uint<248>` remainder (`u128` would
            // silently truncate the high 120 bits), so fail loudly if a future
            // Compact pin makes the witness reachable instead of pretending to
            // compute the reduction.
            unreachable!("get_schnorr_reduction is unreachable for the pinned Ledger8 DID Compact output")
        }

        fn local_controller_public_key<'a>(
            &self,
            ctx: &WitnessContext<Ledger<'a>, DidPrivateState>,
        ) -> (DidPrivateState, compact_runtime::JubjubPoint) {
            (
                ctx.private_state.clone(),
                derive_public_key_from_seed(&ctx.private_state.controller_seed),
            )
        }

        fn local_recovery_authority_public_key<'a>(
            &self,
            ctx: &WitnessContext<Ledger<'a>, DidPrivateState>,
        ) -> (DidPrivateState, compact_runtime::JubjubPoint) {
            (
                ctx.private_state.clone(),
                derive_public_key_from_seed(&ctx.private_state.recovery_seed),
            )
        }

        fn current_timestamp<'a>(&self, ctx: &WitnessContext<Ledger<'a>, DidPrivateState>) -> (DidPrivateState, u64) {
            (ctx.private_state.clone(), 1)
        }
    }

    struct PrivateStateStore(Mutex<DidPrivateState>);

    impl DidPrivateStateStore<DidPrivateState> for PrivateStateStore {
        fn load(&self) -> Result<DidPrivateState, BackendError> {
            Ok(self
                .0
                .lock()
                .map_err(|_| BackendError::Other("private-state lock poisoned".into()))?
                .clone())
        }

        fn store(&self, state: DidPrivateState) -> Result<(), BackendError> {
            *self
                .0
                .lock()
                .map_err(|_| BackendError::Other("private-state lock poisoned".into()))? = state;
            Ok(())
        }
    }

    struct SeedSigner {
        controller_seed: Mutex<[u8; 32]>,
        recovery_seed: Mutex<[u8; 32]>,
    }

    impl Drop for SeedSigner {
        fn drop(&mut self) {
            if let Ok(seed) = self.controller_seed.get_mut() {
                seed.zeroize();
            }
            if let Ok(seed) = self.recovery_seed.get_mut() {
                seed.zeroize();
            }
        }
    }

    impl SeedSigner {
        fn new(controller_seed: [u8; 32], recovery_seed: [u8; 32]) -> Self {
            Self {
                controller_seed: Mutex::new(controller_seed),
                recovery_seed: Mutex::new(recovery_seed),
            }
        }

        fn promote_controller_seed(&self, mut next: [u8; 32]) -> Result<(), BackendError> {
            let mut current = self
                .controller_seed
                .lock()
                .map_err(|_| BackendError::Other("controller signer lock poisoned".into()))?;
            current.zeroize();
            *current = next;
            next.zeroize();
            Ok(())
        }

        fn sign(
            seed: &[u8; 32],
            digest: [compact_runtime::Fr; 4],
        ) -> Result<compact_runtime::SchnorrSignature, BackendError> {
            let secret = seed_to_secret_scalar(seed);
            let nonce = authorization_nonce_from_seed(seed, &digest);
            let public_key = derive_public_key_from_seed(seed);
            let announcement = EmbeddedGroupAffine::generator() * nonce;
            let challenge = field_digest_challenge(&announcement, &public_key, &digest)?;
            Ok(compact_runtime::SchnorrSignature {
                announcement,
                response: field_from_scalar(&(nonce + challenge * secret)),
            })
        }
    }

    impl DidAuthorizationSigner for SeedSigner {
        fn sign_controller(
            &self,
            digest: [compact_runtime::Fr; 4],
        ) -> Result<compact_runtime::SchnorrSignature, BackendError> {
            let seed = self
                .controller_seed
                .lock()
                .map_err(|_| BackendError::Other("controller signer lock poisoned".into()))?;
            Self::sign(&seed, digest)
        }

        fn sign_recovery(
            &self,
            digest: [compact_runtime::Fr; 4],
        ) -> Result<compact_runtime::SchnorrSignature, BackendError> {
            let seed = self
                .recovery_seed
                .lock()
                .map_err(|_| BackendError::Other("recovery signer lock poisoned".into()))?;
            Self::sign(&seed, digest)
        }
    }

    fn authorization_nonce_from_seed(seed: &[u8; 32], digest: &[compact_runtime::Fr; 4]) -> EmbeddedFr {
        let mut nonce_seed =
            Vec::with_capacity(midnight_did_jubjub_schnorr::NONCE_DOMAIN_V1.len() + seed.len() + digest.len() * 32);
        nonce_seed.extend_from_slice(midnight_did_jubjub_schnorr::NONCE_DOMAIN_V1.as_bytes());
        nonce_seed.extend_from_slice(seed);
        for field in digest {
            nonce_seed.extend_from_slice(&fr_be_bytes(field));
        }
        hash_to_scalar(&Sha256::digest(nonce_seed))
    }

    fn field_digest_challenge(
        announcement: &EmbeddedGroupAffine,
        public_key: &EmbeddedGroupAffine,
        digest: &[compact_runtime::Fr; 4],
    ) -> Result<EmbeddedFr, BackendError> {
        let ann_x = announcement
            .x()
            .ok_or_else(|| BackendError::Other("Schnorr announcement is the identity".into()))?;
        let ann_y = announcement
            .y()
            .ok_or_else(|| BackendError::Other("Schnorr announcement is the identity".into()))?;
        let pk_x = public_key
            .x()
            .ok_or_else(|| BackendError::Other("Schnorr public key is the identity".into()))?;
        let pk_y = public_key
            .y()
            .ok_or_else(|| BackendError::Other("Schnorr public key is the identity".into()))?;
        let challenge = transient_hash(&[ann_x, ann_y, pk_x, pk_y, digest[0], digest[1], digest[2], digest[3]]);
        let mut le = challenge.as_le_bytes();
        le.resize(32, 0);
        le[31] = 0;
        EmbeddedFr::from_le_bytes(&le)
            .ok_or_else(|| BackendError::Other("2^248 Schnorr challenge reduction escaped scalar field".into()))
    }

    fn hash_to_scalar(digest32: &[u8]) -> EmbeddedFr {
        let mut le = [0u8; 32];
        for (slot, byte) in le.iter_mut().zip(digest32.iter().take(32).rev()) {
            *slot = *byte;
        }
        EmbeddedFr::from_le_bytes_wide(&le).expect("32-byte nonce hash reduces to a Jubjub scalar")
    }

    fn fr_be_bytes(field: &compact_runtime::Fr) -> [u8; 32] {
        let mut out = [0u8; 32];
        let le = field.as_le_bytes();
        let n = le.len().min(32);
        out[32 - n..].copy_from_slice(&le[..n]);
        out.reverse();
        out
    }

    fn verify_field_digest(
        public_key: &EmbeddedGroupAffine,
        digest: &[compact_runtime::Fr; 4],
        signature: &JubjubSchnorrSignature,
    ) -> bool {
        let Ok(challenge) = field_digest_challenge(&signature.announcement, public_key, digest) else {
            return false;
        };
        EmbeddedGroupAffine::generator() * signature.response == signature.announcement + *public_key * challenge
    }

    #[test]
    fn generated_did_compact_does_not_call_schnorr_reduction_witness() {
        let generated = include_str!("../src/contract/generated.rs");
        assert_eq!(
            generated.matches("get_schnorr_reduction").count(),
            1,
            "the pinned Compact output must not call the unrepresentable Uint<248> reduction witness"
        );
        assert!(generated.contains("midnight_compact_runtime::schnorr_verify_jubjub"));
    }

    #[test]
    fn seed_signer_matches_ts_vector_and_signs_full_field_digest() {
        const SEED: [u8; 32] = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
            30, 31, 32,
        ];
        const TS_DIGEST: [u64; 4] = [
            0x2bdb0067176fd1bf,
            0xb0172636b6c91955,
            0xe28eed1304bc16d9,
            0xcbb1501030aa4576,
        ];
        const TS_SIG96_HEX: &str = "02b4bfc039ddca33a2bc807a2df358682a81a6dd0db45eaf9567f00d000211460abff840b93c8fbc864111ba6009a31d227a9e04d44adcc6a44c0b103bb459da0603b2f0bc6eb850600cc297da66b157c88c53a731cfda0887153d531eabcd9c";

        let digest_fr = TS_DIGEST.map(compact_runtime::Fr::from);
        let compact_sig = SeedSigner::sign(&SEED, digest_fr).expect("TS vector signs");
        let suite_sig = JubjubSchnorrSignature {
            announcement: compact_sig.announcement,
            response: scalar_from_field(&compact_sig.response),
        };
        assert_eq!(
            hex::encode(encode_signature(&suite_sig).expect("signature encodes")),
            TS_SIG96_HEX
        );
        assert!(verify_field_digest(
            &derive_public_key_from_seed(&SEED),
            &digest_fr,
            &suite_sig
        ));

        let mut high = [0u8; 32];
        high[8] = 1;
        high[30] = 7;
        let high_digest = [
            compact_runtime::Fr::from_le_bytes(&high).expect("high field"),
            digest_fr[1],
            digest_fr[2],
            digest_fr[3],
        ];
        let high_sig = SeedSigner::sign(&SEED, high_digest).expect("full field digest signs without truncation");
        let high_suite_sig = JubjubSchnorrSignature {
            announcement: high_sig.announcement,
            response: scalar_from_field(&high_sig.response),
        };
        assert!(verify_field_digest(
            &derive_public_key_from_seed(&SEED),
            &high_digest,
            &high_suite_sig
        ));
        assert_ne!(
            hex::encode(encode_signature(&high_suite_sig).expect("high signature encodes")),
            TS_SIG96_HEX,
            "high-field authorization digest must not collapse to the TS low-limb vector"
        );
    }

    fn point_hex_from_seed(seed: &[u8; 32]) -> midnight_did_runtime::contract_call::JubjubPointHex {
        let point = derive_public_key_from_seed(seed);
        let x = point.x().expect("derived public key has x");
        let y = point.y().expect("derived public key has y");
        midnight_did_runtime::contract_call::JubjubPointHex::new(
            midnight_did_runtime::contract_call::NewJubjubPointHex {
                x: hex::encode(x.as_le_bytes()),
                y: hex::encode(y.as_le_bytes()),
            },
        )
        .expect("valid derived point")
    }

    struct FileCheckpointDustStateProvider {
        path: PathBuf,
    }

    impl LocalLedger8DustStateProvider for FileCheckpointDustStateProvider {
        fn load_dust_state(
            &self,
            _owner: &midnight_ledger::dust::DustPublicKey,
            parameters: midnight_ledger::dust::DustParameters,
        ) -> Result<midnight_ledger::dust::DustLocalState<DefaultDB>, BackendError> {
            let bytes = std::fs::read(&self.path)
                .map_err(|e| BackendError::Network(format!("read private DUST checkpoint: {e}")))?;
            let state: midnight_ledger::dust::DustLocalState<DefaultDB> = tagged_deserialize(&bytes[..])
                .map_err(|e| BackendError::Decode(format!("decode private DUST checkpoint: {e}")))?;
            if state.params != parameters {
                return Err(BackendError::Other(
                    "private DUST checkpoint parameters do not match chain tip".into(),
                ));
            }
            Ok(state)
        }

        fn save_dust_state(
            &self,
            _owner: &midnight_ledger::dust::DustPublicKey,
            state: &midnight_ledger::dust::DustLocalState<DefaultDB>,
        ) -> Result<(), BackendError> {
            let mut bytes = Vec::new();
            tagged_serialize(state, &mut bytes)
                .map_err(|e| BackendError::Decode(format!("encode private DUST checkpoint: {e}")))?;
            std::fs::write(&self.path, bytes)
                .map_err(|e| BackendError::Network(format!("write private DUST checkpoint: {e}")))
        }
    }

    struct DidArtifacts {
        operations: midnight_storage::storage::HashMap<
            compact_runtime::EntryPointBuf,
            compact_runtime::ContractOperation,
            DefaultDB,
        >,
        call_configs: HashMap<String, LedgerContractCallConfig>,
        proving_keys: LedgerProofKeyMap,
    }

    fn load_artifacts(root: &Path) -> Result<DidArtifacts, BackendError> {
        let mut operations = midnight_storage::storage::HashMap::new();
        let mut call_configs = HashMap::new();
        let mut proving_keys = LedgerProofKeyMap::new();
        for (circuit, artifact) in EXERCISED {
            let prover_key = std::fs::read(root.join("keys").join(format!("{artifact}.prover")))
                .map_err(|e| BackendError::Network(format!("read {artifact}.prover: {e}")))?;
            let verifier_key = std::fs::read(root.join("keys").join(format!("{artifact}.verifier")))
                .map_err(|e| BackendError::Network(format!("read {artifact}.verifier: {e}")))?;
            let ir_source = std::fs::read(root.join("zkir").join(format!("{artifact}.bzkir")))
                .or_else(|_| std::fs::read(root.join("keys").join(format!("{artifact}.zkir"))))
                .map_err(|e| BackendError::Network(format!("read {artifact} ZKIR: {e}")))?;
            let vk: VerifierKey = tagged_deserialize(&verifier_key[..])
                .map_err(|e| BackendError::Decode(format!("decode {artifact}.verifier: {e}")))?;
            let location = format!("midnight/did/{circuit}");
            operations = operations.insert(
                compact_runtime::EntryPointBuf::from(circuit.as_bytes()),
                compact_runtime::ContractOperation::new(Some(vk.clone())),
            );
            call_configs.insert(
                (*circuit).to_owned(),
                LedgerContractCallConfig {
                    operation: compact_runtime::ContractOperation::new(Some(vk)),
                    communication_commitment_rand: compact_runtime::Fr::from(7u64),
                    key_location: KeyLocation(std::borrow::Cow::Owned(location.clone())),
                },
            );
            proving_keys.insert(
                location,
                ProvingKeyMaterial {
                    prover_key,
                    verifier_key,
                    ir_source,
                },
            );
        }
        Ok(DidArtifacts {
            operations,
            call_configs,
            proving_keys,
        })
    }

    fn sample_method() -> LedgerVerificationMethod {
        LedgerVerificationMethod {
            id: "#key-1".to_owned(),
            typ: VerificationMethodType::JsonWebKey,
            public_key_jwk: LedgerPublicKeyJwk {
                kty: KeyType::EC,
                crv: CurveType::Secp256k1,
                x: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned(),
                y: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned(),
            },
        }
    }

    pub fn run() {
        let node_http = super::env("MIDNIGHT_DID_LEDGER8_NODE_URL").expect("MIDNIGHT_DID_LEDGER8_NODE_URL");
        let node_ws =
            super::env("MIDNIGHT_DID_LEDGER8_NODE_WS_URL").unwrap_or_else(|| node_http.replace("http://", "ws://"));
        let indexer = super::env("MIDNIGHT_DID_LEDGER8_INDEXER_URL").expect("MIDNIGHT_DID_LEDGER8_INDEXER_URL");
        let proof = super::env("MIDNIGHT_DID_LEDGER8_PROOF_URL").expect("MIDNIGHT_DID_LEDGER8_PROOF_URL");
        let artifact_root = match super::env("MIDNIGHT_DID_ZK_CONFIG_PATH") {
            Some(path) => PathBuf::from(path),
            None => {
                eprintln!("skip: set MIDNIGHT_DID_ZK_CONFIG_PATH for managed DID proving artifacts");
                return;
            }
        };
        let checkpoint = match super::env("MIDNIGHT_DID_LEDGER8_DUST_CHECKPOINT") {
            Some(path) => PathBuf::from(path),
            None => {
                eprintln!(
                    "skip: set MIDNIGHT_DID_LEDGER8_DUST_CHECKPOINT with a private indexer-replayed DUST checkpoint"
                );
                return;
            }
        };
        let seed = match super::standalone_funding_seed() {
            Some(seed) => seed,
            None => {
                eprintln!("skip: set MIDNIGHT_DID_LEDGER8_FUNDER_SEED_HEX or OXID_STANDALONE_FUNDER_SEED_HEX");
                return;
            }
        };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async move {
            let artifacts = load_artifacts(&artifact_root).expect("DID artifacts load");
            let deployment_config = LedgerDeploymentConfig {
                operations: artifacts.operations,
                maintenance_authority: compact_runtime::ContractMaintenanceAuthority::default(),
                nonce: midnight_base_crypto::hash::HashOutput(
                    [0x42; midnight_base_crypto::hash::PERSISTENT_HASH_BYTES],
                ),
            };
            let controller_seed = [0x21; 32];
            let recovery_seed = [0x22; 32];
            let private_state = Arc::new(PrivateStateStore(Mutex::new(DidPrivateState {
                controller_seed,
                recovery_seed,
            })));
            let signer = Arc::new(SeedSigner::new(controller_seed, recovery_seed));
            let executor = Arc::new(GeneratedDidExecutor::new(
                LifecycleWitnesses,
                private_state.clone(),
                signer.clone(),
                compact_runtime::ContractAddress::default(),
            ));
            let deployment = executor.deployment_request().expect("deployment request");
            let deploy = deployment.to_contract_deploy(deployment_config.clone());
            let address_hex = format!("0x{}", hex::encode(deploy.address().0.0));

            let mut wallet_config = LocalLedger8WalletConfig::new("midnight-standalone", 3_600, 1, deployment_config);
            wallet_config.contract_calls = artifacts.call_configs;
            wallet_config.proving_keys = artifacts.proving_keys;
            let wallet = Arc::new(
                LocalLedger8WalletProvider::new(
                    wallet_config,
                    LocalLedger8DustSeed::new(seed),
                    Arc::new(FileCheckpointDustStateProvider {
                        path: checkpoint.clone(),
                    }),
                )
                .expect("local wallet"),
            );
            let proof_provider = Arc::new(HttpProofProvider::new(proof).expect("proof provider"));
            let node = Arc::new(SubxtNodeProvider::new(node_ws));
            let indexer_provider =
                Arc::new(HttpIndexerProvider::new(indexer, address_hex.clone()).expect("indexer provider"));
            let call_executor = Arc::new(GeneratedDidExecutor::new(
                LifecycleWitnesses,
                private_state.clone(),
                signer.clone(),
                deploy.address(),
            ));
            let backend = LiveBackend::with_providers(LiveProviders::new(
                call_executor,
                wallet,
                proof_provider,
                node,
                indexer_provider,
            ));

            let deployed = backend.submit_deployment(deployment).await.expect("deploy finality");
            assert!(deployed.block_height > 0);
            let initial = backend.read_snapshot().await.expect("resolve after deploy");
            assert!(initial.active);
            assert!(!initial.deactivated);

            backend
                .submit_tx(BuiltTx {
                    bytes: DidContractCall::SetVerificationMethod {
                        method: sample_method(),
                        mutation: MapMutation::Insert,
                    }
                    .encode(),
                })
                .await
                .expect("set verification method");
            let after_method = backend.read_snapshot().await.expect("resolve method");
            assert!(!after_method.deactivated);
            assert!(
                after_method.verification_methods.contains_key("#key-1"),
                "verification method must be visible after finality"
            );

            backend
                .submit_tx(BuiltTx {
                    bytes: DidContractCall::SetVerificationMethodRelation {
                        relation: LedgerVerificationMethodRelation::Authentication,
                        method_id: "#key-1".to_owned(),
                        mutation: SetMutation::Insert,
                    }
                    .encode(),
                })
                .await
                .expect("set verification method relation");
            let after_relation = backend.read_snapshot().await.expect("resolve relation");
            assert!(!after_relation.deactivated);
            assert!(
                after_relation.relation_contains(LedgerVerificationMethodRelation::Authentication, "#key-1"),
                "authentication relation must include #key-1 after finality"
            );

            let rotation_seed = [0x33; 32];
            let point = point_hex_from_seed(&rotation_seed);
            backend
                .submit_tx(BuiltTx {
                    bytes: DidContractCall::RotateControllerKey {
                        new_public_key: point.clone(),
                    }
                    .encode(),
                })
                .await
                .expect("rotate controller");
            let after_rotate = backend.read_snapshot().await.expect("resolve rotate");
            assert!(!after_rotate.deactivated);
            assert_eq!(
                after_rotate.controller_public_key_hex,
                point.x(),
                "controller key must equal the requested rotated key x-coordinate after finality"
            );
            assert_ne!(
                after_rotate.controller_public_key_hex, initial.controller_public_key_hex,
                "controller key must change after rotation finality"
            );
            signer
                .promote_controller_seed(rotation_seed)
                .expect("promote rotated signer custody");
            let mut rotated_private_state = private_state.load().expect("private state after rotate");
            rotated_private_state.controller_seed.zeroize();
            rotated_private_state.controller_seed = rotation_seed;
            private_state
                .store(rotated_private_state)
                .expect("promote rotated private state custody");

            backend
                .submit_tx(BuiltTx {
                    bytes: DidContractCall::Deactivate.encode(),
                })
                .await
                .expect("deactivate");
            let after_deactivate = backend.read_snapshot().await.expect("resolve deactivate");
            assert!(after_deactivate.deactivated);
            assert!(!after_deactivate.active);

            drop(backend);
            let restarted_checkpoint = std::fs::read(&checkpoint).expect("checkpoint survives restart");
            assert!(
                !restarted_checkpoint.is_empty(),
                "private checkpoint must persist across restart"
            );
            let restarted_indexer = HttpIndexerProvider::new(
                super::env("MIDNIGHT_DID_LEDGER8_INDEXER_URL").expect("MIDNIGHT_DID_LEDGER8_INDEXER_URL"),
                address_hex,
            )
            .expect("restarted indexer");
            let restarted_snapshot = restarted_indexer.read_snapshot().await.expect("restarted resolve");
            assert_eq!(restarted_snapshot.deactivated, after_deactivate.deactivated);
            assert_eq!(restarted_snapshot.operation_count, after_deactivate.operation_count);
        });
    }
}
/// Ignored because it spends standalone/dev funds. When the operator supplies
/// `MIDNIGHT_DID_LEDGER8_FUNDER_SEED_HEX` (or `OXID_STANDALONE_FUNDER_SEED_HEX`)
/// together with node/indexer/proof URLs, this test is the executable harness for
/// the pure-Rust lifecycle: synchronize DUST from the indexer/checkpoint,
/// construct DID deployment/calls, remote-prove, submit through Subxt
/// `Midnight.send_mn_transaction`, resolve independently after finality, rotate,
/// deactivate, recreate providers from checkpoint, and reconcile without replaying
/// a duplicate submission.
#[test]
#[ignore = "requires a live standalone stack and an operator-supplied funding seed"]
fn ledger8_standalone_ignored_pure_rust_lifecycle_requires_secret_seed() {
    let node = env("MIDNIGHT_DID_LEDGER8_NODE_URL").expect("MIDNIGHT_DID_LEDGER8_NODE_URL");
    let indexer = env("MIDNIGHT_DID_LEDGER8_INDEXER_URL").expect("MIDNIGHT_DID_LEDGER8_INDEXER_URL");
    let proof = env("MIDNIGHT_DID_LEDGER8_PROOF_URL").expect("MIDNIGHT_DID_LEDGER8_PROOF_URL");
    let seed = match standalone_funding_seed() {
        Some(seed) => seed,
        None => {
            eprintln!(
                "skip: set MIDNIGHT_DID_LEDGER8_FUNDER_SEED_HEX or OXID_STANDALONE_FUNDER_SEED_HEX to run the spending lifecycle"
            );
            return;
        }
    };
    assert_ne!(seed, [0u8; 32], "standalone funding seed must not be all-zero");

    let health = rpc(&node, "system_health", serde_json::json!([]));
    assert_eq!(health["isSyncing"], false, "node is syncing: {health}");
    let schema = graphql(&indexer, "{ __schema { queryType { fields { name } } } }");
    assert!(schema.get("data").is_some(), "indexer schema must be readable");
    let (proof_status, _) = http_request(
        "GET",
        &format!("{}/health", proof.trim_end_matches('/')),
        None,
        "application/json",
    );
    assert_eq!(proof_status, 200, "proof server must be healthy");

    #[cfg(all(feature = "http", feature = "node-subxt"))]
    {
        live_lifecycle::run();
    }
    #[cfg(not(all(feature = "http", feature = "node-subxt")))]
    {
        eprintln!("skip: rebuild this ignored lifecycle with features `http,node-subxt` to submit transactions");
    }
}

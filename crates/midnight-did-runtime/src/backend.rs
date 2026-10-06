//! I/O substrate abstraction for the Midnight DID contract.
//!
//! The [`Backend`] trait is the three-method seam between
//! [`crate::Contract<B>`]'s circuit-call surface and whichever stack is
//! actually shuttling bytes to a Midnight node. Production code wraps a
//! wallet SDK + proof server + indexer in [`LiveBackend`]; api-layer
//! tests use [`RecordingBackend`] (in-memory, records every submit);
//! the resolver consumer uses [`ResolverBackend`] (read-only snapshot).
//!
//! ## v0.4.0 — Path 2 typed-envelope strategy
//!
//! R2-2 (ADR 0008) lands the typed [`crate::contract_call::DidContractCall`]
//! envelope on top of [`Backend::submit_tx`]: `Contract<B>` serialises each
//! circuit invocation into [`BuiltTx::bytes`] via
//! [`crate::contract_call::DidContractCall::encode`], and the recording
//! backend decodes the envelope back into typed call records via
//! [`Self::recorded_calls`]. [`Backend::read_snapshot`] is a parallel read
//! path that bypasses the submit/encode round-trip and hands callers a
//! plain-data [`DidLedgerSnapshot`] directly.
//!
//! See `doc/adr/0008-contract-abstraction-reform.md` for the full
//! rationale; the original `BuiltTx`-opaque scaffold landed in R2-1 and is
//! preserved here so `LiveBackend` stays implementable once the wallet
//! bridge is in place — only the recording backend cares about the JSON
//! envelope shape.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
// Re-export the upstream raw-state types under the backend module so
// downstream consumers (api-layer tests, future custom backends) can
// implement `Backend` without taking a direct `compact-runtime` dep.
pub use compact_runtime::{ChargedState as RawChargedState, DefaultDB as RawDb};
use compact_runtime::{ChargedState, DefaultDB, empty_charged_state};

use crate::contract_call::{
    DidContractCall, DidLedgerSnapshot, JubjubPointHex, LedgerSchnorrJubjubVerificationMethod, LedgerService,
    LedgerVerificationMethod, LedgerVerificationMethodRelation, MapMutation, SetMutation,
};

/// A transaction built and proven by the upstream wallet + proof stack,
/// ready for submission via [`Backend::submit_tx`].
///
/// v0.4.0 (Path 2): the bytes are produced by
/// [`crate::contract_call::DidContractCall::encode`] when the active backend
/// is `RecordingBackend`. Once the wallet/proof bridge lands, `LiveBackend`
/// will populate this with the upstream Midnight transaction shape.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuiltTx {
    /// Opaque transaction bytes. The active backend owns the encoding;
    /// recording backends use [`DidContractCall::encode`] /
    /// [`DidContractCall::decode`].
    pub bytes: Vec<u8>,
}

/// Finalisation data for a submitted transaction.
///
/// Mirrors the shape of the legacy `midnight_did_api::contract::FinalizedTxData`.
/// Once R2-2 lands the api crate re-exports this type so the duplicate
/// definition is retired.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FinalizedTxData {
    /// Transaction hash (hex). Synthesised by [`RecordingBackend::submit_tx`]
    /// from a blake2b of the envelope bytes; empty until [`LiveBackend`]
    /// is wired.
    pub tx_hash: String,
    /// Block height the transaction was included in. Synthesised by
    /// [`RecordingBackend::submit_tx`] from the recorded-call count.
    pub block_height: u64,
}

/// Errors raised by a [`Backend`] implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// Network / RPC failure talking to the Midnight node or indexer.
    Network(String),
    /// The on-chain state, or the [`DidContractCall`] envelope, could not
    /// be decoded into the expected shape.
    Decode(String),
    /// The backend is read-only — used by [`ResolverBackend`] to reject
    /// any [`Backend::submit_tx`] call.
    ReadOnly,
    /// A required live provider is not configured.
    Unconfigured(&'static str),
    /// A live operation is structurally unsupported by the current native DTO surface.
    Unsupported(String),
    /// A provider timed out.
    Timeout(String),
    /// A provider or caller cancelled the operation.
    Cancelled(String),
    /// A finalized node receipt could not be reconciled with indexer state.
    Reconciliation(String),
    /// Catch-all for backend-specific failure modes not yet modelled.
    Other(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(m) => write!(f, "backend network failure: {m}"),
            Self::Decode(m) => write!(f, "backend decode failure: {m}"),
            Self::ReadOnly => write!(f, "backend is read-only"),
            Self::Unconfigured(p) => write!(f, "live backend provider is not configured: {p}"),
            Self::Unsupported(m) => write!(f, "unsupported live backend operation: {m}"),
            Self::Timeout(m) => write!(f, "backend timeout: {m}"),
            Self::Cancelled(m) => write!(f, "backend operation cancelled: {m}"),
            Self::Reconciliation(m) => write!(f, "backend reconciliation failure: {m}"),
            Self::Other(m) => write!(f, "backend error: {m}"),
        }
    }
}

impl std::error::Error for BackendError {}

/// Substrate-agnostic I/O abstraction for the DID contract.
///
/// Three methods: submit a (proven, signed) envelope; read the raw
/// [`ChargedState`] (used by the future wallet bridge); read a decoded
/// [`DidLedgerSnapshot`] (used today by `Contract<B>::read_snapshot`).
#[async_trait]
pub trait Backend: Send + Sync {
    /// Submit a built transaction envelope and return its finalisation data.
    async fn submit_tx(&self, tx: BuiltTx) -> Result<FinalizedTxData, BackendError>;

    /// Read the raw on-chain [`ChargedState`].
    ///
    /// Kept for the future wallet/proof bridge that will decode it
    /// alongside upstream `Ledger::<DefaultDB>::new(...)`. `Contract<B>`
    /// callers should prefer [`Self::read_snapshot`].
    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError>;

    /// Read a plain-data [`DidLedgerSnapshot`].
    ///
    /// This is the read path `Contract<B>::read_snapshot` drives. For
    /// [`LiveBackend`] this will eventually wire the `Ledger -> DidLedgerSnapshot`
    /// mapper; for [`RecordingBackend`] and [`ResolverBackend`] the snapshot
    /// is stored verbatim by the test / consumer.
    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError>;
}

// ─────────────────────────────────────────────────────────────────────
// LiveBackend
// ─────────────────────────────────────────────────────────────────────

/// Typed Ledger8 transaction accepted by proof-server `/prove-tx` before proving.
pub type LedgerProofInputTransaction = midnight_ledger::structure::Transaction<
    midnight_base_crypto::schnorr::Signature,
    midnight_ledger::structure::ProofPreimageMarker,
    midnight_transient_crypto::commitment::PedersenRandomness,
    compact_runtime::InMemoryDB,
>;

/// Typed Ledger8 transaction returned by proof-server `/prove-tx` after proving.
pub type LedgerProofOutputTransaction = midnight_ledger::structure::Transaction<
    midnight_base_crypto::schnorr::Signature,
    midnight_ledger::structure::ProofMarker,
    midnight_transient_crypto::commitment::PedersenRandomness,
    compact_runtime::InMemoryDB,
>;

/// Proving key payload map for the exact Ledger8 `/prove-tx` protocol.
pub type LedgerProofKeyMap = HashMap<String, midnight_transient_crypto::proofs::ProvingKeyMaterial>;

/// Exact typed payload consumed by Ledger8 proof-server `/prove-tx`.
pub type LedgerProveTxRequest = (LedgerProofInputTransaction, LedgerProofKeyMap);

/// Typed configuration needed to turn Compact proof material into Ledger8's
/// pre-partition contract call.
#[derive(Clone, Debug)]
pub struct LedgerContractCallConfig {
    /// Contract operation/verifier-key material for this entry point.
    pub operation: compact_runtime::ContractOperation,
    /// Communication randomness used by Ledger8 contract-call construction.
    pub communication_commitment_rand: compact_runtime::Fr,
    /// Proving-key lookup location for this circuit.
    pub key_location: midnight_transient_crypto::proofs::KeyLocation,
}

/// Secret-bearing Compact private transcript outputs.
///
/// This wrapper intentionally omits `Debug`/serde so witness output material is
/// only handed across explicit custody/prover boundaries.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct DidPrivateTranscriptOutputs(Vec<compact_runtime::AlignedValue>);

impl DidPrivateTranscriptOutputs {
    /// Borrow the ordered Compact witness outputs.
    pub fn as_slice(&self) -> &[compact_runtime::AlignedValue] {
        &self.0
    }

    /// Consume into owned ordered Compact witness outputs.
    pub fn into_vec(self) -> Vec<compact_runtime::AlignedValue> {
        self.0
    }
}

/// Compact constructor proof data and transcripts extracted from generated `initial_state`.
#[derive(Clone)]
pub struct DidConstructorProofMaterial {
    /// Stable generated constructor identifier (currently `constructor`).
    pub constructor_id: String,
    /// Contract address assigned to the constructor proof context.
    pub contract_address: compact_runtime::ContractAddress,
    /// Initial query context (pre-state/effects) for Ledger transcript replay.
    pub initial_query_context: compact_runtime::QueryContext<DefaultDB>,
    /// Final query context after constructor execution.
    pub final_query_context: compact_runtime::QueryContext<DefaultDB>,
    /// Compact-aligned public constructor input.
    pub input: compact_runtime::AlignedValue,
    /// Ledger8 public transcript operations, in Compact order.
    pub public_transcript: Vec<compact_runtime::Op<compact_runtime::ResultModeVerify, DefaultDB>>,
    /// Secret witness transcript outputs.
    pub private_transcript_outputs: DidPrivateTranscriptOutputs,
    /// Compact-aligned public constructor output.
    pub output: compact_runtime::AlignedValue,
}

impl fmt::Debug for DidConstructorProofMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DidConstructorProofMaterial")
            .field("constructor_id", &self.constructor_id)
            .field("contract_address", &self.contract_address)
            .field("input", &"<AlignedValue>")
            .field("public_transcript_len", &self.public_transcript.len())
            .field("private_transcript_outputs", &"<redacted>")
            .field("output", &"<AlignedValue>")
            .finish()
    }
}

/// Generated DID deployment material before wallet funding/proving.
///
/// The constructor is typed: callers receive both the Compact-generated initial
/// state and constructor proof material. Wallet providers can wrap the proof
/// material in Ledger8's deploy/contract-call prototype for balancing/proving.
#[derive(Clone)]
pub struct DidDeploymentRequest<PS> {
    /// Compact-generated initial on-chain contract state.
    pub initial_contract_state: ChargedState<DefaultDB>,
    /// Post-constructor private state to persist inside custody.
    pub initial_private_state: PS,
    /// Constructor-local zswap state from Compact runtime.
    pub initial_zswap_local_state: compact_runtime::ZswapLocalState<DefaultDB>,
    /// Generated constructor proof data and transcripts.
    pub constructor: DidConstructorProofMaterial,
}

/// Compact proof data and transcripts extracted from a generated DID circuit.
#[derive(Clone)]
pub struct DidPrePartitionContractCall {
    /// Generated circuit/entry-point identifier.
    pub circuit_id: String,
    /// Contract address the circuit executed against.
    pub contract_address: compact_runtime::ContractAddress,
    /// Initial query context (pre-state/effects) for Ledger transcript replay.
    pub initial_query_context: compact_runtime::QueryContext<DefaultDB>,
    /// Final query context after circuit execution.
    pub final_query_context: compact_runtime::QueryContext<DefaultDB>,
    /// Compact-aligned public input.
    pub input: compact_runtime::AlignedValue,
    /// Ledger8 public transcript operations, in Compact order.
    pub public_transcript: Vec<compact_runtime::Op<compact_runtime::ResultModeVerify, DefaultDB>>,
    /// Secret witness transcript outputs.
    pub private_transcript_outputs: DidPrivateTranscriptOutputs,
    /// Compact-aligned public output.
    pub output: compact_runtime::AlignedValue,
}

impl fmt::Debug for DidPrePartitionContractCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DidPrePartitionContractCall")
            .field("circuit_id", &self.circuit_id)
            .field("contract_address", &self.contract_address)
            .field("input", &"<AlignedValue>")
            .field("public_transcript_len", &self.public_transcript.len())
            .field("private_transcript_outputs", &"<redacted>")
            .field("output", &"<AlignedValue>")
            .finish()
    }
}

#[allow(clippy::too_many_arguments)]
fn ledger_prepartition_from_parts(
    id: &str,
    address: compact_runtime::ContractAddress,
    initial_query_context: compact_runtime::QueryContext<DefaultDB>,
    public_transcript: Vec<compact_runtime::Op<compact_runtime::ResultModeVerify, DefaultDB>>,
    private_outputs: DidPrivateTranscriptOutputs,
    input: compact_runtime::AlignedValue,
    output: compact_runtime::AlignedValue,
    config: LedgerContractCallConfig,
) -> midnight_ledger::construct::PrePartitionContractCall<DefaultDB> {
    midnight_ledger::construct::PrePartitionContractCall {
        address,
        entry_point: compact_runtime::EntryPointBuf::from(id.as_bytes()),
        op: config.operation,
        pre_transcript: midnight_ledger::construct::PreTranscript {
            context: initial_query_context,
            program: public_transcript,
            comm_comm: None,
        },
        private_transcript_outputs: private_outputs.into_vec(),
        input,
        output,
        communication_commitment_rand: config.communication_commitment_rand,
        key_location: config.key_location,
    }
}

impl DidPrePartitionContractCall {
    /// Convert generated Compact call proof material into Ledger8's typed
    /// pre-partition contract call using injected wallet/custody configuration.
    pub fn into_ledger_prepartition_contract_call(
        self,
        config: LedgerContractCallConfig,
    ) -> midnight_ledger::construct::PrePartitionContractCall<DefaultDB> {
        ledger_prepartition_from_parts(
            &self.circuit_id,
            self.contract_address,
            self.initial_query_context,
            self.public_transcript,
            self.private_transcript_outputs,
            self.input,
            self.output,
            config,
        )
    }
}

impl DidConstructorProofMaterial {
    /// Convert generated Compact constructor proof material into Ledger8's typed
    /// pre-partition contract call using injected wallet/custody configuration.
    pub fn into_ledger_prepartition_contract_call(
        self,
        config: LedgerContractCallConfig,
    ) -> midnight_ledger::construct::PrePartitionContractCall<DefaultDB> {
        ledger_prepartition_from_parts(
            &self.constructor_id,
            self.contract_address,
            self.initial_query_context,
            self.public_transcript,
            self.private_transcript_outputs,
            self.input,
            self.output,
            config,
        )
    }
}

/// Provider-native unbalanced DID transaction body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnbalancedDidTransaction {
    /// Opaque provider-native bytes. Wallet provider owns the encoding.
    pub bytes: Vec<u8>,
}

/// Provider-native balanced DID transaction body.
///
/// For the built-in HTTP proof adapter this is exactly the Ledger8 `/prove-tx`
/// request body: tagged serialization of
/// `(Transaction<Signature, ProofPreimageMarker, PedersenRandomness, InMemoryDB>,
/// HashMap<String, ProvingKeyMaterial>)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalancedDidTransaction {
    tagged_prove_tx_request: Vec<u8>,
}

impl BalancedDidTransaction {
    /// Build from the exact typed Ledger8 proof-server request tuple.
    pub fn from_ledger_prove_tx_request(request: &LedgerProveTxRequest) -> Result<Self, BackendError> {
        let mut tagged_prove_tx_request = Vec::new();
        midnight_serialize::tagged_serialize(request, &mut tagged_prove_tx_request)
            .map_err(|e| BackendError::Decode(format!("serialize /prove-tx request: {e}")))?;
        Ok(Self {
            tagged_prove_tx_request,
        })
    }

    /// Test/fake-provider constructor for already-tagged Ledger8 `/prove-tx` request bytes.
    pub fn from_tagged_prove_tx_request_bytes_for_test(tagged_prove_tx_request: Vec<u8>) -> Self {
        Self {
            tagged_prove_tx_request,
        }
    }

    /// Borrow exact tagged Ledger8 `/prove-tx` request bytes.
    pub fn tagged_prove_tx_request_bytes(&self) -> &[u8] {
        &self.tagged_prove_tx_request
    }
}

/// Provider-native proven DID transaction body.
///
/// The built-in HTTP proof adapter validates this as a tagged Ledger8 proven
/// transaction response before handing it back to custody for signing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvedDidTransaction {
    tagged_proven_tx: Vec<u8>,
}

impl ProvedDidTransaction {
    /// Build from typed Ledger8 proven transaction.
    pub fn from_ledger_proven_tx(tx: &LedgerProofOutputTransaction) -> Result<Self, BackendError> {
        let mut tagged_proven_tx = Vec::new();
        midnight_serialize::tagged_serialize(tx, &mut tagged_proven_tx)
            .map_err(|e| BackendError::Decode(format!("serialize proven tx: {e}")))?;
        Ok(Self { tagged_proven_tx })
    }

    /// Validate and wrap an exact tagged Ledger8 proven transaction response.
    pub fn from_tagged_proven_tx_response(tagged_proven_tx: Vec<u8>) -> Result<Self, BackendError> {
        let _: LedgerProofOutputTransaction = midnight_serialize::tagged_deserialize(&tagged_proven_tx[..])
            .map_err(|e| BackendError::Decode(format!("decode /prove-tx response: {e}")))?;
        Ok(Self { tagged_proven_tx })
    }

    /// Test/fake-provider constructor for tagged bytes that are not parsed.
    pub fn from_tagged_proven_tx_bytes_for_test(tagged_proven_tx: Vec<u8>) -> Self {
        Self { tagged_proven_tx }
    }

    /// Borrow exact tagged Ledger8 proven transaction bytes.
    pub fn tagged_proven_tx_bytes(&self) -> &[u8] {
        &self.tagged_proven_tx
    }
}

/// Provider-native signed DID transaction body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedDidTransaction {
    /// Provider-native bytes ready for node submission.
    pub bytes: Vec<u8>,
}

/// Durable node finality receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerFinalityReceipt {
    /// Transaction hash or canonical provider transaction id.
    pub tx_hash: String,
    /// Finalized block height.
    pub block_height: u64,
    /// Optional finality checkpoint/token.
    pub finality_token: Option<String>,
}

/// Executes generated Rust `did.compact` circuits and extracts proof data.
pub trait DidContractExecutor: Send + Sync {
    /// Execute `call` against `state`, returning Compact proof data for Ledger8 construction.
    fn execute(
        &self,
        state: ChargedState<DefaultDB>,
        call: DidContractCall,
    ) -> Result<DidPrePartitionContractCall, BackendError>;
}

/// Custody signer for controller/recovery authorization digests.
pub trait DidAuthorizationSigner: Send + Sync {
    /// Sign a controller-authorized digest inside custody.
    fn sign_controller(
        &self,
        digest: [compact_runtime::Fr; 4],
    ) -> Result<compact_runtime::SchnorrSignature, BackendError>;

    /// Sign a recovery-authorized digest inside custody.
    fn sign_recovery(
        &self,
        digest: [compact_runtime::Fr; 4],
    ) -> Result<compact_runtime::SchnorrSignature, BackendError>;
}

/// Private-state source/sink. Implementations keep secrets inside custody.
pub trait DidPrivateStateStore<PS>: Send + Sync {
    /// Load current private state.
    fn load(&self) -> Result<PS, BackendError>;
    /// Persist the post-circuit private state.
    fn store(&self, state: PS) -> Result<(), BackendError>;
}

/// Generated contract executor over concrete Compact witnesses and custody.
pub struct GeneratedDidExecutor<PS, W> {
    witnesses: W,
    private_state: Arc<dyn DidPrivateStateStore<PS>>,
    signer: Arc<dyn DidAuthorizationSigner>,
    contract_address: compact_runtime::ContractAddress,
}

impl<PS, W> fmt::Debug for GeneratedDidExecutor<PS, W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeneratedDidExecutor")
            .field("witnesses", &"<Witnesses>")
            .field("private_state", &"<DidPrivateStateStore>")
            .field("signer", &"<DidAuthorizationSigner>")
            .field("contract_address", &self.contract_address)
            .finish()
    }
}

impl<PS, W> GeneratedDidExecutor<PS, W> {
    /// Build a generated executor from witness, private-state, and custody providers.
    pub fn new(
        witnesses: W,
        private_state: Arc<dyn DidPrivateStateStore<PS>>,
        signer: Arc<dyn DidAuthorizationSigner>,
        contract_address: compact_runtime::ContractAddress,
    ) -> Self {
        Self {
            witnesses,
            private_state,
            signer,
            contract_address,
        }
    }

    /// Execute the Compact constructor and return typed deployment material.
    pub fn deployment_request(&self) -> Result<DidDeploymentRequest<PS>, BackendError>
    where
        PS: Clone,
        W: crate::contract::Witnesses<PS> + Clone,
    {
        let private_state = self.private_state.load()?;
        let ctx = compact_runtime::ConstructorContext {
            initial_private_state: private_state,
            empty_zswap_local_state: compact_runtime::ZswapLocalState::new(),
            cost_model: compact_runtime::INITIAL_COST_MODEL.clone(),
            gas_limit: None,
        };
        let result = crate::contract::Contract::new(self.witnesses.clone())
            .initial_state(ctx)
            .map_err(|e| BackendError::Other(format!("generated initial_state: {e}")))?;
        self.private_state.store(result.current_private_state.clone())?;
        let constructor = constructor_material_from_generated(result.constructor_proof_data);
        Ok(DidDeploymentRequest {
            initial_contract_state: result.current_contract_state,
            initial_private_state: result.current_private_state,
            initial_zswap_local_state: result.current_zswap_local_state,
            constructor,
        })
    }
}

impl<PS, W> DidContractExecutor for GeneratedDidExecutor<PS, W>
where
    PS: Clone + Send + Sync + 'static,
    W: crate::contract::Witnesses<PS> + Clone + Send + Sync + 'static,
{
    fn execute(
        &self,
        state: ChargedState<DefaultDB>,
        call: DidContractCall,
    ) -> Result<DidPrePartitionContractCall, BackendError> {
        use crate::contract as generated;

        let expected_version = generated::ledger(&state)
            .version()
            .map_err(|e| BackendError::Decode(format!("ledger version: {e}")))?;
        let contract_id = generated::ledger(&state)
            .id()
            .map_err(|e| BackendError::Decode(format!("ledger id: {e}")))?;
        let private_state = self.private_state.load()?;
        let mut ctx = compact_runtime::CircuitContext::new(state, private_state);
        ctx.current_query_context.address = self.contract_address;
        let contract = generated::Contract::new(self.witnesses.clone());
        let circuit = circuit_name(&call)?;

        let out = match call {
            DidContractCall::RotateControllerKey { new_public_key } => {
                let pk = point_from_hex(&new_public_key)?;
                let digest = generated::pure_circuits::rotate_controller_key_authorization_digest(
                    contract_id,
                    expected_version,
                    pk,
                )
                .map_err(|e| BackendError::Other(format!("rotate digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .rotate_controller_key(ctx, pk, sig, expected_version)
                    .map_err(|e| BackendError::Other(format!("generated rotate_controller_key: {e}")))?
            }
            DidContractCall::RecoverControllerKey { new_public_key } => {
                let pk = point_from_hex(&new_public_key)?;
                let digest = generated::pure_circuits::recover_controller_key_authorization_digest(
                    contract_id,
                    expected_version,
                    pk,
                )
                .map_err(|e| BackendError::Other(format!("recover digest: {e}")))?;
                let sig = self.signer.sign_recovery(digest)?;
                contract
                    .recover_controller_key(ctx, pk, sig, expected_version)
                    .map_err(|e| BackendError::Other(format!("generated recover_controller_key: {e}")))?
            }
            DidContractCall::SetVerificationMethod { method, mutation } => {
                let method = verification_method_to_generated(method);
                let mutation = map_mutation_to_generated(mutation);
                let digest = generated::pure_circuits::set_verification_method_authorization_digest(
                    contract_id,
                    expected_version,
                    method.clone(),
                    mutation,
                )
                .map_err(|e| BackendError::Other(format!("set VM digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .set_verification_method(ctx, method, mutation, sig, expected_version)
                    .map_err(|e| BackendError::Other(format!("generated set_verification_method: {e}")))?
            }
            DidContractCall::RemoveVerificationMethod { method_id } => {
                let id = opaque(method_id);
                let digest = generated::pure_circuits::remove_verification_method_authorization_digest(
                    contract_id,
                    expected_version,
                    id.clone(),
                )
                .map_err(|e| BackendError::Other(format!("remove VM digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .remove_verification_method(ctx, id, sig, expected_version)
                    .map_err(|e| BackendError::Other(format!("generated remove_verification_method: {e}")))?
            }
            DidContractCall::SetSchnorrJubjubVerificationMethod { method, mutation } => {
                let method = schnorr_vm_to_generated(method)?;
                let mutation = map_mutation_to_generated(mutation);
                let digest = generated::pure_circuits::set_schnorr_jubjub_verification_method_authorization_digest(
                    contract_id,
                    expected_version,
                    method.clone(),
                    mutation,
                )
                .map_err(|e| BackendError::Other(format!("set Schnorr VM digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .set_schnorr_jubjub_verification_method(ctx, method, mutation, sig, expected_version)
                    .map_err(|e| {
                        BackendError::Other(format!("generated set_schnorr_jubjub_verification_method: {e}"))
                    })?
            }
            DidContractCall::RemoveSchnorrJubjubVerificationMethod { method_id } => {
                let id = opaque(method_id);
                let digest = generated::pure_circuits::remove_schnorr_jubjub_verification_method_authorization_digest(
                    contract_id,
                    expected_version,
                    id.clone(),
                )
                .map_err(|e| BackendError::Other(format!("remove Schnorr VM digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .remove_schnorr_jubjub_verification_method(ctx, id, sig, expected_version)
                    .map_err(|e| {
                        BackendError::Other(format!("generated remove_schnorr_jubjub_verification_method: {e}"))
                    })?
            }
            DidContractCall::VerifySchnorrJubjubDigestSignature {
                method_id,
                digest,
                signature,
            } => contract
                .verify_schnorr_jubjub_digest_signature(
                    ctx,
                    opaque(method_id),
                    digest_to_generated(&digest)?,
                    signature_to_generated(&signature)?,
                )
                .map_err(|e| BackendError::Other(format!("generated verify_schnorr_jubjub_digest_signature: {e}")))?,
            DidContractCall::SetVerificationMethodRelation {
                relation,
                method_id,
                mutation,
            } => {
                let relation = relation_to_generated(relation);
                let id = opaque(method_id);
                let mutation = set_mutation_to_generated(mutation);
                let digest = generated::pure_circuits::set_verification_method_relation_authorization_digest(
                    contract_id,
                    expected_version,
                    relation,
                    id.clone(),
                    mutation,
                )
                .map_err(|e| BackendError::Other(format!("set relation digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .set_verification_method_relation(ctx, relation, id, mutation, sig, expected_version)
                    .map_err(|e| BackendError::Other(format!("generated set_verification_method_relation: {e}")))?
            }
            DidContractCall::SetService { service, mutation } => {
                let service = service_to_generated(service);
                let mutation = map_mutation_to_generated(mutation);
                let digest = generated::pure_circuits::set_service_authorization_digest(
                    contract_id,
                    expected_version,
                    service.clone(),
                    mutation,
                )
                .map_err(|e| BackendError::Other(format!("set service digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .set_service(ctx, service, mutation, sig, expected_version)
                    .map_err(|e| BackendError::Other(format!("generated set_service: {e}")))?
            }
            DidContractCall::RemoveService { service_id } => {
                let id = opaque(service_id);
                let digest = generated::pure_circuits::remove_service_authorization_digest(
                    contract_id,
                    expected_version,
                    id.clone(),
                )
                .map_err(|e| BackendError::Other(format!("remove service digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .remove_service(ctx, id, sig, expected_version)
                    .map_err(|e| BackendError::Other(format!("generated remove_service: {e}")))?
            }
            DidContractCall::SetAlsoKnownAs { alias_uri, mutation } => {
                let alias = opaque(alias_uri);
                let mutation = set_mutation_to_generated(mutation);
                let digest = generated::pure_circuits::set_also_known_as_authorization_digest(
                    contract_id,
                    expected_version,
                    alias.clone(),
                    mutation,
                )
                .map_err(|e| BackendError::Other(format!("set aka digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .set_also_known_as(ctx, alias, mutation, sig, expected_version)
                    .map_err(|e| BackendError::Other(format!("generated set_also_known_as: {e}")))?
            }
            DidContractCall::Deactivate => {
                let digest = generated::pure_circuits::deactivate_authorization_digest(contract_id, expected_version)
                    .map_err(|e| BackendError::Other(format!("deactivate digest: {e}")))?;
                let sig = self.signer.sign_controller(digest)?;
                contract
                    .deactivate(ctx, sig, expected_version)
                    .map_err(|e| BackendError::Other(format!("generated deactivate: {e}")))?
            }
            DidContractCall::ReadLedger => {
                return Err(BackendError::Decode("ReadLedger is not a mutating circuit".into()));
            }
        };
        self.private_state.store(out.context.current_private_state.clone())?;
        proof_call_from_trace(out.context.call_proof_data_trace, circuit)
    }
}

fn circuit_name(call: &DidContractCall) -> Result<&'static str, BackendError> {
    Ok(match call {
        DidContractCall::ReadLedger => return Err(BackendError::Decode("ReadLedger is not a mutating circuit".into())),
        DidContractCall::RotateControllerKey { .. } => "rotate_controller_key",
        DidContractCall::RecoverControllerKey { .. } => "recover_controller_key",
        DidContractCall::SetVerificationMethod { .. } => "set_verification_method",
        DidContractCall::RemoveVerificationMethod { .. } => "remove_verification_method",
        DidContractCall::SetSchnorrJubjubVerificationMethod { .. } => "set_schnorr_jubjub_verification_method",
        DidContractCall::RemoveSchnorrJubjubVerificationMethod { .. } => "remove_schnorr_jubjub_verification_method",
        DidContractCall::VerifySchnorrJubjubDigestSignature { .. } => "verify_schnorr_jubjub_digest_signature",
        DidContractCall::SetVerificationMethodRelation { .. } => "set_verification_method_relation",
        DidContractCall::SetService { .. } => "set_service",
        DidContractCall::RemoveService { .. } => "remove_service",
        DidContractCall::SetAlsoKnownAs { .. } => "set_also_known_as",
        DidContractCall::Deactivate => "deactivate",
    })
}

fn opaque(s: String) -> compact_runtime::std_lib::OpaqueString {
    compact_runtime::std_lib::OpaqueString::from(s)
}

fn fr_from_hex(s: &str, field: &str) -> Result<compact_runtime::Fr, BackendError> {
    let bytes = hex::decode(s).map_err(|e| BackendError::Decode(format!("{field}: {e}")))?;
    compact_runtime::Fr::from_le_bytes(&bytes).ok_or_else(|| BackendError::Decode(format!("{field}: invalid Fr")))
}

fn point_from_hex(p: &JubjubPointHex) -> Result<compact_runtime::JubjubPoint, BackendError> {
    let x = fr_from_hex(p.x(), "JubjubPoint.x")?;
    let y = fr_from_hex(p.y(), "JubjubPoint.y")?;
    compact_runtime::jubjub_point_from_field_repr(&[x, y])
        .ok_or_else(|| BackendError::Decode("invalid Jubjub point".into()))
}

fn digest_to_generated(
    d: &crate::contract_call::SchnorrJubjubDigest,
) -> Result<[compact_runtime::Fr; 4], BackendError> {
    Ok([
        fr_from_hex(&d.limbs()[0], "digest[0]")?,
        fr_from_hex(&d.limbs()[1], "digest[1]")?,
        fr_from_hex(&d.limbs()[2], "digest[2]")?,
        fr_from_hex(&d.limbs()[3], "digest[3]")?,
    ])
}

fn signature_to_generated(
    sig: &crate::contract_call::SchnorrJubjubSignature,
) -> Result<compact_runtime::SchnorrSignature, BackendError> {
    let bytes = hex::decode(sig.bytes_hex()).map_err(|e| BackendError::Decode(format!("signature: {e}")))?;
    if bytes.len() != 96 {
        return Err(BackendError::Decode("signature must be 96 bytes".into()));
    }
    let x = compact_runtime::Fr::from_le_bytes(&bytes[0..32])
        .ok_or_else(|| BackendError::Decode("signature R.x".into()))?;
    let y = compact_runtime::Fr::from_le_bytes(&bytes[32..64])
        .ok_or_else(|| BackendError::Decode("signature R.y".into()))?;
    let response = compact_runtime::Fr::from_le_bytes(&bytes[64..96])
        .ok_or_else(|| BackendError::Decode("signature response".into()))?;
    let announcement = compact_runtime::jubjub_point_from_field_repr(&[x, y])
        .ok_or_else(|| BackendError::Decode("signature announcement".into()))?;
    Ok(compact_runtime::SchnorrSignature { announcement, response })
}

fn map_mutation_to_generated(m: MapMutation) -> crate::contract::MapMutation {
    match m {
        MapMutation::Insert => crate::contract::MapMutation::Insert,
        MapMutation::Update => crate::contract::MapMutation::Update,
    }
}
fn set_mutation_to_generated(m: SetMutation) -> crate::contract::SetMutation {
    match m {
        SetMutation::Insert => crate::contract::SetMutation::Insert,
        SetMutation::Remove => crate::contract::SetMutation::Remove,
    }
}
fn relation_to_generated(r: LedgerVerificationMethodRelation) -> crate::contract::VerificationMethodRelation {
    match r {
        LedgerVerificationMethodRelation::Undefined => crate::contract::VerificationMethodRelation::Undefined,
        LedgerVerificationMethodRelation::Authentication => crate::contract::VerificationMethodRelation::Authentication,
        LedgerVerificationMethodRelation::AssertionMethod => {
            crate::contract::VerificationMethodRelation::AssertionMethod
        }
        LedgerVerificationMethodRelation::KeyAgreement => crate::contract::VerificationMethodRelation::KeyAgreement,
        LedgerVerificationMethodRelation::CapabilityInvocation => {
            crate::contract::VerificationMethodRelation::CapabilityInvocation
        }
        LedgerVerificationMethodRelation::CapabilityDelegation => {
            crate::contract::VerificationMethodRelation::CapabilityDelegation
        }
    }
}

fn vm_type_to_generated(
    v: midnight_did_domain::did_document::VerificationMethodType,
) -> crate::contract::VerificationMethodType {
    match v {
        midnight_did_domain::did_document::VerificationMethodType::Undefined => {
            crate::contract::VerificationMethodType::Undefined
        }
        midnight_did_domain::did_document::VerificationMethodType::JsonWebKey => {
            crate::contract::VerificationMethodType::JsonWebKey
        }
    }
}
fn key_type_to_generated(v: midnight_did_domain::did_document::KeyType) -> crate::contract::KeyType {
    match v {
        midnight_did_domain::did_document::KeyType::EC => crate::contract::KeyType::EC,
        midnight_did_domain::did_document::KeyType::RSA => crate::contract::KeyType::RSA,
        midnight_did_domain::did_document::KeyType::oct => crate::contract::KeyType::oct,
        midnight_did_domain::did_document::KeyType::OKP => crate::contract::KeyType::OKP,
    }
}
fn curve_type_to_generated(v: midnight_did_domain::did_document::CurveType) -> crate::contract::CurveType {
    match v {
        midnight_did_domain::did_document::CurveType::Ed25519 => crate::contract::CurveType::Ed25519,
        midnight_did_domain::did_document::CurveType::X25519 => crate::contract::CurveType::X25519,
        midnight_did_domain::did_document::CurveType::Jubjub => crate::contract::CurveType::Jubjub,
        midnight_did_domain::did_document::CurveType::P256 => crate::contract::CurveType::P256,
        midnight_did_domain::did_document::CurveType::Secp256k1 => crate::contract::CurveType::Secp256k1,
        midnight_did_domain::did_document::CurveType::BLS12381G1 => crate::contract::CurveType::BLS12381G1,
        midnight_did_domain::did_document::CurveType::BLS12381G2 => crate::contract::CurveType::BLS12381G2,
    }
}
fn verification_method_to_generated(m: LedgerVerificationMethod) -> crate::contract::VerificationMethod {
    crate::contract::VerificationMethod {
        id: opaque(m.id),
        typ: vm_type_to_generated(m.typ),
        publicKeyJwk: crate::contract::PublicKeyJwk {
            kty: key_type_to_generated(m.public_key_jwk.kty),
            crv: curve_type_to_generated(m.public_key_jwk.crv),
            x: opaque(m.public_key_jwk.x),
            y: opaque(m.public_key_jwk.y),
        },
    }
}
fn schnorr_vm_to_generated(
    m: LedgerSchnorrJubjubVerificationMethod,
) -> Result<crate::contract::SchnorrJubjubVerificationMethod, BackendError> {
    Ok(crate::contract::SchnorrJubjubVerificationMethod {
        id: opaque(m.id),
        publicKey: point_from_hex(&m.public_key)?,
    })
}
fn service_to_generated(s: LedgerService) -> crate::contract::Service {
    crate::contract::Service {
        id: opaque(s.id),
        typ: opaque(s.typ),
        serviceEndpoint: opaque(s.service_endpoint),
    }
}

fn constructor_material_from_generated(
    proof: compact_runtime::ConstructorProofData<DefaultDB>,
) -> DidConstructorProofMaterial {
    let (input, public_transcript, private_outputs, output) = proof.proof_data.into_parts();
    DidConstructorProofMaterial {
        constructor_id: proof.constructor_id,
        contract_address: proof.contract_address,
        initial_query_context: proof.initial_query_context,
        final_query_context: proof.final_query_context,
        input,
        public_transcript,
        private_transcript_outputs: DidPrivateTranscriptOutputs(private_outputs.into_vec()),
        output,
    }
}

fn proof_call_from_trace(
    trace: compact_runtime::CallProofDataTrace<DefaultDB>,
    circuit: &str,
) -> Result<DidPrePartitionContractCall, BackendError> {
    let call = trace
        .into_vec()
        .into_iter()
        .rev()
        .find(|c| c.circuit_id == circuit)
        .ok_or_else(|| BackendError::Decode(format!("generated proof trace did not contain {circuit}")))?;
    let (input, public_transcript, private_outputs, output) = call.proof_data.into_parts();
    Ok(DidPrePartitionContractCall {
        circuit_id: call.circuit_id,
        contract_address: call.contract_address,
        initial_query_context: call.initial_query_context,
        final_query_context: call.final_query_context,
        input,
        public_transcript,
        private_transcript_outputs: DidPrivateTranscriptOutputs(private_outputs.into_vec()),
        output,
    })
}

/// Wallet/custody provider for funding, balancing, and signing.
#[async_trait]
pub trait LedgerWalletProvider: Send + Sync {
    /// Materialize a provider-native unbalanced transaction from generated constructor proof data.
    async fn build_deployment_tx(
        &self,
        deployment: DidConstructorProofMaterial,
    ) -> Result<UnbalancedDidTransaction, BackendError>;

    /// Materialize a provider-native unbalanced transaction from generated proof data.
    async fn build_unbalanced_tx(
        &self,
        call: DidPrePartitionContractCall,
    ) -> Result<UnbalancedDidTransaction, BackendError>;

    /// Fund and fee-balance an unbalanced transaction.
    async fn balance_tx(&self, tx: UnbalancedDidTransaction) -> Result<BalancedDidTransaction, BackendError>;

    /// Sign a proven transaction inside custody.
    async fn sign_tx(&self, tx: ProvedDidTransaction) -> Result<SignedDidTransaction, BackendError>;
}

/// Proof-server provider.
#[async_trait]
pub trait LedgerProofProvider: Send + Sync {
    /// Ask the proof service to prove the balanced transaction.
    async fn prove_tx(&self, tx: BalancedDidTransaction) -> Result<ProvedDidTransaction, BackendError>;
}

/// Node submission/finality provider.
#[async_trait]
pub trait LedgerNodeProvider: Send + Sync {
    /// Submit and wait for durable finality.
    async fn submit_and_wait(&self, tx: SignedDidTransaction) -> Result<LedgerFinalityReceipt, BackendError>;
}

/// Indexer/public-data provider.
#[async_trait]
pub trait LedgerIndexerProvider: Send + Sync {
    /// Read serialized contract state bytes from the indexer.
    async fn read_state_bytes(&self) -> Result<Vec<u8>, BackendError>;

    /// Read raw charged state decoded from indexer bytes.
    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError> {
        crate::state_decode::charged_state_from_bytes(&self.read_state_bytes().await?)
    }

    /// Read DID snapshot decoded from live ledger state.
    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        let state = self.read_state().await?;
        crate::state_decode::decode_ledger_snapshot(&state)
    }

    /// Confirm indexer has observed the finalized transaction/state.
    async fn reconcile_finality(&self, receipt: &LedgerFinalityReceipt) -> Result<(), BackendError>;
}

/// Dependency-injected live providers.
#[derive(Clone)]
pub struct LiveProviders {
    executor: Arc<dyn DidContractExecutor>,
    wallet: Arc<dyn LedgerWalletProvider>,
    proof: Arc<dyn LedgerProofProvider>,
    node: Arc<dyn LedgerNodeProvider>,
    indexer: Arc<dyn LedgerIndexerProvider>,
}

impl fmt::Debug for LiveProviders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveProviders")
            .field("executor", &"<DidContractExecutor>")
            .field("wallet", &"<LedgerWalletProvider>")
            .field("proof", &"<LedgerProofProvider>")
            .field("node", &"<LedgerNodeProvider>")
            .field("indexer", &"<LedgerIndexerProvider>")
            .finish()
    }
}

impl LiveProviders {
    /// Create a provider bundle.
    pub fn new(
        executor: Arc<dyn DidContractExecutor>,
        wallet: Arc<dyn LedgerWalletProvider>,
        proof: Arc<dyn LedgerProofProvider>,
        node: Arc<dyn LedgerNodeProvider>,
        indexer: Arc<dyn LedgerIndexerProvider>,
    ) -> Self {
        Self {
            executor,
            wallet,
            proof,
            node,
            indexer,
        }
    }
}

/// Production backend: generated DID Compact execution plus injected Ledger8 providers.
#[derive(Debug, Default, Clone)]
pub struct LiveBackend {
    providers: Option<LiveProviders>,
}

impl LiveBackend {
    /// Construct an unconfigured backend. Live methods return typed errors, not panics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a configured backend.
    pub fn with_providers(providers: LiveProviders) -> Self {
        Self {
            providers: Some(providers),
        }
    }

    fn providers(&self) -> Result<&LiveProviders, BackendError> {
        self.providers
            .as_ref()
            .ok_or(BackendError::Unconfigured("LiveProviders"))
    }

    /// Submit a generated constructor/deployment request through the same
    /// wallet/proof/node/finality pipeline used by mutating calls.
    pub async fn submit_deployment<PS>(
        &self,
        deployment: DidDeploymentRequest<PS>,
    ) -> Result<FinalizedTxData, BackendError> {
        let providers = self.providers()?;
        let unbalanced = providers.wallet.build_deployment_tx(deployment.constructor).await?;
        let balanced = providers.wallet.balance_tx(unbalanced).await?;
        let proved = providers.proof.prove_tx(balanced).await?;
        let signed = providers.wallet.sign_tx(proved).await?;
        let receipt = providers.node.submit_and_wait(signed).await?;
        providers.indexer.reconcile_finality(&receipt).await?;
        Ok(FinalizedTxData {
            tx_hash: receipt.tx_hash,
            block_height: receipt.block_height,
        })
    }
}

#[async_trait]
impl Backend for LiveBackend {
    async fn submit_tx(&self, tx: BuiltTx) -> Result<FinalizedTxData, BackendError> {
        let providers = self.providers()?;
        let call = DidContractCall::decode(&tx.bytes)?;
        let state = providers.indexer.read_state().await?;
        let proof_call = providers.executor.execute(state, call)?;
        let unbalanced = providers.wallet.build_unbalanced_tx(proof_call).await?;
        let balanced = providers.wallet.balance_tx(unbalanced).await?;
        let proved = providers.proof.prove_tx(balanced).await?;
        let signed = providers.wallet.sign_tx(proved).await?;
        let receipt = providers.node.submit_and_wait(signed).await?;
        providers.indexer.reconcile_finality(&receipt).await?;
        Ok(FinalizedTxData {
            tx_hash: receipt.tx_hash,
            block_height: receipt.block_height,
        })
    }

    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError> {
        self.providers()?.indexer.read_state().await
    }

    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        self.providers()?.indexer.read_snapshot().await
    }
}

#[cfg(feature = "http")]
/// HTTP proof-server adapter for Ledger8 `/prove-tx` tagged transaction payloads.
#[derive(Debug, Clone)]
pub struct HttpProofProvider {
    client: reqwest::Client,
    prove_tx_url: String,
}

#[cfg(feature = "http")]
impl HttpProofProvider {
    /// Create a proof adapter. `base_url` may be the server root or `/prove-tx` endpoint.
    ///
    /// The body is the exact Ledger8 tagged-serialize payload
    /// `(Transaction<Signature, ProofPreimageMarker, PedersenRandomness, InMemoryDB>,
    /// HashMap<String, ProvingKeyMaterial>)` expected by
    /// `proof-server/src/endpoints.rs::prove_transaction`; responses are validated as
    /// tagged proven transactions before custody signing.
    pub fn new(base_url: impl Into<String>) -> Result<Self, BackendError> {
        let mut prove_tx_url = base_url.into();
        if !prove_tx_url.ends_with("/prove-tx") {
            prove_tx_url = format!("{}/prove-tx", prove_tx_url.trim_end_matches('/'));
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| BackendError::Network(format!("proof client: {e}")))?;
        Ok(Self { client, prove_tx_url })
    }
}

#[cfg(feature = "http")]
#[async_trait]
impl LedgerProofProvider for HttpProofProvider {
    async fn prove_tx(&self, tx: BalancedDidTransaction) -> Result<ProvedDidTransaction, BackendError> {
        let response = self
            .client
            .post(&self.prove_tx_url)
            .body(tx.tagged_prove_tx_request_bytes().to_vec())
            .send()
            .await
            .map_err(|e| BackendError::Network(format!("proof transport: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(BackendError::Network(format!("prove-tx HTTP {status}")));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| BackendError::Network(format!("proof body: {e}")))?;
        ProvedDidTransaction::from_tagged_proven_tx_response(bytes.to_vec())
    }
}

#[cfg(feature = "http")]
/// Generic JSON-RPC node adapter for transaction submission/finality.
#[derive(Debug, Clone)]
pub struct HttpNodeProvider {
    client: reqwest::Client,
    rpc_url: String,
    submit_method: String,
}

#[cfg(feature = "http")]
impl HttpNodeProvider {
    /// Create a node adapter. The method is intentionally configurable because
    /// standalone/node deployments name the submit/finality RPC differently.
    pub fn new(rpc_url: impl Into<String>, submit_method: impl Into<String>) -> Result<Self, BackendError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| BackendError::Network(format!("node client: {e}")))?;
        Ok(Self {
            client,
            rpc_url: rpc_url.into(),
            submit_method: submit_method.into(),
        })
    }
}

#[cfg(feature = "http")]
#[async_trait]
impl LedgerNodeProvider for HttpNodeProvider {
    async fn submit_and_wait(&self, tx: SignedDidTransaction) -> Result<LedgerFinalityReceipt, BackendError> {
        #[derive(serde::Deserialize)]
        struct RpcResponse {
            result: Option<serde_json::Value>,
            error: Option<serde_json::Value>,
        }
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": self.submit_method,
            "params": [format!("0x{}", hex::encode(tx.bytes))],
        });
        let response = self
            .client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Network(format!("node transport: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(BackendError::Network(format!("node HTTP {status}")));
        }
        let payload: RpcResponse = response
            .json()
            .await
            .map_err(|e| BackendError::Decode(format!("node response: {e}")))?;
        if let Some(err) = payload.error {
            return Err(BackendError::Network(format!("node RPC error: {err}")));
        }
        let result = payload
            .result
            .ok_or_else(|| BackendError::Decode("node response missing result".into()))?;
        let tx_hash = result
            .get("txHash")
            .or_else(|| result.get("hash"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let block_height = result
            .get("blockHeight")
            .or_else(|| result.get("height"))
            .and_then(|v| v.as_u64())
            .unwrap_or_default();
        if tx_hash.is_empty() || block_height == 0 {
            return Err(BackendError::Decode("node result missing txHash/blockHeight".into()));
        }
        Ok(LedgerFinalityReceipt {
            tx_hash,
            block_height,
            finality_token: result
                .get("finalityToken")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
        })
    }
}

#[cfg(feature = "http")]
/// Minimal GraphQL-over-HTTP indexer adapter for `contractAction(address){state}`.
#[derive(Debug, Clone)]
pub struct HttpIndexerProvider {
    client: reqwest::Client,
    graphql_url: String,
    address_hex: String,
}

#[cfg(feature = "http")]
impl HttpIndexerProvider {
    /// Create an HTTP indexer provider.
    pub fn new(graphql_url: impl Into<String>, address_hex: impl Into<String>) -> Result<Self, BackendError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| BackendError::Network(format!("indexer client: {e}")))?;
        Ok(Self {
            client,
            graphql_url: graphql_url.into(),
            address_hex: address_hex.into(),
        })
    }
}

#[cfg(feature = "http")]
#[async_trait]
impl LedgerIndexerProvider for HttpIndexerProvider {
    async fn read_state_bytes(&self) -> Result<Vec<u8>, BackendError> {
        #[derive(serde::Deserialize)]
        struct GraphQlResponse {
            data: Option<Data>,
            errors: Option<Vec<GraphQlError>>,
        }
        #[derive(serde::Deserialize)]
        struct Data {
            #[serde(rename = "contractAction")]
            contract_action: Option<Action>,
        }
        #[derive(serde::Deserialize)]
        struct Action {
            state: String,
        }
        #[derive(serde::Deserialize)]
        struct GraphQlError {
            message: String,
        }
        let body = serde_json::json!({
            "query": "query CONTRACT_STATE_QUERY($address: HexEncoded!, $offset: ContractActionOffset) { contractAction(address: $address, offset: $offset) { state } }",
            "variables": { "address": self.address_hex, "offset": null },
        });
        let response = self
            .client
            .post(&self.graphql_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Network(format!("indexer transport: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(BackendError::Network(format!("indexer HTTP {status}")));
        }
        let payload: GraphQlResponse = response
            .json()
            .await
            .map_err(|e| BackendError::Decode(format!("indexer response: {e}")))?;
        if let Some(errors) = payload.errors
            && !errors.is_empty()
        {
            return Err(BackendError::Network(
                errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; "),
            ));
        }
        let state = payload
            .data
            .and_then(|d| d.contract_action)
            .ok_or_else(|| BackendError::Other(format!("no contract state for {}", self.address_hex)))?
            .state;
        hex::decode(state.trim_start_matches("0x")).map_err(|e| BackendError::Decode(format!("indexer state hex: {e}")))
    }

    async fn reconcile_finality(&self, _receipt: &LedgerFinalityReceipt) -> Result<(), BackendError> {
        let _ = self.read_state_bytes().await?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────
// RecordingBackend
// ─────────────────────────────────────────────────────────────────────

/// In-memory mock backend used by api-layer tests.
///
/// [`Backend::submit_tx`] decodes the envelope into a [`DidContractCall`]
/// and pushes it onto an internal call list ([`Self::recorded_calls`]).
/// [`Backend::read_state`] returns a clone of the stored
/// [`ChargedState`] (defaults to [`empty_charged_state`]).
/// [`Backend::read_snapshot`] returns a clone of the stored
/// [`DidLedgerSnapshot`] and records a synthetic
/// [`DidContractCall::ReadLedger`] entry to preserve the legacy
/// `RecordedCall::ReadLedger` test-sequencing semantics.
pub struct RecordingBackend {
    calls: Mutex<Vec<DidContractCall>>,
    state: Mutex<ChargedState<DefaultDB>>,
    snapshot: Mutex<DidLedgerSnapshot>,
}

impl fmt::Debug for RecordingBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let calls_len = self.calls.lock().map(|v| v.len()).unwrap_or(0);
        f.debug_struct("RecordingBackend")
            .field("recorded_call_count", &calls_len)
            .field("state", &"<ChargedState<DefaultDB>>")
            .field("snapshot", &"<DidLedgerSnapshot>")
            .finish()
    }
}

impl Default for RecordingBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordingBackend {
    /// Construct a fresh [`RecordingBackend`] with no recorded calls and
    /// an empty [`ChargedState`] + default [`DidLedgerSnapshot`].
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            state: Mutex::new(empty_charged_state::<DefaultDB>()),
            snapshot: Mutex::new(DidLedgerSnapshot::default()),
        }
    }

    /// Construct a [`RecordingBackend`] seeded with a specific [`ChargedState`].
    pub fn with_state(state: ChargedState<DefaultDB>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            state: Mutex::new(state),
            snapshot: Mutex::new(DidLedgerSnapshot::default()),
        }
    }

    /// Construct a [`RecordingBackend`] seeded with a specific [`DidLedgerSnapshot`].
    pub fn with_snapshot(snapshot: DidLedgerSnapshot) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            state: Mutex::new(empty_charged_state::<DefaultDB>()),
            snapshot: Mutex::new(snapshot),
        }
    }

    /// Snapshot of every decoded [`DidContractCall`] in submission order
    /// (including synthetic [`DidContractCall::ReadLedger`] entries pushed
    /// by [`Self::read_snapshot`]).
    pub fn recorded_calls(&self) -> Vec<DidContractCall> {
        self.calls.lock().unwrap().clone()
    }

    /// Replace the [`ChargedState`] returned by [`Backend::read_state`].
    pub fn set_state(&self, state: ChargedState<DefaultDB>) {
        *self.state.lock().unwrap() = state;
    }

    /// Replace the snapshot returned by [`Backend::read_snapshot`].
    pub fn set_snapshot(&self, snapshot: DidLedgerSnapshot) {
        *self.snapshot.lock().unwrap() = snapshot;
    }
}

#[async_trait]
impl Backend for RecordingBackend {
    async fn submit_tx(&self, tx: BuiltTx) -> Result<FinalizedTxData, BackendError> {
        let call = DidContractCall::decode(&tx.bytes)?;
        let mut calls = self.calls.lock().unwrap();
        calls.push(call);
        // Synthesise deterministic finalisation data from the recorded
        // count + the envelope bytes so callers that look at
        // `tx_hash` / `block_height` see consistent values.
        let block_height = calls.len() as u64;
        let tx_hash = synth_tx_hash(&tx.bytes);
        Ok(FinalizedTxData { tx_hash, block_height })
    }

    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError> {
        Ok(self.state.lock().unwrap().clone())
    }

    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        self.calls.lock().unwrap().push(DidContractCall::ReadLedger);
        Ok(self.snapshot.lock().unwrap().clone())
    }
}

/// Deterministic synthetic tx hash for the recording backend. Uses a
/// short hex prefix of blake2b-256 — opaque to consumers, deterministic
/// for tests.
fn synth_tx_hash(bytes: &[u8]) -> String {
    use blake2::{Blake2b512, Digest};
    let mut hasher = Blake2b512::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    // 16 hex chars (8 bytes) is plenty for the tests + keeps the
    // assertion-friendly short form.
    hex::encode(&out[..8])
}

// ─────────────────────────────────────────────────────────────────────
// ResolverBackend
// ─────────────────────────────────────────────────────────────────────

/// Read-only backend for the resolver consumer.
///
/// [`Backend::submit_tx`] always returns [`BackendError::ReadOnly`].
/// [`Backend::read_state`] returns a clone of the [`ChargedState`] supplied
/// at construction; [`Backend::read_snapshot`] returns a clone of the
/// [`DidLedgerSnapshot`] supplied at construction. Drops the wallet / proof-server /
/// indexer dep cone for consumers that only need the resolve path.
pub struct ResolverBackend {
    /// [`ChargedState`] served on every [`Backend::read_state`] call.
    pub state: ChargedState<DefaultDB>,
    /// Snapshot served on every [`Backend::read_snapshot`] call.
    pub snapshot: DidLedgerSnapshot,
}

impl fmt::Debug for ResolverBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolverBackend")
            .field("state", &"<ChargedState<DefaultDB>>")
            .field("snapshot", &"<DidLedgerSnapshot>")
            .finish()
    }
}

impl ResolverBackend {
    /// Construct a [`ResolverBackend`] over `state` + an empty snapshot.
    pub fn new(state: ChargedState<DefaultDB>) -> Self {
        Self {
            state,
            snapshot: DidLedgerSnapshot::default(),
        }
    }

    /// Construct a [`ResolverBackend`] over a specific snapshot (empty
    /// raw state).
    pub fn with_snapshot(snapshot: DidLedgerSnapshot) -> Self {
        Self {
            state: empty_charged_state::<DefaultDB>(),
            snapshot,
        }
    }
}

#[async_trait]
impl Backend for ResolverBackend {
    async fn submit_tx(&self, _tx: BuiltTx) -> Result<FinalizedTxData, BackendError> {
        Err(BackendError::ReadOnly)
    }

    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError> {
        Ok(self.state.clone())
    }

    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        Ok(self.snapshot.clone())
    }
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_call::DidContractCall;

    fn point_hex(byte: u8) -> crate::contract_call::JubjubPointHex {
        let h = hex::encode([byte; 32]);
        crate::contract_call::JubjubPointHex::new(crate::contract_call::NewJubjubPointHex { x: h.clone(), y: h })
            .unwrap()
    }

    fn ledger_config() -> LedgerContractCallConfig {
        LedgerContractCallConfig {
            operation: compact_runtime::ContractOperation::new(None),
            communication_commitment_rand: compact_runtime::Fr::from(7u64),
            key_location: compact_runtime::transient_crypto::proofs::KeyLocation(std::borrow::Cow::Borrowed(
                "did-test-key",
            )),
        }
    }

    fn constructor_material() -> DidConstructorProofMaterial {
        let state = empty_charged_state::<DefaultDB>();
        let qctx = compact_runtime::QueryContext::new(state, compact_runtime::ContractAddress::default());
        DidConstructorProofMaterial {
            constructor_id: "constructor".into(),
            contract_address: compact_runtime::ContractAddress::default(),
            initial_query_context: qctx.clone(),
            final_query_context: qctx,
            input: compact_runtime::AlignedValue::from(0u8),
            public_transcript: Vec::new(),
            private_transcript_outputs: DidPrivateTranscriptOutputs::default(),
            output: compact_runtime::AlignedValue::from(0u8),
        }
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn recording_backend_decodes_and_records_submit() {
        let rt = rt();
        let backend = RecordingBackend::new();
        let call1 = DidContractCall::Deactivate;
        let call2 = DidContractCall::RotateControllerKey {
            new_public_key: point_hex(3),
        };
        let tx1 = BuiltTx { bytes: call1.encode() };
        let tx2 = BuiltTx { bytes: call2.encode() };
        let f1 = rt.block_on(backend.submit_tx(tx1)).unwrap();
        let f2 = rt.block_on(backend.submit_tx(tx2)).unwrap();
        assert_eq!(f1.block_height, 1);
        assert_eq!(f2.block_height, 2);
        assert!(!f1.tx_hash.is_empty());
        let recorded = backend.recorded_calls();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], call1);
        assert_eq!(recorded[1], call2);
    }

    #[test]
    fn recording_backend_submit_rejects_garbage_envelope() {
        let rt = rt();
        let backend = RecordingBackend::new();
        let res = rt.block_on(backend.submit_tx(BuiltTx {
            bytes: vec![0xff, 0xfe],
        }));
        assert!(matches!(res, Err(BackendError::Decode(_))));
        // No call recorded on decode failure.
        assert_eq!(backend.recorded_calls().len(), 0);
    }

    #[test]
    fn recording_backend_read_snapshot_records_synthetic_read_ledger() {
        let rt = rt();
        let snap = DidLedgerSnapshot {
            version: 7,
            ..DidLedgerSnapshot::default()
        };
        let backend = RecordingBackend::with_snapshot(snap.clone());
        let read = rt.block_on(backend.read_snapshot()).unwrap();
        assert_eq!(read, snap);
        assert_eq!(backend.recorded_calls(), vec![DidContractCall::ReadLedger]);
    }

    #[test]
    fn resolver_backend_rejects_submit() {
        let rt = rt();
        let backend = ResolverBackend::new(empty_charged_state::<DefaultDB>());
        let res = rt.block_on(backend.submit_tx(BuiltTx::default()));
        assert_eq!(res, Err(BackendError::ReadOnly));
    }

    #[test]
    fn resolver_backend_returns_state_snapshot() {
        let rt = rt();
        let sentinel = empty_charged_state::<DefaultDB>();
        let backend = ResolverBackend::new(sentinel.clone());
        let read = rt.block_on(backend.read_state()).expect("read_state");
        assert_eq!(read, sentinel);
    }

    #[test]
    fn resolver_backend_returns_snapshot() {
        let rt = rt();
        let snap = DidLedgerSnapshot {
            version: 42,
            ..DidLedgerSnapshot::default()
        };
        let backend = ResolverBackend::with_snapshot(snap.clone());
        let got = rt.block_on(backend.read_snapshot()).unwrap();
        assert_eq!(got, snap);
    }

    #[test]
    fn generated_material_converts_to_ledger8_prepartition_shape() {
        let state = empty_charged_state::<DefaultDB>();
        let qctx = compact_runtime::QueryContext::new(state, compact_runtime::ContractAddress::default());
        let call = DidPrePartitionContractCall {
            circuit_id: "deactivate".into(),
            contract_address: compact_runtime::ContractAddress::default(),
            initial_query_context: qctx.clone(),
            final_query_context: qctx,
            input: compact_runtime::AlignedValue::from(1u8),
            public_transcript: Vec::new(),
            private_transcript_outputs: DidPrivateTranscriptOutputs::default(),
            output: compact_runtime::AlignedValue::from(2u8),
        };
        let ledger = call.into_ledger_prepartition_contract_call(ledger_config());
        assert_eq!(&ledger.entry_point[..], b"deactivate");
        assert_eq!(ledger.pre_transcript.program.len(), 0);
        assert_eq!(ledger.private_transcript_outputs.len(), 0);
        assert_eq!(ledger.communication_commitment_rand, compact_runtime::Fr::from(7u64));
    }

    #[test]
    fn constructor_material_converts_to_ledger8_prepartition_shape() {
        let ledger = constructor_material().into_ledger_prepartition_contract_call(ledger_config());
        assert_eq!(&ledger.entry_point[..], b"constructor");
        assert_eq!(ledger.pre_transcript.program.len(), 0);
        assert_eq!(ledger.private_transcript_outputs.len(), 0);
    }

    #[derive(Default)]
    struct FakeLive {
        events: Mutex<Vec<&'static str>>,
        fail_balance: Mutex<Option<BackendError>>,
        fail_prove: Mutex<Option<BackendError>>,
        fail_submit: Mutex<Option<BackendError>>,
        fail_reconcile: Mutex<Option<BackendError>>,
        snapshot: Mutex<DidLedgerSnapshot>,
    }

    impl FakeLive {
        fn events(&self) -> Vec<&'static str> {
            self.events.lock().unwrap().clone()
        }

        fn providers(self: &Arc<Self>) -> LiveProviders {
            LiveProviders::new(self.clone(), self.clone(), self.clone(), self.clone(), self.clone())
        }
    }

    impl DidContractExecutor for FakeLive {
        fn execute(
            &self,
            _state: ChargedState<DefaultDB>,
            call: DidContractCall,
        ) -> Result<DidPrePartitionContractCall, BackendError> {
            self.events.lock().unwrap().push("execute");
            assert_eq!(call, DidContractCall::Deactivate);
            let state = empty_charged_state::<DefaultDB>();
            let qctx = compact_runtime::QueryContext::new(state, compact_runtime::ContractAddress::default());
            Ok(DidPrePartitionContractCall {
                circuit_id: "deactivate".into(),
                contract_address: compact_runtime::ContractAddress::default(),
                initial_query_context: qctx.clone(),
                final_query_context: qctx,
                input: compact_runtime::AlignedValue::from(0u8),
                public_transcript: Vec::new(),
                private_transcript_outputs: DidPrivateTranscriptOutputs::default(),
                output: compact_runtime::AlignedValue::from(0u8),
            })
        }
    }

    #[async_trait]
    impl LedgerWalletProvider for FakeLive {
        async fn build_deployment_tx(
            &self,
            deployment: DidConstructorProofMaterial,
        ) -> Result<UnbalancedDidTransaction, BackendError> {
            self.events.lock().unwrap().push("build_deployment");
            assert_eq!(deployment.constructor_id, "constructor");
            Ok(UnbalancedDidTransaction {
                bytes: b"unbalanced".to_vec(),
            })
        }

        async fn build_unbalanced_tx(
            &self,
            call: DidPrePartitionContractCall,
        ) -> Result<UnbalancedDidTransaction, BackendError> {
            self.events.lock().unwrap().push("build_unbalanced");
            assert_eq!(call.circuit_id, "deactivate");
            Ok(UnbalancedDidTransaction {
                bytes: b"unbalanced".to_vec(),
            })
        }

        async fn balance_tx(&self, tx: UnbalancedDidTransaction) -> Result<BalancedDidTransaction, BackendError> {
            self.events.lock().unwrap().push("balance");
            assert_eq!(tx.bytes, b"unbalanced");
            if let Some(err) = self.fail_balance.lock().unwrap().take() {
                return Err(err);
            }
            Ok(BalancedDidTransaction::from_tagged_prove_tx_request_bytes_for_test(
                b"balanced".to_vec(),
            ))
        }

        async fn sign_tx(&self, tx: ProvedDidTransaction) -> Result<SignedDidTransaction, BackendError> {
            self.events.lock().unwrap().push("sign");
            assert_eq!(tx.tagged_proven_tx_bytes(), b"proved");
            Ok(SignedDidTransaction {
                bytes: b"signed".to_vec(),
            })
        }
    }

    #[async_trait]
    impl LedgerProofProvider for FakeLive {
        async fn prove_tx(&self, tx: BalancedDidTransaction) -> Result<ProvedDidTransaction, BackendError> {
            self.events.lock().unwrap().push("prove");
            assert_eq!(tx.tagged_prove_tx_request_bytes(), b"balanced");
            if let Some(err) = self.fail_prove.lock().unwrap().take() {
                return Err(err);
            }
            Ok(ProvedDidTransaction::from_tagged_proven_tx_bytes_for_test(
                b"proved".to_vec(),
            ))
        }
    }

    #[async_trait]
    impl LedgerNodeProvider for FakeLive {
        async fn submit_and_wait(&self, tx: SignedDidTransaction) -> Result<LedgerFinalityReceipt, BackendError> {
            self.events.lock().unwrap().push("submit");
            assert_eq!(tx.bytes, b"signed");
            if let Some(err) = self.fail_submit.lock().unwrap().take() {
                return Err(err);
            }
            Ok(LedgerFinalityReceipt {
                tx_hash: "tx-1".into(),
                block_height: 12,
                finality_token: Some("final".into()),
            })
        }
    }

    #[async_trait]
    impl LedgerIndexerProvider for FakeLive {
        async fn read_state_bytes(&self) -> Result<Vec<u8>, BackendError> {
            unreachable!("fake overrides read_state")
        }

        async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError> {
            self.events.lock().unwrap().push("read_state");
            Ok(empty_charged_state::<DefaultDB>())
        }

        async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
            self.events.lock().unwrap().push("read_snapshot");
            Ok(self.snapshot.lock().unwrap().clone())
        }

        async fn reconcile_finality(&self, receipt: &LedgerFinalityReceipt) -> Result<(), BackendError> {
            self.events.lock().unwrap().push("reconcile");
            assert_eq!(receipt.tx_hash, "tx-1");
            if let Some(err) = self.fail_reconcile.lock().unwrap().take() {
                return Err(err);
            }
            Ok(())
        }
    }

    #[test]
    fn live_backend_unconfigured_returns_typed_errors() {
        let rt = rt();
        let backend = LiveBackend::new();
        assert_eq!(
            rt.block_on(backend.read_state()),
            Err(BackendError::Unconfigured("LiveProviders"))
        );
        assert_eq!(
            rt.block_on(backend.submit_tx(BuiltTx::default())),
            Err(BackendError::Unconfigured("LiveProviders"))
        );
    }

    #[test]
    fn live_backend_deploy_runs_provider_pipeline_and_returns_finality() {
        let rt = rt();
        let fake = Arc::new(FakeLive::default());
        let backend = LiveBackend::with_providers(fake.providers());
        let deployment = DidDeploymentRequest {
            initial_contract_state: empty_charged_state::<DefaultDB>(),
            initial_private_state: (),
            initial_zswap_local_state: compact_runtime::ZswapLocalState::new(),
            constructor: constructor_material(),
        };
        let got = rt.block_on(backend.submit_deployment(deployment)).unwrap();
        assert_eq!(got.tx_hash, "tx-1");
        assert_eq!(got.block_height, 12);
        assert_eq!(
            fake.events(),
            vec!["build_deployment", "balance", "prove", "sign", "submit", "reconcile"]
        );
    }

    #[test]
    fn live_backend_submit_runs_provider_pipeline_and_returns_finality() {
        let rt = rt();
        let fake = Arc::new(FakeLive::default());
        let backend = LiveBackend::with_providers(fake.providers());
        let receipt = rt
            .block_on(backend.submit_tx(BuiltTx {
                bytes: DidContractCall::Deactivate.encode(),
            }))
            .unwrap();
        assert_eq!(
            receipt,
            FinalizedTxData {
                tx_hash: "tx-1".into(),
                block_height: 12
            }
        );
        assert_eq!(
            fake.events(),
            vec![
                "read_state",
                "execute",
                "build_unbalanced",
                "balance",
                "prove",
                "sign",
                "submit",
                "reconcile"
            ]
        );
    }

    #[test]
    fn live_backend_read_snapshot_uses_indexer_mapping_port() {
        let rt = rt();
        let fake = Arc::new(FakeLive::default());
        fake.snapshot.lock().unwrap().version = 99;
        let backend = LiveBackend::with_providers(fake.providers());
        let got = rt.block_on(backend.read_snapshot()).unwrap();
        assert_eq!(got.version, 99);
        assert_eq!(fake.events(), vec!["read_snapshot"]);
    }

    #[test]
    fn live_backend_failure_timeout_cancellation_are_not_retried() {
        let rt = rt();

        let fake = Arc::new(FakeLive::default());
        *fake.fail_balance.lock().unwrap() = Some(BackendError::Other("insufficient funds".into()));
        let backend = LiveBackend::with_providers(fake.providers());
        let err = rt
            .block_on(backend.submit_tx(BuiltTx {
                bytes: DidContractCall::Deactivate.encode(),
            }))
            .unwrap_err();
        assert_eq!(err, BackendError::Other("insufficient funds".into()));
        assert_eq!(
            fake.events(),
            vec!["read_state", "execute", "build_unbalanced", "balance"]
        );

        let fake = Arc::new(FakeLive::default());
        *fake.fail_prove.lock().unwrap() = Some(BackendError::Timeout("proof-server".into()));
        let backend = LiveBackend::with_providers(fake.providers());
        let err = rt
            .block_on(backend.submit_tx(BuiltTx {
                bytes: DidContractCall::Deactivate.encode(),
            }))
            .unwrap_err();
        assert_eq!(err, BackendError::Timeout("proof-server".into()));
        assert_eq!(
            fake.events(),
            vec!["read_state", "execute", "build_unbalanced", "balance", "prove"]
        );

        let fake = Arc::new(FakeLive::default());
        *fake.fail_submit.lock().unwrap() = Some(BackendError::Cancelled("node".into()));
        let backend = LiveBackend::with_providers(fake.providers());
        let err = rt
            .block_on(backend.submit_tx(BuiltTx {
                bytes: DidContractCall::Deactivate.encode(),
            }))
            .unwrap_err();
        assert_eq!(err, BackendError::Cancelled("node".into()));
        assert_eq!(
            fake.events(),
            vec![
                "read_state",
                "execute",
                "build_unbalanced",
                "balance",
                "prove",
                "sign",
                "submit"
            ]
        );
    }

    #[test]
    fn live_backend_reconciliation_failure_and_restart() {
        let rt = rt();
        let fake = Arc::new(FakeLive::default());
        *fake.fail_reconcile.lock().unwrap() = Some(BackendError::Reconciliation("indexer lag".into()));
        let backend = LiveBackend::with_providers(fake.providers());
        let err = rt
            .block_on(backend.submit_tx(BuiltTx {
                bytes: DidContractCall::Deactivate.encode(),
            }))
            .unwrap_err();
        assert_eq!(err, BackendError::Reconciliation("indexer lag".into()));

        let restarted = LiveBackend::with_providers(fake.providers());
        let receipt = rt
            .block_on(restarted.submit_tx(BuiltTx {
                bytes: DidContractCall::Deactivate.encode(),
            }))
            .unwrap();
        assert_eq!(receipt.tx_hash, "tx-1");
    }
}

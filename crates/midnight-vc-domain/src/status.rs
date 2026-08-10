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

//! Credential-status vocabulary — port of `packages/core/status/src/bindings.ts`
//! and `policy.ts`.
//!
//! This is the plain-data half of the upstream `status` package: the status
//! modes a credential can be bound to, the binding payload itself, and the
//! verifier-side acceptance policy / evidence shapes. No clock, authority or
//! registry is selected here — that is the status-verification slice.
//!
//! ## Deliberate deviations from the TypeScript source
//!
//! - The TS discriminated union `StatusBinding = NoStatusBinding |
//!   CredentialStatusBinding` becomes an internally-tagged Rust enum
//!   ([`StatusBinding`]) keyed on `mode`. The wire form is unchanged
//!   (`{"mode":"none"}` / `{"mode":"same-contract-live","statusType":…}`), and
//!   the invariant "only enabled modes carry a payload" becomes a type
//!   invariant instead of a review convention.
//! - `StatusReference` / `StatusHandle` are both TS `string | Uint8Array`; they
//!   share one Rust type, [`StatusOpaque`], with the two type aliases kept for
//!   readability at use sites.
//! - `bigint` fields become `u64` (versions, ages and timestamps are
//!   non-negative and well inside 64 bits).
//! - `StatusEvidence.payload?: unknown` becomes `Option<serde_json::Value>`.
//!
//! ## Not ported in this slice
//!
//! `packages/core/status/src/outcomes.ts` (verification outcome codes and the
//! `StatusVerificationError` throw path) and `ports.ts` (reader / writer /
//! verifier port interfaces) are behavioural rather than plain data and belong
//! with the status-verification work, not the credential model.

use serde::{Deserialize, Serialize};

/// How a credential's status is located and checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusMode {
    /// The credential carries no status binding.
    None,
    /// Status lives in the issuing contract's own live state.
    SameContractLive,
    /// Status is a non-membership proof against an external revoked set.
    ///
    /// Spelled `external-nonmembership` on the wire (one word, matching TS).
    ExternalNonmembership,
    /// Status is attested by a signing authority.
    AuthorityAttested,
}

/// The TS `EnabledStatusMode` — [`StatusMode`] minus [`StatusMode::None`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnabledStatusMode {
    /// See [`StatusMode::SameContractLive`].
    SameContractLive,
    /// See [`StatusMode::ExternalNonmembership`].
    ExternalNonmembership,
    /// See [`StatusMode::AuthorityAttested`].
    AuthorityAttested,
}

impl From<EnabledStatusMode> for StatusMode {
    fn from(mode: EnabledStatusMode) -> Self {
        match mode {
            EnabledStatusMode::SameContractLive => Self::SameContractLive,
            EnabledStatusMode::ExternalNonmembership => Self::ExternalNonmembership,
            EnabledStatusMode::AuthorityAttested => Self::AuthorityAttested,
        }
    }
}

impl StatusMode {
    /// The enabled counterpart, or `None` for [`StatusMode::None`].
    #[must_use]
    pub const fn enabled(self) -> Option<EnabledStatusMode> {
        match self {
            Self::None => None,
            Self::SameContractLive => Some(EnabledStatusMode::SameContractLive),
            Self::ExternalNonmembership => Some(EnabledStatusMode::ExternalNonmembership),
            Self::AuthorityAttested => Some(EnabledStatusMode::AuthorityAttested),
        }
    }
}

/// An opaque, credential-bound status value: either text or raw bytes.
///
/// Rust counterpart of the TS `string | Uint8Array` unions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StatusOpaque {
    /// A textual location / domain / handle.
    Text(String),
    /// Raw bytes (e.g. a 32-byte commitment).
    Bytes(Vec<u8>),
}

/// An opaque credential-bound status location or domain.
pub type StatusReference = StatusOpaque;
/// An opaque credential-bound status handle or commitment.
pub type StatusHandle = StatusOpaque;

/// Payload carried by every *enabled* status mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusBindingDetails {
    /// Mode-specific status type discriminator.
    pub status_type: String,
    /// Where the status lives.
    pub status_reference: StatusReference,
    /// Which entry within that location this credential is.
    pub status_handle: StatusHandle,
}

/// A credential's status binding — the TS `StatusBinding` union.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum StatusBinding {
    /// The TS `NoStatusBinding`.
    None,
    /// Status in the issuing contract's live state.
    SameContractLive(StatusBindingDetails),
    /// Non-membership against an external revoked set.
    ExternalNonmembership(StatusBindingDetails),
    /// Authority-attested status.
    AuthorityAttested(StatusBindingDetails),
}

impl StatusBinding {
    /// Build an enabled binding for `mode`.
    #[must_use]
    pub fn enabled(mode: EnabledStatusMode, details: StatusBindingDetails) -> Self {
        match mode {
            EnabledStatusMode::SameContractLive => Self::SameContractLive(details),
            EnabledStatusMode::ExternalNonmembership => Self::ExternalNonmembership(details),
            EnabledStatusMode::AuthorityAttested => Self::AuthorityAttested(details),
        }
    }

    /// The binding's mode.
    #[must_use]
    pub const fn mode(&self) -> StatusMode {
        match self {
            Self::None => StatusMode::None,
            Self::SameContractLive(_) => StatusMode::SameContractLive,
            Self::ExternalNonmembership(_) => StatusMode::ExternalNonmembership,
            Self::AuthorityAttested(_) => StatusMode::AuthorityAttested,
        }
    }

    /// The binding payload, absent for [`StatusBinding::None`].
    #[must_use]
    pub const fn details(&self) -> Option<&StatusBindingDetails> {
        match self {
            Self::None => None,
            Self::SameContractLive(details)
            | Self::ExternalNonmembership(details)
            | Self::AuthorityAttested(details) => Some(details),
        }
    }
}

/// Marker for the literal `"status"` capability kind.
///
/// Encodes the TS `& { readonly kind: "status" }` intersection so a
/// [`StatusCapabilityDescriptor`] cannot claim any other kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusCapabilityKind {
    /// The only permitted value.
    #[default]
    #[serde(rename = "status")]
    Status,
}

/// TS `StatusCapabilityDescriptor` — a capability descriptor pinned to
/// `kind: "status"`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusCapabilityDescriptor {
    /// Stable capability identifier.
    pub id: String,
    /// Always [`StatusCapabilityKind::Status`].
    #[serde(default)]
    pub kind: StatusCapabilityKind,
    /// Optional semantic version of the capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Whether verifiers must support this capability.
    pub required: bool,
}

impl StatusCapabilityDescriptor {
    /// TS `defineStatusCapability`.
    #[must_use]
    pub fn new(id: impl Into<String>, required: bool, version: Option<String>) -> Self {
        Self {
            id: id.into(),
            kind: StatusCapabilityKind::Status,
            version,
            required,
        }
    }

    /// Widen to the generic capability descriptor of [`crate::types`].
    #[must_use]
    pub fn to_capability_descriptor(&self) -> crate::types::CredentialCapabilityDescriptor {
        crate::types::CredentialCapabilityDescriptor {
            id: self.id.clone(),
            kind: crate::types::CredentialCapabilityKind::Status,
            version: self.version.clone(),
            required: self.required,
        }
    }
}

/// Lifecycle state a status check can report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusState {
    /// Credential is usable.
    Active,
    /// Temporarily not usable.
    Suspended,
    /// Permanently not usable.
    Revoked,
    /// Past its validity window.
    Expired,
}

/// Version and evidence-age requirements; no clock or authority is selected here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreshnessPolicy {
    /// Lowest acceptable registry version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_version: Option<u64>,
    /// Largest acceptable evidence age.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_age: Option<u64>,
    /// Whether stale evidence must be refused outright.
    pub require_fresh_evidence: bool,
}

/// Verifier-side acceptance policy for credential status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPolicy {
    /// Whether a status binding is mandatory.
    pub required: bool,
    /// Modes the verifier will accept.
    pub accepted_modes: Vec<EnabledStatusMode>,
    /// Optional freshness requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<FreshnessPolicy>,
    /// Optional allow-list of acceptable states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_states: Option<Vec<StatusState>>,
}

/// An observed status fact, as produced by a status reader.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvidence {
    /// Mode the evidence was gathered under.
    pub mode: StatusMode,
    /// Observed lifecycle state.
    pub state: StatusState,
    /// Registry version the observation came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    /// When the observation was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<u64>,
    /// When the observation stops being usable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Mode-specific opaque payload (proof bytes, attestation, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// A status binding as seen by a query — every field optional, because the
/// query may be built before the binding is fully known.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusBindingForQuery {
    /// Mode being queried.
    pub mode: StatusMode,
    /// Mode-specific status type discriminator, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_type: Option<String>,
    /// Status location, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_reference: Option<StatusReference>,
    /// Status handle, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_handle: Option<StatusHandle>,
}

impl From<&StatusBinding> for StatusBindingForQuery {
    fn from(binding: &StatusBinding) -> Self {
        Self {
            mode: binding.mode(),
            status_type: binding.details().map(|d| d.status_type.clone()),
            status_reference: binding.details().map(|d| d.status_reference.clone()),
            status_handle: binding.details().map(|d| d.status_handle.clone()),
        }
    }
}

/// A binding plus the policy it must satisfy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusQuery {
    /// The credential's status binding.
    pub binding: StatusBindingForQuery,
    /// The verifier's acceptance policy.
    pub policy: StatusPolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn assert_wire_round_trip<T>(value: &T, expected: &str)
    where
        T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).expect("serialize");
        assert_eq!(json, expected, "wire spelling drifted from the TypeScript form");
        let round_tripped: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&round_tripped, value);
    }

    fn details() -> StatusBindingDetails {
        StatusBindingDetails {
            status_type: "midnight:revocation-registry".into(),
            status_reference: StatusOpaque::Text("0x1234".into()),
            status_handle: StatusOpaque::Bytes(vec![1, 2, 3]),
        }
    }

    const ENABLED_MODES: [(EnabledStatusMode, StatusMode, &str); 3] = [
        (
            EnabledStatusMode::SameContractLive,
            StatusMode::SameContractLive,
            "same-contract-live",
        ),
        (
            EnabledStatusMode::ExternalNonmembership,
            StatusMode::ExternalNonmembership,
            "external-nonmembership",
        ),
        (
            EnabledStatusMode::AuthorityAttested,
            StatusMode::AuthorityAttested,
            "authority-attested",
        ),
    ];

    #[test]
    fn status_mode_wire_spellings_match_typescript() {
        assert_wire_round_trip(&StatusMode::None, "\"none\"");
        for (enabled, mode, literal) in ENABLED_MODES {
            assert_wire_round_trip(&mode, &format!("\"{literal}\""));
            assert_wire_round_trip(&enabled, &format!("\"{literal}\""));
        }
    }

    #[test]
    fn enabled_and_status_modes_convert_both_ways() {
        assert_eq!(StatusMode::None.enabled(), None);
        for (enabled, mode, _) in ENABLED_MODES {
            assert_eq!(StatusMode::from(enabled), mode);
            assert_eq!(mode.enabled(), Some(enabled));
        }
    }

    #[test]
    fn status_opaque_round_trips_text_and_bytes() {
        assert_wire_round_trip(&StatusOpaque::Text("handle".into()), "\"handle\"");
        assert_wire_round_trip(&StatusOpaque::Bytes(vec![0, 255]), "[0,255]");
    }

    #[test]
    fn no_status_binding_is_just_the_mode_tag() {
        assert_wire_round_trip(&StatusBinding::None, r#"{"mode":"none"}"#);
        assert_eq!(StatusBinding::None.mode(), StatusMode::None);
        assert_eq!(StatusBinding::None.details(), None);
    }

    #[test]
    fn enabled_status_binding_inlines_its_details_alongside_the_mode() {
        assert_wire_round_trip(
            &StatusBinding::SameContractLive(details()),
            r#"{"mode":"same-contract-live","statusType":"midnight:revocation-registry","statusReference":"0x1234","statusHandle":[1,2,3]}"#,
        );
    }

    #[test]
    fn status_binding_constructor_covers_every_enabled_mode() {
        for (enabled, mode, _) in ENABLED_MODES {
            let binding = StatusBinding::enabled(enabled, details());
            assert_eq!(binding.mode(), mode);
            assert_eq!(binding.details(), Some(&details()));
            // Every enabled variant must survive a JSON round trip.
            let json = serde_json::to_string(&binding).expect("serialize");
            assert_eq!(
                serde_json::from_str::<StatusBinding>(&json).expect("deserialize"),
                binding
            );
        }
    }

    #[test]
    fn status_binding_rejects_an_unknown_mode() {
        assert!(serde_json::from_str::<StatusBinding>(r#"{"mode":"made-up"}"#).is_err());
    }

    #[test]
    fn status_capability_descriptor_pins_kind_to_status() {
        let descriptor = StatusCapabilityDescriptor::new("status", true, Some("0.1.0".into()));
        assert_eq!(descriptor.kind, StatusCapabilityKind::Status);
        assert_wire_round_trip(
            &descriptor,
            r#"{"id":"status","kind":"status","version":"0.1.0","required":true}"#,
        );
    }

    #[test]
    fn status_capability_descriptor_omits_an_absent_version() {
        let descriptor = StatusCapabilityDescriptor::new("status", false, None);
        assert_wire_round_trip(&descriptor, r#"{"id":"status","kind":"status","required":false}"#);
    }

    #[test]
    fn status_capability_descriptor_rejects_any_other_kind() {
        assert!(
            serde_json::from_str::<StatusCapabilityDescriptor>(r#"{"id":"x","kind":"proof","required":true}"#).is_err()
        );
        // `kind` defaults to "status" when omitted, matching the TS literal type.
        let defaulted: StatusCapabilityDescriptor =
            serde_json::from_str(r#"{"id":"x","required":true}"#).expect("deserialize");
        assert_eq!(defaulted.kind, StatusCapabilityKind::Status);
        assert_eq!(StatusCapabilityKind::default(), StatusCapabilityKind::Status);
    }

    #[test]
    fn status_capability_widens_to_the_generic_capability_descriptor() {
        let widened = StatusCapabilityDescriptor::new("status", true, Some("1.0.0".into())).to_capability_descriptor();
        assert_eq!(widened.id, "status");
        assert_eq!(widened.kind, crate::types::CredentialCapabilityKind::Status);
        assert_eq!(widened.version.as_deref(), Some("1.0.0"));
        assert!(widened.required);
    }

    #[test]
    fn status_state_wire_spellings() {
        for (state, literal) in [
            (StatusState::Active, "\"active\""),
            (StatusState::Suspended, "\"suspended\""),
            (StatusState::Revoked, "\"revoked\""),
            (StatusState::Expired, "\"expired\""),
        ] {
            assert_wire_round_trip(&state, literal);
        }
    }

    #[test]
    fn freshness_policy_omits_absent_bounds() {
        assert_wire_round_trip(&FreshnessPolicy::default(), r#"{"requireFreshEvidence":false}"#);
        assert_wire_round_trip(
            &FreshnessPolicy {
                minimum_version: Some(7),
                maximum_age: Some(600),
                require_fresh_evidence: true,
            },
            r#"{"minimumVersion":7,"maximumAge":600,"requireFreshEvidence":true}"#,
        );
    }

    #[test]
    fn status_policy_omits_absent_freshness_and_states() {
        assert_wire_round_trip(
            &StatusPolicy {
                required: true,
                accepted_modes: vec![EnabledStatusMode::ExternalNonmembership],
                freshness: None,
                accepted_states: None,
            },
            r#"{"required":true,"acceptedModes":["external-nonmembership"]}"#,
        );
    }

    #[test]
    fn status_policy_emits_freshness_and_accepted_states() {
        assert_wire_round_trip(
            &StatusPolicy {
                required: false,
                accepted_modes: vec![EnabledStatusMode::AuthorityAttested],
                freshness: Some(FreshnessPolicy {
                    minimum_version: None,
                    maximum_age: None,
                    require_fresh_evidence: true,
                }),
                accepted_states: Some(vec![StatusState::Active, StatusState::Suspended]),
            },
            r#"{"required":false,"acceptedModes":["authority-attested"],"freshness":{"requireFreshEvidence":true},"acceptedStates":["active","suspended"]}"#,
        );
    }

    #[test]
    fn status_evidence_carries_an_opaque_json_payload() {
        assert_wire_round_trip(
            &StatusEvidence {
                mode: StatusMode::AuthorityAttested,
                state: StatusState::Revoked,
                version: Some(3),
                observed_at: Some(1_700_000_000),
                expires_at: Some(1_700_003_600),
                payload: Some(serde_json::json!({ "signature": "0xdead" })),
            },
            r#"{"mode":"authority-attested","state":"revoked","version":3,"observedAt":1700000000,"expiresAt":1700003600,"payload":{"signature":"0xdead"}}"#,
        );
    }

    #[test]
    fn status_evidence_omits_every_absent_optional() {
        assert_wire_round_trip(
            &StatusEvidence {
                mode: StatusMode::None,
                state: StatusState::Active,
                version: None,
                observed_at: None,
                expires_at: None,
                payload: None,
            },
            r#"{"mode":"none","state":"active"}"#,
        );
    }

    #[test]
    fn query_binding_projects_an_enabled_binding() {
        let binding = StatusBinding::ExternalNonmembership(details());
        let projected = StatusBindingForQuery::from(&binding);
        assert_eq!(projected.mode, StatusMode::ExternalNonmembership);
        assert_eq!(projected.status_type.as_deref(), Some("midnight:revocation-registry"));
        assert_eq!(projected.status_reference, Some(StatusOpaque::Text("0x1234".into())));
        assert_eq!(projected.status_handle, Some(StatusOpaque::Bytes(vec![1, 2, 3])));
    }

    #[test]
    fn query_binding_projects_the_none_binding_to_bare_mode() {
        let projected = StatusBindingForQuery::from(&StatusBinding::None);
        assert_wire_round_trip(&projected, r#"{"mode":"none"}"#);
        assert_eq!(projected.status_type, None);
        assert_eq!(projected.status_reference, None);
        assert_eq!(projected.status_handle, None);
    }

    #[test]
    fn status_query_round_trips() {
        let query = StatusQuery {
            binding: StatusBindingForQuery::from(&StatusBinding::SameContractLive(details())),
            policy: StatusPolicy {
                required: true,
                accepted_modes: vec![EnabledStatusMode::SameContractLive],
                freshness: None,
                accepted_states: None,
            },
        };
        let json = serde_json::to_string(&query).expect("serialize");
        assert_eq!(serde_json::from_str::<StatusQuery>(&json).expect("deserialize"), query);
        assert!(json.contains(r#""binding":{"mode":"same-contract-live""#));
        assert!(json.contains(r#""policy":{"required":true"#));
    }
}

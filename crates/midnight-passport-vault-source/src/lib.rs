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

//! Authenticated source distribution for the Passport Vault Compact contract.
//!
//! This crate intentionally has no runtime, ledger, proving, or generated-code
//! dependencies. Consumers that only need to authenticate or compile the source
//! do not inherit the Midnight Ledger, Compact runtime, Halo2, or ZKIR graphs.
//! Generated bindings and proving artifacts belong in a separate heavy leaf.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

/// Stable relative path of the contract within the published crate.
pub const CONTRACT_PATH: &str = "contract/passport-vault.compact";

/// Byte-identical reviewed Passport Vault Compact source.
pub const CONTRACT_SOURCE: &str = include_str!("../contract/passport-vault.compact");

/// SHA-256 digest of [`CONTRACT_SOURCE`].
pub const CONTRACT_SHA256: &str = "2ebc5b34dd440bc9a9736408f29f5003e7a78f26a564b392be2af36de69102f4";

/// Size in bytes of [`CONTRACT_SOURCE`].
pub const CONTRACT_BYTES: usize = 23_776;

/// One source file in the contract's authenticated transitive include closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSource {
    /// Stable path relative to the published crate root.
    pub path: &'static str,
    /// Byte-identical Compact source at [`Self::path`].
    pub source: &'static str,
}

/// Transitive Compact sources required by [`CONTRACT_SOURCE`], in canonical path order.
pub const INCLUDE_CLOSURE: &[AuthenticatedSource] = &[
    AuthenticatedSource {
        path: "contract/vendored/credentials.compact",
        source: include_str!("../contract/vendored/credentials.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/composable.compact",
        source: include_str!("../contract/vendored/credentials/composable.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/holder-bindings.compact",
        source: include_str!("../contract/vendored/credentials/holder-bindings.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/issue.compact",
        source: include_str!("../contract/vendored/credentials/issue.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/present.compact",
        source: include_str!("../contract/vendored/credentials/present.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/proofs.compact",
        source: include_str!("../contract/vendored/credentials/proofs.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/protocols.compact",
        source: include_str!("../contract/vendored/credentials/protocols.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/relations.compact",
        source: include_str!("../contract/vendored/credentials/relations.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/status-bindings.compact",
        source: include_str!("../contract/vendored/credentials/status-bindings.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/types.compact",
        source: include_str!("../contract/vendored/credentials/types.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/vc.compact",
        source: include_str!("../contract/vendored/credentials/vc.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/credentials/vp.compact",
        source: include_str!("../contract/vendored/credentials/vp.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/digital-passport-credential.compact",
        source: include_str!("../contract/vendored/digital-passport-credential.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/digital-passport-credential/claims.compact",
        source: include_str!("../contract/vendored/digital-passport-credential/claims.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/digital-passport-credential/helpers.compact",
        source: include_str!("../contract/vendored/digital-passport-credential/helpers.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/digital-passport-credential/model.compact",
        source: include_str!("../contract/vendored/digital-passport-credential/model.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/digital-passport-credential/protocol-model.compact",
        source: include_str!("../contract/vendored/digital-passport-credential/protocol-model.compact"),
    },
    AuthenticatedSource {
        path: "contract/vendored/digital-passport-credential/validation.compact",
        source: include_str!("../contract/vendored/digital-passport-credential/validation.compact"),
    },
];

/// SHA-256 of the include sources concatenated in [`INCLUDE_CLOSURE`] order.
pub const INCLUDE_CLOSURE_SHA256: &str = "76b91bc29be9f41753eeef232bd696fb8181af0670e52d658e6e7da04adde127";

/// Total bytes in the authenticated transitive include closure.
pub const INCLUDE_CLOSURE_BYTES: usize = 68_290;

/// Machine-readable source provenance and compatibility manifest.
pub const SOURCE_MANIFEST: &str = include_str!("../manifest.json");

/// A reviewed proof circuit baseline for this contract source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CircuitBaseline {
    /// Exported Compact circuit name.
    pub name: &'static str,
    /// Smallest reviewed proving parameter exponent.
    pub k: u8,
    /// Reviewed measured row count.
    pub rows: u32,
}

/// Circuits and proof-size baselines authenticated with this source version.
pub const CIRCUIT_BASELINES: &[CircuitBaseline] = &[
    CircuitBaseline {
        name: "setTrustedIssuer",
        k: 13,
        rows: 5_416,
    },
    CircuitBaseline {
        name: "createLock",
        k: 11,
        rows: 1_823,
    },
    CircuitBaseline {
        name: "depositToLock",
        k: 10,
        rows: 834,
    },
    CircuitBaseline {
        name: "claimFromLock",
        k: 17,
        rows: 124_785,
    },
    CircuitBaseline {
        name: "withdrawFromLock",
        k: 11,
        rows: 1_212,
    },
];

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn source_matches_authenticated_shape() {
        assert_eq!(CONTRACT_SOURCE.len(), CONTRACT_BYTES);
        assert_eq!(format!("{:x}", Sha256::digest(CONTRACT_SOURCE)), CONTRACT_SHA256);
        assert!(CONTRACT_SOURCE.contains("pragma language_version >= 0.20;"));
        for circuit in CIRCUIT_BASELINES {
            assert!(
                CONTRACT_SOURCE.contains(&format!("circuit {}(", circuit.name)),
                "missing {}",
                circuit.name
            );
        }
    }

    #[test]
    fn include_closure_is_complete_and_authenticated() {
        use std::collections::{HashMap, HashSet};
        use std::path::{Component, Path};

        let mut digest = Sha256::new();
        let sources: HashMap<_, _> = std::iter::once((CONTRACT_PATH, CONTRACT_SOURCE))
            .chain(INCLUDE_CLOSURE.iter().map(|file| (file.path, file.source)))
            .collect();
        for file in INCLUDE_CLOSURE {
            digest.update(file.source);
        }
        assert_eq!(
            INCLUDE_CLOSURE.iter().map(|file| file.source.len()).sum::<usize>(),
            INCLUDE_CLOSURE_BYTES
        );
        assert_eq!(format!("{:x}", digest.finalize()), INCLUDE_CLOSURE_SHA256);

        let mut pending = vec![CONTRACT_PATH.to_owned()];
        let mut reached = HashSet::new();
        while let Some(path) = pending.pop() {
            if !reached.insert(path.clone()) {
                continue;
            }
            let source = sources
                .get(path.as_str())
                .unwrap_or_else(|| panic!("missing authenticated source {path}"));
            for line in source.lines().map(str::trim) {
                let Some(include) = line
                    .strip_prefix("include \"")
                    .and_then(|line| line.strip_suffix("\";"))
                else {
                    continue;
                };
                let candidate = Path::new(&path)
                    .parent()
                    .unwrap()
                    .join(include)
                    .with_extension("compact");
                let mut normalized = Vec::new();
                for component in candidate.components() {
                    match component {
                        Component::Normal(part) => normalized.push(part.to_str().unwrap()),
                        Component::ParentDir => {
                            normalized.pop();
                        }
                        Component::CurDir => {}
                        _ => panic!("unsupported include path in {path}: {include}"),
                    }
                }
                let resolved = normalized.join("/");
                assert!(
                    sources.contains_key(resolved.as_str()),
                    "missing include {resolved} from {path}"
                );
                pending.push(resolved);
            }
        }
        assert_eq!(
            reached.len(),
            sources.len(),
            "authenticated closure contains unreachable files"
        );
    }

    #[test]
    fn manifest_matches_public_constants() {
        let manifest: serde_json::Value = serde_json::from_str(SOURCE_MANIFEST).unwrap();
        assert_eq!(manifest["contract"]["path"], CONTRACT_PATH);
        assert_eq!(manifest["contract"]["sha256"], CONTRACT_SHA256);
        assert_eq!(manifest["contract"]["bytes"], CONTRACT_BYTES);
        assert_eq!(manifest["includeClosure"]["sha256"], INCLUDE_CLOSURE_SHA256);
        assert_eq!(manifest["includeClosure"]["bytes"], INCLUDE_CLOSURE_BYTES);
        let manifest_paths: Vec<_> = manifest["includeClosure"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(
            manifest_paths,
            INCLUDE_CLOSURE.iter().map(|file| file.path).collect::<Vec<_>>()
        );
        let manifest_circuits = manifest["circuits"].as_array().unwrap();
        assert_eq!(manifest_circuits.len(), CIRCUIT_BASELINES.len());
        for (manifest_circuit, baseline) in manifest_circuits.iter().zip(CIRCUIT_BASELINES) {
            assert_eq!(manifest_circuit["name"], baseline.name);
            assert_eq!(manifest_circuit["k"], baseline.k);
            assert_eq!(manifest_circuit["rows"], baseline.rows);
        }
    }
}

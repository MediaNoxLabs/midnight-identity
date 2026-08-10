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

//! Codegen target for Midnight Verifiable Credentials (issue #13, ADR 0009
//! split rule 1).
//!
//! The VC-layer counterpart of `midnight-did-runtime`: it holds the Rust
//! bindings the flake-pinned `compactc --rust --skip-ts` emits for the VC
//! Compact contracts, and nothing else yet. It is a separate crate from
//! [`midnight_vc_domain`] because `compact-runtime` drags in the halo2 /
//! arkworks stack, which does not build for `wasm32-unknown-unknown` (ADR 0006,
//! ADR 0009 rule 1).
//!
//! [`midnight_vc_domain`]: https://docs.rs/midnight-vc-domain
//!
//! See [`contract`] for the per-contract modules, the exact upstream entry
//! points, and the regeneration workflow.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

/// Crate version reported by the build.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod contract;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_reported() {
        assert!(!VERSION.is_empty());
    }
}

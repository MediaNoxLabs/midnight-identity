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

//! Credential-family bindings for Midnight Verifiable Credentials.
//!
//! This crate is the home of the Rust bindings the flake-pinned
//! `compactc --rust --skip-ts` emits for the **credential-family** prototype
//! contracts (ADR 0009 split rule 2 — families are pinned by external
//! consumers and track upstream family releases, a different lifecycle from
//! the core contracts surveyed into [`midnight_vc_runtime`]). It lands with
//! the **digital-passport** family only; `birth` / `birth-secret` are
//! additive follow-ups (a new feature, module, and codegen entry point each).
//!
//! Every family is gated behind its own cargo feature, and `default` is
//! empty: the family surfaces are prototype-grade and churn upstream, so
//! every consumer's opt-in is explicit. A bare dependency compiles an empty
//! crate — see [`contract`] for the per-family registry.
//!
//! Like [`midnight_vc_runtime`], the crate is a codegen target: it drags in
//! `midnight-compact-runtime` → halo2 / arkworks, which do not build for
//! `wasm32-unknown-unknown` (ADR 0006), and it is `publish = false` while
//! those upstreams stay unpublished (see `doc/publishing.md` — consumers use
//! a git dependency on this repository).
//!
//! # Mixing warning
//!
//! Each family's generated module `include`s the core credentials primitive
//! and therefore *redeclares* the core types (`Credential`, `Proof`, …).
//! Do not mix types across family modules, or with
//! [`midnight_vc_runtime`]'s core modules — they are distinct copies by
//! construction, even where they carry the same names.
//!
//! [`midnight_vc_runtime`]: https://github.com/MediaNoxLabs/midnight-identity/tree/develop/crates/midnight-vc-runtime

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

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

//! Generated Compact bindings for the Midnight VC contracts.
//!
//! One module per contract entry point, emitted by the flake-pinned
//! `compactc --rust --skip-ts`. **Do not edit these files by hand** — run
//! `just codegen-vc` after moving the
//! `third_party/midnight-verifiable-credentials` pin or bumping the compact
//! flake input, and `just codegen-vc-check` to assert the committed output is
//! reproducible.
//!
//! | module | entry point (in the vendored submodule) |
//! |---|---|
//! | [`credentials`] | `packages/core/primitives/credentials/src/credentials.compact` |
//! | [`iso_registry`] | `packages/core/primitives/iso-registry/src/iso-registry.compact` |
//! | [`same_holder`] | `packages/core/capabilities/same-holder/src/same-holder.compact` |
//! | [`revocation_registry`] | `packages/registry/status-registry/src/revocation-registry.compact` |
//!
//! The modules are re-exported as modules rather than glob-re-exported:
//! `same-holder.compact` `include`s `credentials.compact`, so its generated
//! output redeclares every credentials type and a flat re-export would collide.
//!
//! `revocation_registry` joined in the compact 0.31.111 pin: it was blocked
//! on compiler gap G1 (struct-field projection in trapping-arithmetic
//! operands, MediaNoxLabs/compact#5) until that fix promoted to the stable
//! `codegen-rust` branch.

#![allow(missing_docs)] // generated modules carry their own header docs

pub mod credentials;
pub mod iso_registry;
pub mod revocation_registry;
pub mod same_holder;

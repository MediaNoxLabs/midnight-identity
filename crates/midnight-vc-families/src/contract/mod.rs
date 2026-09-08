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

//! Generated Compact bindings for the Midnight VC credential families.
//!
//! One module per family entry point, emitted by the flake-pinned
//! `compactc --rust --skip-ts` and gated behind the family's cargo feature.
//! **Do not edit these files by hand** — run `just codegen-vc` after moving
//! the `third_party/midnight-verifiable-credentials` pin or bumping the
//! compact flake input, and `just codegen-vc-check` to assert the committed
//! output is reproducible.
//!
//! | module | feature | entry point (in the vendored submodule) |
//! |---|---|---|
//! | `digital_passport` | `digital-passport` | `packages/prototypes/credential-families/digital-passport/src/digital-passport-credential.compact` |
//!
//! The modules are re-exported as modules rather than glob-re-exported:
//! each family entry point `include`s `credentials.compact`, so its
//! generated output redeclares every core credentials type and a flat
//! re-export would collide (same rationale as `midnight-vc-runtime`'s
//! registry). Consumers must not mix types across family modules.
//!
//! Onboarding a further family is purely additive: a new cargo feature, a
//! new `#[cfg(feature)]`-gated module line here, and a new entry in the
//! `just codegen-vc` list — no existing family's feature, module, or
//! generated file changes.

#![allow(missing_docs)] // generated modules carry their own header docs

#[cfg(feature = "digital-passport")]
pub mod digital_passport;

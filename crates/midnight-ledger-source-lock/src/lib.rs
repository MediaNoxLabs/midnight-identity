// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

//! Resolver-only constraints for the repository's immutable Ledger source cone.
//!
//! This private crate intentionally exports no runtime API. Its exact registry
//! constraints cause Cargo to select the workspace root's Git patches instead
//! of newer registry releases allowed by Compact's broad version requirements.

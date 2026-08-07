# Default target lists available recipes.
default:
    @just --list

# Re-generate Rust code from did.compact via the patched compact compiler.
#
# Per compactc --help: `compactc <flag> ... <source-pathname> <target-directory-pathname>`.
# With --rust + --skip-ts the compiler emits:
#   <target>/contract/lib.rs              -- Rust source (consumed)
#   <target>/Cargo.toml                   -- emitted Cargo manifest (NOT consumed; we maintain our own)
#   <target>/compiler/contract-info.json  -- analyzer metadata (not consumed in cycle 1)
#   <target>/zkir/<circuit>.zkir          -- ZKIR per exported circuit
#   <target>/keys/<circuit>.{prover,verifier}  -- proving + verifier keys (emitted by zkir tool)
# Source: third_party compact compiler/passes.ss (generate-everything pass).
codegen:
    @mkdir -p target-gen
    @rm -rf target-gen/contract-out
    git submodule update --init third_party/midnight-did
    compactc --rust --skip-ts \
        third_party/midnight-did/packages/contract/src/did.compact \
        target-gen/contract-out
    @mkdir -p crates/midnight-did-runtime/src/contract
    cp target-gen/contract-out/contract/lib.rs crates/midnight-did-runtime/src/contract/generated.rs
    @mkdir -p crates/midnight-did-runtime/assets/keys
    # Recursive find picks up keys whether they live in keys/, zkir/, or contract/.
    find target-gen/contract-out -name "*.zkir"     -exec cp {} crates/midnight-did-runtime/assets/keys/ \;
    find target-gen/contract-out -name "*.prover"   -exec cp {} crates/midnight-did-runtime/assets/keys/ \;
    find target-gen/contract-out -name "*.verifier" -exec cp {} crates/midnight-did-runtime/assets/keys/ \;
    cargo fmt -p midnight-did-runtime

# Verify re-running codegen produces no diff (regression signal for CI).
codegen-check: codegen
    git diff --exit-code -- crates/midnight-did-runtime/src/contract crates/midnight-did-runtime/assets/keys

# Our 7 workspace crates. Bare `cargo fmt/clippy/nextest` also sweep the
# path-mounted third_party crates (cargo absorbs them as workspace
# members), and we don't gate vendored code — so every recipe scopes to
# this list, mirroring CI.
crate_flags := "-p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did -p midnight-did-runtime -p midnight-did-uniffi -p midnight-did-cli -p midnight-did-indexer -p midnight-did-resolver -p midnight-did-jubjub-schnorr"

build:
    cargo build --all-targets {{crate_flags}}

test:
    cargo nextest run {{crate_flags}}

fmt:
    cargo fmt {{crate_flags}}
    taplo fmt

fmt-check:
    cargo fmt {{crate_flags}} -- --check
    taplo fmt --check

lint:
    cargo clippy --all-targets {{crate_flags}} -- -D warnings

# Line-coverage floor enforced by `coverage-gate` (and CI). Raise it as
# coverage improves; never lower it to admit a regression.
coverage_floor := "85"

# Coverage scope: every first-party crate; excludes the codegen artifact
# (gated by codegen-check, not tests) and service/demo bin entrypoints.
coverage_crates := "-p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did -p midnight-did-runtime -p midnight-did-indexer -p midnight-did-jubjub-schnorr -p midnight-did-resolver -p midnight-did-uniffi -p midnight-did-cli"
coverage_exclude := 'contract/generated\.rs|src/main\.rs|src/bin/'

# HTML coverage report for humans.
coverage:
    cargo llvm-cov --locked \
        {{coverage_crates}} \
        --ignore-filename-regex '{{coverage_exclude}}' \
        --html --open

# Same scope, but fails if line coverage drops below the floor. CI gate.
coverage-gate:
    cargo llvm-cov --locked \
        {{coverage_crates}} \
        --ignore-filename-regex '{{coverage_exclude}}' \
        --summary-only --fail-under-lines {{coverage_floor}}

# LCOV export for CI artifact upload / external services.
coverage-lcov:
    cargo llvm-cov --locked \
        {{coverage_crates}} \
        --ignore-filename-regex '{{coverage_exclude}}' \
        --lcov --output-path target/lcov.info

# Ratchet nag: fail-free warning when measured coverage exceeds the floor
# by >5 points — time to raise `coverage_floor` in the same PR.
coverage-ratchet:
    #!/usr/bin/env bash
    set -euo pipefail
    measured=$(cargo llvm-cov report --summary-only --ignore-filename-regex '{{coverage_exclude}}|third_party/' 2>/dev/null | awk '/^TOTAL/ {print $(NF-3)}' | tr -d '%')
    floor={{coverage_floor}}
    headroom=$(echo "$measured $floor" | awk '{printf "%.2f", $1 - $2}')
    echo "coverage: measured=${measured}% floor=${floor}% headroom=${headroom}"
    if (( $(echo "$headroom > 5" | bc -l) )); then
        echo "::warning::coverage floor is stale (headroom ${headroom} > 5) — raise coverage_floor in justfile"
    fi

ci: fmt-check lint build test coverage-gate coverage-ratchet

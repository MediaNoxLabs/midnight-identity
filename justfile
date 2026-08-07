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
crate_flags := "-p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did -p midnight-did-runtime -p midnight-did-uniffi -p midnight-did-cli -p midnight-did-indexer -p midnight-did-resolver"

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
coverage_floor := "80"

# Coverage over the four CI-gated crates (domain, method, api, umbrella),
# excluding the codegen artifact generated.rs. HTML report for humans.
coverage:
    cargo llvm-cov --locked \
        -p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did \
        --ignore-filename-regex 'contract/generated\.rs' \
        --html --open

# Same scope, but fails if line coverage drops below the floor. CI gate.
coverage-gate:
    cargo llvm-cov --locked \
        -p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did \
        --ignore-filename-regex 'contract/generated\.rs' \
        --summary-only --fail-under-lines {{coverage_floor}}

# LCOV export for CI artifact upload / external services.
coverage-lcov:
    cargo llvm-cov --locked \
        -p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did \
        --ignore-filename-regex 'contract/generated\.rs' \
        --lcov --output-path target/lcov.info

ci: fmt-check lint build test coverage-gate

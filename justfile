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
#
# Gate scope: `src/contract/generated.rs` AND the 12 `assets/keys/*.zkir`
# circuit artifacts — both are tracked and byte-reproducible under the
# flake-pinned compactc (verified 2026-08-10), so a change in circuit
# lowering shows up here rather than silently.
#
# NOT gated: `*.prover` / `*.verifier` proving keys. The devshell has no
# `zkir` tool ("Warning: ZKIR not found; skipping final circuit
# compilation"), so `just codegen` never emits them and there is nothing
# to compare. Wire them in only alongside a zkir-providing devshell.
codegen-check: codegen
    git diff --exit-code -- crates/midnight-did-runtime/src/contract crates/midnight-did-runtime/assets/keys

# Re-generate the VC Rust bindings from the pinned VC contract sources.
#
# The four core-contract modules land in midnight-vc-runtime from the frozen
# monorepo pin. The Digital Passport family lands in midnight-vc-families from
# its standalone v0.1.0-rc1 source. That family consumes the generic VC core
# from the published npm package, so the recipe stages a pinned, checksummed
# copy before invoking the compiler.
vc_core_version := "0.1.0-rc3"
vc_core_sha256 := "1066ac930221526fcf23608982285db659006a9be7f7b695e2bfe2820c856997"

codegen-vc:
    #!/usr/bin/env bash
    set -euo pipefail
    git submodule update --init third_party/midnight-verifiable-credentials \
        third_party/midnight-verifiable-credential-digital-passport
    mkdir -p target-gen crates/midnight-vc-runtime/src/contract crates/midnight-vc-families/src/contract
    vc=third_party/midnight-verifiable-credentials/packages
    passport=third_party/midnight-verifiable-credential-digital-passport/packages/midnight-verifiable-credential-digital-passport

    core_pkg="@midnight-ntwrk/credential-compact"
    core_ver="{{vc_core_version}}"
    core_tgz="target-gen/${core_pkg##*/}-${core_ver}.tgz"
    core_dir="target-gen/${core_pkg##*/}-${core_ver}"
    if [ ! -f "$core_tgz" ]; then
        curl -sfL --output "$core_tgz" \
            "https://registry.npmjs.org/${core_pkg}/-/${core_pkg##*/}-${core_ver}.tgz"
    fi
    # `sha256sum -c` is shared by GNU coreutils and the Darwin-compatible
    # implementation in our devshell; GNU-only `--strict` breaks macOS CI.
    echo "{{vc_core_sha256}}  $core_tgz" | sha256sum -c -
    if [ ! -d "$core_dir/package/dist" ]; then
        rm -rf "$core_dir"
        mkdir -p "$core_dir"
        tar -xzf "$core_tgz" -C "$core_dir"
    fi
    staging="$passport/core-compact-staging"
    rm -rf "$staging"
    mkdir -p "$staging/credentials"
    cp "$core_dir/package/dist/credentials.compact" "$staging/credentials.compact"
    find "$core_dir/package/dist/credentials" -type f -name '*.compact' \
        -exec cp {} "$staging/credentials/" \;

    # "<module>:<crate>:<entry point>" — module name is the Rust file under
    # the selected crate's src/contract/ directory.
    contracts=(
        "credentials:midnight-vc-runtime:$vc/core/primitives/credentials/src/credentials.compact"
        "iso_registry:midnight-vc-runtime:$vc/core/primitives/iso-registry/src/iso-registry.compact"
        "same_holder:midnight-vc-runtime:$vc/core/capabilities/same-holder/src/same-holder.compact"
        "revocation_registry:midnight-vc-runtime:$vc/registry/status-registry/src/revocation-registry.compact"
        "digital_passport:midnight-vc-families:$passport/src/digital-passport-credential.compact"
    )
    for entry in "${contracts[@]}"; do
        module="${entry%%:*}"
        rest="${entry#*:}"
        crate="${rest%%:*}"
        entry_point="${rest#*:}"
        out="target-gen/vc-${module}-out"
        rm -rf "$out"
        compactc --rust --skip-ts "$entry_point" "$out"
        {
            echo "//! GENERATED — do not edit; run \`just codegen-vc\`."
            echo "//!"
            if [ "$module" = "digital_passport" ]; then
                echo "//! Family source: \`${entry_point#third_party/}\` at tag \`v0.1.0-rc1\`"
                echo "//! in the pinned \`third_party/midnight-verifiable-credential-digital-passport\` submodule;"
                echo "//! core contract \`@midnight-ntwrk/credential-compact@{{vc_core_version}}\` (npm),"
                echo "//! staged into the family repo's \`core-compact-staging/\` by this recipe."
            else
                echo "//! Source: \`${entry_point#third_party/midnight-verifiable-credentials/}\`"
                echo "//! in the pinned \`third_party/midnight-verifiable-credentials\` submodule."
            fi
            cat "$out/contract/lib.rs"
        } > "crates/${crate}/src/contract/${module}.rs"
    done
    cargo fmt -p midnight-vc-runtime -p midnight-vc-families

# Verify re-running the VC codegen produces no diff (regression signal for CI).
codegen-vc-check: codegen-vc
    git diff --exit-code -- crates/midnight-vc-runtime/src/contract crates/midnight-vc-families/src/contract

# Our first-party workspace crates. Bare `cargo fmt/clippy/nextest` also
# sweep the path-mounted third_party crates (cargo absorbs them as
# workspace members), and we don't gate vendored code — so every recipe
# scopes to this list, mirroring CI.
crate_flags := "-p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did -p midnight-did-runtime -p midnight-did-uniffi -p midnight-did-cli -p midnight-did-indexer -p midnight-did-resolver -p midnight-did-jubjub-schnorr -p midnight-passport-vault-source -p midnight-vc-domain -p midnight-vc-families -p midnight-vc-proof -p midnight-vc-runtime"

# Family bindings are opt-in (`default = []`). Run a separate feature-complete
# pass so `--all-features` does not change unrelated workspace crates.
families_flags := "-p midnight-vc-families --all-features"

build:
    cargo build --all-targets {{crate_flags}}
    cargo build --all-targets {{families_flags}}

test:
    cargo nextest run {{crate_flags}}
    cargo nextest run {{families_flags}}

fmt:
    cargo fmt {{crate_flags}}
    taplo fmt

fmt-check:
    cargo fmt {{crate_flags}} -- --check
    taplo fmt --check

lint:
    cargo clippy --all-targets {{crate_flags}} -- -D warnings
    cargo clippy --all-targets {{families_flags}} -- -D warnings

# Line-coverage floor enforced by `coverage-gate` (and CI). Raise it as
# coverage improves; never lower it to admit a regression.
coverage_floor := "87"

# Coverage scope: every first-party crate; excludes the codegen artifact
# (gated by codegen-check, not tests) and service/demo bin entrypoints.
coverage_crates := "-p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did -p midnight-did-runtime -p midnight-did-indexer -p midnight-did-jubjub-schnorr -p midnight-did-resolver -p midnight-did-uniffi -p midnight-did-cli -p midnight-passport-vault-source -p midnight-vc-domain -p midnight-vc-families -p midnight-vc-proof -p midnight-vc-runtime"
coverage_features := "--features midnight-vc-families/digital-passport"
# `contract/generated\.rs` is the DID codegen artifact; the three
# `contract/{credentials,iso_registry,same_holder}\.rs` files are the VC ones.
# Generated code is gated by `codegen-check` / `codegen-vc-check`, not by tests;
# every hand-written line in those crates stays in scope.
coverage_exclude := 'contract/generated\.rs|contract/credentials\.rs|contract/iso_registry\.rs|contract/same_holder\.rs|contract/revocation_registry\.rs|contract/digital_passport\.rs|src/main\.rs|src/bin/'

# HTML coverage report for humans.
coverage:
    cargo llvm-cov --locked \
        {{coverage_crates}} \
        {{coverage_features}} \
        --ignore-filename-regex '{{coverage_exclude}}' \
        --html --open

# Same scope, but fails if line coverage drops below the floor. CI gate.
coverage-gate:
    cargo llvm-cov --locked \
        {{coverage_crates}} \
        {{coverage_features}} \
        --ignore-filename-regex '{{coverage_exclude}}' \
        --summary-only --fail-under-lines {{coverage_floor}}

# LCOV export for CI artifact upload / external services.
coverage-lcov:
    cargo llvm-cov --locked \
        {{coverage_crates}} \
        {{coverage_features}} \
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

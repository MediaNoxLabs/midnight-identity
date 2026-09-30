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

# Re-generate the VC Rust bindings from the pinned, released VC packages.
#
# Both inputs are sha256-pinned release tarballs, downloaded into target-gen/:
#
# - `@midnight-ntwrk/credential-compact` (npm) supplies the generic VC/VP core.
#   Its `credentials.compact` lands in midnight-vc-runtime as `credentials`.
# - `@midnight-ntwrk/midnight-vc-passport` supplies the Digital Passport family,
#   which lands in midnight-vc-families as `digital_passport` (npm).
#
# The family `include`s the core from `../core-compact-staging/`, so the recipe
# stages the same pinned core copy there before invoking the compiler.
vc_core_version := "0.2.0"
vc_core_sha256 := "095b6056912059e5b281b39d2d133a1048d42b5a6104da4367951c2773d70c4f"
vc_passport_version := "0.1.0-rc3"
vc_passport_sha256 := "8da5443407ea1d6b126dbf4c2e8963a4ae245c49f81ac8835556cbd2395a1623"

codegen-vc:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p target-gen crates/midnight-vc-runtime/src/contract crates/midnight-vc-families/src/contract

    # fetch_package <tarball> <url> <sha256> <extract dir>: download once,
    # verify the checksum on every run, then extract a fresh copy.
    fetch_package() {
        local tgz="$1" url="$2" sha="$3" dir="$4"
        if [ ! -f "$tgz" ]; then
            curl -sfL --output "$tgz" "$url"
        fi
        # `sha256sum -c` is shared by GNU coreutils and the Darwin-compatible
        # implementation in our devshell; GNU-only `--strict` breaks macOS CI.
        echo "$sha  $tgz" | sha256sum -c -
        rm -rf "$dir"
        mkdir -p "$dir"
        tar -xzf "$tgz" -C "$dir"
    }

    core_pkg="@midnight-ntwrk/credential-compact"
    core_ver="{{vc_core_version}}"
    core_dir="target-gen/${core_pkg##*/}-${core_ver}"
    fetch_package "$core_dir.tgz" \
        "https://registry.npmjs.org/${core_pkg}/-/${core_pkg##*/}-${core_ver}.tgz" \
        "{{vc_core_sha256}}" "$core_dir"
    core="$core_dir/package/dist"

    passport_pkg="@midnight-ntwrk/midnight-vc-passport"
    passport_ver="{{vc_passport_version}}"
    passport_dir="target-gen/${passport_pkg##*/}-${passport_ver}"
    fetch_package "$passport_dir.tgz" \
        "https://registry.npmjs.org/${passport_pkg}/-/${passport_pkg##*/}-${passport_ver}.tgz" \
        "{{vc_passport_sha256}}" "$passport_dir"
    passport="$passport_dir/package"

    staging="$passport/core-compact-staging"
    mkdir -p "$staging/credentials"
    cp "$core/credentials.compact" "$staging/credentials.compact"
    find "$core/credentials" -type f -name '*.compact' \
        -exec cp {} "$staging/credentials/" \;

    # "<module>:<crate>:<entry point>" — module name is the Rust file under
    # the selected crate's src/contract/ directory.
    contracts=(
        "credentials:midnight-vc-runtime:$core/credentials.compact"
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
                echo "//! Family source: \`src/digital-passport-credential.compact\` in"
                echo "//! \`@midnight-ntwrk/midnight-vc-passport@{{vc_passport_version}}\` (npm); core"
                echo "//! contract \`@midnight-ntwrk/credential-compact@{{vc_core_version}}\` (npm),"
                echo "//! staged into the package's \`core-compact-staging/\` by this recipe."
            else
                echo "//! Source: \`dist/credentials.compact\` in"
                echo "//! \`@midnight-ntwrk/credential-compact@{{vc_core_version}}\` (npm)."
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
crate_flags := "-p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did -p midnight-did-runtime -p midnight-did-uniffi -p midnight-did-cli -p midnight-did-indexer -p midnight-did-resolver -p midnight-did-jubjub-schnorr -p midnight-passport-account-source -p midnight-passport-account -p midnight-passport-vault-source -p midnight-vc-domain -p midnight-vc-families -p midnight-vc-proof -p midnight-vc-runtime"

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
coverage_crates := "-p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did -p midnight-did-runtime -p midnight-did-indexer -p midnight-did-jubjub-schnorr -p midnight-did-resolver -p midnight-did-uniffi -p midnight-did-cli -p midnight-passport-account-source -p midnight-passport-account -p midnight-passport-vault-source -p midnight-vc-domain -p midnight-vc-families -p midnight-vc-proof -p midnight-vc-runtime"
coverage_features := "--features midnight-vc-families/digital-passport"
# `contract/generated\.rs` is the DID codegen artifact;
# `contract/{credentials,digital_passport}\.rs` are the VC ones.
# Generated code is gated by `codegen-check` / `codegen-vc-check`, not by tests;
# every hand-written line in those crates stays in scope.
coverage_exclude := 'contract/generated\.rs|contract/credentials\.rs|contract/digital_passport\.rs|src/main\.rs|src/bin/'

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

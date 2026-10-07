// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const ledgerRevision = "b85f5d8e503fd1d7a1b128bbc1d7156baf823a65";
const compactRevision = "66ec5f032d2bccbaf2b9c03e5e47d7bd43defe82";
const proofsRevision = "532629b044a88473a7175f4a96c2511c91156136";
const ledgerVersions = new Map([
  ["midnight-base-crypto", "1.0.1"],
  ["midnight-coin-structure", "2.0.2"],
  ["midnight-onchain-runtime", "3.1.1"],
  ["midnight-onchain-state", "3.0.1"],
  ["midnight-onchain-vm", "3.1.1"],
  ["midnight-serialize", "1.1.1"],
  ["midnight-storage", "2.0.3"],
  ["midnight-transient-crypto", "2.1.1"],
  ["midnight-zkir", "2.1.1"],
  ["midnight-zswap", "8.2.0-rc.1"],
  ["midnight-ledger", "8.2.0-rc.1"],
]);
const ledgerCrates = [...ledgerVersions.keys()];

function escaped(value) {
  return value.replaceAll(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function exactGitDependency(name, repository, revision) {
  return new RegExp(`^${escaped(name)}\\s*=\\s*\\{[^}]*git\\s*=\\s*"${escaped(repository)}"[^}]*rev\\s*=\\s*"${revision}"[^}]*\\}`, "mu");
}

test("published workspace dependencies use one immutable Ledger and Compact graph", async () => {
  const manifest = await readFile(path.join(root, "Cargo.toml"), "utf8");
  const ledgerRepository = "https://github.com/MediaNoxLabs/midnight-ledger.git";
  for (const crate of ledgerCrates) {
    const matches = manifest.match(new RegExp(exactGitDependency(crate, ledgerRepository, ledgerRevision), "gmu")) ?? [];
    assert.equal(matches.length, 2, `${crate} must be pinned in workspace.dependencies and patch.crates-io`);
  }
  for (const alias of ["compact-runtime", "midnight-compact-runtime"]) {
    assert.match(
      manifest,
      exactGitDependency(alias, "https://github.com/MediaNoxLabs/compact.git", compactRevision),
    );
  }
  assert.match(
    manifest,
    exactGitDependency("midnight-proofs", "https://github.com/MediaNoxLabs/midnight-zk.git", proofsRevision),
  );
  assert.doesNotMatch(manifest, /path\s*=\s*"third_party\/(?:midnight-ledger|compact)/u);
});

test("the source lock constrains Cargo to the patched Ledger versions", async () => {
  const [rootManifest, runtimeManifest, sourceLockManifest] = await Promise.all([
    readFile(path.join(root, "Cargo.toml"), "utf8"),
    readFile(path.join(root, "crates", "midnight-did-runtime", "Cargo.toml"), "utf8"),
    readFile(path.join(root, "crates", "midnight-ledger-source-lock", "Cargo.toml"), "utf8"),
  ]);
  assert.match(rootManifest, /"crates\/midnight-ledger-source-lock"/u);
  assert.match(runtimeManifest, /^midnight-ledger-source-lock\s*=\s*\{\s*workspace\s*=\s*true\s*\}/mu);
  for (const [crate, version] of ledgerVersions) {
    assert.match(
      sourceLockManifest,
      new RegExp(`^${escaped(crate)}\\s*=\\s*"=${escaped(version)}"$`, "mu"),
    );
  }
});

test("Nix pins the same immutable sources and exposes only opt-in Cargo overrides", async () => {
  const [flake, wrapper] = await Promise.all([
    readFile(path.join(root, "flake.nix"), "utf8"),
    readFile(path.join(root, "scripts", "cargo-with-nix-overrides.sh"), "utf8"),
  ]);
  assert.match(flake, new RegExp(`rev\\s*=\\s*"${ledgerRevision}"`, "u"));
  assert.match(flake, new RegExp(`rev\\s*=\\s*"${compactRevision}"`, "u"));
  assert.match(flake, new RegExp(`rev\\s*=\\s*"${proofsRevision}"`, "u"));
  assert.match(wrapper, /patch\.crates-io/u);
  assert.match(wrapper, /patch\.\\"https:\/\/github\.com\/MediaNoxLabs\/midnight-ledger\.git\\"/u);
  assert.match(wrapper, /patch\.\\"https:\/\/github\.com\/MediaNoxLabs\/compact\.git\\"/u);
  for (const crate of ledgerCrates) assert.match(wrapper, new RegExp(escaped(crate), "u"));
});

test("clean external-consumer smoke is wired into CI and documents its supported cone", async () => {
  const [script, workflow, documentation] = await Promise.all([
    readFile(path.join(root, "scripts", "ci", "git-consumer-smoke.sh"), "utf8"),
    readFile(path.join(root, ".github", "workflows", "ci.yml"), "utf8"),
    readFile(path.join(root, "crates", "midnight-did-runtime", "README.md"), "utf8"),
  ]);
  await access(path.join(root, "scripts", "cargo-with-nix-overrides.sh"));
  assert.match(script, /mktemp/u);
  assert.match(script, /cargo metadata/u);
  assert.match(script, /cargo check/u);
  assert.match(script, /midnight-did-runtime/u);
  assert.match(script, /http,node-subxt/u);
  assert.match(script, new RegExp(ledgerRevision, "u"));
  assert.match(script, new RegExp(proofsRevision, "u"));
  assert.match(workflow, /bash scripts\/ci\/git-consumer-smoke\.sh/u);
  for (const phrase of [
    "Immutable Git consumption",
    "default feature set is empty",
    "`http`",
    "`node-subxt`",
    "Cargo does not inherit `[patch]` tables",
    compactRevision,
    proofsRevision,
    "cargo-with-nix-overrides.sh",
  ]) assert.ok(documentation.includes(phrase), phrase);
});

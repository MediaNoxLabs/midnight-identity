#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

const root = path.resolve(import.meta.dirname, "../..");
const defaultOxidReference = "/Users/ysh/iohk/midnight-identity-reference-worktrees/issue-29/oxid";
const oxidReference = process.env.OXID_REFERENCE_ROOT ?? defaultOxidReference;
function run(command, args, options = {}) {
  return execFileSync(command, args, { cwd: root, encoding: "utf8", timeout: 60_000, ...options });
}
function digest(file) { return createHash("sha256").update(readFileSync(file)).digest("hex"); }

function generate(out, { checkout = true, reference = oxidReference } = {}) {
  const args = ["scripts/compact/passport-vault-ledger8-artifacts.mjs", "--out", out];
  if (checkout) args.push("--oxid-reference", reference);
  const output = run("node", args);
  return JSON.parse(output);
}
function withOxidClone(t, remoteUrl, body) {
  if (!existsSync(path.join(oxidReference, ".git"))) {
    t.skip("set OXID_REFERENCE_ROOT to a clean Oxid checkout for checkout-mode provenance validation");
    return;
  }
  const dir = mkdtempSync(path.join(tmpdir(), "pv-ledger8-remote-"));
  try {
    const clone = path.join(dir, "oxid");
    run("git", ["clone", "--quiet", oxidReference, clone]);
    run("git", ["-C", clone, "checkout", "--quiet", "50a21b47d280f06cafad94961d4a40f0a83fb3b4"]);
    run("git", ["-C", clone, "update-ref", "refs/remotes/origin/develop", "HEAD"]);
    run("git", ["-C", clone, "remote", "set-url", "origin", remoteUrl]);
    body(clone, dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("Passport Vault Ledger 8 manifest generation is deterministic and source-authenticated", (t) => {
  if (!existsSync(path.join(oxidReference, ".git"))) {
    t.skip("set OXID_REFERENCE_ROOT to a clean Oxid checkout for checkout-mode provenance validation");
    return;
  }
  const dir = mkdtempSync(path.join(tmpdir(), "pv-ledger8-"));
  try {
    const first = path.join(dir, "first.json");
    const second = path.join(dir, "second.json");
    const firstResult = generate(first);
    const secondResult = generate(second);
    assert.equal(digest(first), digest(second));
    assert.equal(firstResult.sha256, secondResult.sha256);
    const manifest = JSON.parse(readFileSync(first, "utf8"));
    assert.equal(manifest.package.ledgerLine, "8.x");
    assert.equal(manifest.source.contractSha256, "2ebc5b34dd440bc9a9736408f29f5003e7a78f26a564b392be2af36de69102f4");
    assert.equal(manifest.source.includeClosureSha256, "76b91bc29be9f41753eeef232bd696fb8181af0670e52d658e6e7da04adde127");
    assert.equal(manifest.referenceBaseline.headSha, "50a21b47d280f06cafad94961d4a40f0a83fb3b4");
    assert.equal(manifest.referenceBaseline.observationMode, "pinned-baseline");
    assert.deepEqual(manifest.circuits.map(({ id }) => id), [
      "setTrustedIssuer",
      "createLock",
      "depositToLock",
      "claimFromLock",
      "withdrawFromLock",
    ]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("checkout mode accepts equivalent canonical GitHub Oxid remote forms", (t) => {
  for (const remote of [
    "https://github.com/MediaNoxLabs/oxid.git",
    "https://github.com/MediaNoxLabs/oxid",
    "git@github.com:MediaNoxLabs/oxid.git",
    "ssh://git@github.com/MediaNoxLabs/oxid.git",
  ]) {
    withOxidClone(t, remote, (clone, dir) => {
      const out = path.join(dir, `${remote.replace(/[^a-z0-9]+/giu, "-")}.json`);
      generate(out, { reference: clone });
      const manifest = JSON.parse(readFileSync(out, "utf8"));
      assert.equal(manifest.referenceBaseline.repository, "MediaNoxLabs/oxid");
      assert.equal(manifest.referenceBaseline.remote, "https://github.com/MediaNoxLabs/oxid.git");
    });
  }
});

test("checkout mode rejects fork and wrong-owner GitHub remote forms", (t) => {
  for (const remote of [
    "https://github.com/OtherOwner/oxid.git",
    "git@github.com:MediaNoxLabs/oxid-fork.git",
    "ssh://git@github.com/OtherOwner/oxid.git",
  ]) {
    withOxidClone(t, remote, (clone, dir) => {
      assert.throws(() => generate(path.join(dir, "manifest.json"), { reference: clone }), /unexpected Oxid remote/u);
    });
  }
});

test("hosted CI mode uses the committed Oxid baseline without a developer-local checkout", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "pv-ledger8-baseline-"));
  try {
    const out = path.join(dir, "manifest.json");
    generate(out, { checkout: false });
    const manifest = JSON.parse(readFileSync(out, "utf8"));
    assert.equal(manifest.referenceBaseline.observationMode, "pinned-baseline");
    assert.equal(manifest.referenceBaseline.sourcePathSha256, "6a9a915aa29a22a0574dbd638deced8a529d0bdd278eb81e475feaa0812cb46a");
    assert.equal(manifest.materialization.derivationRevision, "50a21b47d280f06cafad94961d4a40f0a83fb3b4");
    assert.equal(manifest.materialization.emitsArtifactDigests, true);
    assert.equal(manifest.cacheKeyInputs.materializerRevision, "50a21b47d280f06cafad94961d4a40f0a83fb3b4");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("baseline source provenance must match the authenticated local source", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "pv-ledger8-baseline-mismatch-"));
  try {
    const baseline = path.join(dir, "baseline.json");
    const out = path.join(dir, "manifest.json");
    const mismatched = JSON.parse(readFileSync(path.join(root, "artifacts/passport-vault-ledger8/oxid-baseline.json"), "utf8"));
    mismatched.oxidVaultRevision = "0000000000000000000000000000000000000000";
    writeFileSync(baseline, `${JSON.stringify(mismatched, null, 2)}\n`);
    assert.throws(() => run("node", [
      "scripts/compact/passport-vault-ledger8-artifacts.mjs",
      "--baseline", baseline,
      "--out", out,
    ]), /vault revision does not match/u);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("baseline credential provenance must match the authenticated include closure", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "pv-ledger8-credential-mismatch-"));
  try {
    const baseline = path.join(dir, "baseline.json");
    const out = path.join(dir, "manifest.json");
    const mismatched = JSON.parse(readFileSync(path.join(root, "artifacts/passport-vault-ledger8/oxid-baseline.json"), "utf8"));
    mismatched.oxidCredentialRevision = "0000000000000000000000000000000000000000";
    writeFileSync(baseline, `${JSON.stringify(mismatched, null, 2)}\n`);
    assert.throws(() => run("node", [
      "scripts/compact/passport-vault-ledger8-artifacts.mjs",
      "--baseline", baseline,
      "--out", out,
    ]), /credential revision does not match/u);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("baseline compact toolchain pin must match the authenticated materializer", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "pv-ledger8-toolchain-mismatch-"));
  try {
    const baseline = path.join(dir, "baseline.json");
    const out = path.join(dir, "manifest.json");
    const mismatched = JSON.parse(readFileSync(path.join(root, "artifacts/passport-vault-ledger8/oxid-baseline.json"), "utf8"));
    mismatched.compactInputRevision = "0000000000000000000000000000000000000000";
    writeFileSync(baseline, `${JSON.stringify(mismatched, null, 2)}\n`);
    assert.throws(() => run("node", [
      "scripts/compact/passport-vault-ledger8-artifacts.mjs",
      "--baseline", baseline,
      "--out", out,
    ]), /compactInputRevision does not match/u);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("absolute baseline paths are preserved", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "pv-ledger8-absolute-baseline-"));
  try {
    const baseline = path.join(dir, "baseline.json");
    const out = path.join(dir, "manifest.json");
    writeFileSync(baseline, readFileSync(path.join(root, "artifacts/passport-vault-ledger8/oxid-baseline.json")));
    const output = run("node", [
      "scripts/compact/passport-vault-ledger8-artifacts.mjs",
      "--baseline", baseline,
      "--out", out,
    ]);
    assert.equal(JSON.parse(output).ok, true);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("committed Passport Vault artifact leaf excludes heavy generated outputs", () => {
  const tracked = run("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"]).split("\0").filter(Boolean);
  const leafFiles = tracked.filter((file) => file.startsWith("artifacts/passport-vault-ledger8/"));
  const forbidden = leafFiles.filter((file) =>
    /(?:^|\/)(?:target|result)(?:\/|$)/u.test(file)
    || /\.(?:zkir|bzkir|prover|verifier)$/u.test(file)
    || /(?:^|\/)bls_midnight_2p\d+$/u.test(file));
  assert.deepEqual(forbidden, []);
  assert.deepEqual(leafFiles.sort(), [
    "artifacts/passport-vault-ledger8/manifest.json",
    "artifacts/passport-vault-ledger8/oxid-baseline.json",
  ]);
  const sourceCargo = readFileSync(path.join(root, "crates/midnight-passport-vault-source/Cargo.toml"), "utf8");
  assert.match(sourceCargo, /\[dependencies\]\s*(?:\n\s*)*\[dev-dependencies\]/u);
  assert.doesNotMatch(sourceCargo, /midnight-(?:ledger|zkir|compact-runtime|proofs)|halo2|compactc/u);
});

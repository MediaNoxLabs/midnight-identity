// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { constants } from "node:fs";
import { access, readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { EXPECTED_CONFIG, auditConfig } from "../../scripts/git-hooks/configure.mjs";
import { parsePushUpdates, stagedDiffErrors } from "../../scripts/git-hooks/local-policy.mjs";
import { validateReceipt } from "../../scripts/factory/local-gate.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const base = "a".repeat(40);
const head = "b".repeat(40);
const receipt = {
  schemaVersion: 1,
  repository: "MediaNoxLabs/midnight-identity",
  issue: 63,
  branch: "chore/issue-63",
  deliveryTarget: "develop",
  baseRef: "fix/issue-58",
  baseSha: base,
  headSha: head,
  createdAt: "2026-09-15T00:00:00.000Z",
  targetPlan: { schemaVersion: 1, deliveryProfile: "production-ready", mode: "affected", targets: ["policy"] },
  checks: [
    { id: "factory-contract", outcome: "passed" },
    { id: "contribution-policy", outcome: "passed" },
    { id: "secret-scan", outcome: "passed" },
    { id: "diff-check", outcome: "passed" },
  ],
};

test("local gate receipt binds exact repository, branch, base, head, plan, and checks", () => {
  const options = {
    head,
    branch: "chore/issue-63",
    resolveRef: () => base,
    ancestor: () => true,
  };
  assert.deepEqual(validateReceipt(receipt, options), []);
  for (const candidate of [
    { ...receipt, repository: "example/other" },
    { ...receipt, branch: "chore/issue-64" },
    { ...receipt, baseSha: "c".repeat(40) },
    { ...receipt, headSha: "c".repeat(40) },
    { ...receipt, targetPlan: null },
    { ...receipt, checks: receipt.checks.map((check, index) => index === 0 ? { ...check, outcome: "failed" } : check) },
    { ...receipt, unexpected: true },
  ]) assert.ok(validateReceipt(candidate, options).length > 0, JSON.stringify(candidate));
  assert.ok(validateReceipt(receipt, { ...options, ancestor: () => false }).length > 0);
});

test("pre-push parsing preserves branch updates and permits deletion and tag records", () => {
  const zero = "0".repeat(40);
  assert.deepEqual(parsePushUpdates([
    `refs/heads/chore/issue-63 ${head} refs/heads/chore/issue-63 ${zero}`,
    `refs/heads/chore/issue-62 ${zero} refs/heads/chore/issue-62 ${head}`,
    `refs/tags/v0.6.0 ${head} refs/tags/v0.6.0 ${zero}`,
  ].join("\n")), [
    { localRef: "refs/heads/chore/issue-63", localSha: head, remoteRef: "refs/heads/chore/issue-63", remoteSha: zero },
    { localRef: "refs/heads/chore/issue-62", localSha: zero, remoteRef: "refs/heads/chore/issue-62", remoteSha: head },
    { localRef: "refs/tags/v0.6.0", localSha: head, remoteRef: "refs/tags/v0.6.0", remoteSha: zero },
  ]);
  assert.throws(() => parsePushUpdates("malformed"), /malformed ref update/u);
});

test("pre-commit policy rejects whitespace errors and staged credential patterns", () => {
  assert.deepEqual(stagedDiffErrors("+ordinary change\n"), []);
  const simulatedCredential = `+token=${"github_pat_"}${"a".repeat(24)}\n`;
  assert.ok(stagedDiffErrors(simulatedCredential).length > 0);
  assert.ok(stagedDiffErrors("", "file.md:1: trailing whitespace").length > 0);
});

test("Git configuration audit is repository-local and idempotent", () => {
  assert.deepEqual(auditConfig((key) => EXPECTED_CONFIG[key]), []);
  assert.deepEqual(auditConfig(() => ""), [
    { key: "core.hooksPath", expected: ".githooks", actual: null },
    { key: "commit.gpgsign", expected: "true", actual: null },
  ]);
});

test("bootstrap and hooks are tracked executable entrypoints", async () => {
  for (const file of [
    "bootstrap.sh", ".githooks/commit-msg", ".githooks/pre-commit", ".githooks/pre-push",
  ]) await access(path.join(root, file), constants.X_OK);
  const bootstrap = await readFile(path.join(root, "bootstrap.sh"), "utf8");
  for (const mode of ["--check", "--configure-git", "--local-gate", "--verify-local-gate", "--pi"]) {
    assert.ok(bootstrap.includes(mode), mode);
  }
});

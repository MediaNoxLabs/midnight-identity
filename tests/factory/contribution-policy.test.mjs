// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import {
  isPlatformGeneratedCommit,
  validateBranch,
  validateCommit,
  validatePullRequestBase,
  validateSubject,
} from "../../scripts/factory/contribution-policy.mjs";

test("only GitHub-committed merge commits qualify as generated automation", () => {
  const candidate = {
    parents: `${"a".repeat(40)} ${"b".repeat(40)}`,
    committerName: "GitHub",
    committerEmail: "noreply@github.com",
    subject: "Merge pull request #56 from MediaNoxLabs/chore/issue-56",
  };
  assert.equal(isPlatformGeneratedCommit(candidate), true);
  assert.equal(isPlatformGeneratedCommit({
    ...candidate,
    subject: "chore(factory): establish proportional delivery (#56)",
  }), true);
  assert.equal(isPlatformGeneratedCommit({ ...candidate, parents: "a".repeat(40) }), false);
  assert.equal(isPlatformGeneratedCommit({ ...candidate, committerName: "Contributor" }), false);
});

test("GitHub squash commits receive only the narrow generated-commit exception", () => {
  const candidate = {
    parents: "a".repeat(40),
    committerName: "GitHub",
    committerEmail: "noreply@github.com",
    subject: "fix(factory): validate the pull-request base (#58)",
  };
  assert.equal(isPlatformGeneratedCommit(candidate), false);
  assert.equal(isPlatformGeneratedCommit({ ...candidate, allowSquash: true }), true);
  assert.equal(isPlatformGeneratedCommit({ ...candidate, subject: "unscoped squash (#58)", allowSquash: true }), false);
  assert.equal(isPlatformGeneratedCommit({ ...candidate, subject: "fix(factory): missing PR suffix", allowSquash: true }), false);
  assert.equal(isPlatformGeneratedCommit({ ...candidate, committerEmail: "attacker@example.test", allowSquash: true }), false);
});

test("issue branch grammar accepts only approved types and positive issue numbers", () => {
  assert.deepEqual(validateBranch("chore/issue-56"), []);
  for (const branch of ["feature/issue-56", "chore/issue-0", "chore/issue-56-name", "develop"]) {
    assert.equal(validateBranch(branch).length, 1, branch);
  }
});

test("commit and PR subjects require approved type and scope", () => {
  assert.deepEqual(validateSubject("chore(factory): establish proportional delivery"), []);
  assert.deepEqual(validateSubject("feat(did)!: change the public contract"), []);
  for (const subject of [
    "chore: missing scope", "feature(factory): wrong type", "chore(app): wrong scope", "Chore(factory): uppercase",
  ]) assert.equal(validateSubject(subject).length, 1, subject);
});

test("PR bases require one explicit durable target and stacked-parent disposition", () => {
  const direct = "<!-- factory-delivery-target: develop -->\n<!-- factory-stacked-parent: none -->";
  assert.deepEqual(validatePullRequestBase({ base: "develop", head: "fix/issue-58", body: direct }), []);
  assert.ok(validatePullRequestBase({ base: "main", head: "fix/issue-58", body: direct }).length > 0);

  const release = "<!-- factory-delivery-target: rust-codegen -->\n<!-- factory-stacked-parent: none -->";
  assert.deepEqual(validatePullRequestBase({ base: "rust-codegen", head: "chore/issue-60", body: release }), []);

  const stacked = "<!-- factory-delivery-target: develop -->\n<!-- factory-stacked-parent: fix/issue-54 -->";
  assert.deepEqual(validatePullRequestBase({ base: "fix/issue-54", head: "chore/issue-56", body: stacked }), []);
  for (const candidate of [
    { base: "develop", head: "chore/issue-56", body: stacked },
    { base: "feature/issue-54", head: "chore/issue-56", body: stacked },
    { base: "fix/issue-54", head: "fix/issue-54", body: stacked },
    { base: "develop", head: "fix/issue-58", body: "" },
    { base: "develop", head: "fix/issue-58", body: `${direct}\n${direct}` },
  ]) assert.ok(validatePullRequestBase(candidate).length > 0, JSON.stringify(candidate));
});

test("authored commits require one exact author signoff and a good local signature", () => {
  const commit = {
    sha: "a".repeat(40),
    authorName: "Ada Example",
    authorEmail: "ada@example.test",
    subject: "test(factory): prove the negative policy path",
    body: "test(factory): prove the negative policy path\n\nSigned-off-by: Ada Example <ada@example.test>\n",
    signature: "G",
    generatedMerge: false,
  };
  assert.deepEqual(validateCommit(commit, { requireLocalSignature: true }), []);
  assert.equal(validateCommit({ ...commit, signature: "N" }, { requireLocalSignature: true }).length, 1);
  assert.equal(validateCommit({ ...commit, body: commit.body.replace("Ada", "Grace") }).length, 1);
});

test("GitHub-generated commits use hosted verification instead of a local keyring", () => {
  const commit = {
    sha: "b".repeat(40),
    authorName: "GitHub",
    authorEmail: "noreply@github.com",
    subject: "Merge branch 'develop' into chore/issue-56",
    body: "Merge branch 'develop' into chore/issue-56\n",
    signature: "G",
    generatedMerge: true,
  };
  assert.deepEqual(validateCommit(commit, { requireLocalSignature: true }), []);
  assert.deepEqual(validateCommit({ ...commit, signature: "E" }, { requireLocalSignature: true }), []);
});

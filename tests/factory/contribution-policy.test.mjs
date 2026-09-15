// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import {
  isPlatformGeneratedMerge,
  validateBranch,
  validateCommit,
  validateSubject,
} from "../../scripts/factory/contribution-policy.mjs";

test("only GitHub-committed merge commits qualify as generated automation", () => {
  const candidate = {
    parents: `${"a".repeat(40)} ${"b".repeat(40)}`,
    committerName: "GitHub",
    committerEmail: "noreply@github.com",
    subject: "Merge pull request #56 from MediaNoxLabs/chore/issue-56",
  };
  assert.equal(isPlatformGeneratedMerge(candidate), true);
  assert.equal(isPlatformGeneratedMerge({
    ...candidate,
    subject: "chore(factory): establish proportional delivery (#56)",
  }), true);
  assert.equal(isPlatformGeneratedMerge({ ...candidate, parents: "a".repeat(40) }), false);
  assert.equal(isPlatformGeneratedMerge({ ...candidate, committerName: "Contributor" }), false);
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

test("the narrow GitHub-generated merge exception does not exempt signature verification", () => {
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
  assert.equal(validateCommit({ ...commit, signature: "N" }, { requireLocalSignature: true }).length, 1);
});

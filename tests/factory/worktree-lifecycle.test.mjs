// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import { evaluateCloseout, parseWorktreePorcelain } from "../../scripts/factory/worktree-lifecycle.mjs";

const head = "a".repeat(40);

function eligible() {
  return {
    entry: { path: "/repo-worktrees/issue-56", head, branch: "chore/issue-56", locked: false, prunable: false },
    exactPath: "/repo-worktrees/issue-56",
    expectHead: head,
    currentPath: "/repo",
    primaryPath: "/repo",
    dirty: false,
    active: false,
    pr: {
      state: "MERGED",
      headRefOid: head,
      headRefName: "chore/issue-56",
      baseRefName: "develop",
      mergeCommit: { oid: "b".repeat(40) },
    },
  };
}

test("porcelain parsing preserves locked, detached, and prunable state", () => {
  const parsed = parseWorktreePorcelain([
    "worktree /repo", `HEAD ${head}`, "branch refs/heads/develop", "",
    "worktree /repo-worktrees/issue-56", `HEAD ${head}`, "detached", "locked owner", "prunable missing", "",
  ].join("\n"));
  assert.equal(parsed.length, 2);
  assert.equal(parsed[1].detached, true);
  assert.equal(parsed[1].locked, "owner");
  assert.equal(parsed[1].prunable, "missing");
});

test("an exact clean inactive merged issue worktree is eligible", () => {
  assert.deepEqual(evaluateCloseout(eligible()), []);
});

test("unknown, dirty, locked, active, current, mismatched, and unmerged worktrees are preserved", () => {
  assert.match(evaluateCloseout({ ...eligible(), entry: null }).join(";"), /not a registered/u);
  for (const mutation of [
    { dirty: true },
    { active: true },
    { currentPath: "/repo-worktrees/issue-56/src" },
    { expectHead: "c".repeat(40) },
    { pr: { ...eligible().pr, state: "OPEN" } },
    { entry: { ...eligible().entry, locked: "owner" } },
  ]) assert.ok(evaluateCloseout({ ...eligible(), ...mutation }).length > 0, JSON.stringify(mutation));
});

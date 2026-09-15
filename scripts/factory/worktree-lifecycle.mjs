#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync, spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import os from "node:os";
import path from "node:path";

const REPOSITORY = "MediaNoxLabs/midnight-identity";
const SHA = /^[0-9a-f]{40}$/u;
const ISSUE_BRANCH = /^(?:feat|fix|docs|refactor|test|ci|chore)\/issue-[1-9]\d*$/u;
const MERGE_BASE = /^(?:develop|rust-codegen|milestone-(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))$/u;

function git(cwd, args, options = {}) {
  return execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    ...options,
  }).trim();
}

export function parseWorktreePorcelain(raw) {
  return raw.trim().split(/\n\n+/u).filter(Boolean).map((record) => {
    const entry = { path: null, head: null, branch: null, bare: false, detached: false, locked: false, prunable: false };
    for (const line of record.split("\n")) {
      const [key, ...rest] = line.split(" ");
      const value = rest.join(" ");
      if (key === "worktree") entry.path = value;
      else if (key === "HEAD") entry.head = value;
      else if (key === "branch") entry.branch = value.replace(/^refs\/heads\//u, "");
      else if (key === "bare") entry.bare = true;
      else if (key === "detached") entry.detached = true;
      else if (key === "locked") entry.locked = value || true;
      else if (key === "prunable") entry.prunable = value || true;
    }
    return entry;
  });
}

function worktrees(cwd) {
  return parseWorktreePorcelain(git(cwd, ["worktree", "list", "--porcelain"]));
}

function kilobytes(candidate) {
  try {
    return Number(execFileSync("du", ["-sk", candidate], {
      encoding: "utf8", timeout: 30_000, stdio: ["ignore", "pipe", "ignore"],
    }).trim().split(/\s+/u)[0]);
  } catch {
    return null;
  }
}

function status(candidate) {
  try {
    return git(candidate, ["status", "--porcelain"]);
  } catch {
    return null;
  }
}

function activeCwd(candidate) {
  const result = spawnSync("lsof", ["-a", "-d", "cwd", "+D", candidate], {
    encoding: "utf8",
    timeout: 5000,
    maxBuffer: 4 * 1024 * 1024,
  });
  if (result.error?.code === "ENOENT" || result.error?.code === "ETIMEDOUT") return null;
  if (result.status === 1 && !result.stdout.trim()) return false;
  if (result.status !== 0) return null;
  return result.stdout.trim().split("\n").slice(1).filter(Boolean).length > 0;
}

export function evaluateCloseout({ entry, exactPath, expectHead, currentPath, primaryPath, dirty, active, pr }) {
  const errors = [];
  if (!entry) return ["path is not a registered Git worktree"];
  if (entry.path !== exactPath) errors.push("registered worktree path does not match the exact canonical path");
  if (exactPath === primaryPath) errors.push("primary checkout is never a closeout target");
  if (currentPath === exactPath || currentPath.startsWith(`${exactPath}${path.sep}`)) errors.push("leave the target worktree before closeout");
  if (!ISSUE_BRANCH.test(entry.branch ?? "")) errors.push("worktree branch is detached or not issue-backed");
  if (entry.locked) errors.push("worktree is locked");
  if (entry.prunable) errors.push("worktree metadata is prunable or incomplete");
  if (entry.head !== expectHead) errors.push("worktree head does not match --expect-head");
  if (dirty === null) errors.push("worktree cleanliness is unavailable");
  else if (dirty) errors.push("worktree is dirty or has untracked files");
  if (active === null) errors.push("active-process observation is unavailable");
  else if (active) errors.push("a process has a working directory inside the worktree");
  if (!pr || pr.state !== "MERGED") errors.push("pull request is not merged");
  if (pr?.headRefOid !== expectHead) errors.push("merged pull-request head does not match --expect-head");
  if (pr?.headRefName !== entry.branch) errors.push("pull-request branch does not match the worktree branch");
  if (!MERGE_BASE.test(pr?.baseRefName ?? "")) errors.push("pull-request base is not an approved delivery branch");
  if (!SHA.test(pr?.mergeCommit?.oid ?? "")) errors.push("merged pull request has no exact merge commit");
  return errors;
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index === -1 ? undefined : argv[index + 1];
}

function positive(value, name) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 1) throw new Error(`${name} must be a positive integer`);
  return parsed;
}

function canonicalPath(candidate) {
  if (!candidate || !path.isAbsolute(candidate) || candidate.includes("*") || candidate.includes("?")) {
    throw new Error("--path must be one explicit absolute path without globs");
  }
  const resolved = realpathSync(candidate);
  if (["/", realpathSync(os.homedir())].includes(resolved)) throw new Error("refusing a broad closeout path");
  return resolved;
}

function inspect(cwd) {
  const currentPath = realpathSync(git(cwd, ["rev-parse", "--show-toplevel"]));
  const entries = worktrees(cwd);
  const primaryPath = entries[0]?.path ? realpathSync(entries[0].path) : null;
  return entries.map((entry) => {
    const exactPath = entry.path && !entry.prunable ? realpathSync(entry.path) : entry.path;
    const dirtyState = entry.path && !entry.prunable ? status(entry.path) : null;
    return {
      ...entry,
      path: exactPath,
      current: exactPath === currentPath,
      primary: exactPath === primaryPath,
      dirty: dirtyState === null ? null : dirtyState.length > 0,
      sizeKiB: entry.path && !entry.prunable ? kilobytes(entry.path) : null,
      targetKiB: entry.path && !entry.prunable ? kilobytes(path.join(entry.path, "target")) : null,
    };
  });
}

function audit(cwd, json, stdout) {
  const entries = inspect(cwd);
  if (json) stdout.write(`${JSON.stringify({ schemaVersion: 1, worktrees: entries }, null, 2)}\n`);
  else {
    stdout.write("path\tbranch\thead\tdirty\tlocked\tcurrent\tprimary\tsizeKiB\ttargetKiB\n");
    for (const entry of entries) {
      stdout.write([
        entry.path, entry.branch ?? "detached", entry.head ?? "unknown",
        entry.dirty ?? "unknown", Boolean(entry.locked), entry.current,
        entry.primary, entry.sizeKiB ?? "unknown", entry.targetKiB ?? 0,
      ].join("\t") + "\n");
    }
  }
  return 0;
}

function closeout(argv, cwd, stdout) {
  const exactPath = canonicalPath(option(argv, "--path"));
  const expectHead = option(argv, "--expect-head");
  if (!expectHead || !SHA.test(expectHead)) throw new Error("--expect-head must be an exact lowercase 40-character SHA");
  const prNumber = positive(option(argv, "--pr"), "--pr");
  const currentPath = realpathSync(git(cwd, ["rev-parse", "--show-toplevel"]));
  const entries = worktrees(cwd).map((entry) => ({
    ...entry,
    path: entry.path && !entry.prunable ? realpathSync(entry.path) : entry.path,
  }));
  const primaryPath = entries[0]?.path ?? null;
  const entry = entries.find(({ path: candidate }) => candidate === exactPath);
  const pr = JSON.parse(execFileSync("gh", [
    "pr", "view", String(prNumber), "--repo", REPOSITORY,
    "--json", "state,headRefOid,headRefName,baseRefName,mergeCommit,url",
  ], { encoding: "utf8", maxBuffer: 2 * 1024 * 1024 }));
  const dirtyState = status(exactPath);
  const errors = evaluateCloseout({
    entry,
    exactPath,
    expectHead,
    currentPath,
    primaryPath,
    dirty: dirtyState === null ? null : dirtyState.length > 0,
    active: activeCwd(exactPath),
    pr,
  });
  if (errors.length) throw new Error(errors.join("; "));

  git(cwd, ["fetch", "--quiet", "origin", pr.baseRefName]);
  const reachable = spawnSync("git", ["merge-base", "--is-ancestor", pr.mergeCommit.oid, `origin/${pr.baseRefName}`], { cwd });
  if (reachable.status !== 0) throw new Error("merge commit is not reachable from the fetched delivery base");
  stdout.write(`eligible: PR #${prNumber} ${entry.branch} ${expectHead} at ${exactPath}\n`);
  if (!argv.includes("--execute")) {
    stdout.write("audit only; pass --execute to remove this exact worktree and local branch\n");
    return 0;
  }
  git(cwd, ["worktree", "remove", exactPath]);
  git(cwd, ["branch", "-D", entry.branch]);
  stdout.write(`removed exact worktree and local branch: ${exactPath} (${entry.branch})\n`);
  return 0;
}

export function run(argv = process.argv.slice(2), {
  cwd = process.cwd(),
  stdout = process.stdout,
  stderr = process.stderr,
} = {}) {
  try {
    if (argv[0] === "audit") return audit(cwd, argv.includes("--json"), stdout);
    if (argv[0] === "closeout") return closeout(argv.slice(1), cwd, stdout);
    throw new Error("usage: worktree-lifecycle.mjs audit [--json] | closeout --pr N --path ABS --expect-head SHA [--execute]");
  } catch (error) {
    stderr.write(`worktree lifecycle: ${error.message}\n`);
    return 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = run();
}

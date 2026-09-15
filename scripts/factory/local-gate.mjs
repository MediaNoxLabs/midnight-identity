#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync, spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { validateBranch } from "./contribution-policy.mjs";

const REPOSITORY = "MediaNoxLabs/midnight-identity";
const SHA = /^[0-9a-f]{40}$/u;
const DELIVERY_TARGET = /^(?:develop|rust-codegen)$/u;
const BASE_REF = /^(?:origin\/)?(?:(?:develop|rust-codegen)|(?:feat|fix|docs|refactor|test|ci|chore)\/issue-[1-9]\d*)$/u;
const RECEIPT_BASE_REF = /^origin\/(?:(?:develop|rust-codegen)|(?:feat|fix|docs|refactor|test|ci|chore)\/issue-[1-9]\d*)$/u;
const CHECK_IDS = Object.freeze(["factory-contract", "contribution-policy", "secret-scan", "diff-check"]);
const TARGET_IDS = new Set(["policy", "rust", "unit", "wasm", "coverage", "did-codegen", "vc-codegen"]);
const RECEIPT_KEYS = Object.freeze([
  "schemaVersion", "repository", "issue", "branch", "deliveryTarget", "baseRef",
  "baseSha", "headSha", "createdAt", "targetPlan", "checks",
]);

function command(program, args, options = {}) {
  return execFileSync(program, args, {
    encoding: "utf8", maxBuffer: 32 * 1024 * 1024, ...options,
  }).trim();
}

function runCommand(program, args, cwd) {
  const result = spawnSync(program, args, { cwd, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${program} ${args.join(" ")} failed with ${result.status}`);
}

function git(cwd, args, options = {}) {
  return command("git", args, { cwd, ...options });
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index === -1 ? undefined : argv[index + 1];
}

function receiptDir(cwd) {
  const common = git(cwd, ["rev-parse", "--path-format=absolute", "--git-common-dir"]);
  return path.join(common, "midnight-identity-factory", "local-gates-v1");
}

function receiptPath(cwd, head) {
  return path.join(receiptDir(cwd), `${head}.json`);
}

function issueFromBranch(branch) {
  const match = /\/issue-([1-9]\d*)$/u.exec(branch);
  return match ? Number(match[1]) : null;
}

function isAncestor(cwd, base, head) {
  return spawnSync("git", ["merge-base", "--is-ancestor", base, head], { cwd }).status === 0;
}

export function canonicalRemoteBaseRef(baseRef) {
  if (!BASE_REF.test(baseRef ?? "")) {
    throw new Error("--base must be a durable target or issue-backed branch ref");
  }
  return baseRef.startsWith("origin/") ? baseRef : `origin/${baseRef}`;
}

function branchFromRemoteRef(baseRef) {
  if (!RECEIPT_BASE_REF.test(baseRef)) throw new Error("base ref is not an approved origin branch");
  return baseRef.slice("origin/".length);
}

function fetchRemoteBranch(cwd, branch) {
  runCommand("git", [
    "fetch", "--no-tags", "--no-recurse-submodules", "origin",
    `+refs/heads/${branch}:refs/remotes/origin/${branch}`,
  ], cwd);
}

function remoteSha(cwd, baseRef) {
  const branch = branchFromRemoteRef(baseRef);
  const expectedRef = `refs/heads/${branch}`;
  const output = git(cwd, ["ls-remote", "--exit-code", "origin", expectedRef]);
  const matches = output.split("\n").filter(Boolean).map((line) => line.trim().split(/\s+/u));
  if (matches.length !== 1 || matches[0][1] !== expectedRef || !SHA.test(matches[0][0])) {
    throw new Error(`remote base ${baseRef} did not resolve exactly once`);
  }
  return matches[0][0];
}

export function validateReceipt(receipt, {
  repository = REPOSITORY,
  head,
  branch,
  resolveRef,
  ancestor,
} = {}) {
  const errors = [];
  if (!receipt || typeof receipt !== "object" || Array.isArray(receipt)) return ["receipt must be an object"];
  const keys = Object.keys(receipt).sort();
  if (JSON.stringify(keys) !== JSON.stringify([...RECEIPT_KEYS].sort())) errors.push("receipt fields do not match the closed v1 schema");
  if (receipt.schemaVersion !== 1) errors.push("receipt schemaVersion must be 1");
  if (receipt.repository !== repository) errors.push("receipt repository does not match");
  if (validateBranch(receipt.branch ?? "").length > 0) errors.push("receipt branch is not issue-backed");
  if (receipt.issue !== issueFromBranch(receipt.branch ?? "")) errors.push("receipt issue does not match its branch");
  if (!DELIVERY_TARGET.test(receipt.deliveryTarget ?? "")) errors.push("receipt delivery target is not durable");
  if (!RECEIPT_BASE_REF.test(receipt.baseRef ?? "")) errors.push("receipt base ref is not an approved origin branch");
  if (!SHA.test(receipt.baseSha ?? "")) errors.push("receipt base SHA is invalid");
  if (!SHA.test(receipt.headSha ?? "")) errors.push("receipt head SHA is invalid");
  if (!Number.isFinite(Date.parse(receipt.createdAt ?? ""))) errors.push("receipt timestamp is invalid");
  if (head && receipt.headSha !== head) errors.push("receipt head is stale");
  if (branch && receipt.branch !== branch) errors.push("receipt branch does not match");
  if (!receipt.targetPlan
      || receipt.targetPlan.schemaVersion !== 1
      || receipt.targetPlan.deliveryProfile !== "production-ready"
      || !["affected", "full"].includes(receipt.targetPlan.mode)
      || !Array.isArray(receipt.targetPlan.targets)
      || receipt.targetPlan.targets.length === 0
      || new Set(receipt.targetPlan.targets).size !== receipt.targetPlan.targets.length
      || receipt.targetPlan.targets.some((target) => !TARGET_IDS.has(target))) {
    errors.push("receipt target plan is missing");
  }
  if (!Array.isArray(receipt.checks)
      || JSON.stringify(receipt.checks.map(({ id }) => id)) !== JSON.stringify(CHECK_IDS)
      || receipt.checks.some((check) => check.outcome !== "passed"
        || JSON.stringify(Object.keys(check).sort()) !== JSON.stringify(["id", "outcome"]))) {
    errors.push("receipt checks are incomplete or failed");
  }
  if (resolveRef && SHA.test(receipt.baseSha ?? "")) {
    let currentBase = null;
    try { currentBase = resolveRef(receipt.baseRef); } catch { /* reported below */ }
    if (currentBase !== receipt.baseSha) errors.push("receipt base is stale or unavailable");
  }
  if (ancestor && SHA.test(receipt.baseSha ?? "") && SHA.test(receipt.headSha ?? "")
      && !ancestor(receipt.baseSha, receipt.headSha)) errors.push("receipt base is not an ancestor of its head");
  return errors;
}

function runGate(argv, cwd, stdout) {
  const branch = git(cwd, ["branch", "--show-current"]);
  const branchErrors = validateBranch(branch);
  if (branchErrors.length > 0) throw new Error(branchErrors.join("; "));
  const deliveryTarget = option(argv, "--delivery-target");
  if (!DELIVERY_TARGET.test(deliveryTarget ?? "")) throw new Error("--delivery-target must be develop or rust-codegen");
  const baseRef = canonicalRemoteBaseRef(option(argv, "--base") ?? deliveryTarget);
  if (git(cwd, ["status", "--porcelain"])) throw new Error("worktree must be clean before creating an exact-head receipt");
  fetchRemoteBranch(cwd, deliveryTarget);
  const baseBranch = branchFromRemoteRef(baseRef);
  if (baseBranch !== deliveryTarget) fetchRemoteBranch(cwd, baseBranch);
  const baseSha = git(cwd, ["rev-parse", `${baseRef}^{commit}`]);
  const headSha = git(cwd, ["rev-parse", "HEAD^{commit}"]);
  if (!isAncestor(cwd, baseSha, headSha)) throw new Error("selected base is not an ancestor of HEAD");

  runCommand(process.execPath, ["scripts/factory/check.mjs"], cwd);
  runCommand(process.execPath, [
    "scripts/factory/contribution-policy.mjs", "--base", baseSha, "--head", headSha,
    "--branch", branch, "--local-signatures", "--repo", REPOSITORY,
    "--github-generated-signatures",
  ], cwd);
  runCommand(process.execPath, ["scripts/factory/secret-scan.mjs", "--base", baseSha, "--head", headSha], cwd);
  runCommand("git", ["diff", "--check", `${baseSha}...${headSha}`], cwd);
  const targetPlan = JSON.parse(command(process.execPath, [
    "scripts/ci/target-plan.mjs", "--base", baseSha, "--head", headSha,
    "--delivery-profile", "production-ready", "--format", "json",
  ], { cwd }));
  const receipt = {
    schemaVersion: 1,
    repository: REPOSITORY,
    issue: issueFromBranch(branch),
    branch,
    deliveryTarget,
    baseRef,
    baseSha,
    headSha,
    createdAt: new Date().toISOString(),
    targetPlan,
    checks: CHECK_IDS.map((id) => ({ id, outcome: "passed" })),
  };
  const errors = validateReceipt(receipt, {
    head: headSha,
    branch,
    resolveRef: (ref) => remoteSha(cwd, ref),
    ancestor: (base, head) => isAncestor(cwd, base, head),
  });
  if (errors.length > 0) throw new Error(errors.join("; "));
  const directory = receiptDir(cwd);
  mkdirSync(directory, { recursive: true, mode: 0o700 });
  chmodSync(directory, 0o700);
  const destination = receiptPath(cwd, headSha);
  const temporary = `${destination}.${process.pid}.tmp`;
  try {
    writeFileSync(temporary, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600, flag: "wx" });
    renameSync(temporary, destination);
  } finally {
    rmSync(temporary, { force: true });
  }
  stdout.write(`local gate: wrote exact-head receipt for ${branch} ${headSha}\n`);
  stdout.write(`local gate: hosted targets remain ${targetPlan.targets.join(", ")}\n`);
  return 0;
}

function verifyGate(argv, cwd, stdout) {
  const head = option(argv, "--head");
  const branch = option(argv, "--branch");
  if (!SHA.test(head ?? "")) throw new Error("--head must be an exact lowercase 40-character SHA");
  if (validateBranch(branch ?? "").length > 0) throw new Error("--branch must be an issue-backed branch");
  const receipt = JSON.parse(readFileSync(receiptPath(cwd, head), "utf8"));
  const errors = validateReceipt(receipt, {
    head,
    branch,
    resolveRef: (ref) => remoteSha(cwd, ref),
    ancestor: (base, candidate) => isAncestor(cwd, base, candidate),
  });
  if (errors.length > 0) throw new Error(errors.join("; "));
  stdout.write(`local gate: valid exact-head receipt for ${branch} ${head}\n`);
  return 0;
}

export function run(argv = process.argv.slice(2), {
  cwd = process.cwd(), stdout = process.stdout, stderr = process.stderr,
} = {}) {
  try {
    if (argv[0] === "run") return runGate(argv.slice(1), cwd, stdout);
    if (argv[0] === "verify") return verifyGate(argv.slice(1), cwd, stdout);
    throw new Error("usage: local-gate.mjs run [--base REF] --delivery-target TARGET | verify --head SHA --branch BRANCH");
  } catch (error) {
    stderr.write(`local gate: ${error.message}\n`);
    return 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = run();
}

#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { validateBranch } from "../factory/contribution-policy.mjs";
import { scanAddedLines } from "../factory/secret-scan.mjs";

const SHA = /^[0-9a-f]{40}$/u;
const ZERO_SHA = /^0{40}$/u;

function command(program, args, options = {}) {
  const output = execFileSync(program, args, {
    encoding: "utf8", maxBuffer: 32 * 1024 * 1024, ...options,
  });
  return typeof output === "string" ? output.trim() : "";
}

export function parsePushUpdates(input) {
  return input.split("\n").filter(Boolean).map((line) => {
    const fields = line.trim().split(/\s+/u);
    if (fields.length !== 4 || !SHA.test(fields[1]) || !SHA.test(fields[3])) {
      throw new Error("pre-push received a malformed ref update");
    }
    return { localRef: fields[0], localSha: fields[1], remoteRef: fields[2], remoteSha: fields[3] };
  });
}

export function validatePushUpdate(update) {
  if (ZERO_SHA.test(update.localSha) || !update.remoteRef.startsWith("refs/heads/")) {
    return { branch: null, errors: [] };
  }

  const branch = update.remoteRef.slice("refs/heads/".length);
  const errors = validateBranch(branch);
  if (update.localRef.startsWith("refs/heads/")) {
    const localBranch = update.localRef.slice("refs/heads/".length);
    if (localBranch !== branch) {
      errors.push(`local branch ${localBranch} cannot update remote branch ${branch}`);
    }
  } else if (update.localRef !== "HEAD") {
    errors.push(`unsupported local ref for issue branch ${branch}: ${update.localRef}`);
  }
  return { branch, errors };
}

export function stagedDiffErrors(diff, diffCheckError = "") {
  const errors = [];
  if (diffCheckError) errors.push(diffCheckError.trim());
  for (const finding of scanAddedLines(diff)) {
    errors.push(`staged diff line ${finding.line} matched ${finding.rule}`);
  }
  return errors;
}

function runNode(root, args) {
  command(process.execPath, args, { cwd: root, stdio: "inherit" });
}

function preCommit(root, stdout, stderr) {
  let diffCheckError = "";
  try {
    command("git", ["diff", "--cached", "--check"], { cwd: root });
  } catch (error) {
    diffCheckError = error.stdout?.toString() || error.message;
  }
  const diff = command("git", ["diff", "--cached", "--no-ext-diff", "--unified=0", "--"], { cwd: root });
  const errors = stagedDiffErrors(diff, diffCheckError);
  if (errors.length > 0) {
    errors.forEach((error) => stderr.write(`pre-commit: ${error}\n`));
    return 1;
  }
  stdout.write("pre-commit: staged diff is clean and contains no credential patterns\n");
  return 0;
}

function prePush(root, input, stdout, stderr) {
  const updates = parsePushUpdates(input);
  for (const update of updates) {
    const { branch, errors } = validatePushUpdate(update);
    if (branch === null) continue;
    if (errors.length > 0) {
      errors.forEach((error) => stderr.write(`pre-push: ${error}\n`));
      return 1;
    }
    try {
      runNode(root, ["scripts/factory/local-gate.mjs", "verify", "--head", update.localSha, "--branch", branch]);
    } catch (error) {
      stderr.write(`pre-push: exact-head local gate is not valid for ${branch}: ${error.message}\n`);
      return 1;
    }
  }
  stdout.write("pre-push: every issue branch update has a valid exact-head local gate\n");
  return 0;
}

export function run(argv = process.argv.slice(2), {
  cwd = process.cwd(), stdin = null, stdout = process.stdout, stderr = process.stderr,
} = {}) {
  try {
    const root = command("git", ["rev-parse", "--show-toplevel"], { cwd });
    if (argv[0] === "commit-msg") {
      if (!argv[1]) throw new Error("commit-msg requires the message file path");
      runNode(root, ["scripts/factory/contribution-policy.mjs", "--message-file", argv[1]]);
      return 0;
    }
    if (argv[0] === "pre-commit") return preCommit(root, stdout, stderr);
    if (argv[0] === "pre-push") return prePush(root, stdin ?? readFileSync(0, "utf8"), stdout, stderr);
    throw new Error("usage: local-policy.mjs commit-msg FILE | pre-commit | pre-push");
  } catch (error) {
    stderr.write(`local git policy: ${error.message}\n`);
    return 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = run();
}

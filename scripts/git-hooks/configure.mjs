#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { accessSync, constants } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

export const EXPECTED_CONFIG = Object.freeze({
  "core.hooksPath": ".githooks",
  "commit.gpgsign": "true",
});

function git(cwd, args, options = {}) {
  return execFileSync("git", args, {
    cwd, encoding: "utf8", maxBuffer: 1024 * 1024, ...options,
  }).trim();
}

export function auditConfig(read) {
  const mismatches = [];
  for (const [key, expected] of Object.entries(EXPECTED_CONFIG)) {
    const actual = read(key);
    if (actual !== expected) mismatches.push({ key, expected, actual: actual || null });
  }
  return mismatches;
}

function localValue(cwd, key) {
  try {
    return git(cwd, ["config", "--local", "--get", key]);
  } catch {
    return "";
  }
}

export function run(argv = process.argv.slice(2), {
  cwd = process.cwd(), stdout = process.stdout, stderr = process.stderr,
} = {}) {
  try {
    const root = git(cwd, ["rev-parse", "--show-toplevel"]);
    for (const hook of ["commit-msg", "pre-commit", "pre-push"]) {
      accessSync(path.join(root, ".githooks", hook), constants.X_OK);
    }
    const mode = argv[0] ?? "audit";
    if (mode === "apply") {
      if (!argv.includes("--execute")) throw new Error("apply is an audit unless --execute is explicit");
      for (const [key, value] of Object.entries(EXPECTED_CONFIG)) {
        git(root, ["config", "--local", key, value]);
      }
    } else if (mode !== "audit") {
      throw new Error("usage: configure.mjs audit | apply --execute");
    }
    const mismatches = auditConfig((key) => localValue(root, key));
    if (mismatches.length > 0) {
      mismatches.forEach(({ key, expected, actual }) => stderr.write(
        `git hooks: ${key} is ${actual ?? "unset"}; expected ${expected}\n`,
      ));
      return 1;
    }
    stdout.write("git hooks: repository-local policy configured\n");
    return 0;
  } catch (error) {
    stderr.write(`git hooks: ${error.message}\n`);
    return 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = run();
}

#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const RULES = Object.freeze([
  ["github-token", /(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})/u],
  ["aws-access-key", /AKIA[0-9A-Z]{16}/u],
  ["private-key", /-----BEGIN [A-Z ]*PRIVATE KEY-----/u],
  ["bearer-token", /\bBearer\s+[A-Za-z0-9._~+/-]{20,}={0,2}/iu],
  ["assigned-secret", /\b(?:api[_-]?key|client[_-]?secret|password|private[_-]?key|token)\s*[:=]\s*["']?[A-Za-z0-9/+_.-]{20,}={0,2}/iu],
]);

export function scanAddedLines(diff) {
  const findings = [];
  for (const [index, line] of diff.split("\n").entries()) {
    if (!line.startsWith("+") || line.startsWith("+++")) continue;
    const value = line.slice(1);
    for (const [rule, pattern] of RULES) {
      if (pattern.test(value)) findings.push({ line: index + 1, rule });
    }
  }
  return findings;
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index === -1 ? undefined : argv[index + 1];
}

export function run(argv = process.argv.slice(2), {
  cwd = process.cwd(), stdout = process.stdout, stderr = process.stderr,
} = {}) {
  const base = option(argv, "--base");
  const head = option(argv, "--head") ?? "HEAD";
  if (!base) {
    stderr.write("secret scan: --base is required\n");
    return 1;
  }
  try {
    const diff = execFileSync("git", ["diff", "--no-ext-diff", "--unified=0", base, head, "--"], {
      cwd, encoding: "utf8", maxBuffer: 32 * 1024 * 1024,
    });
    const findings = scanAddedLines(diff);
    if (findings.length) {
      findings.forEach(({ line, rule }) => stderr.write(`secret scan: added diff line ${line} matched ${rule}\n`));
      return 1;
    }
    stdout.write("secret scan: no credential patterns in added lines\n");
    return 0;
  } catch (error) {
    stderr.write(`secret scan: ${error.message}\n`);
    return 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = run();
}

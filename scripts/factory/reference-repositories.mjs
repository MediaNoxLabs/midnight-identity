#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const SHA = /^[0-9a-f]{40}$/u;
const REQUIRED_EVIDENCE = [
  "remote", "exactSourceSha", "cleanStateBefore", "cleanStateAfter",
  "sourcePaths", "licenseProvenance", "beforeStatus", "afterStatus",
];

function command(program, args, options = {}) {
  return execFileSync(program, args, { encoding: "utf8", timeout: 30_000, ...options }).trim();
}

export function loadExtractionPolicy(repoRoot = root) {
  return JSON.parse(readFileSync(path.join(repoRoot, ".pi", "extraction-policy.json"), "utf8"));
}

export function validateExtractionPolicy(policy) {
  const errors = [];
  if (policy?.schemaVersion !== 1) errors.push("schemaVersion must be 1");
  if (policy?.mutationTarget?.repository !== "MediaNoxLabs/midnight-identity") {
    errors.push("mutation target must be MediaNoxLabs/midnight-identity");
  }
  const references = policy?.referenceRepositories;
  const expected = new Map([
    ["MediaNoxLabs/oxid", { remote: "https://github.com/MediaNoxLabs/oxid.git" }],
    ["input-output-hk/lace-id-portal", {
      remote: "https://github.com/input-output-hk/lace-id-portal.git",
      remoteSymbolicHead: "refs/heads/develop",
    }],
  ]);
  if (!Array.isArray(references) || references.length !== expected.size) {
    errors.push("referenceRepositories must list exactly the allowed consumer repositories");
  }
  for (const reference of references ?? []) {
    const expectedReference = expected.get(reference.repository);
    if (!expectedReference) errors.push(`unsupported reference repository: ${reference.repository ?? "<missing>"}`);
    if (reference.allowedRemote !== expectedReference?.remote) errors.push(`${reference.repository}: allowedRemote mismatch`);
    if (expectedReference?.remoteSymbolicHead && reference.remoteSymbolicHead !== expectedReference.remoteSymbolicHead) {
      errors.push(`${reference.repository}: remoteSymbolicHead must be ${expectedReference.remoteSymbolicHead}`);
    }
    if (reference.access !== "read-only") errors.push(`${reference.repository}: access must be read-only`);
    if (reference.defaultReference !== "origin/develop") errors.push(`${reference.repository}: defaultReference must be origin/develop`);
    for (const field of REQUIRED_EVIDENCE) {
      if (!reference.requiredEvidence?.includes(field)) errors.push(`${reference.repository}: missing evidence ${field}`);
    }
  }
  for (const condition of [
    "unknown-reference-repository", "reference-remote-mismatch", "reference-head-not-exact",
    "reference-status-mutated", "missing-source-paths", "missing-license-provenance", "mutation-target-mismatch",
  ]) if (!policy?.stopConditions?.includes(condition)) errors.push(`missing stop condition ${condition}`);
  return errors;
}

export function observeReference({ repository, checkoutPath, policy = loadExtractionPolicy() }) {
  const reference = policy.referenceRepositories.find((entry) => entry.repository === repository);
  if (!reference) throw new Error(`unsupported reference repository: ${repository}`);
  const remote = command("git", ["remote", "get-url", "origin"], { cwd: checkoutPath });
  if (remote !== reference.allowedRemote) throw new Error(`${repository}: origin remote mismatch: ${remote}`);
  const headSha = command("git", ["rev-parse", reference.defaultReference], { cwd: checkoutPath });
  if (!SHA.test(headSha)) throw new Error(`${repository}: ${reference.defaultReference} did not resolve to an exact SHA`);
  const status = command("git", ["status", "--short"], { cwd: checkoutPath });
  return {
    repository,
    remote,
    reference: reference.defaultReference,
    headSha,
    clean: status.length === 0,
    status,
  };
}

export function compareReferenceObservations(before, after) {
  const errors = [];
  for (const field of ["repository", "remote", "reference", "headSha", "clean", "status"]) {
    if (before?.[field] !== after?.[field]) errors.push(`reference ${field} changed`);
  }
  return errors;
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index === -1 ? undefined : argv[index + 1];
}

export function run(argv = process.argv.slice(2), { stdout = process.stdout, stderr = process.stderr } = {}) {
  try {
    const policy = loadExtractionPolicy();
    const errors = validateExtractionPolicy(policy);
    if (errors.length > 0) throw new Error(errors.join("; "));
    if (argv[0] === "observe") {
      const repository = option(argv, "--repository");
      const checkoutPath = option(argv, "--path");
      if (!repository || !checkoutPath) throw new Error("observe requires --repository and --path");
      stdout.write(`${JSON.stringify(observeReference({ repository, checkoutPath, policy }), null, 2)}\n`);
    } else if (argv.length === 0 || argv[0] === "check") {
      stdout.write("reference repository policy: ok\n");
    } else {
      throw new Error(`unknown command: ${argv[0]}`);
    }
    return 0;
  } catch (error) {
    stderr.write(`reference repository policy: ${error.message}\n`);
    return 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = run();
}

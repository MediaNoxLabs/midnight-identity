#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

export const REPOSITORY = "MediaNoxLabs/midnight-identity";
const SAFE_ID = /^[a-z0-9][a-z0-9._:-]{0,63}$/u;
const SAFE_MODEL = /^[A-Za-z0-9][A-Za-z0-9._/-]{0,127}$/u;
const SAFE_REASONING = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/u;
const SHA = /^[0-9a-f]{40}$/u;
const FAILURE_CLASSES = new Set([
  "none", "implementation", "review", "ci", "environment", "provider",
  "transport", "authority", "canceled",
]);
const OUTCOMES = new Set(["passed", "failed", "canceled", "skipped"]);
const TOP_KEYS = Object.freeze([
  "schemaVersion", "repository", "issue", "pr", "headSha",
  "deliveryProfile", "model", "reasoningProfile", "startedAt",
  "completedAt", "durations", "tokens", "resources", "validations", "failure",
]);
const SECRET = /(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|\bBearer\s+\S+)/iu;

function exactKeys(value, keys, at, errors) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    errors.push(`${at} must be an object`);
    return false;
  }
  const actual = Object.keys(value);
  for (const key of keys) if (!actual.includes(key)) errors.push(`${at}.${key} is required`);
  for (const key of actual) if (!keys.includes(key)) errors.push(`${at}.${key} is not allowed`);
  return true;
}

function nonNegative(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function nullableNonNegative(value) {
  return value === null || nonNegative(value);
}

function timestamp(value) {
  if (typeof value !== "string") return null;
  const milliseconds = Date.parse(value);
  return Number.isFinite(milliseconds) && new Date(milliseconds).toISOString() === value
    ? milliseconds
    : null;
}

function scanSecrets(value, errors, depth = 0) {
  if (depth > 8) {
    errors.push("record nesting exceeds the privacy bound");
    return;
  }
  if (typeof value === "string" && SECRET.test(value)) errors.push("record contains a secret-like value");
  else if (Array.isArray(value)) value.forEach((entry) => scanSecrets(entry, errors, depth + 1));
  else if (value && typeof value === "object") Object.values(value).forEach((entry) => scanSecrets(entry, errors, depth + 1));
}

export function validateRecord(record) {
  const errors = [];
  if (!exactKeys(record, TOP_KEYS, "$", errors)) return { ok: false, errors };
  if (record.schemaVersion !== 1) errors.push("$.schemaVersion must equal 1");
  if (record.repository !== REPOSITORY) errors.push(`$.repository must equal ${REPOSITORY}`);
  if (!Number.isSafeInteger(record.issue) || record.issue < 1) errors.push("$.issue must be a positive integer");
  if (record.pr !== null && (!Number.isSafeInteger(record.pr) || record.pr < 1)) errors.push("$.pr must be null or a positive integer");
  if (typeof record.headSha !== "string" || !SHA.test(record.headSha)) errors.push("$.headSha must be an exact lowercase SHA");
  if (!["prototype", "production-ready"].includes(record.deliveryProfile)) errors.push("$.deliveryProfile is invalid");
  if (typeof record.model !== "string" || !SAFE_MODEL.test(record.model)) errors.push("$.model is invalid");
  if (typeof record.reasoningProfile !== "string" || !SAFE_REASONING.test(record.reasoningProfile)) errors.push("$.reasoningProfile is invalid");
  const started = timestamp(record.startedAt);
  const completed = timestamp(record.completedAt);
  if (started === null) errors.push("$.startedAt must be a canonical UTC timestamp");
  if (completed === null) errors.push("$.completedAt must be a canonical UTC timestamp");
  if (started !== null && completed !== null && completed < started) errors.push("$.completedAt precedes $.startedAt");

  const durationKeys = ["elapsedMs", "implementationMs", "reviewMs", "ciMs", "retryMs"];
  if (exactKeys(record.durations, durationKeys, "$.durations", errors)) {
    if (!nonNegative(record.durations.elapsedMs)) errors.push("$.durations.elapsedMs must be non-negative");
    for (const key of durationKeys.slice(1)) {
      if (!nullableNonNegative(record.durations[key])) errors.push(`$.durations.${key} must be null or non-negative`);
      if (nonNegative(record.durations.elapsedMs) && nonNegative(record.durations[key])
          && record.durations[key] > record.durations.elapsedMs) errors.push(`$.durations.${key} exceeds elapsedMs`);
    }
    if (started !== null && completed !== null && nonNegative(record.durations.elapsedMs)
        && completed - started !== record.durations.elapsedMs) errors.push("$.durations.elapsedMs does not match timestamps");
  }

  if (record.tokens !== null && exactKeys(record.tokens, ["input", "output", "cacheRead", "cacheWrite"], "$.tokens", errors)) {
    for (const [key, value] of Object.entries(record.tokens)) if (!nonNegative(value)) errors.push(`$.tokens.${key} must be non-negative`);
  }
  if (record.tokens !== null && (!record.tokens || typeof record.tokens !== "object" || Array.isArray(record.tokens))) {
    errors.push("$.tokens must be null or an object");
  }

  const resourceKeys = ["diskDeltaBytes", "peakWorktreeBytes", "peakTargetBytes", "peakRssBytes"];
  if (exactKeys(record.resources, resourceKeys, "$.resources", errors)) {
    if (record.resources.diskDeltaBytes !== null && !Number.isSafeInteger(record.resources.diskDeltaBytes)) errors.push("$.resources.diskDeltaBytes must be null or an integer");
    for (const key of resourceKeys.slice(1)) if (!nullableNonNegative(record.resources[key])) errors.push(`$.resources.${key} must be null or non-negative`);
    if (nonNegative(record.resources.peakTargetBytes) && nonNegative(record.resources.peakWorktreeBytes)
        && record.resources.peakTargetBytes > record.resources.peakWorktreeBytes) errors.push("$.resources.peakTargetBytes exceeds peakWorktreeBytes");
  }

  if (!Array.isArray(record.validations) || record.validations.length < 1 || record.validations.length > 64) {
    errors.push("$.validations must contain 1..64 entries");
  } else {
    const names = new Set();
    record.validations.forEach((entry, index) => {
      const at = `$.validations[${index}]`;
      if (!exactKeys(entry, ["name", "kind", "outcome", "durationMs"], at, errors)) return;
      if (typeof entry.name !== "string" || !SAFE_ID.test(entry.name)) errors.push(`${at}.name is invalid`);
      else if (names.has(entry.name)) errors.push(`${at}.name is duplicated`);
      else names.add(entry.name);
      if (!["local", "ci"].includes(entry.kind)) errors.push(`${at}.kind is invalid`);
      if (!OUTCOMES.has(entry.outcome)) errors.push(`${at}.outcome is invalid`);
      if (!nullableNonNegative(entry.durationMs)) errors.push(`${at}.durationMs must be null or non-negative`);
    });
  }

  if (exactKeys(record.failure, ["classification", "retryCount"], "$.failure", errors)) {
    if (!FAILURE_CLASSES.has(record.failure.classification)) errors.push("$.failure.classification is invalid");
    if (!nonNegative(record.failure.retryCount) || record.failure.retryCount > 100) errors.push("$.failure.retryCount is invalid");
  }
  scanSecrets(record, errors);
  return { ok: errors.length === 0, errors };
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

export function template(argv) {
  const issue = positive(option(argv, "--issue"), "--issue");
  const prValue = option(argv, "--pr");
  const headSha = option(argv, "--head");
  const model = option(argv, "--model");
  const reasoningProfile = option(argv, "--reasoning");
  if (!headSha || !SHA.test(headSha)) throw new Error("--head must be an exact lowercase SHA");
  if (!model || !SAFE_MODEL.test(model)) throw new Error("--model must be the bounded harness-reported model identifier");
  if (!reasoningProfile || !SAFE_REASONING.test(reasoningProfile)) throw new Error("--reasoning must be the bounded harness-reported profile");
  return {
    schemaVersion: 1,
    repository: REPOSITORY,
    issue,
    pr: prValue === undefined ? null : positive(prValue, "--pr"),
    headSha,
    deliveryProfile: option(argv, "--delivery-profile") ?? "production-ready",
    model,
    reasoningProfile,
    startedAt: null,
    completedAt: null,
    durations: { elapsedMs: null, implementationMs: null, reviewMs: null, ciMs: null, retryMs: null },
    tokens: null,
    resources: { diskDeltaBytes: null, peakWorktreeBytes: null, peakTargetBytes: null, peakRssBytes: null },
    validations: [],
    failure: { classification: "none", retryCount: 0 },
  };
}

function formatBytes(value) {
  if (value === null) return "unavailable";
  const sign = value < 0 ? "-" : "";
  const absolute = Math.abs(value);
  if (absolute < 1024) return `${value} B`;
  return `${sign}${(absolute / (1024 ** 2)).toFixed(1)} MiB`;
}

function formatDuration(value) {
  return value === null ? "unavailable" : `${(value / 1000).toFixed(1)} s`;
}

export function formatRecord(record) {
  const check = validateRecord(record);
  if (!check.ok) throw new Error(check.errors.join("; "));
  const validations = record.validations.map((entry) => `${entry.name}:${entry.outcome}`).join(", ");
  const lines = [
    `### Factory run #${record.issue}${record.pr ? ` / PR #${record.pr}` : ""}`,
    "",
    `Exact head \`${record.headSha}\`; profile \`${record.deliveryProfile}\`; model \`${record.model}\` / \`${record.reasoningProfile}\`.`,
    "",
    "| Measure | Result |",
    "| --- | --- |",
    `| Elapsed | ${formatDuration(record.durations.elapsedMs)} |`,
    `| Implementation / review / CI / retry | ${["implementationMs", "reviewMs", "ciMs", "retryMs"].map((key) => formatDuration(record.durations[key])).join(" / ")} |`,
    `| Disk delta / peak worktree / peak target / peak RSS | ${formatBytes(record.resources.diskDeltaBytes)} / ${formatBytes(record.resources.peakWorktreeBytes)} / ${formatBytes(record.resources.peakTargetBytes)} / ${formatBytes(record.resources.peakRssBytes)} |`,
    `| Checks | ${validations} |`,
    `| Failure / retries | ${record.failure.classification} / ${record.failure.retryCount} |`,
    "",
    `<!-- factory-metrics:v1 ${JSON.stringify(record)} -->`,
  ];
  return `${lines.join("\n")}\n`;
}

export function run(argv = process.argv.slice(2), { stdout = process.stdout, stderr = process.stderr } = {}) {
  const command = argv[0];
  try {
    if (command === "template") {
      stdout.write(`${JSON.stringify(template(argv.slice(1)), null, 2)}\n`);
      return 0;
    }
    const file = option(argv, "--file");
    if (!file) throw new Error("--file is required");
    const record = JSON.parse(readFileSync(file, "utf8"));
    const validation = validateRecord(record);
    if (command === "validate") {
      if (!validation.ok) throw new Error(validation.errors.join("; "));
      stdout.write("factory metrics: valid\n");
      return 0;
    }
    if (command === "format") {
      stdout.write(formatRecord(record));
      return 0;
    }
    throw new Error("usage: metrics.mjs template ... | validate --file PATH | format --file PATH");
  } catch (error) {
    stderr.write(`factory metrics: ${error.message}\n`);
    return 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = run();
}

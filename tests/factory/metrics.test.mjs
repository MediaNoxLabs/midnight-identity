// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { formatRecord, validateRecord } from "../../scripts/factory/metrics.mjs";

function record() {
  return {
    schemaVersion: 1,
    repository: "MediaNoxLabs/midnight-identity",
    issue: 56,
    pr: 57,
    headSha: "a".repeat(40),
    deliveryProfile: "production-ready",
    model: "openai-codex/gpt-example",
    reasoningProfile: "high",
    startedAt: "2026-09-15T03:00:00.000Z",
    completedAt: "2026-09-15T03:10:00.000Z",
    durations: { elapsedMs: 600000, implementationMs: 300000, reviewMs: 60000, ciMs: 180000, retryMs: 0 },
    tokens: null,
    resources: { diskDeltaBytes: 1048576, peakWorktreeBytes: 2097152, peakTargetBytes: 0, peakRssBytes: null },
    validations: [
      { name: "policy", kind: "local", outcome: "passed", durationMs: 1000 },
      { name: "required-ci", kind: "ci", outcome: "passed", durationMs: 180000 },
    ],
    failure: { classification: "none", retryCount: 0 },
  };
}

test("closed v1 records validate and format one hidden canonical payload", () => {
  const candidate = record();
  assert.deepEqual(validateRecord(candidate), { ok: true, errors: [] });
  const markdown = formatRecord(candidate);
  assert.match(markdown, /Exact head `aaaaaaaa/u);
  assert.equal((markdown.match(/<!-- factory-metrics:v1 /gu) ?? []).length, 1);
});

test("unknown fields, inconsistent durations, duplicate checks, and secrets fail closed", () => {
  const unknown = { ...record(), extra: true };
  assert.equal(validateRecord(unknown).ok, false);
  const duration = record();
  duration.durations.elapsedMs = 1;
  assert.equal(validateRecord(duration).ok, false);
  const duplicate = record();
  duplicate.validations.push({ ...duplicate.validations[0] });
  assert.equal(validateRecord(duplicate).ok, false);
  const secret = record();
  secret.model = `ghp_${"x".repeat(30)}`;
  assert.equal(validateRecord(secret).ok, false);
});

test("the committed JSON Schema stays closed and names every metrics field", async () => {
  const schema = JSON.parse(await readFile(new URL("../../doc/factory/work-item-metrics-v1.schema.json", import.meta.url), "utf8"));
  assert.equal(schema.additionalProperties, false);
  for (const key of ["model", "reasoningProfile", "durations", "tokens", "resources", "validations", "failure"]) {
    assert.ok(schema.required.includes(key), key);
    assert.ok(schema.properties[key], key);
  }
});

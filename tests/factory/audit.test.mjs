// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import { parsePackageSpec, summarizeRemote } from "../../scripts/factory/audit.mjs";

test("Pi packages must use exact npm versions and disabled surfaces stay visible", () => {
  assert.deepEqual(parsePackageSpec("npm:dev-loops@0.9.0"), {
    name: "dev-loops", version: "0.9.0", exact: true, disabled: false,
  });
  const disabled = parsePackageSpec({
    source: "npm:pi-taskflow@0.2.10", extensions: [], skills: [], prompts: [], themes: [],
  });
  assert.equal(disabled.exact, true);
  assert.equal(disabled.disabled, true);
  assert.equal(parsePackageSpec("npm:dev-loops@latest").exact, false);
});

test("remote summaries separate CI duration, retry causes, cache bytes, and throughput", () => {
  const now = Date.parse("2026-09-15T00:00:00.000Z");
  const report = summarizeRemote({
    now,
    runs: [
      { created_at: "2026-09-14T23:50:00Z", updated_at: "2026-09-15T00:00:00Z", conclusion: "success", run_attempt: 1 },
      {
        created_at: "2026-09-14T23:30:00Z", updated_at: "2026-09-14T23:50:00Z",
        conclusion: "success", run_attempt: 2, retry_causes: ["Policy and factory contracts:failure"],
      },
    ],
    caches: [
      { key: "linux-cargo-a", size_in_bytes: 100, last_accessed_at: "2026-09-14T00:00:00Z" },
      { key: "mac-cargo-a", size_in_bytes: 200, last_accessed_at: "2026-07-01T00:00:00Z" },
    ],
    pulls: [{ merged_at: "2026-09-10T00:00:00Z" }, { merged_at: "2026-07-01T00:00:00Z" }],
  });
  assert.equal(report.ci.medianMs, 600000);
  assert.equal(report.ci.p90Ms, 1200000);
  assert.equal(report.ci.retries, 1);
  assert.equal(report.ci.retryCauses["Policy and factory contracts:failure"], 1);
  assert.equal(report.cache.bytes, 300);
  assert.equal(report.cache.accessedLast7Days, 1);
  assert.equal(report.cache.staleOver30Days, 1);
  assert.equal(report.delivery.mergedPullRequestsLast30Days, 1);
});

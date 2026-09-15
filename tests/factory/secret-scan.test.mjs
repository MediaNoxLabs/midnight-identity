// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import { scanAddedLines } from "../../scripts/factory/secret-scan.mjs";

test("the diff scanner ignores context/removals and rejects added credentials", () => {
  const secret = `ghp_${"a".repeat(32)}`;
  const diff = ["--- a/file", "+++ b/file", `-${secret}`, "+safe placeholder", `+token=${secret}`].join("\n");
  const findings = scanAddedLines(diff);
  assert.deepEqual(findings.map(({ rule }) => rule), ["github-token", "assigned-secret"]);
});

test("GitHub expression references and redacted examples are not credentials", () => {
  const diff = "+GH_TOKEN: ${{ github.token }}\n+token=REDACTED\n+api_key=example";
  assert.deepEqual(scanAddedLines(diff), []);
});

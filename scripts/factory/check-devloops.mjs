#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";

const executable = process.platform === "win32" ? "npx.cmd" : "npx";
const output = execFileSync(executable, ["--yes", "dev-loops@0.9.0", "gates"], {
  cwd: process.cwd(),
  encoding: "utf8",
  maxBuffer: 4 * 1024 * 1024,
  timeout: 120_000,
});
const [draft = "", preApproval = ""] = output.split(/^pre-approval gate:\s*$/mu);
const angleNames = (section) => [...section.matchAll(/^  ([a-z][a-z0-9-]*)\s{2,}/gmu)]
  .map((match) => match[1]);
const expected = [
  [draft, /^draft gate:\s*$/mu, "draft gate"],
  [draft, /^  requireCi: false\s*$/mu, "draft CI policy"],
  [draft, /^  correctness\s+/mu, "draft correctness angle"],
  [preApproval, /^  requireCi: true \(always enforced\)\s*$/mu, "pre-approval CI policy"],
  [preApproval, /^  security\s+/mu, "pre-approval security angle"],
];
const missing = expected.filter(([source, pattern]) => !pattern.test(source)).map(([, , label]) => label);
if (JSON.stringify(angleNames(draft)) !== JSON.stringify(["correctness"])) missing.push("exact draft angles");
if (JSON.stringify(angleNames(preApproval)) !== JSON.stringify(["security"])) missing.push("exact pre-approval angles");
if (missing.length > 0) {
  process.stderr.write(`dev-loops configuration did not resolve the expected repository gates: ${missing.join(", ")}\n`);
  process.exit(1);
}
process.stdout.write("dev-loops configuration: pinned package resolved repository gates\n");

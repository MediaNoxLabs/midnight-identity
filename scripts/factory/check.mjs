#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

run(process.execPath, ["--test", "tests/factory/*.test.mjs"]);
run(process.execPath, ["scripts/factory/check-devloops.mjs"]);
run(process.execPath, ["scripts/factory/audit.mjs", "--format", "summary"]);

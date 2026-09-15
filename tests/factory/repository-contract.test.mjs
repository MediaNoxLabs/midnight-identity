// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { access, readFile, readdir } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

test("Pi settings and delivery profiles enforce bounded exact-pinned operation", async () => {
  const [settings, profiles, subagents] = await Promise.all([
    readFile(path.join(root, ".pi", "settings.json"), "utf8").then(JSON.parse),
    readFile(path.join(root, ".pi", "delivery-profiles.json"), "utf8").then(JSON.parse),
    readFile(path.join(root, ".pi", "subagent-policy.json"), "utf8").then(JSON.parse),
  ]);
  assert.deepEqual(settings.packages, [
    "npm:dev-loops@0.9.0", "npm:pi-subagents@0.42.1", "npm:typebox@1.3.9",
  ]);
  assert.equal(settings.retry.maxRetries, 1);
  assert.equal(settings.retry.provider.maxRetries, 0);
  assert.equal(profiles.defaultProfile, "production-ready");
  assert.equal(profiles.profiles.prototype.remoteMutation, false);
  assert.equal(profiles.profiles.prototype.targets.hostedCi, false);
  assert.equal(profiles.profiles["production-ready"].qualityBudget.targetPercent, 70);
  assert.equal(profiles.profiles["production-ready"].qualityBudget.mandatoryInvariantsPercent, 100);
  assert.equal(subagents.maxSubagentDepth, 1);
  assert.equal(subagents.parallel.maxTasks, 1);
});

test("supervisor owns delegation and the worker cannot create nested agents", async () => {
  const [supervisor, worker] = await Promise.all([
    readFile(path.join(root, ".pi", "agents", "factory-supervisor.agent.md"), "utf8"),
    readFile(path.join(root, ".pi", "agents", "factory-worker.agent.md"), "utf8"),
  ]);
  assert.match(supervisor, /tools: .*subagent/u);
  assert.match(supervisor, /exactly one `factory-worker`/u);
  assert.doesNotMatch(worker.split("---", 3)[1], /\bsubagent\b/u);
  assert.match(worker, /maxSubagentDepth: 0/u);
});

test("the constitution stays concise and protects repository boundaries", async () => {
  const source = await readFile(path.join(root, "AGENT.md"), "utf8");
  assert.ok(source.split(/\s+/u).length < 2000);
  for (const contract of [
    "reusable Midnight-specific libraries", "Chain-neutral protocols", "one isolated worktree per issue",
    "prototype", "production-ready", "70%", "GitHub-verifiable OpenPGP", "human",
  ]) assert.ok(source.includes(contract), contract);
});

test("factory Markdown relative links resolve", async () => {
  const factoryDir = path.join(root, "doc", "factory");
  const files = [
    path.join(root, "AGENT.md"), path.join(root, "README.md"), path.join(root, "CONTRIBUTING.md"),
    ...(await readdir(factoryDir)).filter((name) => name.endsWith(".md")).map((name) => path.join(factoryDir, name)),
  ];
  for (const file of files) {
    const source = await readFile(file, "utf8");
    for (const match of source.matchAll(/\[[^\]]+\]\(([^)]+)\)/gu)) {
      const target = match[1].split("#", 1)[0];
      if (!target || /^(?:https?:|mailto:|file:)/u.test(target)) continue;
      await assert.doesNotReject(access(path.resolve(path.dirname(file), decodeURIComponent(target))), `${file}: ${target}`);
    }
  }
});

test("workflow pins actions, uses a stable aggregator, and never caches target", async () => {
  const workflow = await readFile(path.join(root, ".github", "workflows", "ci.yml"), "utf8");
  for (const match of workflow.matchAll(/^\s*uses:\s*[^@\s]+@([^\s#]+)/gmu)) {
    assert.match(match[1], /^[0-9a-f]{40}$/u, match[0]);
  }
  assert.match(workflow, /name: Required CI/u);
  assert.match(workflow, /base\.ref == 'rust-codegen'/u);
  assert.match(workflow, /actionlint\/cmd\/actionlint@v1\.7\.7/u);
  assert.match(workflow, /actions\/cache\/restore@/u);
  assert.match(workflow, /actions\/cache\/save@/u);
  assert.doesNotMatch(workflow, /flakehub|magic[- ]nix[- ]cache/iu);
  assert.doesNotMatch(workflow, /^\s+target\/?\s*$/mu);
});

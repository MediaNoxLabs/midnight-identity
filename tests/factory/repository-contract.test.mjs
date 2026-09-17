// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { access, readFile, readdir } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { validateExtractionPolicy } from "../../scripts/factory/reference-repositories.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

test("Pi settings and delivery profiles enforce bounded exact-pinned operation", async () => {
  const [settings, profiles, subagents] = await Promise.all([
    readFile(path.join(root, ".pi", "settings.json"), "utf8").then(JSON.parse),
    readFile(path.join(root, ".pi", "delivery-profiles.json"), "utf8").then(JSON.parse),
    readFile(path.join(root, ".pi", "subagent-policy.json"), "utf8").then(JSON.parse),
  ]);
  assert.equal(settings.defaultProvider, "openai-codex");
  assert.equal(settings.defaultModel, "gpt-5.5");
  assert.equal(settings.defaultThinkingLevel, "medium");
  assert.deepEqual(settings.packages, [
    { source: "npm:dev-loops@1.0.2", extensions: [] }, "npm:pi-subagents@0.34.0", "npm:typebox@1.3.9",
  ]);
  assert.equal(settings.subagents.defaultModel, "openai-codex/gpt-5.5");
  assert.equal(settings.subagents.defaultThinking, "medium");
  assert.equal(settings.retry.maxRetries, 1);
  assert.equal(settings.retry.provider.maxRetries, 0);
  assert.equal(profiles.defaultProfile, "production-ready");
  assert.equal(profiles.profiles.prototype.remoteMutation, false);
  assert.equal(profiles.profiles.prototype.targets.hostedCi, false);
  assert.equal(profiles.profiles["production-ready"].qualityBudget.targetPercent, 70);
  assert.equal(profiles.profiles["production-ready"].qualityBudget.mandatoryInvariantsPercent, 100);
  assert.equal(profiles.reviewTriage.machineReadable, true);
  assert.deepEqual(profiles.reviewTriage.ignoreKinds, ["status-summary", "duplicate", "stale-automated-noise"]);
  assert.deepEqual(profiles.reviewTriage.actionableDisposition, ["same-pr-fix", "follow-up-issue"]);
  assert.deepEqual(profiles.reviewTriage.samePrRequiredFor, [
    "acceptance", "correctness", "security", "provenance", "required-test", "ci",
  ]);
  assert.equal(profiles.reviewTriage.followUpAllowedFor, "bounded-nonblocking-polish-only");
  assert.equal(profiles.reviewTriage.readinessBlockedByUnresolvedThreads, true);
  assert.equal(subagents.maxSubagentDepth, 1);
  assert.equal(subagents.maxSubagentSpawnsPerSession, 1);
  assert.equal(subagents.maxSubagentSpawnsPerRun, 1);
  assert.equal(subagents.globalConcurrencyLimit, 1);
  assert.equal(subagents.toolBudget.soft, 40);
  assert.equal(subagents.toolBudget.hard, 60);
  assert.equal(subagents.toolBudget.block, "*");
  assert.equal(subagents.usageBudget.tokens.soft, 80000);
  assert.equal(subagents.usageBudget.tokens.hard, 120000);
  assert.equal(subagents.parallel.maxTasks, 1);
});

test("extraction policy is machine-readable and fails closed around consumer provenance", async () => {
  const policy = await readFile(path.join(root, ".pi", "extraction-policy.json"), "utf8").then(JSON.parse);
  assert.deepEqual(validateExtractionPolicy(policy), []);
  assert.equal(policy.mutationTarget.repository, "MediaNoxLabs/midnight-identity");
  assert.deepEqual(policy.referenceRepositories.map(({ repository }) => repository), [
    "MediaNoxLabs/oxid", "input-output-hk/lace-id-portal",
  ]);
  const portal = policy.referenceRepositories.find(({ repository }) => repository === "input-output-hk/lace-id-portal");
  assert.equal(portal.allowedRemote, "https://github.com/input-output-hk/lace-id-portal.git");
  assert.equal(portal.remoteSymbolicHead, "refs/heads/develop");
  assert.equal(portal.defaultReference, "origin/develop");
  for (const reference of policy.referenceRepositories) {
    assert.equal(reference.access, "read-only");
    assert.equal(reference.defaultReference, "origin/develop");
    for (const field of [
      "remote", "exactSourceSha", "cleanStateBefore", "cleanStateAfter", "sourcePaths",
      "licenseProvenance", "beforeStatus", "afterStatus",
    ]) assert.ok(reference.requiredEvidence.includes(field), `${reference.repository}: ${field}`);
  }
  assert.ok(validateExtractionPolicy({ ...policy, mutationTarget: { repository: "MediaNoxLabs/oxid" } }).length > 0);
  assert.ok(validateExtractionPolicy({
    ...policy,
    referenceRepositories: policy.referenceRepositories.map((entry, index) => index === 0 ? { ...entry, access: "write" } : entry),
  }).length > 0);
  assert.ok(validateExtractionPolicy({
    ...policy,
    referenceRepositories: policy.referenceRepositories.map((entry, index) => index === 0
      ? { ...entry, requiredEvidence: entry.requiredEvidence.filter((field) => field !== "licenseProvenance") }
      : entry),
  }).length > 0);
});

test("Pi package pin rationale is based on package-source ABI evidence", async () => {
  const documentation = await readFile(path.join(root, "doc", "pi-development.md"), "utf8");
  assert.match(documentation, /pi-subagents@0\.34\.0/u);
  assert.match(documentation, /pi-subagents@0\.35\.0/u);
  assert.match(documentation, /@earendil-works\/pi-ai\/compat/u);
  assert.match(documentation, /Pi 0\.75\.4 does not provide/u);
  assert.match(documentation, /tracked-extension startup smoke before final approval/u);
  assert.doesNotMatch(documentation, /runtime compatible because .*bootstrap\.sh --pi --version/iu);
});

test("supervisor owns delegation and the worker cannot create nested agents", async () => {
  const [supervisor, worker] = await Promise.all([
    readFile(path.join(root, ".pi", "agents", "factory-supervisor.agent.md"), "utf8"),
    readFile(path.join(root, ".pi", "agents", "factory-worker.agent.md"), "utf8"),
  ]);
  assert.match(supervisor, /tools: .*subagent/u);
  assert.match(supervisor, /exactly one `factory-worker`/u);
  assert.match(supervisor, /filter status,\s+summary, duplicate, and stale automated noise/u);
  assert.match(supervisor, /Acceptance, correctness,\s+security, provenance, required-test, and CI findings are same-PR fixes/u);
  assert.match(supervisor, /Never\s+claim readiness while any review thread remains unresolved/u);
  assert.doesNotMatch(worker.split("---", 3)[1], /\bsubagent\b/u);
  assert.match(worker, /maxSubagentDepth: 0/u);
});

test("the constitution stays concise and protects repository boundaries", async () => {
  const source = await readFile(path.join(root, "AGENT.md"), "utf8");
  assert.ok(source.split(/\s+/u).length < 2000);
  for (const contract of [
    "reusable Midnight-specific libraries", "Chain-neutral protocols", "one isolated worktree per issue",
    "prototype", "production-ready", "70%", "GitHub-verifiable OpenPGP", "human",
    "Review triage filters", "Never claim readiness with unresolved review threads",
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
  await access(path.join(root, "scripts", "ci", "rust-target.sh"));
  for (const match of workflow.matchAll(/^\s*uses:\s*[^@\s]+@([^\s#]+)/gmu)) {
    assert.match(match[1], /^[0-9a-f]{40}$/u, match[0]);
  }
  assert.match(workflow, /name: Required CI/u);
  assert.match(workflow, /types: \[opened, synchronize, reopened, edited\]/u);
  assert.match(workflow, /base\.ref == 'rust-codegen'/u);
  assert.match(workflow, /actionlint\/cmd\/actionlint@v1\.7\.7/u);
  assert.match(workflow, /--allow-generated-squash/u);
  assert.match(
    await readFile(path.join(root, "scripts", "factory", "local-gate.mjs"), "utf8"),
    /--github-generated-signatures/u,
  );
  assert.match(workflow, /find \.github\/workflows/u);
  assert.doesNotMatch(workflow, /actionlint@v1\.7\.7 \.github\/workflows\/ci\.yml/u);
  assert.match(workflow, /actions\/cache\/restore@/u);
  assert.match(workflow, /actions\/cache\/save@/u);
  assert.doesNotMatch(workflow, /flakehub|magic[- ]nix[- ]cache/iu);
  assert.doesNotMatch(workflow, /^\s+target\/?\s*$/mu);
});

test("pull-request template records exact delivery target and stack disposition", async () => {
  const template = await readFile(path.join(root, ".github", "pull_request_template.md"), "utf8");
  assert.equal([...template.matchAll(/factory-delivery-target:/gu)].length, 1);
  assert.equal([...template.matchAll(/factory-stacked-parent:/gu)].length, 1);
  assert.match(template, /No review thread remains unresolved before readiness is claimed/u);
});

test("release verification and publishing include every publishable library crate", async () => {
  const workflow = await readFile(path.join(root, ".github", "workflows", "release.yml"), "utf8");
  assert.match(
    workflow,
    /PUBLISH_CRATES: "midnight-did-domain midnight-did-method midnight-passport-account-source midnight-passport-vault-source"/u,
  );
  assert.match(workflow, /patch\.crates-io\.midnight-did-domain\.path/u);
  assert.match(workflow, /midnight-did-domain@\$\{version\}/u);
});

#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { loadExtractionPolicy, validateExtractionPolicy } from "./reference-repositories.mjs";

const REPOSITORY = "MediaNoxLabs/midnight-identity";
const EXACT_VERSION = /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/u;

function command(program, args, options = {}) {
  return execFileSync(program, args, {
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
    timeout: 30_000,
    ...options,
  }).trim();
}

export function parsePackageSpec(entry) {
  const source = typeof entry === "string" ? entry : entry?.source;
  const match = typeof source === "string" ? /^npm:(.+)@([^@]+)$/u.exec(source) : null;
  return match ? {
    name: match[1],
    version: match[2],
    exact: EXACT_VERSION.test(match[2]),
    disabled: typeof entry === "object" && ["extensions", "skills", "prompts", "themes"]
      .every((key) => Array.isArray(entry[key]) && entry[key].length === 0),
  } : { name: source ?? "invalid", version: null, exact: false, disabled: false };
}

function percentile(values, quantile) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.max(0, Math.ceil(sorted.length * quantile) - 1)];
}

export function summarizeRemote({ runs = [], caches = [], pulls = [], now = Date.now() }) {
  const durations = runs
    .map((run) => Date.parse(run.updated_at) - Date.parse(run.created_at))
    .filter((value) => Number.isFinite(value) && value >= 0);
  const conclusions = {};
  for (const run of runs) conclusions[run.conclusion ?? "in-progress"] = (conclusions[run.conclusion ?? "in-progress"] ?? 0) + 1;
  const retryRuns = runs.filter((run) => Number(run.run_attempt) > 1);
  const retryCauses = {};
  for (const run of retryRuns) {
    const causes = Array.isArray(run.retry_causes) && run.retry_causes.length > 0
      ? run.retry_causes
      : ["unavailable"];
    for (const cause of causes) retryCauses[cause] = (retryCauses[cause] ?? 0) + 1;
  }
  const since = now - 30 * 24 * 60 * 60_000;
  const recentCacheThreshold = now - 7 * 24 * 60 * 60_000;
  const staleCacheThreshold = now - 30 * 24 * 60 * 60_000;
  const cacheAccess = caches.map((cache) => Date.parse(cache.last_accessed_at)).filter(Number.isFinite);
  const mergedLast30Days = pulls.filter((pr) => pr.merged_at && Date.parse(pr.merged_at) >= since).length;
  return {
    ci: {
      samples: durations.length,
      medianMs: percentile(durations, 0.5),
      p90Ms: percentile(durations, 0.9),
      conclusions,
      retries: retryRuns.length,
      retryCauses,
    },
    cache: {
      entries: caches.length,
      bytes: caches.reduce((sum, cache) => sum + (Number(cache.size_in_bytes) || 0), 0),
      keys: caches.map((cache) => cache.key).filter(Boolean).sort(),
      accessSamples: cacheAccess.length,
      accessedLast7Days: cacheAccess.filter((value) => value >= recentCacheThreshold).length,
      staleOver30Days: cacheAccess.filter((value) => value < staleCacheThreshold).length,
    },
    delivery: { mergedPullRequestsLast30Days: mergedLast30Days },
  };
}

function enrichRetryCauses(runs) {
  let observations = 0;
  return runs.map((run) => {
    if (Number(run.run_attempt) <= 1 || observations >= 10) return run;
    observations += 1;
    const causes = new Set();
    const firstAttempt = Math.max(1, Number(run.run_attempt) - 3);
    for (let attempt = firstAttempt; attempt < Number(run.run_attempt); attempt += 1) {
      try {
        const jobs = JSON.parse(command("gh", [
          "api", `repos/${REPOSITORY}/actions/runs/${run.id}/attempts/${attempt}/jobs?per_page=100`,
          "--jq", ".jobs",
        ]));
        for (const job of jobs) {
          if (["failure", "timed_out", "action_required", "cancelled"].includes(job.conclusion)) {
            causes.add(`${job.name}:${job.conclusion}`);
          }
        }
      } catch {
        causes.add("unavailable");
      }
    }
    return { ...run, retry_causes: [...causes].sort() };
  });
}

function localAudit(repoRoot) {
  const settings = JSON.parse(readFileSync(path.join(repoRoot, ".pi", "settings.json"), "utf8"));
  const packages = (settings.packages ?? []).map(parsePackageSpec);
  const extractionPolicy = loadExtractionPolicy(repoRoot);
  const workflow = readFileSync(path.join(repoRoot, ".github", "workflows", "ci.yml"), "utf8");
  const worktreeReport = JSON.parse(command("node", [
    path.join(repoRoot, "scripts", "factory", "worktree-lifecycle.mjs"), "audit", "--json",
  ], { cwd: repoRoot }));
  const findings = [];
  for (const packageEntry of packages) {
    if (!packageEntry.exact) findings.push({ severity: "error", code: "pi-package-not-exact", package: packageEntry.name });
    if (packageEntry.disabled) findings.push({ severity: "notice", code: "pi-package-disabled", package: packageEntry.name });
  }
  for (const error of validateExtractionPolicy(extractionPolicy)) {
    findings.push({ severity: "error", code: "extraction-policy-invalid", detail: error });
  }
  for (const [code, pattern] of [
    ["missing-stable-aggregator", /name:\s+Required CI/u],
    ["missing-scheduled-audit", /^\s+schedule:/mu],
    ["missing-read-only-cache", /actions\/cache\/restore@/u],
    ["missing-trusted-cache-writer", /actions\/cache\/save@/u],
  ]) if (!pattern.test(workflow)) findings.push({ severity: "error", code });
  if (/path:\s*[|>]?[\s\S]{0,160}(?:^|\n)\s*target\/?$/mu.test(workflow)) {
    findings.push({ severity: "error", code: "mutable-target-cache" });
  }
  const reviewCandidates = [];
  for (const tree of worktreeReport.worktrees) {
    if (tree.primary || tree.current) continue;
    const reasons = [];
    if (tree.dirty !== false) reasons.push(tree.dirty === true ? "dirty" : "cleanliness-unknown");
    if (tree.locked) reasons.push("locked");
    if (reasons.length === 0) reasons.push("ownership-or-merge-proof-required");
    reviewCandidates.push({ path: tree.path, preservationReasons: reasons });
  }
  return {
    packages,
    extractionPolicy: {
      references: extractionPolicy.referenceRepositories.map(({ repository, access }) => ({ repository, access })),
    },
    workflow: { stableAggregator: /name:\s+Required CI/u.test(workflow), scheduled: /^\s+schedule:/mu.test(workflow) },
    worktrees: {
      count: worktreeReport.worktrees.length,
      totalKiB: worktreeReport.worktrees.reduce((sum, tree) => sum + (tree.sizeKiB ?? 0), 0),
      totalTargetKiB: worktreeReport.worktrees.reduce((sum, tree) => sum + (tree.targetKiB ?? 0), 0),
      reviewCandidates,
    },
    findings,
  };
}

function onlineAudit(packages) {
  const packageVersions = packages.map((entry) => {
    try {
      const latest = command("npm", ["view", entry.name, "version", "--json"]);
      const parsed = JSON.parse(latest);
      const current = Array.isArray(parsed) ? parsed.at(-1) : parsed;
      return { ...entry, latest: current, stale: current !== entry.version };
    } catch (error) {
      return { ...entry, latest: null, stale: null, error: error.message.split("\n", 1)[0] };
    }
  });
  try {
    const runs = enrichRetryCauses(JSON.parse(command("gh", [
      "api", `repos/${REPOSITORY}/actions/workflows/ci.yml/runs?per_page=50`, "--jq", ".workflow_runs",
    ])));
    const caches = JSON.parse(command("gh", [
      "api", `repos/${REPOSITORY}/actions/caches?per_page=100`, "--jq", ".actions_caches",
    ]));
    const pulls = JSON.parse(command("gh", [
      "api", `repos/${REPOSITORY}/pulls?state=closed&per_page=100&sort=updated&direction=desc`,
    ]));
    return { available: true, packageVersions, ...summarizeRemote({ runs, caches, pulls }) };
  } catch (error) {
    return { available: false, packageVersions, error: error.message.split("\n", 1)[0] };
  }
}

function formatDuration(value) {
  return value === null || value === undefined ? "unavailable" : `${(value / 60_000).toFixed(1)} min`;
}

export function markdown(report) {
  const online = report.online;
  const stale = online?.packageVersions?.filter((entry) => entry.stale).map((entry) => `${entry.name} ${entry.version} -> ${entry.latest}`) ?? [];
  const unavailablePackages = online?.packageVersions?.filter((entry) => entry.latest === null)
    .map((entry) => entry.name) ?? [];
  const packageVersionSummary = !online
    ? "online audit disabled"
    : unavailablePackages.length > 0
      ? `unavailable (${unavailablePackages.join(", ")})`
      : stale.length > 0
        ? stale.join(", ")
        : "none observed";
  const lines = [
    "# Factory audit",
    "",
    `Generated ${report.generatedAt}; local findings: ${report.local.findings.length}.`,
    "",
    "| Signal | Value |",
    "| --- | --- |",
    `| Pi packages | ${report.local.packages.map((entry) => `${entry.name}@${entry.version}`).join(", ")} |`,
    `| Disabled Pi packages | ${report.local.packages.filter((entry) => entry.disabled).map((entry) => entry.name).join(", ") || "none observed"} |`,
    `| Stale Pi packages | ${packageVersionSummary} |`,
    `| Worktrees / target disk | ${report.local.worktrees.count} / ${report.local.worktrees.totalTargetKiB} KiB |`,
    `| Preservation candidates | ${report.local.worktrees.reviewCandidates.length} |`,
    `| CI median / p90 | ${online?.available ? `${formatDuration(online.ci.medianMs)} / ${formatDuration(online.ci.p90Ms)}` : "unavailable"} |`,
    `| CI retries / prior-attempt failed jobs | ${online?.available ? `${online.ci.retries} / ${JSON.stringify(online.ci.retryCauses)}` : "unavailable"} |`,
    `| Cache entries / bytes | ${online?.available ? `${online.cache.entries} / ${online.cache.bytes}` : "unavailable"} |`,
    `| Cache accessed <=7d / stale >30d | ${online?.available ? `${online.cache.accessedLast7Days} / ${online.cache.staleOver30Days} (${online.cache.accessSamples} observed)` : "unavailable"} |`,
    `| Merged PRs (30 days) | ${online?.available ? online.delivery.mergedPullRequestsLast30Days : "unavailable"} |`,
    "",
    "Audit is read-only. Review findings and preservation candidates manually; no package, cache, worktree, workflow, issue, or PR was mutated.",
  ];
  return `${lines.join("\n")}\n`;
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index === -1 ? undefined : argv[index + 1];
}

export function run(argv = process.argv.slice(2), {
  cwd = process.cwd(), stdout = process.stdout, stderr = process.stderr,
} = {}) {
  try {
    const repoRoot = command("git", ["rev-parse", "--show-toplevel"], { cwd });
    const local = localAudit(repoRoot);
    const report = {
      schemaVersion: 1,
      repository: REPOSITORY,
      generatedAt: new Date().toISOString(),
      local,
      online: argv.includes("--online") ? onlineAudit(local.packages) : null,
    };
    const format = option(argv, "--format") ?? "summary";
    if (format === "json") stdout.write(`${JSON.stringify(report, null, 2)}\n`);
    else if (format === "markdown") stdout.write(markdown(report));
    else if (format === "summary") stdout.write(`factory audit: ${local.findings.length} local finding(s), ${local.worktrees.count} worktree(s)\n`);
    else throw new Error(`unknown output format: ${format}`);
    return local.findings.some(({ severity }) => severity === "error") ? 1 : 0;
  } catch (error) {
    stderr.write(`factory audit: ${error.message}\n`);
    return 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = run();
}

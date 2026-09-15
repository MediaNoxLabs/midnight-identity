#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

export const TYPES = Object.freeze(["feat", "fix", "docs", "refactor", "test", "ci", "chore"]);
export const SCOPES = Object.freeze([
  "factory", "ci", "docs", "did", "vc", "compact", "runtime", "domain",
  "method", "api", "resolver", "indexer", "cli", "ffi", "deps", "release",
]);

const BRANCH = new RegExp(`^(?:${TYPES.join("|")})/issue-[1-9]\\d*$`, "u");
const SUBJECT = new RegExp(
  `^(?:${TYPES.join("|")})\\((?:${SCOPES.join("|")})\\)(?:!)?: [a-z0-9][^\\n]{0,71}$`,
  "u",
);
const DELIVERY_TARGET = /^(?:develop|rust-codegen)$/u;
const DELIVERY_TARGET_MARKER = /<!--\s*factory-delivery-target:\s*([^\s]+)\s*-->/gu;
const STACKED_PARENT_MARKER = /<!--\s*factory-stacked-parent:\s*([^\s]+)\s*-->/gu;
const SQUASH_SUBJECT = /^(.*) \(#[1-9]\d*\)$/u;
const SAFE_REPO = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/u;
const SAFE_SHA = /^[0-9a-f]{40}$/u;

export function validateBranch(branch) {
  return BRANCH.test(branch)
    ? []
    : [`branch must match <type>/issue-N with an approved type: ${branch}`];
}

export function validateSubject(subject) {
  return SUBJECT.test(subject)
    ? []
    : [`subject must be a scoped Conventional Commit with an approved type/scope: ${subject}`];
}

function markerValues(body, pattern) {
  return [...body.matchAll(pattern)].map((match) => match[1]);
}

export function validatePullRequestBase({ base, head, body }) {
  const errors = [];
  const targets = markerValues(body ?? "", DELIVERY_TARGET_MARKER);
  const parents = markerValues(body ?? "", STACKED_PARENT_MARKER);
  if (targets.length !== 1) {
    errors.push(`pull-request body must contain exactly one factory delivery-target marker; found ${targets.length}`);
  }
  if (parents.length !== 1) {
    errors.push(`pull-request body must contain exactly one factory stacked-parent marker; found ${parents.length}`);
  }
  if (errors.length) return errors;

  const [target] = targets;
  const [parent] = parents;
  if (!DELIVERY_TARGET.test(target)) errors.push(`unsupported final delivery target: ${target}`);
  if (parent === "none") {
    if (base !== target) errors.push(`pull-request base ${base} does not match final delivery target ${target}`);
  } else {
    errors.push(...validateBranch(parent).map((error) => `stacked parent ${error}`));
    if (base !== parent) errors.push(`pull-request base ${base} does not match recorded stacked parent ${parent}`);
    if (head === parent) errors.push("pull-request head and stacked parent must be different branches");
  }
  return errors;
}

function exactSignoff(authorName, authorEmail) {
  return `Signed-off-by: ${authorName} <${authorEmail}>`;
}

export function validateCommit(commit, { requireLocalSignature = false } = {}) {
  const errors = [];
  if (!commit.generatedMerge) errors.push(...validateSubject(commit.subject));
  const signoff = exactSignoff(commit.authorName, commit.authorEmail);
  const count = commit.body.split("\n").filter((line) => line.trim() === signoff).length;
  if (!commit.generatedMerge && count !== 1) {
    errors.push(`${commit.sha}: expected exactly one author-matching DCO trailer; found ${count}`);
  }
  if (requireLocalSignature && !commit.generatedMerge && commit.signature !== "G") {
    errors.push(`${commit.sha}: local OpenPGP verification is ${commit.signature || "unknown"}, expected G`);
  }
  if (requireLocalSignature && !commit.generatedMerge && commit.signatureFormat !== "openpgp") {
    errors.push(`${commit.sha}: signature format is ${commit.signatureFormat || "unknown"}, expected openpgp`);
  }
  return errors;
}

export function signatureFormatFromCommit(rawCommit) {
  const marker = /^gpgsig -----BEGIN ([A-Z0-9 ]+)-----$/mu.exec(rawCommit)?.[1];
  if (marker === "PGP SIGNATURE") return "openpgp";
  if (marker === "SSH SIGNATURE") return "ssh";
  if (marker === "SIGNED MESSAGE") return "x509";
  return null;
}

export function isPlatformGeneratedCommit({
  parents, committerName, committerEmail, subject, allowSquash = false,
}) {
  if (committerName !== "GitHub" || committerEmail !== "noreply@github.com") return false;
  const parentCount = parents.trim().split(/\s+/u).filter(Boolean).length;
  if (parentCount > 1) return true;
  const squash = SQUASH_SUBJECT.exec(subject);
  return allowSquash && parentCount === 1 && squash !== null && validateSubject(squash[1]).length === 0;
}

function git(cwd, args) {
  return execFileSync("git", args, { cwd, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 }).trim();
}

export function readCommits(base, head, cwd = process.cwd(), { allowGeneratedSquash = false } = {}) {
  const shas = git(cwd, ["rev-list", "--reverse", `${base}..${head}`]).split("\n").filter(Boolean);
  return shas.map((sha) => {
    const raw = execFileSync("git", [
      "show", "-s", "--format=%an%x00%ae%x00%cn%x00%ce%x00%s%x00%B%x00%G?%x00%P", sha,
    ], { cwd, encoding: "utf8", maxBuffer: 1024 * 1024 });
    const [authorName, authorEmail, committerName, committerEmail, subject, body, signature, parents] = raw.split("\0");
    const generatedMerge = isPlatformGeneratedCommit({
      parents, committerName, committerEmail, subject, allowSquash: allowGeneratedSquash,
    });
    const rawCommit = git(cwd, ["cat-file", "commit", sha]);
    const signatureFormat = signatureFormatFromCommit(rawCommit);
    return { sha, authorName, authorEmail, subject, body, signature, signatureFormat, generatedMerge };
  });
}

function githubVerified(repo, sha) {
  if (!SAFE_REPO.test(repo) || !SAFE_SHA.test(sha)) throw new Error("invalid repository or commit identity");
  return execFileSync("gh", [
    "api", `repos/${repo}/commits/${sha}`, "--jq", ".commit.verification.verified",
  ], { encoding: "utf8", maxBuffer: 1024 * 1024 }).trim() === "true";
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index === -1 ? undefined : argv[index + 1];
}

export function run(argv = process.argv.slice(2), {
  cwd = process.cwd(),
  stdout = process.stdout,
  stderr = process.stderr,
} = {}) {
  const messageFile = option(argv, "--message-file");
  if (messageFile) {
    const subject = readFileSync(messageFile, "utf8").split("\n", 1)[0];
    const errors = validateSubject(subject);
    if (errors.length) {
      errors.forEach((entry) => stderr.write(`${entry}\n`));
      return 1;
    }
    stdout.write("contribution policy: commit subject valid\n");
    return 0;
  }

  const errors = [];
  const branch = option(argv, "--branch");
  if (branch) errors.push(...validateBranch(branch));
  const title = option(argv, "--pr-title");
  if (title) errors.push(...validateSubject(title));
  const prBase = option(argv, "--pr-base");
  if (prBase) {
    errors.push(...validatePullRequestBase({
      base: prBase,
      head: branch,
      body: option(argv, "--pr-body") ?? "",
    }));
  }
  const base = option(argv, "--base");
  const head = option(argv, "--head") ?? "HEAD";
  if (!base) errors.push("--base is required for commit-range validation");
  const commits = base ? readCommits(base, head, cwd, {
    allowGeneratedSquash: argv.includes("--allow-generated-squash"),
  }) : [];
  if (commits.length === 0) errors.push("commit range is empty");
  for (const commit of commits) {
    errors.push(...validateCommit(commit, { requireLocalSignature: argv.includes("--local-signatures") }));
  }

  const repo = option(argv, "--repo");
  const githubSignatures = argv.includes("--github-signatures");
  const githubGeneratedSignatures = argv.includes("--github-generated-signatures");
  if (githubSignatures || githubGeneratedSignatures) {
    if (!repo) errors.push("--repo is required with GitHub signature verification");
    else {
      for (const commit of commits) {
        if (!githubSignatures && !commit.generatedMerge) continue;
        if (!githubVerified(repo, commit.sha)) errors.push(`${commit.sha}: GitHub does not report a verified signature`);
      }
    }
  }
  if (argv.includes("--local-signatures") && !githubSignatures && !githubGeneratedSignatures
      && commits.some(({ generatedMerge }) => generatedMerge)) {
    errors.push("GitHub-generated commits require --github-generated-signatures with local verification");
  }

  if (errors.length) {
    errors.forEach((entry) => stderr.write(`${entry}\n`));
    return 1;
  }
  stdout.write(`contribution policy: ${commits.length} commit(s) valid\n`);
  return 0;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = run();
}

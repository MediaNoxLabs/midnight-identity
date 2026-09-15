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
  if (requireLocalSignature && commit.signature !== "G") {
    errors.push(`${commit.sha}: local OpenPGP verification is ${commit.signature || "unknown"}, expected G`);
  }
  return errors;
}

export function isPlatformGeneratedMerge({ parents, committerName, committerEmail }) {
  return parents.trim().split(/\s+/u).filter(Boolean).length > 1
    && committerName === "GitHub"
    && committerEmail === "noreply@github.com";
}

function git(cwd, args) {
  return execFileSync("git", args, { cwd, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 }).trim();
}

export function readCommits(base, head, cwd = process.cwd()) {
  const shas = git(cwd, ["rev-list", "--reverse", `${base}..${head}`]).split("\n").filter(Boolean);
  return shas.map((sha) => {
    const raw = execFileSync("git", [
      "show", "-s", "--format=%an%x00%ae%x00%cn%x00%ce%x00%s%x00%B%x00%G?%x00%P", sha,
    ], { cwd, encoding: "utf8", maxBuffer: 1024 * 1024 });
    const [authorName, authorEmail, committerName, committerEmail, subject, body, signature, parents] = raw.split("\0");
    const generatedMerge = isPlatformGeneratedMerge({ parents, committerName, committerEmail, subject });
    return { sha, authorName, authorEmail, subject, body, signature, generatedMerge };
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
  const base = option(argv, "--base");
  const head = option(argv, "--head") ?? "HEAD";
  if (!base) errors.push("--base is required for commit-range validation");
  const commits = base ? readCommits(base, head, cwd) : [];
  if (commits.length === 0) errors.push("commit range is empty");
  for (const commit of commits) {
    errors.push(...validateCommit(commit, { requireLocalSignature: argv.includes("--local-signatures") }));
  }

  const repo = option(argv, "--repo");
  if (argv.includes("--github-signatures")) {
    if (!repo) errors.push("--repo is required with --github-signatures");
    else {
      for (const commit of commits) {
        if (!githubVerified(repo, commit.sha)) errors.push(`${commit.sha}: GitHub does not report a verified signature`);
      }
    }
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

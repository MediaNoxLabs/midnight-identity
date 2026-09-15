#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

export const DeliveryProfile = Object.freeze({
  PROTOTYPE: "prototype",
  PRODUCTION_READY: "production-ready",
});

export const Target = Object.freeze({
  POLICY: "policy",
  RUST: "rust",
  UNIT: "unit",
  WASM: "wasm",
  COVERAGE: "coverage",
  DID_CODEGEN: "did-codegen",
  VC_CODEGEN: "vc-codegen",
});

export const TARGETS = Object.freeze(Object.values(Target));

export const ALL_PACKAGES = Object.freeze([
  "midnight-did-domain",
  "midnight-did-method",
  "midnight-did-runtime",
  "midnight-did-api",
  "midnight-did",
  "midnight-did-cli",
  "midnight-did-indexer",
  "midnight-did-resolver",
  "midnight-did-uniffi",
  "midnight-did-jubjub-schnorr",
  "midnight-vc-domain",
  "midnight-vc-runtime",
]);

const DEPENDENTS = Object.freeze({
  "midnight-did-domain": [
    "midnight-did-domain", "midnight-did-method", "midnight-did-runtime",
    "midnight-did-api", "midnight-did", "midnight-did-cli",
    "midnight-did-indexer", "midnight-did-resolver", "midnight-did-uniffi",
  ],
  "midnight-did-method": [
    "midnight-did-method", "midnight-did-runtime", "midnight-did-api",
    "midnight-did", "midnight-did-cli", "midnight-did-indexer",
    "midnight-did-resolver", "midnight-did-uniffi",
  ],
  "midnight-did-runtime": [
    "midnight-did-runtime", "midnight-did-api", "midnight-did",
    "midnight-did-cli", "midnight-did-indexer", "midnight-did-resolver",
    "midnight-did-uniffi",
  ],
  "midnight-did-api": [
    "midnight-did-api", "midnight-did", "midnight-did-cli",
    "midnight-did-indexer", "midnight-did-resolver", "midnight-did-uniffi",
  ],
  "midnight-did-indexer": ["midnight-did-indexer", "midnight-did-resolver"],
  "midnight-did": ["midnight-did"],
  "midnight-did-cli": ["midnight-did-cli"],
  "midnight-did-resolver": ["midnight-did-resolver"],
  "midnight-did-uniffi": ["midnight-did-uniffi"],
  "midnight-did-jubjub-schnorr": ["midnight-did-jubjub-schnorr"],
  "midnight-vc-domain": ["midnight-vc-domain"],
  "midnight-vc-runtime": ["midnight-vc-runtime"],
});

const DOC_FILE = /(?:^|\/)(?:README|CONTRIBUTING|CHANGELOG|SECURITY|CODE_OF_CONDUCT)\.md$|\.md$/u;
const BUILD_FILE = /^(?:Cargo\.toml|Cargo\.lock|flake\.nix|flake\.lock|justfile|rustfmt\.toml|taplo\.toml)$|^nix\//u;

function normalize(candidate) {
  return candidate.replaceAll("\\", "/").replace(/^\.\//u, "");
}

function ordered(values, authority) {
  const selected = new Set(values);
  return authority.filter((value) => selected.has(value));
}

export function classifyPath(candidate) {
  const file = normalize(candidate);
  if (!file) return { area: "unknown" };
  if (BUILD_FILE.test(file) || file === ".gitmodules") return { area: "build" };
  if (file === "third_party/midnight-did" || file.startsWith("third_party/midnight-did/")) {
    return { area: "did-codegen", package: "midnight-did-runtime" };
  }
  if (file === "third_party/midnight-verifiable-credentials"
      || file.startsWith("third_party/midnight-verifiable-credentials/")) {
    return { area: "vc-codegen", package: "midnight-vc-runtime" };
  }
  if (/^crates\/midnight-did-runtime\/(?:src\/contract|assets\/keys)\//u.test(file)) {
    return { area: "did-codegen", package: "midnight-did-runtime" };
  }
  if (/^crates\/midnight-vc-runtime\/src\/contract\//u.test(file)) {
    return { area: "vc-codegen", package: "midnight-vc-runtime" };
  }
  const crate = /^crates\/([^/]+)\//u.exec(file)?.[1];
  if (crate && ALL_PACKAGES.includes(crate)) return { area: "rust", package: crate };
  if (file === "AGENT.md" || file === ".devloops" || file.startsWith(".pi/")
      || file.startsWith("doc/factory/") || file.startsWith("scripts/factory/")
      || file.startsWith("tests/factory/") || file === "bootstrap.sh"
      || file.startsWith(".githooks/") || file === ".gitignore"
      || file === ".gitattributes" || file === "CODEOWNERS") return { area: "factory" };
  if (file.startsWith(".github/") || file.startsWith("scripts/ci/")) return { area: "ci" };
  if (DOC_FILE.test(file) || file.startsWith("doc/")) return { area: "docs" };
  return { area: "unknown" };
}

export function makeTargetPlan(paths, {
  deliveryProfile = DeliveryProfile.PRODUCTION_READY,
  forceFull = false,
  includeAudit = false,
} = {}) {
  if (!Object.values(DeliveryProfile).includes(deliveryProfile)) {
    throw new Error(`unknown delivery profile: ${deliveryProfile}`);
  }

  const changedPaths = [...new Set(paths.map(normalize).filter(Boolean))].sort();
  const classified = changedPaths.map(classifyPath);
  const areas = [...new Set(classified.map(({ area }) => area))].sort();
  const unavailable = changedPaths.length === 0;
  const full = forceFull || unavailable || areas.includes("unknown") || areas.includes("build");
  const packages = new Set();

  if (full) {
    ALL_PACKAGES.forEach((name) => packages.add(name));
  } else {
    for (const entry of classified) {
      if (!entry.package) continue;
      for (const name of DEPENDENTS[entry.package] ?? [entry.package]) packages.add(name);
    }
  }

  const targets = new Set([Target.POLICY]);
  const rustChanged = full || packages.size > 0;
  if (rustChanged) {
    targets.add(Target.RUST);
    targets.add(Target.UNIT);
  }
  if (full || areas.includes("did-codegen")) targets.add(Target.DID_CODEGEN);
  if (full || areas.includes("vc-codegen")) targets.add(Target.VC_CODEGEN);
  const wasmChanged = full || classified.some(({ package: name }) =>
    ["midnight-did-domain", "midnight-did-method", "midnight-vc-domain"].includes(name));
  if (deliveryProfile === DeliveryProfile.PRODUCTION_READY) {
    if (wasmChanged) targets.add(Target.WASM);
    if (rustChanged) targets.add(Target.COVERAGE);
  }

  return {
    schemaVersion: 1,
    deliveryProfile,
    mode: full ? "full" : "affected",
    diffAvailable: !unavailable,
    changedPaths,
    areas,
    packages: ordered(packages, ALL_PACKAGES),
    targets: ordered(targets, TARGETS),
    audit: includeAudit,
  };
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index === -1 ? undefined : argv[index + 1];
}

function has(argv, name) {
  return argv.includes(name);
}

function changedPaths(base, head, cwd) {
  if (!base || !head || /^0+$/u.test(base)) return null;
  try {
    for (const ref of [base, head]) {
      execFileSync("git", ["rev-parse", "--verify", `${ref}^{commit}`], {
        cwd, stdio: "ignore",
      });
    }
    const output = execFileSync("git", ["diff", "--no-renames", "--name-only", "-z", base, head, "--"], {
      cwd,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    });
    return output.split("\0").filter(Boolean);
  } catch {
    return null;
  }
}

function githubOutput(plan) {
  const selected = new Set(plan.targets);
  const values = {
    schema_version: plan.schemaVersion,
    delivery_profile: plan.deliveryProfile,
    mode: plan.mode,
    areas: plan.areas.join(","),
    packages_csv: plan.packages.join(","),
    targets: plan.targets.join(","),
    audit: plan.audit,
  };
  for (const target of TARGETS) values[target.replaceAll("-", "_")] = selected.has(target);
  return `${Object.entries(values).map(([key, value]) => `${key}=${value}`).join("\n")}\n`;
}

export function run(argv = process.argv.slice(2), {
  cwd = process.cwd(),
  stdout = process.stdout,
  stderr = process.stderr,
} = {}) {
  const explicitPaths = option(argv, "--paths");
  const paths = explicitPaths === undefined
    ? changedPaths(option(argv, "--base"), option(argv, "--head"), cwd)
    : explicitPaths.split(",").filter(Boolean);
  if (paths === null) stderr.write("[target-plan] diff unavailable; selecting the full production plan\n");
  const plan = makeTargetPlan(paths ?? [], {
    deliveryProfile: option(argv, "--delivery-profile") ?? DeliveryProfile.PRODUCTION_READY,
    forceFull: has(argv, "--full") || paths === null,
    includeAudit: has(argv, "--audit"),
  });
  const format = option(argv, "--format") ?? "summary";
  if (format === "json") stdout.write(`${JSON.stringify(plan, null, 2)}\n`);
  else if (format === "github") stdout.write(githubOutput(plan));
  else if (format === "summary") {
    stdout.write(`${plan.deliveryProfile}/${plan.mode}: ${plan.targets.join(", ")} [${plan.areas.join(", ")}]\n`);
  } else throw new Error(`unknown output format: ${format}`);
  return plan;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) run();

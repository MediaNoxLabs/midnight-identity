#!/usr/bin/env node
// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const sourceRoot = path.join(root, "crates/midnight-passport-vault-source");
const defaultBaseline = "artifacts/passport-vault-ledger8/oxid-baseline.json";

function opt(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}
function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function repoPath(rel) { return path.isAbsolute(rel) ? rel : path.join(root, rel); }
function read(rel) { return readFileSync(repoPath(rel)); }
function json(rel) { return JSON.parse(read(rel)); }
function git(args, cwd = root) { return execFileSync("git", args, { cwd, encoding: "utf8", timeout: 30_000 }).trim(); }
function parsePinnedRev(name, text) {
  const match = new RegExp(`${name}\\s*=\\s*"([0-9a-f]{40})"`, "u").exec(text);
  if (!match) throw new Error(`missing pinned ${name}`);
  return match[1];
}
function parseQuoted(name, text) {
  const match = new RegExp(`${name}\\s*=\\s*"([^"]+)"`, "u").exec(text);
  if (!match) throw new Error(`missing ${name}`);
  return match[1];
}
function githubRepositoryIdentity(remote) {
  const value = remote.trim();
  const patterns = [
    /^https:\/\/github\.com\/([^/]+)\/([^/]+?)(?:\.git)?\/?$/u,
    /^git@github\.com:([^/]+)\/([^/]+?)(?:\.git)?$/u,
    /^ssh:\/\/git@github\.com\/([^/]+)\/([^/]+?)(?:\.git)?\/?$/u,
  ];
  for (const pattern of patterns) {
    const match = pattern.exec(value);
    if (match) return `${match[1]}/${match[2]}`;
  }
  throw new Error(`unsupported GitHub remote URL: ${remote}`);
}
function sortJson(value) {
  if (Array.isArray(value)) return value.map(sortJson);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, sortJson(value[key])]));
  }
  return value;
}

const out = opt("--out") ?? path.join(root, "artifacts/passport-vault-ledger8/manifest.json");
const oxidRoot = opt("--oxid-reference") ?? process.env.OXID_REFERENCE_ROOT;
const baselinePath = opt("--baseline") ?? process.env.OXID_BASELINE_FILE ?? defaultBaseline;
const sourceManifest = json("crates/midnight-passport-vault-source/manifest.json");
const contractBytes = readFileSync(path.join(sourceRoot, sourceManifest.contract.path));
if (contractBytes.length !== sourceManifest.contract.bytes) throw new Error("contract byte count drift");
if (sha256(contractBytes) !== sourceManifest.contract.sha256) throw new Error("contract digest drift");
let closureBytes = 0;
const closureHash = createHash("sha256");
for (const rel of sourceManifest.includeClosure.files) {
  const bytes = readFileSync(path.join(sourceRoot, rel));
  closureBytes += bytes.length;
  closureHash.update(bytes);
}
if (closureBytes !== sourceManifest.includeClosure.bytes) throw new Error("include closure byte count drift");
if (closureHash.digest("hex") !== sourceManifest.includeClosure.sha256) throw new Error("include closure digest drift");
if (!/^8(?:\.|$)/u.test(sourceManifest.compatibility.ledgerLine)) throw new Error("not a Ledger 8 source manifest");

const oxidDerivationPath = "nix/packages/passport-vault-compact-artifacts.nix";
function readOxidBaseline() {
  if (oxidRoot) {
    const oxidDerivation = readFileSync(path.join(oxidRoot, oxidDerivationPath), "utf8");
    const oxidRemote = git(["remote", "get-url", "origin"], oxidRoot);
    const oxidRepository = githubRepositoryIdentity(oxidRemote);
    const oxidSha = git(["rev-parse", "origin/develop"], oxidRoot);
    const oxidHead = git(["rev-parse", "HEAD"], oxidRoot);
    const oxidStatus = git(["status", "--short"], oxidRoot);
    if (oxidRepository !== "MediaNoxLabs/oxid") throw new Error(`unexpected Oxid remote: ${oxidRemote}`);
    if (oxidStatus !== "") throw new Error("Oxid reference checkout is dirty");
    if (oxidHead !== oxidSha) throw new Error("Oxid reference checkout HEAD must match origin/develop");
    const sourcePathSha256 = sha256(oxidDerivation);
    const pinnedBaseline = json(baselinePath);
    if (pinnedBaseline.headSha !== oxidSha || pinnedBaseline.sourcePathSha256 !== sourcePathSha256) {
      throw new Error("Oxid checkout does not match the committed pinned baseline");
    }
    if (typeof pinnedBaseline.compactInputRevision !== "string" || pinnedBaseline.compactInputRevision.length === 0) {
      throw new Error("committed Oxid baseline missing compactInputRevision");
    }
    const compactInputRevision = parseQuoted("compilerRevision", oxidDerivation);
    if (pinnedBaseline.compactInputRevision !== compactInputRevision) {
      throw new Error("committed Oxid baseline compactInputRevision does not match Oxid derivation");
    }
    return {
      repository: "MediaNoxLabs/oxid",
      remote: "https://github.com/MediaNoxLabs/oxid.git",
      reference: "origin/develop",
      headSha: oxidSha,
      sourcePath: oxidDerivationPath,
      sourcePathSha256,
      licenseProvenance: "Apache-2.0 repository license; derivation headerless Nix expression observed read-only from Oxid baseline",
      oxidVaultRevision: parseQuoted("vaultRevision", oxidDerivation),
      oxidCredentialRevision: parseQuoted("vcRevision", oxidDerivation),
      oxidUpstreamVaultLockDigest: parseQuoted("upstreamVaultLockDigest", oxidDerivation),
      compactInputRevision,
      observationMode: "pinned-baseline",
    };
  }
  const baseline = json(baselinePath);
  for (const field of ["repository", "remote", "reference", "headSha", "sourcePath", "sourcePathSha256", "licenseProvenance", "oxidVaultRevision", "oxidCredentialRevision", "oxidUpstreamVaultLockDigest"]) {
    if (typeof baseline[field] !== "string" || baseline[field].length === 0) throw new Error(`baseline missing ${field}`);
  }
  if (baseline.repository !== "MediaNoxLabs/oxid") throw new Error("baseline repository mismatch");
  if (githubRepositoryIdentity(baseline.remote) !== "MediaNoxLabs/oxid") throw new Error("baseline remote mismatch");
  if (baseline.reference !== "origin/develop") throw new Error("baseline reference mismatch");
  if (!/^[0-9a-f]{40}$/u.test(baseline.headSha)) throw new Error("baseline head is not an exact SHA");
  if (!/^[0-9a-f]{64}$/u.test(baseline.sourcePathSha256)) throw new Error("baseline source digest is not a sha256");
  return { ...baseline, remote: "https://github.com/MediaNoxLabs/oxid.git", observationMode: "pinned-baseline" };
}
const oxidBaseline = readOxidBaseline();
const flake = readFileSync(path.join(root, "flake.nix"), "utf8");
const repositoryCompactInputRevision = parsePinnedRev("rev", flake.match(/compact = \{[\s\S]*?\n    \};/u)?.[0] ?? "");
const repositoryLedgerRevision = parsePinnedRev("rev", flake.match(/midnight-ledger = \{[\s\S]*?\n    \};/u)?.[0] ?? "");
const repositoryProofRevision = parsePinnedRev("rev", flake.match(/midnight-zk = \{[\s\S]*?\n    \};/u)?.[0] ?? "");
if (typeof oxidBaseline.compactInputRevision !== "string" || oxidBaseline.compactInputRevision.length === 0) {
  throw new Error("Oxid baseline missing compactInputRevision");
}
if (oxidBaseline.compactInputRevision !== sourceManifest.compatibility.compilerRevision) {
  throw new Error("Oxid baseline compactInputRevision does not match authenticated source compiler revision");
}
const compactInputRevision = oxidBaseline.compactInputRevision;
if (oxidBaseline.oxidVaultRevision !== sourceManifest.provenance.originalRevision) {
  throw new Error("Oxid baseline vault revision does not match authenticated source provenance");
}
if (oxidBaseline.oxidCredentialRevision !== sourceManifest.provenance.reviewedCredentialRevision) {
  throw new Error("Oxid baseline credential revision does not match authenticated source provenance");
}
if (sourceManifest.provenance.reviewedCredentialIncludeClosureSha256 !== sourceManifest.includeClosure.sha256) {
  throw new Error("credential provenance does not match authenticated include closure");
}
if (oxidBaseline.oxidUpstreamVaultLockDigest !== "c8d47db1b63785765b8875a3eaa33221b61c779ce7d7cc6e83d8d386a50c0e9f") {
  throw new Error("Oxid baseline vault lock digest mismatch");
}

const lockDigest = sha256(read("flake.lock"));
const artifactPaths = [
  "compiler/contract-info.json",
  "contract/index.d.ts",
  "contract/index.js",
  "contract/index.js.map",
  ...sourceManifest.circuits.flatMap(({ name }) => [
    `zkir/${name}.bzkir`,
    `zkir/${name}.zkir`,
    `keys/${name}.prover`,
    `keys/${name}.verifier`,
  ]),
  "params/bls_midnight_2p10",
  "params/bls_midnight_2p11",
  "params/bls_midnight_2p13",
  "params/bls_midnight_2p17",
];
const heavy = [/^zkir\//u, /^keys\//u, /^params\//u];
const sourceIdentity = `${sourceManifest.contract.sha256}:${sourceManifest.includeClosure.sha256}`;
const abiStateVectorIdentity = sha256(JSON.stringify({
  contract: sourceManifest.contract.sha256,
  circuits: sourceManifest.circuits,
  language: sourceManifest.compatibility.generatedLanguageVersion,
  runtime: sourceManifest.compatibility.generatedRuntimeVersion,
}));
const publicInputIdentity = sha256(JSON.stringify({
  source: sourceIdentity,
  circuits: sourceManifest.circuits.map(({ name, k, rows }) => ({ name, k, rows })),
}));

const manifest = sortJson({
  schemaVersion: 1,
  package: {
    name: "midnight-passport-vault-ledger8-artifacts",
    version: "0.5.0",
    delivery: "pinned-nix-artifact-leaf",
    ledgerLine: "8.x",
  },
  source: {
    package: sourceManifest.package,
    contractPath: sourceManifest.contract.path,
    contractBytes: sourceManifest.contract.bytes,
    contractSha256: sourceManifest.contract.sha256,
    includeClosureAlgorithm: sourceManifest.includeClosure.algorithm,
    includeClosureBytes: sourceManifest.includeClosure.bytes,
    includeClosureSha256: sourceManifest.includeClosure.sha256,
    includeClosureFiles: sourceManifest.includeClosure.files,
    sourceIdentity,
    provenance: sourceManifest.provenance,
  },
  referenceBaseline: {
    repository: oxidBaseline.repository,
    remote: oxidBaseline.remote,
    reference: oxidBaseline.reference,
    headSha: oxidBaseline.headSha,
    sourcePath: oxidBaseline.sourcePath,
    sourcePathSha256: oxidBaseline.sourcePathSha256,
    licenseProvenance: oxidBaseline.licenseProvenance,
    observationMode: "pinned-baseline",
  },
  toolchain: {
    compactCliVersion: sourceManifest.compatibility.compactCliVersion,
    compactCompilerVersion: sourceManifest.compatibility.compilerVersion,
    compactCompilerRevision: sourceManifest.compatibility.compilerRevision,
    compactInputRevision,
    ledgerRepository: "https://github.com/MediaNoxLabs/midnight-ledger",
    repositoryCompactInputRevision,
    repositoryLedgerRevision,
    proofRepository: "https://github.com/MediaNoxLabs/midnight-zk",
    repositoryProofRevision,
    generatedLanguageVersion: sourceManifest.compatibility.generatedLanguageVersion,
    generatedRuntimeVersion: sourceManifest.compatibility.generatedRuntimeVersion,
    oxidVaultRevision: oxidBaseline.oxidVaultRevision,
    oxidCredentialRevision: oxidBaseline.oxidCredentialRevision,
    oxidUpstreamVaultLockDigest: oxidBaseline.oxidUpstreamVaultLockDigest,
    repositoryLockSha256: lockDigest,
  },
  generatedSurface: {
    artifactPaths,
    abiStateVectorIdentity,
    publicInputIdentity,
    contractInfoExpectations: {
      compilerVersion: sourceManifest.compatibility.compilerVersion,
      languageVersion: sourceManifest.compatibility.generatedLanguageVersion,
      runtimeVersion: sourceManifest.compatibility.generatedRuntimeVersion,
      circuits: sourceManifest.circuits.map(({ name }) => name),
    },
  },
  circuits: sourceManifest.circuits.map(({ name, k, rows }) => ({ id: name, k, rows })),
  cacheKeyInputs: {
    sourceIdentity,
    compactCompilerRevision: sourceManifest.compatibility.compilerRevision,
    compactInputRevision,
    nixLockSha256: lockDigest,
    materializerRevision: oxidBaseline.headSha,
    materializerSourcePathSha256: oxidBaseline.sourcePathSha256,
  },
  materialization: {
    command: `nix build github:MediaNoxLabs/oxid/${oxidBaseline.headSha}#passportVaultCompactArtifacts`,
    outputManifest: "manifest.json",
    derivationRepository: oxidBaseline.repository,
    derivationRevision: oxidBaseline.headSha,
    derivationPath: oxidBaseline.sourcePath,
    derivationSha256: oxidBaseline.sourcePathSha256,
    verifiesSourceBeforeGeneration: true,
    emitsArtifactDigests: true,
    note: "The pinned Oxid Ledger 8 derivation is the reviewed materializer for the heavy artifact set; this repository commits the authenticated upstream leaf manifest and keeps generated outputs out of Git until Oxid #123 cuts over to an immutable midnight-identity revision.",
  },
  packagePolicy: {
    generationCommand: "node scripts/compact/passport-vault-ledger8-artifacts.mjs --out artifacts/passport-vault-ledger8/manifest.json",
    committedArtifacts: ["artifacts/passport-vault-ledger8/manifest.json"],
    reproduciblyDerivedNotCommitted: artifactPaths.filter((entry) => heavy.some((pattern) => pattern.test(entry))),
    forbiddenGitPathPatterns: ["**/*.zkir", "**/*.bzkir", "**/*.prover", "**/*.verifier", "**/bls_midnight_2p*", "target/", "result"],
  },
  metrics: {
    coldGenerationTimeSeconds: null,
    warmGenerationTimeSeconds: null,
    artifactBytes: null,
    cacheBytes: null,
    dependencyClosureImpact: "source crate remains unchanged; artifact leaf records pinned Nix/Compact/Ledger/proof lineage outside midnight-passport-vault-source dependencies",
  },
});
mkdirSync(path.dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(JSON.stringify({ ok: true, out: path.relative(root, out), sha256: sha256(readFileSync(out)), artifactPaths: artifactPaths.length }, null, 2));

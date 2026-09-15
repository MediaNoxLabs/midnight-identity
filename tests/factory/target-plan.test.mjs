// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import {
  ALL_PACKAGES,
  DeliveryProfile,
  TARGETS,
  Target,
  classifyPath,
  makeTargetPlan,
} from "../../scripts/ci/target-plan.mjs";

test("documentation, factory, and CI changes avoid the Rust and Nix closure", () => {
  for (const paths of [
    ["README.md", "doc/factory/ci.md"],
    ["AGENT.md", ".pi/settings.json", "scripts/factory/audit.mjs", ".gitignore"],
    [".github/workflows/ci.yml", "scripts/ci/target-plan.mjs"],
  ]) {
    const plan = makeTargetPlan(paths);
    assert.deepEqual(plan.targets, [Target.POLICY], paths.join(","));
    assert.deepEqual(plan.packages, []);
  }
});

test("affected crate plans expand to known compatibility dependents", () => {
  const plan = makeTargetPlan(["crates/midnight-did-domain/src/lib.rs"]);
  assert.deepEqual(plan.targets, [Target.POLICY, Target.RUST, Target.UNIT, Target.WASM, Target.COVERAGE]);
  assert.ok(plan.packages.includes("midnight-did-domain"));
  assert.ok(plan.packages.includes("midnight-did-resolver"));
  assert.ok(!plan.packages.includes("midnight-vc-domain"));
});

test("method-only changes enforce the WASM-clean contract", () => {
  const plan = makeTargetPlan(["crates/midnight-did-method/src/lib.rs"]);
  assert.ok(plan.targets.includes(Target.WASM));
});

test("Passport Vault source changes select its Rust, WASM, and coverage gates", () => {
  const plan = makeTargetPlan(["crates/midnight-passport-vault-source/src/lib.rs"]);
  assert.deepEqual(plan.packages, ["midnight-passport-vault-source"]);
  assert.deepEqual(plan.targets, [
    Target.POLICY, Target.RUST, Target.UNIT, Target.WASM, Target.COVERAGE,
  ]);
});

test("prototype keeps directly affected checks but omits broad compatibility lanes", () => {
  const plan = makeTargetPlan(["crates/midnight-vc-domain/src/lib.rs"], {
    deliveryProfile: DeliveryProfile.PROTOTYPE,
  });
  assert.deepEqual(plan.targets, [Target.POLICY, Target.RUST, Target.UNIT]);
});

test("Compact inputs select only their affected generation family", () => {
  const did = makeTargetPlan(["third_party/midnight-did"]);
  assert.ok(did.targets.includes(Target.DID_CODEGEN));
  assert.ok(!did.targets.includes(Target.VC_CODEGEN));
  const vc = makeTargetPlan(["crates/midnight-vc-runtime/src/contract/credentials.rs"]);
  assert.ok(vc.targets.includes(Target.VC_CODEGEN));
  assert.ok(!vc.targets.includes(Target.DID_CODEGEN));
});

test("Digital Passport sources and bindings select the family codegen gate", () => {
  for (const path of [
    "third_party/midnight-verifiable-credential-digital-passport",
    "crates/midnight-vc-families/src/contract/digital_passport.rs",
  ]) {
    const plan = makeTargetPlan([path]);
    assert.deepEqual(plan.packages, ["midnight-vc-families"]);
    assert.ok(plan.targets.includes(Target.VC_CODEGEN));
    assert.ok(!plan.targets.includes(Target.DID_CODEGEN));
  }
});

test("the Rust target compiles credential families with their opt-in features", async () => {
  const script = await import("node:fs/promises").then(({ readFile }) =>
    readFile(new URL("../../scripts/ci/rust-target.sh", import.meta.url), "utf8"));
  assert.match(script, /midnight-vc-families/u);
  assert.match(script, /--all-features/u);
});

test("build, unknown, and empty diff states fail closed", () => {
  for (const paths of [["Cargo.lock"], ["new-root-format.custom"], []]) {
    const plan = makeTargetPlan(paths);
    assert.equal(plan.mode, "full");
    assert.deepEqual(plan.targets, TARGETS);
    assert.deepEqual(plan.packages, ALL_PACKAGES);
  }
});

test("manual full plans can request the read-only periodic audit", () => {
  const plan = makeTargetPlan(["README.md"], { forceFull: true, includeAudit: true });
  assert.deepEqual(plan.targets, TARGETS);
  assert.equal(plan.audit, true);
});

test("unknown delivery profiles and malformed paths are rejected or fail closed", () => {
  assert.throws(() => makeTargetPlan(["README.md"], { deliveryProfile: "quick" }), /unknown delivery profile/u);
  assert.deepEqual(classifyPath("crates\\midnight-did-domain\\src\\lib.rs"), {
    area: "rust", package: "midnight-did-domain",
  });
  assert.deepEqual(classifyPath("unowned/file.bin"), { area: "unknown" });
});

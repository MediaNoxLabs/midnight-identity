// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Captures the TS↔Rust parity vectors in `ts-parity.json` from the released
// `@midnight-ntwrk/midnight-vc-passport` package (TS target of the same
// Compact sources `just codegen-vc` compiles to Rust).
//
// Usage, from an interactive `nix develop` shell (node is on PATH there):
//
//   dir=$(mktemp -d) && cd "$dir" && npm init -y >/dev/null \
//     && npm install --ignore-scripts \
//        https://github.com/midnightntwrk/midnight-vc-passport/releases/download/v0.1.0-rc2/midnight-ntwrk-midnight-vc-passport-0.1.0-rc2.tgz \
//     && node <repo>/crates/midnight-vc-families/tests/fixtures/capture-ts-parity.mjs "$dir" \
//        > <repo>/crates/midnight-vc-families/tests/fixtures/ts-parity.json
//
// Every value is derived from the package's own `createDigitalPassportFixture()`;
// the Rust test rebuilds that fixture independently and must reproduce them.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const root = process.argv[2];
if (!root) {
  throw new Error("usage: capture-ts-parity.mjs <dir with node_modules>");
}
const modules = path.join(root, "node_modules", "@midnight-ntwrk");
const load = (pkg, file) => import(pathToFileURL(path.join(modules, pkg, file)).href);
const version = (pkg) =>
  JSON.parse(readFileSync(path.join(modules, pkg, "package.json"), "utf8")).version;

const { createDigitalPassportFixture } = await load("midnight-vc-passport", "dist/testing.js");
const { pureCircuits } = await load(
  "midnight-vc-passport",
  "dist/managed/digital-passport-credential/contract/index.js",
);
const { pureCircuits: core } = await load("credential-compact", "dist/index.js");

const hex = (bytes) => Buffer.from(bytes).toString("hex");
// Field / scalar values as 32-byte little-endian hex, the Rust `Fr` byte order.
const fieldLe = (value) => {
  const out = new Uint8Array(32);
  let v = value;
  for (let i = 0; i < 32; i += 1) {
    out[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return hex(out);
};
const point = (p) => ({ x: fieldLe(p.x), y: fieldLe(p.y) });
const proof = (p) => ({
  publicKey: point(p.publicKey),
  signatureR: point(p.signature.r),
  signatureS: fieldLe(p.signature.s),
});
const rejection = (run) => {
  try {
    run();
  } catch (error) {
    return String(error.message ?? error);
  }
  throw new Error("expected the circuit to reject");
};

const fixture = createDigitalPassportFixture();
const { credential, credentialProof, presentation, presentationProof } = fixture;
const credentialBodyRoot = pureCircuits.digitalPassportCredentialBodyRoot(credential);
const presentationBodyRoot = pureCircuits.digitalPassportPresentationBodyRoot(presentation);

// Accepting paths must not throw.
pureCircuits.assertValidDigitalPassportCredential(credential, credentialProof);
pureCircuits.assertValidDigitalPassportPresentation(
  credential,
  credentialProof,
  presentation,
  presentationProof,
);

const tamperedClaimRoot = new Uint8Array(credential.claimRoot);
tamperedClaimRoot[0] ^= 0x01;

const vectors = {
  source: {
    package: `@midnight-ntwrk/midnight-vc-passport@${version("midnight-vc-passport")}`,
    core: `@midnight-ntwrk/credential-compact@${version("credential-compact")}`,
    runtime: `@midnight-ntwrk/compact-runtime@${version("compact-runtime")}`,
    fixture: "createDigitalPassportFixture()",
    script: "crates/midnight-vc-families/tests/fixtures/capture-ts-parity.mjs",
  },
  claimCommitments: Object.fromEntries(
    Object.entries(credential.claimCommitments).map(([name, value]) => [name, hex(value)]),
  ),
  claimRoot: hex(credential.claimRoot),
  credentialBodyRoot: hex(credentialBodyRoot),
  presentationBodyRoot: hex(presentationBodyRoot),
  credentialProof: proof(credentialProof),
  issuanceChallenge: fieldLe(core.issuanceProofChallenge(credentialBodyRoot, credentialProof)),
  presentationProof: proof(presentationProof),
  presentationChallenge: fieldLe(
    core.presentationProofChallenge(presentationBodyRoot, presentationProof),
  ),
  tamperedClaimRootRejection: rejection(() =>
    pureCircuits.assertValidDigitalPassportCredential(
      { ...credential, claimRoot: tamperedClaimRoot },
      credentialProof,
    ),
  ),
};

process.stdout.write(`${JSON.stringify(vectors, null, 2)}\n`);

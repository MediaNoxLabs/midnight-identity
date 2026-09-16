<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Midnight VC issuance proof boundary

`midnight-vc-proof` is the hand-written issuance-proof seam over generated
Compact bindings. It owns bounded detached-proof parsing, secure nonce signing,
and structured verification evidence. It does **not** reimplement generated
Compact hash semantics: issuance challenges call
`midnight_vc_runtime::contract::credentials::pure_circuits::issuance_proof_challenge`,
and the digital-passport fixture body root is computed through generated family
bindings.

The DID digest Schnorr challenge from `midnight-did-jubjub-schnorr` is not
interchangeable with this VC issuance challenge. DID signatures challenge a
SHA-256 digest in the DID circuit domain; VC issuance proofs challenge a
credential body root plus proof metadata in the `midnight:vc:issuance` Compact
domain.

The API intentionally stops before trust policy, revocation/status policy,
OpenID transport, and generic VC/VP orchestration. That keeps the future
chain-neutral SDK seam conceptual and adapter-owned while Midnight generated
codecs, body roots, Jubjub equations, and Compact challenge construction remain
inside this repository.

`verify_body_root` verifies only the self-contained cryptographic proof: the
body root, signer method reference, embedded public key, challenge, and Schnorr
equation are internally consistent. It does **not** prove that the signer method
reference resolves to the embedded public key or to any issuer trusted by the
caller. Digital-passport acceptance therefore uses `verify_digital_passport`
with a caller-supplied resolved issuer containing both the expected verification
method reference and its resolved Jubjub public key. DID resolution and trust
policy remain caller-owned; this crate only checks that the credential issuer,
proof signer, and proof key match the caller-resolved issuer before delegating
to the generated full credential validator.

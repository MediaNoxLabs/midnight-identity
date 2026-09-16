# midnight-did-domain

[![docs.rs](https://img.shields.io/docsrs/midnight-did-domain)](https://docs.rs/midnight-did-domain)
[![crates.io](https://img.shields.io/crates/v/midnight-did-domain.svg)](https://crates.io/crates/midnight-did-domain)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

Pure-data Rust **domain layer** for the [Midnight DID Method][spec]. This
crate holds the W3C-aligned DID Document model, MOD1 offchain frame encoder,
crypto codecs, and resolver/registrar traits — and **nothing else**. It does
not depend on any `midnight-*` runtime/ledger crate, so it builds on stable
Rust, wasm32, and embedded targets without pulling in halo2 or the Midnight
proving stack.

## What this crate provides

- `did_document` — `DidDocument`, `VerificationMethod`, `Service`,
  `PublicKeyJwk`, validators, and JSON (de)serialization.
- `offchain` — MOD1 frame encoder/decoder used for off-chain DID document
  storage (byte-parity with the TypeScript reference implementation).
- `crypto_codecs` — Jubjub point + multibase codecs, BLAKE2b helpers,
  digest types.
- `did_resolver` / `did_registrar` — abstract resolver + registrar traits
  the API layer plugs into.
- `midnight` — `MidnightNetwork` enum (Testnet / Mainnet / DevNet / Undeployed).
- `uri` — DID URI parser + builder.
- `ledger_utils` — hex helpers shared with the API layer.

## Layering

```
midnight-did-domain   ← THIS CRATE (no midnight-* deps, wasm-clean)
       ↑
midnight-did-api      (async ops + DidContract trait, see sibling crate)
       ↑
midnight-did          (runtime, blocked on upstream halo2 skew)
```

The trait-erasure split is documented in [ADR 0002][adr2] and the four-crate
shape in [ADR 0003][adr3].


## Public contract notes

- DID documents, public JWKs, and verification methods preserve unknown JSON
  members with serde `flatten` maps. Public parse/deserialization and
  constructor paths still run validation, so extension preservation does not
  permit invalid DID strings, dangling verification relationships, private JWK
  material, duplicate services, or malformed service shapes.
- Shared hardening limits are intentionally transport-neutral and wasm-clean:
  DID/DID URL strings are capped at 8 KiB; document lists and extension object
  members are capped at 128 entries; extension JSON nesting is capped at 32;
  public text values reject leading/trailing whitespace and control characters.
- `DidResolutionErrorCode::http_status()` and
  `KnownDidResolutionErrorCode::http_status()` expose the numeric HTTP status
  classifier without depending on an HTTP framework: `invalidDid`,
  `invalidDidUrl`, and `invalidOptions` map to 400; `notFound` to 404;
  `deactivated` to 410; `representationNotSupported` to 406;
  `methodNotSupported` and `unsupportedPublicKeyType` to 501; all other known
  or extension codes map to 500. The wire form remains the existing camelCase
  keyword dialect.

## Migration and compatibility

This release is additive for ordinary callers. Existing constructors continue to
build values without extensions. Call `VerificationMethod::new_with_extensions`
when native construction must retain verification-method extension members;
serde and `parse_verification_method` retain them automatically. Direct serde
loading of DID document types is now a validated entry point, so previously
accepted invalid JSON fails closed instead of constructing an invalid value.

## Status

- 51 unit tests passing.
- Byte-parity with the TS `@midnight-ntwrk/midnight-did-domain` package for
  MOD1 frame encoding.
- See [architecture][arch] for the full layering rationale.

## Related crates

- [`midnight-did-api`](https://crates.io/crates/midnight-did-api) — async
  operation builders, DID resolver, and `DidContract` trait. Depends on this
  crate.

## License

Apache-2.0. See `LICENSE` at the workspace root.

[spec]: https://github.com/midnight-ntwrk/midnight-did
[arch]: ../../doc/architecture.md
[adr2]: ../../doc/adr/0002-trait-erasure-for-contract.md
[adr3]: ../../doc/adr/0003-crate-split-2-to-4-with-umbrella.md

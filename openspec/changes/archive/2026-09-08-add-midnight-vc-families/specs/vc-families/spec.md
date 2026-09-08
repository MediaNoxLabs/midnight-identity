# Delta Spec: vc-families

## Purpose

A workspace crate exposing Midnight Verifiable Credentials **credential-family**
bindings — the Rust code emitted by the pinned Compact compiler for each family
contract (digital-passport today) — as a feature-gated library that any party
can consume via a git dependency, with generated output guarded by a
reproducibility gate.

## ADDED Requirements

### Requirement: Per-family feature-gated bindings

The crate SHALL expose each credential-family binding as its own cargo feature
and module. Enabling a family's feature SHALL make that family's generated
types and circuits available under a module named after the family; with no
features enabled the crate SHALL compile successfully while exposing no family
bindings. Features SHALL be strictly additive — enabling any combination MUST
not change the surface of the other enabled families.

#### Scenario: Bare dependency compiles nothing

- **WHEN** the crate is built with default features (none)
- **THEN** the build succeeds and no family binding module is exposed

#### Scenario: Digital-passport opt-in

- **WHEN** the crate is built with the `digital-passport` feature
- **THEN** the digital-passport family binding (types and `pure_circuits`
  surface) is available under its family module

### Requirement: Generated-surface fidelity and reproducibility

A family binding SHALL be the unmodified output of the pinned `compactc`
invocation (same compiler, flags, and vendored contract source revision as the
workspace's other VC bindings), prepended only with a GENERATED marker header.
Regenerating SHALL be byte-reproducible, and the reproducibility gate SHALL
fail if committed generated files drift from regeneration.

#### Scenario: Regeneration is byte-identical

- **WHEN** the codegen workflow is re-run over the vendored submodule at its
  pinned revision
- **THEN** the committed generated family file is unchanged (the check gate
  passes)

#### Scenario: Drift is rejected

- **WHEN** a committed generated family file differs from fresh regeneration
- **THEN** the reproducibility gate fails

### Requirement: Circuits compute (smoke contract)

For every bound family, the crate's test suite SHALL include invariant-style
smoke tests, using deterministic fixtures derived from the family's upstream
testing sources, that exercise at least one representative commitment/root
circuit and assert output determinism, output size, and alteration-sensitivity
(a changed input commitment changes the derived root).

#### Scenario: Claim-root circuit smoke test

- **WHEN** the digital-passport family's claim-root circuit is evaluated twice
  on the same deterministic fixture, and once more on a fixture with one
  commitment altered
- **THEN** the two runs produce identical outputs of the documented byte
  length, and the altered fixture produces a different output

### Requirement: Git-dependency consumption posture

The crate SHALL be `publish = false` and versioned with the workspace while
`compact-runtime` and the `midnight-ledger` crates remain unpublished;
consumers SHALL consume it via a git dependency on this repository. The
publishing blocker SHALL be recorded in the repo's publishing policy document.

#### Scenario: Publishing policy entry

- **WHEN** the crate is added to the workspace
- **THEN** the publishing policy table gains a row naming the crate, its
  blocked status, and the blocker

### Requirement: Additive family onboarding

Adding a further credential family SHALL require only: a new cargo feature, a
new generated module behind it, and a new codegen workflow entry point — with
no modification to any existing family's feature, module, or generated output.

#### Scenario: A second family lands additively

- **WHEN** a second family (e.g. birth) is onboarded
- **THEN** the digital-passport binding's generated file and module surface
  are byte-identical to before the onboarding

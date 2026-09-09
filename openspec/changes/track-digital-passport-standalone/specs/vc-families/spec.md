# Delta: vc-families

## MODIFIED Requirements

### Requirement: Generated-surface fidelity and reproducibility

A family binding SHALL be the unmodified output of the pinned `compactc`
invocation (same compiler and flags as the workspace's other VC bindings) over
the family's **own pinned source**: the standalone credential-family
repository at a recorded release tag, with the generic VC/VP core contract it
builds on taken from a recorded, immutable release of the published core
package and staged per that family repository's own build semantics. The
generated file SHALL record its provenance (family entry point, source tag,
core package version). Regenerating SHALL be byte-reproducible, and the
reproducibility gate SHALL fail if committed generated files drift from
regeneration. The family source SHALL NOT be required to share a vendored
source with the workspace's other VC bindings.

#### Scenario: Regeneration is byte-identical

- **WHEN** the codegen workflow is re-run over the family repository at its
  pinned tag with the pinned core package version
- **THEN** the committed generated family file is unchanged (the check gate
  passes)

#### Scenario: Drift is rejected

- **WHEN** a committed generated family file differs from fresh regeneration
- **THEN** the reproducibility gate fails

#### Scenario: Provenance is recorded

- **WHEN** the family binding is generated
- **THEN** the generated file's header records the family entry point, the
  standalone source tag, and the core package version it was compiled against

#### Scenario: Core integrity is pinned

- **WHEN** the codegen workflow fetches the core contract sources
- **THEN** the fetch is integrity-pinned to the recorded core package version,
  and a fetch that does not match the pin fails the workflow

### Requirement: Circuits compute (smoke contract)

For every bound family, the crate's test suite SHALL include invariant-style
smoke tests, using deterministic fixtures derived from the family's upstream
testing sources, that exercise at least one representative commitment/root
circuit and assert output determinism, output size, and alteration-sensitivity
(a changed input commitment changes the derived root). Where the family
defines a witness-verification circuit, the suite SHALL also assert that a
valid witness passes and corrupted witnesses fail.

#### Scenario: Claim-root circuit smoke test

- **WHEN** the digital-passport family's claim-root circuit is evaluated twice
  on the same deterministic fixture, and once more on a fixture with one
  commitment altered
- **THEN** the two runs produce identical outputs of the documented byte
  length, and the altered fixture produces a different output

#### Scenario: Witness-verified age predicate smoke test

- **WHEN** the digital-passport family's age-predicate witness-verification
  circuit is evaluated with a valid calendar-decomposition witness, then with
  a corrupted quotient field, then with a mismatched day number
- **THEN** the valid witness is accepted, and both corrupted witnesses are
  rejected

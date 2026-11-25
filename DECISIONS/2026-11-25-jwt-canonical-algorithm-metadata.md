# Canonical JWT algorithm metadata

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Supersedes: [JWT algorithm migration](2026-11-25-jwt-algorithm-migration.md).


The missing-ID and raw-root compatibility provisions (where present) are superseded by [Mandatory JWT key identity](2026-11-25-jwt-mandatory-key-identity.md). The remaining security invariants are retained.

## Decision

Each purpose has one active signer. Trusted purpose and exact kid select the
prepared verification key and its algorithm; JOSE alg must match that key.
HS256 and RS256 remain supported within a single keyring model. Algorithm
separation, independent high-entropy HMAC material, revocation, immutable
key identities, and exclusive verification cutoffs remain mandatory.

Every persisted key entry explicitly carries its algorithm, including the
active key and same-algorithm history. Reject missing or null algorithms.
Do not inherit an algorithm from the ring or preserve a previous serialized
representation in the ledger or replica grants. Witness checkpoints
authenticate the exact canonical payload. The SQL schema does not change.

## Rotation

Retain the managed staged promotion protocol of the superseded ADR, except
for its previous-format metadata compatibility: introduce a fresh verifier,
wait for live replica grants to acknowledge it, promote through a higher
durable generation, and retain the outgoing verifier for its outstanding
token lifetime and overlap. Revoked keys must instead be absent; incompatible
grants use the conservative drain. Never promote a formerly active or
pre-enrollment historical key. Unmanaged publication cannot switch the
active algorithm. Returning to a previously used algorithm requires fresh
material, not a retired signer. A symmetric verifier can also sign.

## Pre-release cutover

AGENTS.md §13 rules out the historical homogeneous metadata encoding.
Stop all hosts and operators that use the Auth authority before deployment.
Re-create authority and witness only when the state is confirmed disposable
development data; use fresh signing keys, invalidate outstanding development
tokens and enroll the authority/witness together. Never delete or re-enroll
retained security state merely to make the new decoder accept it. If state
must be preserved, block deployment pending a separately authorized,
authenticated offline transformation retaining every key identity, tombstone,
revocation, revision and authority/witness consistency. No such transformation
is delivered here. Normal staged signer rotation is not a software-format
compatibility bridge.

Missing-kid and raw-root access bridges and other unreleased format cleanup
remain explicitly tracked in the Auth implementation plan; this ADR does not
claim that they have been removed.

## Verification

Source regressions require explicit algorithms, reject absent/null metadata,
check exact algorithm identity and round-trip prepared metadata. Existing
managed rotation, replica, revocation and witness tests remain applicable.
Scoped compilation, tests, ADR verification and independent PostgreSQL rollout
evidence are pending.

# Process-local JWT keyring publication

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Extends: [Immutable prepared JWT snapshots](2026-11-25-jwt-prepared-keyring.md)
- Supersedes: None
- Superseded by: None

## Context

Immutable prepared snapshots protect a single configuration, but the server
clones AuthConfig into long-lived providers. Replacing only a host variable
would leave those providers on obsolete signing and verification material.

## Decision

Auth owns a shared process-local runtime for all four prepared purpose rings.
The host optionally supplies one bounded versioned secret file instead of the
startup cryptographic overrides. Fixed configuration remains supported for
embedders and deployments without the file. Before any host clone is composed,
the file is fully parsed and all purpose rings and host production policy are
validated. The host owns the sole publication capability; transports retain
read-capable AuthConfig clones. The stable MAC root and policy fields are not
part of the rotation payload.

The host polls the complete document every five seconds for its HTTP lifetime.
Preparation and production validation happen before publication. A shared
read/write fence serializes generation and key replacement with synchronous
JWT operations. Each operation observes one complete snapshot. Stale or equal
generations, algorithm changes, adjacent ID/material remapping, cross-purpose
reuse and reactivation of expired retained history fail closed. Malformed or
unavailable candidates retain the last-known-good snapshot. No stale raw
initial signer is retained by the live configuration after replacement.
No request, secret or key material is written to logs. No lock spans async
business IO.

## Sources of truth and ownership

The trusted deployment file is the source for candidate key material and
generation. Auth's validated process-local snapshot is the sole crypto runtime
authority. The host owns file IO, production validation and publication timing;
Auth owns parsing, cryptographic admission and the publication fence.

## Invariants and concurrency boundary

A reader holds exactly one prepared snapshot for a synchronous cryptographic
operation. A writer compares generations and successor identity under the
same lock before replacing the entire value. The lock is not a database
transaction. There is no new persisted state or event projection. Tenant,
channel and locale are not keyring selection inputs; token purpose and existing
signed tenant/issuer/audience claims remain the authority boundaries.

## Failure semantics

Startup fails if the configured source cannot be admitted. Reload failures
retain the admitted process snapshot. The generation floor is not persistent.
No secret payload or raw key is logged.

## Scope and rollout

Use atomic replacement of the trusted mounted file and monotonic generations.
Staging, promotion and retirement are operator-coordinated. The generation
floor and absent-key tombstones are not durable; this decision is not a
multi-replica barrier, a restart anti-rollback guarantee or emergency distributed
revocation. A local crypto fence does not cancel already-authorized async
business transactions. Durable generation admission is extended by the
[durable JWT generation fence](2026-11-25-jwt-generation-fence.md). Replica
convergence evidence remains required before claiming distributed properties.

## Verification

Source regressions cover existing clones observing a new generation, invalid
candidate rollback, stale generations, ID remapping and concurrent publishers.
Run scoped Auth and host checks and exercise file replacement in a live host.
No runtime verification is claimed.

# Process-local JWT keyring publication

- Status: Accepted
- Implementation status: In progress (source implementation; compilation and runtime evidence pending)
- Extends [immutable prepared JWT snapshots](2026-11-25-jwt-prepared-keyring.md).

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

## Scope and rollout

Use atomic replacement of the trusted mounted file and monotonic generations.
Staging, promotion and retirement are operator-coordinated. The generation
floor and absent-key tombstones are not durable; this decision is not a
multi-replica barrier, a restart anti-rollback guarantee or emergency distributed
revocation. A local crypto fence does not cancel already-authorized async
business transactions. Durable generation authority and convergence evidence
are required before claiming those properties.

## Verification

Source regressions cover existing clones observing a new generation, invalid
candidate rollback, stale generations, ID remapping and concurrent publishers.
Run scoped Auth and host checks and exercise file replacement in a live host.
No runtime verification is claimed.

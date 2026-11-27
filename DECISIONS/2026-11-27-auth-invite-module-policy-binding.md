# Invitation static module policy binding

- Date: 2026-11-27
- Decision status: Accepted
- Implementation status: In progress
- Owners: Modules (policy and composition), Auth (invitation ledger), server (host adapter)
- Extends: [Effective module policy](2026-07-22-effective-module-policy-decision.md), [Static module lifecycle revision](2026-08-20-static-module-lifecycle-revision.md), [Role grant revision](2026-11-27-rbac-role-grant-revision.md)

A static effective policy digest can repeat after disable/enable. The owner
lifecycle cursor sequence is monotonic even when that digest repeats. Bind
the tuple of active composition revision, tenant lifecycle cursor sequence,
and current static policy digest in the invite ledger. Hold the owner
composition shared row lock and lifecycle cursor lock until commit.
Read overrides on the same transaction with the canonical static registry
query including manifest co-requisites; no TTL cache or second pool
checkout after transaction admission. Both issue and accept filter persisted
issuer grants using the resulting availability; the original request
ceiling remains independently required at issuance.

This is conservative: a tenant lifecycle transition can invalidate unrelated
invitations in that tenant; a platform composition change can invalidate
invitations platform-wide. No new independent module counter or cache
generation. Composed providers receive the compiled registry explicitly;
uncomposed embedders bind a distinct mode, not an empty static policy.
Composition bootstrap happens before checking out the credential transaction.
Legacy invites without a binding are revoked, preserving receipts and prior
revocations. Stop old binaries before migrating; schema down revokes pending
credentials.

This protects canonical static lifecycle and composition writes only.
It does not claim monotonic history of artifact runtime evidence, maintenance,
channel policies, raw writes bypassing owner cursors, or database rollback.
The static query rejects artifact definitions. PostgreSQL lock ordering,
RLS, multi-pool contention and fresh/incremental migration tests remain
unrun; no readiness promotion.

### Lifecycle cursor continuity integrity

The tenant `module.lifecycle` cursor is durable security authority, not
disposable consumer progress. The Modules migration prevents ordinary DML
from deleting, replacing, moving or resetting that cursor, including SQLite
implicit REPLACE deletes through rowid or alternate UUID representations.
New lifecycle cursors start at zero with no digest; existing legacy
sequence-zero digests are preserved. An advance requires the exact next
integer sequence and a different valid digest. Exact no-op writes remain
legal. PostgreSQL TRUNCATE is rejected for the cursor table because it would
bypass per-row history; other consumers retain normal row mutation semantics.
Install with old writers drained, validate existing rows and reject
inconsistent identity/sequence evidence rather than reconstruct history.
Lifecycle cursors persist even after tenant removal; do not reuse tenant IDs.
The owner cursor remains the source of truth. No new generation table.
This does not couple direct tenant_modules edits to the cursor or prove that
a manually supplied successor digest describes the current policy; the
existing content observation remains necessary. Privileged DDL, restores
and pre-cutover lost history are not covered. Source regressions unrun.

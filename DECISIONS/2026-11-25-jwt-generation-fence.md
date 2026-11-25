# Durable JWT generation fence

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; server host composition
- Extends: [Process-local JWT keyring publication](2026-11-25-jwt-hot-rotation.md)
- Supersedes: None
- Superseded by: None

## Context

A monotonic generation in process memory is lost on restart. An adjacent-key
comparison also cannot detect a key removed in an earlier generation. Both
gaps allow an older configuration or a previously removed credential to return.

## Decision and ownership

Auth owns a database-wide singleton generation fence. The deployment file
remains the source of secret material; the database contains the current
canonical key metadata, historical ID/identity bindings and an authenticated
state envelope, not signing or verification key material. Identity fingerprints
and envelope tags use separate HKDF-derived keys from the stable host MAC root.
RSA reuse identity binds the canonical modulus; exact verification identity
also binds the canonical public exponent, so keeping a modulus cannot mask a
changed verifier.
The envelope binds its generation. The root and the database must remain stable
for this authority. This does not protect against rollback of the database
itself or compromise of the root or database administrator.

The host validates its configuration and production policy before accessing
the fence. After migrations, before exposing any HTTP listener, it admits the
initial file generation. Every subsequent publication first admits the complete
candidate to the database, then publishes to the local runtime. Equal generation
is idempotent only for identical canonical key metadata; lower generations and
same-generation conflicts are rejected. Static mode cannot start after the
database has admitted a versioned keyring. Migration is required even for
static hosts, so an absent fence is not interpreted as an empty authority.

## Invariants and transaction boundary

A single conditional database UPDATE compares the previous generation and
commits the new generation, metadata and authenticated envelope together.
Concurrent writers retry from the actual persisted state; no read/check/write
gap grants authority. Removal is terminal for both the purpose-scoped key ID
and its canonical material identity across all purposes. An active key can
move to verification-only history but never sign again after demotion. New
verification-only staging keys may be promoted once before their cutoff;
history present at initial enrollment is conservatively verification-only
because its previous signing history is unknown. Retained verification
deadlines may shorten, never extend. Access missing-ID migration may only
be retained with the same mapping and a non-increasing deadline, or removed
permanently. Algorithms cannot change through this mechanism. The ledger
bounds total enrolled identities to 8192; exhaustion fails closed rather
than deleting tombstones.

The authority is global to the database, not tenant/channel/locale scoped.
No event projection participates in correctness. There is no transaction
spanning database IO and local crypto; commit precedes local publication.
A crash in that gap recovers by admitting the identical committed generation.
A local publication failure after durable admission terminates the host.
An unknown database outcome during reload terminates the host rather than
continuing with an unproven generation.

## Failure semantics and cutover

Apply the Auth migration before enabling versioned files. Existing static
deployments remain static until the first successful enrollment. All processes
sharing an authority must use the same trusted root and database. Retain the
fence and its backups; a downgrade or restore of the database or running an
older binary bypasses this version of the contract. Stop old writers before
enrollment. No key lifecycle before first enrollment can be reconstructed.
A rejected candidate leaves the current local generation unchanged. File
read/parse failures retain the last locally admitted snapshot. Database
unavailability at startup or during an attempted durable publication fails
closed. No secrets or fingerprints are written to application diagnostics.

## Non-goals

This decision alone is durable publication admission, not a replica readiness
barrier or a per-request check of the shared generation. The subsequent
[cluster lease barrier](2026-11-25-jwt-cluster-lease.md) extends it with bounded
local crypto authority and a conservative generation drain. Distributed emergency
revocation, root rotation, algorithm migration, trusted-clock rollback
protection and cancellation of already-authorized business IO remain separate.

## Alternatives considered

A local checkpoint file cannot serialize independent replicas and has a
deployment-specific persistence boundary. Storing raw key material in the
database expands its secret exposure. A database CAS on authenticated,
bounded metadata preserves the existing secret distribution contract.

## Verification

Source regressions cover same-generation replay/conflict, rollback across
store recreation, permanent tombstones, promotion and retirement, cutoff
and legacy restrictions, root mismatch, corruption/missing-state rejection, exact verifier continuity, and concurrent admission.
Run scoped Auth/host compilation, SQLite regressions, PostgreSQL migration
and independent-pool contention and restart tests before deployment.

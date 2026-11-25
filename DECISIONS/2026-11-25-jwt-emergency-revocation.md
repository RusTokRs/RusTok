# JWT emergency key revocation

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; Auth operator CLI; server host composition
- Extends: [JWT rolling replica admission](2026-11-25-jwt-rolling-admission.md)
- Supersedes: None
- Superseded by: None

## Context

Removing compromised material from a file requires distributing a replacement.
An incident operator needs a durable, irreversible refusal independent of
that distribution, including for stale replicas and verification-only keys.

## Decision

The Auth ledger records an immutable database-clock revocation timestamp on
a previously enrolled purpose/kid binding. No new schema is necessary:
missing timestamps in pre-upgrade authenticated envelopes mean not revoked.
A revocation changes the authenticated ledger tag without changing the
generation or erasing replica grants. All grant/admission/ACK CAS operations
compare that tag. The revocation CAS compares generation and tag only and
returns the lease horizon from the updated row, so unrelated lease-only
writes cannot starve an incident command. A committed revocation prevents new grants for any snapshot
containing the revoked identity and prevents admission of that identity even
under the same generation, another ID or another purpose. The existing
lifetime identity ledger remains authoritative for reuse and removal.
Only enrolled bindings can be revoked; unknown identifiers fail without
modification. A repeated revocation is idempotent and preserves its timestamp.

The supported operator surface is the local trusted Auth CLI
`rustok-cli auth jwt-revoke --purpose access --kid <id> --confirm`.
The process must be pointed at the deployment's authoritative database and
receive the stable host MAC root via `RUSTOK_JWT_REVOCATION_ROOT`, never a
command argument. The command requires an explicit confirmation flag and an
exact purpose (access, password-reset, email-verification, invite). No public
REST or GraphQL endpoint or tenant permission grants deployment-wide
revocation. Output is a content-free receipt with generation, first-revocation
timestamp and conservative lease horizon. The root must authenticate the
existing ledger before mutation. Static generation zero must be enrolled
before this command can target a binding.

## Enforcement and recovery

Lease renewal checks the revocation before the exact-generation predicate,
including for outdated replica snapshots. A replica observing a revoked key
closes the entire local JWT gate under its write fence immediately and
cannot reinstall a lease for that generation. This intentionally stops
unaffected purposes too, rather than risk partial crypto continuity.
No new grant is issued for revoked metadata. An already issued grant can
remain valid until the local ten-second monotonic deadline, subject to the
previous clock assumptions. A concurrent pre-commit grant response is
bounded by the same horizon. Network-isolated replicas cannot renew.
A host polls renewal before reading the file; an unchanged file does not
bypass revocation. Readiness fails when the gate closes.

Recover by distributing a higher complete generation with no revoked
material. If revoked keys are in the current snapshot or an outstanding
grant, recovery uses the conservative drain instead of rolling admission.
The full cluster must be able to verify only non-revoked material before
the new generation is granted. A revoked active signer cannot be retained
as history; revoked historical keys and their legacy mapping must be
removed. A fresh signer or a still-eligible staged signer may replace it.
Previously issued JWTs using revoked keys intentionally become unusable.
Do not clear the ledger or restore an old database to recover availability.

## Limits and deployment

This is bounded emergency enforcement, not instantaneous global cancellation.
The receipt's DB horizon is a conservative observation of existing grants,
not proof that business operations already authorized have stopped.
The [clock discontinuity contract](2026-11-25-jwt-clock-discontinuity.md)
adds bounded detection and dual local deadlines; its independent time-source
assumptions still apply. Root compromise,
standalone unmanaged JWT users, arbitrary clock discontinuity, DB rollback,
and invalidation of already-authorized business transactions are excluded.
Stop older host and CLI binaries before enabling this contract. Older
binaries do not implement revocation and must not be mixed with this release.
All revocations are permanent and consume no extra lifetime identity slots.

## Verification

Source regressions cover durable/idempotent revocation, unknown targets,
wrong root, concurrent renewal and receipt horizon, ambiguous CLI options,
stale and current replica refusal, sticky local fencing,
same-generation rejection, recovery with a new signer, historical-key
revocation, restart and cross-purpose material reuse. Compile and exercise
real independent-pool PostgreSQL/partition races before production.

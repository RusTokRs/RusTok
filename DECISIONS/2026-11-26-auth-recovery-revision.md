# Monotonic account recovery revision

- Date: 2026-11-26
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth; platform user schema
- Extends: [Canonical JWT claims](2026-11-25-jwt-canonical-claims.md)
- Supersedes: None
- Superseded by: None

## Context

An email or status can return to its former value while an old reset or
verification link remains signed and unexpired. Password-hash and mailbox
comparison alone cannot distinguish the earlier account state.

## Decision

Add a nonnegative signed 64-bit recovery revision to the platform users row,
initialized to zero. PostgreSQL and SQLite triggers advance it atomically on
changed email, password hash (including login rehash), status, tenant, or
removal of mailbox verification. Initial verification, profile writes and
exact no-ops do not change it. Overflow fails closed. The SQLite fixture
trigger writes the successor in the same statement transaction; application
issuers use newly inserted or freshly selected user snapshots, not a stale
UPDATE RETURNING revision. The database is trusted; this does not protect
against application-store rollback, row replacement with a reused UUID,
or hostile database administrators.

The Auth server's reset and verification JWT subjects carry canonical
non-nil user ID, decimal revision and normalized email. Reset additionally
retains the password-hash MAC tag. RS256, the purpose rings and existing
MAC root remain unchanged. No legacy subject reader. Confirmation compares
tenant, user, email and revision under the existing user mutation lock and
uses the revision in the conditional write. Only active users may verify
or reset. Repeated verification is idempotent in the same revision.
Successful reset changes the hash and hence advances the revision.

The same revision also governs stored email-change challenges. Issuance
compares the pre-reauthentication snapshot with the locked user and stores
its current revision. Confirmation compares the stored revision with the
locked user, in addition to existing mailbox and password-change timestamp
checks. Legacy pending challenges have no trustworthy issuance revision and
are removed by the Auth migration; a missing revision is never accepted.
The migration also clears pending challenges on schema down rather than
converting them into legacy credentials.

Reset, verification, email change and invite acceptance recheck their
applicable credential deadlines immediately before requesting commit, after
transactional writes and outbox publication. Expiry at that observation
rolls back the entire transition. Authenticated email-change confirmation
also rechecks the locked proving session's lifetime at the transition and
pre-commit boundaries. Invite acceptance checks both signed and persisted
ledger expiry. This is an application observation, not a database commit-time
deadline guarantee or protection against wall-clock rollback.

The revision does not govern direct session or OAuth lifetime, or the
invitation issuer's delegation authority. Existing credential revocation and
email-change request cleanup remain authoritative. No API DTO change.

## Rollout and verification

Stop old Auth binaries, apply the platform and Auth migrations, then deploy the new
reader and issuer together. Outstanding pre-cutover links require a fresh
request. Do not downgrade or restore the application store to bypass the
revision; schema down is for disposable rollback verification only. Existing
users, sessions and OAuth grants are not deleted by migration.

Verify fresh/incremental PostgreSQL and SQLite, no-op writes, mailbox/status
ABA, password rehash, simultaneous confirmation and rollback. Source
regressions are unrun; no deployment readiness is claimed.

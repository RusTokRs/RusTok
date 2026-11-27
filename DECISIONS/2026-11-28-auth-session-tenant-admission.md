# Tenant admission fence for session credential issuance

- Date: 2026-11-28
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth / Server
- Extends: [Proof-bound direct session refresh recovery](./2026-11-26-session-refresh-retry-proof.md), [OAuth browser authorization CSRF boundary](./2026-11-25-oauth-browser-authorization-csrf.md), and [Invite static module policy binding](./2026-11-27-auth-invite-module-policy-binding.md)
- Supersedes: None
- Superseded by: None

## Context

The common access-token resolver checks live tenant existence and activation at
request admission. That check is necessary, but it does not serialize a
credential-issuing transaction with a concurrent tenant deactivation. A request
can pass preflight while the tenant is active and reach its final write after
deactivation has committed.

Session registration, login, and refresh must not issue new session credentials
after tenant deactivation wins. Failed-login lockout writes must also not mutate
a tenant after that fence has been lost. OAuth authorization-code and refresh
issuance, authenticated administration, and invite acceptance already use the
shared active-tenant row fence.

Refresh replay detection has a separate security obligation: losing the
issuance fence must not suppress revocation of a stale refresh-token lineage.
That cleanup is security-reducing and must not issue a replacement credential.

## Decision

Use the canonical persisted tenant row as the final admission fence for session
credential issuance and failed-login state updates.

- Registration, login, refresh, and failed-login lockout mutations acquire the
  active tenant fence inside their final database transaction before taking user/session locks or inserting user/session rows.
- The fence is held until the transaction commits. PostgreSQL uses a shared row
  lock; SQLite uses the existing no-op update lock protocol.
- If the tenant is missing or inactive when the fence is acquired, the
  transaction is rolled back and no session credential or failed-login state
  mutation is committed. The session service maps this condition to the
  non-disclosing not-found response.
- Refresh replay cleanup is independent of issuance admission. If refresh
  preflight was performed while active but the final tenant fence loses to
  deactivation, the issuance transaction is rolled back. A separate
  security-only transaction re-reads the user and exact session under
  user-then-session locks. It revokes a stale lineage outside the retry grace
  window, preserves a lineage inside grace, and never issues tokens.
- The replay cleanup must not trust the preflight session hash as proof that the
  presented token is still current. It must re-read the session after acquiring
  its locks.
- OAuth code/refresh replay cleanup follows the same principle under its
  application/token-family fences: deactivation may deny issuance, but must
  not erase replay evidence or suppress required revocation.

The tenant fence is an issuance boundary, not a global request-cancellation
barrier. Security-reducing logout and revocation operations remain available
without requiring an active tenant.

## Sources of truth and ownership

- \`tenants.is_active\` is the canonical tenant-admission state.
- \`sessions\` and \`session_refresh_history\` are the canonical direct-session
  credential and replay-history records.
- \`oauth_apps\` and \`oauth_tokens\` remain the canonical OAuth application and
  token-family records.
- \`AuthLifecycleService\` owns direct session issuance and refresh replay
  handling. The OAuth token service owns OAuth issuance and replay handling.
  The shared tenant-fence helper owns the backend-specific tenant row lock.
- Cached tenant projections and preflight session snapshots are observations,
  not authority to commit a new credential.

## Invariants

1. No direct session credential is committed after tenant deactivation has
   acquired and committed the tenant row first.
2. The tenant lock is acquired before user and session locks in issuance
   transactions.
3. A refresh transaction that loses tenant admission cannot publish a token.
4. A stale refresh replay outside grace is still durably revoked after tenant
   deactivation, using fresh locked state rather than a stale preflight snapshot.
5. A refresh replay inside grace is not revoked merely because the tenant is
   inactive; the request is denied without recovery or token issuance.
6. Database/clock failures fail closed. They must not fall back to cached
   tenant state or issue credentials.

### Allowed states

- Active tenant: registration, login, and refresh may proceed after acquiring
  the tenant fence and the required user/session locks.
- Inactive or missing tenant: new session credentials are denied.
- Inactive tenant with stale refresh replay: the request is denied and the
  session lineage may be revoked if replay is outside grace.
- Inactive tenant with logout or explicit revocation: security-reducing
  revocation may still commit.

### Forbidden states

- A session is issued from a transaction whose tenant fence was acquired only
  from a stale preflight/cache observation.
- A refresh replay check returns early solely because the preflight snapshot
  showed the presented hash as current.
- A transaction acquires user/session locks and only then attempts the tenant
  admission lock.
- A tenant-inactive refresh path commits replacement access or refresh tokens.

## Non-goals

- This decision does not cancel arbitrary work that was already admitted by
  middleware. Non-credential operations retain their owner-specific transaction
  and effect fences.
- It does not block security-reducing logout or session revocation.
- It does not change password-recovery, email-verification, or email-change
  token semantics; those remain governed by the account recovery revision
  decision.
- It does not add a new database table or migration.

## Data, transaction, and concurrency boundary

The tenant lock and all user/session reads and writes belong to the same
issuance transaction. Where rows are locked, the required order is tenant ->
user -> session; registration inserts user and session rows only after the
tenant fence is held. The transaction retains the tenant lock through commit.

If tenant admission fails during refresh, the original transaction is rolled
back before replay cleanup begins. The cleanup transaction uses user -> session
ordering and is intentionally not an issuance transaction. It commits only
replay revocation state, never replacement credentials.

No new idempotency key or revision is introduced. Existing refresh retry-proof,
history, and expected-session-ID contracts remain authoritative.

## Context dimensions

- Tenant: mandatory; the canonical tenant row is the admission fence.
- Principal: the user and exact session are revalidated under their existing
  row locks.
- Channel, locale, currency, timezone, and trace: not authority inputs for this
  decision.

## Events and projections

No new event family or cache is introduced. Session revocation remains
persisted in the session row and is enforced by existing session validation.
OAuth family revocation remains persisted in the OAuth token rows. Existing
post-commit invalidation and durable reconciliation mechanisms remain
responsible for derived state.

## Failure semantics

- Missing/inactive tenant: deny issuance; return the existing non-disclosing
  tenant-not-found response.
- Tenant-lock, database, or verified-clock failure: fail closed; do not issue.
- Refresh replay within grace: deny because the tenant is inactive, but do not
  revoke the lineage.
- Refresh replay outside grace: commit lineage revocation, then deny issuance.
- Replay-cleanup failure: return an internal error; do not claim successful
  revocation and do not issue credentials.

## Migration and cutover

Not applicable. The decision reuses the existing tenant row and lock helper.
No persisted schema or token format changes.

## Alternatives considered

- **Preflight-only tenant check:** rejected because deactivation can commit
  between preflight and credential issuance.
- **Acquire tenant lock after user/session locks:** rejected because it reverses
  the shared lock order and can deadlock with tenant-scoped administration.
- **Skip replay cleanup when the tenant is inactive:** rejected because tenant
  deactivation must not suppress security-reducing replay revocation.
- **Block logout and explicit revocation too:** rejected because these
  operations reduce access and should remain available during tenant shutdown.

## Verification

Regression coverage is authored for inactive-tenant registration/login,
failed-login lockout writes, current refresh denial, ordinary stale replay
revocation, and stale-preflight replay revalidation after tenant deactivation.
OAuth regression coverage exercises authorization-code and refresh-token replay
after tenant deactivation.

The source-level RBAC evidence contract passes. Full Rust compilation and
runtime tests remain unverified: the workspace check currently stops earlier
in \`crates/libs/rustok-outbox-storage/src/idempotency.rs\`, outside the
\`apps/server\` scope. PostgreSQL runtime evidence is still pending.

## Consequences

Credential issuance is linearized with tenant deactivation, at the cost of a
short tenant-row lock in registration, login, refresh, and failed-login state
transactions. Refresh replay cleanup may open a second transaction after
deactivation wins, but that transaction is restricted to revalidated
security-reducing revocation.

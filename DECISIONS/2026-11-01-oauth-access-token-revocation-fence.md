# OAuth Access-Token Revocation Fence

- Date: 2026-11-01
- Decision status: Accepted
- Implementation status: Implemented
- Owners: platform / auth
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

OAuth access JWTs were previously stateless: once signed, they remained valid
until `exp` even after RFC 7009 revocation, app/consent revocation, credential
change, authorization-code replay, or refresh-token reuse. Refresh lineage and
authorization-code replay evidence were durable, but the access credential itself
was not. Consent revocation also revived the same consent row on re-grant, so a
consent-presence check alone could not distinguish an old epoch from a new grant.

## Decision

1. Every OAuth access JWT carries a `jti` equal to a durable `oauth_tokens` row
   with `token_kind = access`. The row stores the SHA-256 hash of the presented
   JWT, the client app, tenant, grant type, subject, scopes, expiry, family id,
   and (for delegated user tokens) the consent epoch `consent_granted_at`.
2. The host access-token extractor fails closed unless the `jti` row is active,
   hash-bound, client/tenant/grant-scoped, and subject-bound. Direct first-party
   session tokens remain session-fenced and do not require an OAuth access row.
3. RFC 7009 revocation, app revocation, consent revocation, credential change,
   authorization-code replay, and refresh-token reuse revoke the whole token
   family, including access rows. Revocation is idempotent and scoped by client
   and tenant.
4. Delegated refresh validation and access validation compare the stored consent
   epoch with the current active consent. A revoke/re-grant changes the epoch and
   cannot resurrect older credentials.
5. `oauth_tokens.token_kind` discriminates `access` registration rows from
   `refresh` lineage rows. Refresh-only queries (rotation, reuse detection,
   family-root resolution, admin active-token counts) filter on `refresh`.

6. A change to the client's registered redirect URIs, scopes, grant types or
   granted permissions invalidates all of its existing tokens, pending codes
   and active consents in the same app-locked transaction. Equivalent policy
   sets and presentation-only edits preserve credentials. Both manual and
   first-party manifest policy writers enforce this rule.
7. Consent re-grant durably revokes the previous subject's tokens and pending
   codes before publishing the replacement grant, even when the clock repeats
   the previous consent timestamp. Revocation history remains intact.

8. Replay-triggered family revocation is an authenticated client mutation.
   Authorization-code and refresh replay handlers revalidate the current
   active client identity, authentication type, secret hash and admitted
   grant under the same app mutation fence as issuance. A stale client
   snapshot cannot authorize revocation.

9. The refresh lineage's absolute 90-day deadline bounds both newly issued
   access registrations and refresh tokens. Issuance rejects expired prepared
   credentials and rechecks their deadlines under the mutation fence; delegated
   token responses report only remaining whole access-lifetime seconds.
   Newly issued service and delegated access rows store the exact signed
   whole-second JWT expiry. The Auth signer applies the absolute family cap
   before signing and returns the signed expiry with the precise issuance
   sample. The response TTL is calculated after asynchronous usage telemetry.
   Existing rows keep their earlier-of-signed-and-durable validation fence.
10. The duplicate-rotation grace applies only to a consumed refresh row whose
    last-use and revocation timestamps match, with a valid creation/expiry
    interval and nonnegative elapsed time strictly below 30 seconds. An
    ordinary revocation is not rotation evidence. This does not recover a
    lost successful refresh response.

11. Refresh descendants must resolve a root of the same tenant, client,
    subject, authorization-code lineage and consent epoch. The root must
    contain committed rotation evidence and the descendant's scopes must
    be a subset of the root's grant. Under the mutation fence, a family
    with multiple live refresh branches refuses rotation rather than
    arbitrarily selecting a branch.

12. OAuth refresh optionally accepts the secret singleton
    `x-rustok-refresh-retry-key` header. The Auth-owned HKDF/HMAC purpose
    `oauth-refresh-retry` binds tenant, app, family, predecessor UUIDs and
    the 256-bit proof in a fixed 96-byte payload. The successor's digest
    is persisted by the ordinary rotation transaction. Recovery requires
    the same predecessor and proof, exact committed scopes, live successor,
    consistent lineage and consent, current client and subject, and the
    original exclusive 30-second window. It registers fresh access only;
    refresh history and deadlines are unchanged. Non-proof clients retain
    non-recoverable CSPRNG rotations. The header is rejected for other
    grants. Never log proofs or raw credentials. The shared HTTP edge
    marks the proof header sensitive before tracing. A proof-bearing
    request admitted while the predecessor is live or in grace may fail
    after waiting, but does not become a fresh replay revocation.

13. Every OAuth token has one non-nil, immutable family UUID. A fresh
    refresh root is inserted before its access registration. Access-only
    credentials are self-rooted. The database admits at most one unrevoked
    refresh row per family, even when an unrevoked row is already expired.
    Descendants must reference a self-rooted refresh row with the same
    client, tenant, subject, original code, grant and consent epoch.
    PostgreSQL uses NOT NULL, a deferred root foreign key and a write
    guard; SQLite uses an equivalent null/identity write guard and a root
    deletion guard without rebuilding the existing tenant-guarded table.
14. Retention deletes entire families only after the latest member's
    expiry plus the existing 120-day window. Retain code replay evidence
    while any token from that code remains. The cutover materializes NULL
    as self-root only, rejects inconsistent/forked existing families before
    backfill, and never silently discards or reassigns history. Stop old
    hosts before applying the migration. The application has no NULL
    fallback. Downgrade does not undo the truthful identity backfill.

15. Token exchange and access admission use the live Auth credential clock
    at time-sensitive admission and mutation boundaries. Observed backward
    wall steps latch the existing local clock fault; refresh recovery and
    replay classification fail closed if the clock/lease cannot authorize the
    observation. Explicit revocation remains independent of this gate. This is
    not a durable cross-restart or cross-replica time authority.

## Sources of truth and ownership

- Canonical owner: `rustok-auth` owns JWT claims and auth migrations;
  `apps/server` owns OAuth protocol composition and the `oauth_tokens` model.
- Authoritative state: `oauth_tokens` rows, `oauth_consents.granted_at`,
  `oauth_apps.revoked_at`, and `users.password_changed_at`.
- Derived transport: the signed JWT is a bearer projection of the access row;
  it is not independently authoritative for revocation.

## Invariants

### Allowed states

- An access row may be active only while unrevoked, unexpired, and bound to an
  active app/grant/subject/consent epoch.
- A refresh family may contain access and refresh rows sharing `family_id`.

### Forbidden states

- An OAuth access JWT without a matching active access row must be rejected.
- A revoked family must not validate through any sibling access or refresh row.
- A consent re-grant must not validate tokens issued under an older consent epoch.

## Non-goals

- This decision does not change direct first-party session lifetime.
- This decision does not introduce a separate denylist table; the registered
  access row is the revocation fence.

## Data, transaction, and concurrency boundary

- Authorization-code exchange and refresh rotation persist access and refresh
  rows in the same transaction that consumes the one-shot credential.
- Family revocation is a single conditional UPDATE scoped by client/tenant and
  family id; unknown RFC 7009 hashes succeed without revealing existence.
- The additive migration defaults existing rows to `token_kind = refresh`.

## Context dimensions

- Tenant and client app are mandatory predicates on every access-row lookup.
- Locale/channel do not affect credential validity; security lookups do not
  depend on presentation translations.

## Events and projections

- Replay/revocation evidence is the durable token row itself. Domain-event
  publication for code replay remains a separate contract change.

## Failure semantics

- Missing, revoked, expired, mismatched, or database-unavailable access rows fail
  closed with an authentication error. Revocation persistence failures return a
  server error rather than reporting success.

## Migration and cutover

- Add `token_kind` (default `refresh`) and nullable `consent_granted_at` to
  `oauth_tokens`. Existing refresh rows keep their lineage semantics. Access
  JWTs issued before the fence lack a registered `jti` row and are rejected;
  clients must re-authorize or refresh through the registered path.

## Alternatives considered

- A separate denylist table was rejected because it duplicates family/expiry
  state and makes RFC 7009 revoke two stores.
- Shorter TTL alone was rejected because it does not satisfy explicit revocation.

## Verification

- Crate-scoped Rust gates for `rustok-auth` and `rustok-server`.
- SQLite tests for access-row registration, family revocation, consent epoch,
  refresh rotation, and authorization-code replay.
- PostgreSQL/Redis fault injection remains a deployment gate.

## Consequences

- OAuth access validation now performs a durable row lookup. This is the
  intended cost of pre-TTL revocation. Operators must run the additive migration
  before enabling the fence in production.

## Delegated user-administration transaction extension (2026-11-27)

An authenticated scope carries the exact admitted access registration identity,
client/app, digest, original scopes, precise issuance timestamp and consent
epoch. It is not a client-supplied mutation parameter and the digest is omitted
from Debug. GraphQL HTTP and WebSocket initialization must not replace it with
a grant-only scope. A delegated user-admin write acquires PostgreSQL tenant
key-share protection (manifest reconciliation locks tenant before apps), then
the OAuth app mutation fence, ordered user rows and exact access registration,
then the existing invite/module-policy/role fences. SQLite reserves the writer
on the app lock before transactional reads. Validate the live app grant/scopes,
registration subject, identity, digest, epoch, revocation and deadline, and
live consent using the same scope check as the extractor. Filter current grants
through the original scopes; the original request grant ceiling remains
independently required. Keep credential fences through commit; do not reacquire
app locks after users. No raw access JWT is retained in the request scope.

This source slice does not cover service-principal administration, OAuth
app/consent admin mutations, RBAC role-definition writes, artifact permissions,
all tenant activity races or global cross-module deadlock freedom. SQLite
regression authored, compile and live PostgreSQL contention tests unrun.

## OAuth administration owner-transaction extension (2026-11-27)

All six OAuth admin writes now enter the host provider's common authority
decision before invoking the owner service on the same transaction. The
transport guard is preflight only. Direct consent requires real direct-session
evidence, not a grant-only scope or principal-kind label. Service JWTs retain
the same exact registration evidence as delegated JWTs; service subject is the
issuer app rather than a user row. Live service grants are scope- and
module-policy-filtered. For app create/update/rotate/revoke require both the
original request and current settings authority; create/update and rotation
also check the corresponding delegation ceiling. App/tenant-bound prepared
secret material is hashed outside the SQL transaction; rotation preserves CAS.
Response projection and final credential time checks occur before commit.

Acquire tenant FK protection before UUID-ordered issuer and target OAuth app
locks, then actor user and direct session or OAuth registration, then module
policy and role grants. An operation may deliberately revoke its own
credential; the authorization decision is made under the original live
fences before the write, and the original deadline remains exclusive.
This is a source-level implementation, not evidence of global deadlock
freedom. PostgreSQL tenant key-share protects FK ordering, not all changes
to tenant activity; the latter and RBAC/artifact transaction authority still
need cross-module verification. Compile and regression tests remain unrun.

## Typed Auth administration credential context (2026-11-27)

The owner context requires CredentialAuthority with distinct direct-session,
delegated registration, service registration and non-bearer internal variants.
The shared host resolver corroborates tenant/actor and transport identity
with the admitted scope before producing the context. GraphQL and Native
adapters use the trusted AuthPrincipalContext instead of inferring principal
classification again. The RBAC host bridge constructs the same direct-session
context. Exact evidence equality with the currently valid host scope remains
mandatory at the provider boundary; a typed DTO alone confers no authority.
Grant-only contexts fail closed in production. TrustedInternal is explicit,
not a fallback, and accepted only by server unit SQL fixtures; maintenance
uses separate trusted owner APIs. Explicit nested absent scopes mask outer
authority. OAuth registration digest is omitted from Debug. No bearer JWT
is carried in the owner context.

The host still depends on its request scope for provenance, permission ceiling
and invalidation. A fully scope-independent capability and inter-owner
lock verification are separate remaining work. Compilation and tests unrun.

## Credential FK parent admission (2026-11-27)

Migration 000007 adds PostgreSQL BEFORE guards for OAuth application tenant,
credential tenant/app/user and translation app FK references. The token
INSERT additionally retains the non-self family root with KEY SHARE NOWAIT
before the existing lineage validation and deferred root FK. The deferred
FK remains authoritative; its check must not be the first root lock at commit.
Parent contention rejects with SQLSTATE 55P03 rather than waiting from a late
child write. Missing/invisible parents reject; unchanged references and
metadata-only revocation do not acquire new locks. These are FK locks, not
proof of active tenant/app/user, current consent or authorization. The existing
owner mutation and credential clock checks remain necessary. SQLite retains
its serialized writer and existing lineage guards. Deploy the migration with
coordinated writer rollout; PostgreSQL runtime/compile and cross-owner ordering
remain unverified.

SQLite migration 000008 implements the user-delete cascade absent from its
original OAuth token table. The existing insert/update tenant guards cannot
prevent a parent deletion from orphaning lineage. Delete exact tenant/user
descendants before roots in the user deletion transaction, preserving other
users and NULL-subject service credentials. PostgreSQL retains the composite
user FK cascade. Reject existing orphan/cross-tenant lineage at migration;
operator reconciliation is required instead of silently removing history.
SQLite downgrade is denied. Source regression tests remain unrun.

PostgreSQL retention obtains UUID-ordered candidate app locks before deleting
any artifacts and restricts OAuth deletion to these apps. For a candidate
family lock its root and every member nonblockingly, then recheck the full
expiry horizon under Read Committed before deleting descendants and root.
Delete old authorization codes only when no token references them and while
retaining the app fence. Contention aborts the complete sweep rather than
waiting for an earlier owner; retry in a fresh transaction. SQLite retains
serialized deletion. No retention duration or clock contract changes.
PostgreSQL regression is authored, unrun; arbitrary direct SQL, DDL, large
sweep throughput and all cross-owner bulk revocations remain unverified.

User-wide delegated credential revocation and application-wide credential
revocation prelock exact PostgreSQL token/code candidates (and application
consents for the latter) in UUID order, NOWAIT, before the first credential
write. Preserve the respective existing owner fence: never discover an app
lock after a user. Contention rejects the entire caller transaction, with
no partial credential revocation. SQLite uses its serialized writer. The
existing mutation predicates remain authoritative. This covers broad user/app
writers, not every family replay and cross-owner receipt path. PostgreSQL
contention regression is authored, unrun.

## Permanent registration identity (2026-11-27)

Auth migration 000014 records append-only OAuth app/client/token/code UUID
and token/code digest claims on actual row birth. Hard deletion and family
retention cannot release a claimed identity or digest. Token/code hash and
client-ID updates are forbidden; SQLite prevents REPLACE of the unique live
refresh family member even with recursive triggers disabled. Consent re-grant
and managed app reactivation retain their existing contracts. The claim
history is independent of the live token table; no plaintext is stored.
Pre-cutover deletions, terminal timestamp rewrites and privileged DDL/restore
are outside this guarantee. Regression sources unrun.

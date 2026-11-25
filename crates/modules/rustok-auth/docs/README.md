# `rustok-auth` Documentation

`OAuthAppTranslationLifecycle` serializes as `active` or `archived`, matching
its canonical persisted spelling. Host translation snapshots consume this owner
type directly rather than defining another lifecycle enum.

`rustok-auth` is the core authentication module of the platform. It holds JWT lifecycle,
The `AuthLifecycleContext` carries the trusted optional `AuthPrincipalKind` classification; lifecycle self-service operations that depend on a direct user session must reject delegated OAuth and service principals rather than deriving principal kind from transport fields.
credential hashing, refresh/reset/invite/email-verification token flows and
runtime RBAC surface `users:*`.

Auth lifecycle GraphQL (`AuthQuery`, `AuthMutation`) and OAuth GraphQL (`OAuthQuery`,
`OAuthMutation`) are owner-owned by `rustok-auth` behind the `graphql` feature. `apps/server`
only implements `AuthLifecyclePort`, `UserAdminMutationPort`, and `OAuthAdminPort` providers over
the persisted lifecycle/OAuth/email services and registers the corresponding runtimes.
Auth, OAuth and users REST request/response DTOs for login, registration, refresh/logout,
invite/reset, email verification, profile/password, sessions, user list/detail, token,
authorize/consent, browser-session and revocation flows live in `rustok-auth::rest`; the server
controller modules re-export or import those owner DTOs only for OpenAPI/route compatibility.

## Purpose

- keep auth domain logic outside `apps/server`;
- publish the canonical runtime entry type `AuthModule`;
- provide the platform with a unified contract for tokens, claims and credential helpers.

## Scope

- auth configuration, JWT algorithms and host-provided override assembly/validation;
- encode/decode helpers for access/reset/invite/email-verification token flows; JWT encoders reject TTL values that cannot be represented safely as a checked `chrono` duration;
- password hashing, password verification and refresh-token helpers; refresh-token generation uses the OS RNG and propagates RNG failures as a typed `AuthError::RefreshTokenGenerationFailed` instead of panicking;
- auth-owned migrations;
- auth-owned auth/OAuth/users REST DTO/OpenAPI schema surface in `rest.rs`, with host controllers
  limited to transport extraction, persistence adapters and response mapping;
- publication of permission surface `users:*` via `AUTH_USER_PERMISSIONS` and `RusToKModule::permissions()`.
- typed application boundaries `UserAdminMutationPort` and `OAuthAdminPort` for admin commands, OAuth reads and consent lifecycle without module crate dependency on host transport;
- `AuthUserBackfillDbReader` implements a tenant-scoped identity projection with a strict `1..=500` batch bound, checked SQL-limit conversion, deterministic `created_at,id` ordering, and redacted storage/row-read errors for owner-owned profile backfill.
- `AuthUserBootstrapDbWriter` owns tenant-scoped idempotent identity provisioning for installer workflows; email lookup is case-normalized, supported backends are `Postgres`/`Sqlite`, unsupported backends fail closed without panics, and storage/row-decoding failures are exposed only as stable owner errors. Caller-owned transactions remain supported for atomic identity+RBAC installation.
- owner-owned OAuth GraphQL query/mutation/types behind `graphql` feature; `apps/server` only implements the runtime port over the DB and connects roots into the common schema;
- browser OAuth authorization is a host transport flow: cookie-authenticated GET requests are side-effect-free and always render a short-lived HMAC-CSRF confirmation form, even when prior consent exists or the app type does not require consent. Authorization codes are issued only after the CSRF-validated POST; explicit non-ambient Authorization-header requests may retain the pre-authorized fast path.

OAuth persistence is tenant-composite rather than a set of independent foreign
keys. PostgreSQL requires each token, authorization code, and consent to reference
an app and optional/required user in the same tenant; invite-consumption user
links enforce the same rule while retaining their delete-to-null audit semantics.
SQLite uses equivalent
insert/update and parent-tenant-move triggers for local and test profiles. The
forward migration rejects pre-existing cross-tenant relations instead of deleting
security evidence. Server consent reads and revocations include `tenant_id` in the
database predicate as an independent application-layer boundary.

OAuth credential tables deliberately do not use PostgreSQL tenant RLS because the
unauthenticated OAuth protocol must resolve an application from globally unique
`client_id` before a tenant context exists. That exception is limited to OAuth
protocol persistence: tenant-composite database constraints, tenant-qualified
application reads after resolution, active-app checks, grants, scopes, consent,
user state, and session state remain fail-closed boundaries.

The HTTP token endpoint calls the hardened `OAuthTokenService` directly. Authorization
codes and refresh tokens are consumed with compare-and-set updates in the same
transaction that persists a replacement refresh token. The former controller-local
and `OAuthAppService` token issuance paths have been deleted, so router middleware
ordering cannot re-enable a non-transactional replay path.

Invite acceptance and administrative revocation serialize on the same
`auth_invites` issuance row. PostgreSQL/MySQL use a locking read; SQLite acquires
its write fence before rereading the row. A revocation committed first makes
acceptance fail as invalid, an acceptance committed first cannot be retroactively
revoked, and repeated revocation does not emit duplicate lifecycle events. The
unique `auth_invite_consumptions.token_hash` constraint remains the independent
single-use replay fence.

The host additionally composition-gates the OAuth protocol surface and discovery metadata with
`settings.rustok.features.oauth_enabled`; when the gate is closed the routes are not mounted and
OpenAPI omits them. This gate does not replace the per-request active-app, consent, scope, and
credential-epoch checks below.

RFC 7009 revocation is a host transport adapter over a transactional, family-scoped
service operation: the token hash is resolved inside the transaction, `family_id`
coalesces to the row id for pre-lineage rows, and the whole lineage is revoked with
one client/tenant/user-scoped update. Unknown hashes succeed without revealing
existence. Client authentication for revocation uses the security-only active-app
lookup, not presentation translation hydration. OAuth access JWTs are registered
credentials: the signed `jti` names a durable `oauth_tokens` access row, and the
host extractor fails closed unless that row is active and hash/client/tenant/grant/
subject-bound. RFC 7009 and security-family revocation revoke access and refresh
rows together, fencing already-issued access JWTs before their signed TTL.
Delegated rows also capture the consent epoch so revoke/re-grant cannot resurrect
older credentials.

The current inline `oauth_apps.name` and `oauth_apps.description` fields do not yet
satisfy the platform multilingual storage contract. Their translation-table cutover
is an explicit release-plan priority in this module's implementation plan; transports
must not introduce a competing JSON or request-local fallback in the meantime.

## Purpose-separated host MACs

Auth derives distinct 32-byte keys with HKDF-SHA256 for OAuth browser confirmation
and password-reset credential binding. Host adapters use `auth_purpose_mac` rather
than raw shared-secret hashing or local HMAC. The canonical purpose labels
and proof encodings are unversioned: reset credential fingerprints are exactly
64 lowercase hex characters; OAuth consent proofs are a canonical positive
decimal expiry, dot and 64 lowercase hex characters. No prior-format decoder
or key derivation fallback is allowed. Outstanding forms and reset links must
be restarted after the coordinated pre-release host cutover. Persisted
session/refresh digests, JWT key derivation and durable authority MAC domains
are unchanged. See [canonical Auth MAC proofs](../../../../DECISIONS/2026-11-25-canonical-auth-mac-proofs.md).
Source regressions unrun.


## JWT keyring and rotation

Auth uses four independent rings: access (direct and OAuth), password reset,
email verification, and invite. Each has one active key and bounded verification
history. Special JWTs require a purpose-specific `typ` and `kid`; access retains
the existing `JWT` type. The caller selects the ring before inspecting the header. Host startup eagerly
prepares all four rings as one immutable snapshot; clones share both the raw
key inputs and the prepared result. Direct
constructors prepare on first use. Key configuration changes detach the candidate
before mutation; invalid candidates never replace the prior snapshot. Only Auth-emitted JOSE
fields are accepted; unknown fields, duplicate IDs and missing or null IDs
fail closed for every JWT purpose.

Unknown fields in `settings.auth` fail configuration deserialization rather than
silently falling back to JWT defaults. Correct misspelled or obsolete override
names before startup.

Set `settings.auth.jwt_keyring_env` to the environment variable holding access
ring JSON. Access uses the existing `algorithm` and RSA settings. Each RSA
key has one explicit source: inline PEM or its environment reference, never
both. Empty material, invalid environment names and missing environment
values fail closed rather than falling back to another source. Set
`settings.auth.jwt_special_keyrings_env` to a separate environment variable holding
an object with optional `password_reset`, `email_verification` and `invite`
entries. Each entry has `algorithm` (HS256 by default), `keyring`, and, for RS256,
`rsa_private_key_pem` plus `rsa_public_key_pem`. Do not commit secret JSON.

Each ring object accepts:

- `active_kid`: 1–64 ASCII letters, digits, hyphens or underscores.
- `active_hs256_secret`: optional independent signing secret for HS256.
  If omitted, HKDF-SHA256 derives an independent purpose key from the host
  MAC root. The raw root is never a JWT signing or verification key.
- `verification_keys`: up to 16 objects with `kid`, optional `algorithm` (HS256
  or RS256; omission inherits the active ring algorithm), `material` (actual
  HS256 signing secret or RSA public PEM), and exclusive Unix-second `verify_until`.

The unconfigured access ID is `default`; special IDs are `reset-default`,
`verify-default`, and `invite-default`. Example special configuration shape
(use independent high-entropy secret material supplied by the secret manager):

```json
{
  "password_reset": {
    "algorithm": "HS256",
    "keyring": {
      "active_kid": "reset-next",
      "active_hs256_secret": "<independent-secret>"
    }
  }
}
```

RSA keys must parse as actual public/private RSA material with a modulus of at
least 2048 bits. The signing boundary checks that the private key matches the
configured public key; isolation uses canonical RSA moduli, not PEM formatting.
All explicit symmetric material is subject to production secret policy even
with RSA access. Snapshot preparation rejects key material reuse across rings before any key can
be used. Derived
keys isolate disclosure of a derived signing key, not compromise of the
common root; use independent explicit material for stronger isolation.

### Cutover and rotation

All access and special JWTs require an explicit valid key ID. The raw host
MAC root cannot enter any JWT ring, including history. There is no migration
bridge for missing-ID or shared-root tokens. Reissue development tokens and
special links through canonical issuance. The format cutover requires all
hosts and operators to stop and follows the retained-state rules below.
See [mandatory JWT key identity](../../../../DECISIONS/2026-11-25-jwt-mandatory-key-identity.md).

Access JWTs explicitly carry `grant_type` and `scopes`, including direct
session tokens with an empty scope list. Missing or null fields are rejected;
no principal is inferred from absent claims. Direct sessions have no OAuth
client or durable OAuth access-token ID; OAuth access requires both.
Every invite has a mandatory non-nil unique `jti`; missing, null and nil
nonces are rejected. The exact token digest remains the single-use authority.
A baseline-role invite may omit its optional custom-role slug.
Reissue noncanonical development credentials. See the
[canonical JWT claims ADR](../../../../DECISIONS/2026-11-25-jwt-canonical-claims.md).

For subsequent same-algorithm rotations, distribute the next verification
material to all replicas first, then promote it and retain the outgoing
material until its JWTs expire. For derived HS256 keys, the historical
material is the actual lowercase hex-encoded HKDF output (not the root).
Prefer explicit independent secrets for operational rotations. Remove
expired history; never reuse IDs or key material. The exclusive cutoff is
checked both during selection and after signature verification. HS256 verification material
is cryptographically capable of signing; use RSA for distribution to
untrusted verifiers. Startup environment configuration remains supported. An optional
generation-bearing file source supports process-local hot reload (below). Auth request paths reuse
prepared encoding/decoding keys and indexed verification history; the JWT library
still constructs its internal crypto providers per operation.
A source regression now exercises staging, promotion, retirement and exact
key selection using two independent RS256 key pairs, including OAuth access and
missing-ID rejection. Mixed-algorithm verification and managed staged
promotion are also source-implemented; compile and multi-replica execution
evidence remain pending.
See [purpose keyring ADR](../../../../DECISIONS/2026-11-25-jwt-purpose-keyrings.md).

### Managed algorithm migration

For HS256 to RS256, stage a historical verifier with a fresh kid,
`algorithm: "RS256"`, RSA public PEM and a future `verify_until`, while
the active HS256 signer remains unchanged. After all live replica grants
can verify the new key, publish the next generation with the RSA pair
and staged kid active, and retain the old signer in history with
`algorithm: "HS256"`. The old HMAC secret is still sensitive signing material.
Use a cutoff covering the old token TTL plus replica overlap. Retire it in
a later generation. A reverse RS256 to HS256 migration must stage a fresh
independent HMAC secret; demoted or revoked keys never become signers again.
A revoked outgoing key must be removed and the existing drain applies.

The decoder selects a single key by purpose and kid, then requires JOSE
alg to match that key. Historical HMAC secrets remain subject to production
policy even when the active signer is RSA. Do not configure public bytes
as symmetric secrets; PEM-shaped HMAC material is rejected. For a rolling
promotion preserve all accepted verification keys and
deadlines through overlap. An incompatible candidate uses the bounded drain,
not a guaranteed zero-downtime cutover.

Every persisted key entry explicitly records its algorithm, even in a
homogeneous ring. Missing or null algorithm metadata and obsolete ring fields are rejected;
there is no previous-format reader. Stop all Auth hosts and operators for this pre-release
format cutover. Only demonstrably disposable development authority and witness
state may be re-created together, with fresh JWT signing material and
invalidation of outstanding development tokens. Preserve non-disposable state;
do not reset, overwrite or bypass authenticated ledger/checkpoint history.
If such state exists, the cutover is blocked pending an explicit authenticated
offline transformation that retains all identities, revocations and revisions.
Only managed durable generations may change the active algorithm; unmanaged
process-local publication refuses it. Keep the root and witness unchanged
during normal algorithm rotation.
See the [canonical JWT algorithm metadata ADR](../../../../DECISIONS/2026-11-25-jwt-canonical-algorithm-metadata.md).

### Process-local hot reload

Set `RUSTOK_JWT_KEYRING_FILE` to a trusted secret-manager-mounted JSON file.
Do not combine this source with startup JWT algorithm, RSA or keyring overrides.
The file is limited to 256 KiB and requires a generation from 1 through 2^63-1 and a complete
set of four purpose entries. The document has one unversioned format;
obsolete schema tags and unknown fields are rejected. Generation is a
monotonic security-state identifier, not a format version.
See the [canonical keyring document ADR](../../../../DECISIONS/2026-11-25-jwt-canonical-keyring-document.md).
Example shape (replace placeholders with actual
independent high-entropy material or use the documented derived defaults):

```json
{
  "generation": 1,
  "access": {"algorithm": "HS256", "keyring": {"active_kid": "access-1"}},
  "password_reset": {"algorithm": "HS256", "keyring": {"active_kid": "reset-1"}},
  "email_verification": {"algorithm": "HS256", "keyring": {"active_kid": "verify-1"}},
  "invite": {"algorithm": "HS256", "keyring": {"active_kid": "invite-1"}}
}
```

For RS256 entries provide the matching private and public PEM fields alongside
the keyring. Every update is a complete snapshot, not a patch. Replace the
mounted file atomically and increase `generation`. The host checks every two
seconds, validates all four rings and production secret policy before publication,
then replaces the generation and prepared keys together under a write fence.
Existing AuthConfig clones share the live source. Synchronous signing and
verification hold a read fence; no fence is held through database or network IO.
Failed reads, invalid documents, stale generations and incompatible successor
keys retain the last-known-good snapshot. A rejected file revision is retried
when its contents change. Never reuse an ID or key material: durable admission enforces identity
continuity, purpose separation and terminal removal. The root MAC secret and issuer/audience/TTL/password policy are not
rotated here.

### Durable generation admission

Apply the Auth-owned migration before starting the host (including static
configuration mode). The database singleton authenticates canonical key metadata
and lifetime key ID/material bindings with keys derived from the stable host
MAC root. It does not store signing keys, raw HMAC secrets or PEMs. A candidate
must be admitted durably before a local publication, including startup. An
identical canonical generation can restart; older or conflicting generations
cannot. Removing a key ID or material is terminal, even if a later document
renames the material or places it in another purpose. RSA modulus reuse and
exact modulus/exponent continuity are checked separately. An active key moved to
history cannot sign again; newly staged verification keys can be promoted once
before expiry. Initial history is conservatively verification-only; enroll
before staging a future signer. Retained deadlines can only shorten.
The 8192-identity ledger limit
fails closed rather than forgetting retired identities.

The first file enrollment prevents subsequent static-mode startup against
the same database. Keep the MAC root and database authority stable; do not
restore an older database or run an older host binary. Stop old writers before
enrollment. An unavailable database or unknown publication result terminates
the host during admission; a crash after durable commit but before local
publication can recover by restarting with the committed generation. A
rejected candidate retains the local snapshot.

### Cluster JWT lease and drain barrier

All upgraded HTTP hosts (including static mode) require a current database
lease for JWT signing and verification. Renewal confirms the exact durable
generation and canonical metadata. Auth checks a ten-second local monotonic and wall-clock
deadline before and after crypto; the DB records a conservative fifteen-second
maximum lease horizon. The host supervisor runs every two seconds from
bootstrap through serving. For an incompatible generation change the database
closes old renewal for up to 60 seconds and returns Draining until all old
lease horizons have elapsed. The same candidate is retried without requiring
a file edit. Compatible generations can use the rolling path below. A disconnected or
stale host cannot extend its local JWT authority; it can converge after
receiving the committed document. An abandoned drain expires and permits
the still-current generation to resume renewing.

The fallback drain may briefly interrupt JWT operations. It assumes a
single authoritative database primary and independently disciplined host clocks.
Clock guards now detect bounded discontinuities and close the local gate;
see the clock protection contract below. Stop all
older/unmanaged host binaries and apply the Auth migrations before cutover.
A database error terminates the host; static mode exits when its lease is
denied. File-backed stale hosts keep polling with JWT crypto fenced after
expiry. The host's critical `jwt_cluster_lease` readiness check samples the
same local gate after awaited dependency probes, returning 503 when the lease
is unavailable. In-flight business transactions already authorized are not cancelled.
Root rotation and rollback of the independent authority itself remain separate work. Keys removed before first enrollment are
not known to the ledger.

### Rolling replica admission

Every process registers its UUID incarnation and exact prepared verifier
metadata with each lease. After installing a new generation under the
local write fence, the same incarnation ACKs replacement and removes its
older grants. Disconnected or unacknowledged grants remain until their
database-clock deadlines. Reserving a candidate lease is not an ACK.
Add a new verification-only key to a generation retaining the current
signer; distribute it before promoting the new signer in the next document.
Retain the previous signer as verification history for the full remaining
JWT lifetime. Both generations must verify each other's signer for the
entire overlap plus a safety margin, and existing live verification
deadlines must be preserved. A compatible promotion
waits for all outstanding older incompatible grants to be acknowledged
or expire without closing renewal for the current generation (AwaitingReplicas). A new
local lease and the new keys are installed atomically. An incompatible
change or removal of still-valid history uses the existing drain path.
Unchanged waiting files retry automatically.

The registry is bounded to 256 outstanding process/generation grants and
4 MiB. Old anonymous leases from migration must expire before rolling
admission. Delayed file distribution or capacity pressure may fence
individual replicas; this is not a universal zero-downtime guarantee.
Use the trusted primary, disciplined clocks and compatible upgraded hosts.
### Emergency distributed revocation

From the trusted operator environment connected to the same authoritative
database, inject the stable host MAC root via the secret manager as
`RUSTOK_JWT_REVOCATION_ROOT` and execute:

```sh
rustok-cli auth jwt-revoke --purpose access --kid access-1 --confirm
```

Supported purposes are access, password-reset, email-verification and invite.
The target must have been enrolled in the lifetime ledger. Revocation is
permanent, idempotent, and does not require a replacement file. The receipt
reports the first recorded database-clock revocation time and the conservative
outstanding lease horizon (milliseconds since Unix epoch), not an immediate
cluster-wide completion guarantee. The emergency write compares the ledger
tag rather than competing with lease-only updates and returns the horizon
from the updated row. Duplicate CLI options are rejected. Do not pass the
root on the command line.

The authenticated ledger denies new grants and admission containing revoked
material. A replica observing revocation fences its whole local JWT runtime
and readiness, including if its file is unchanged; disconnected replicas
expire their local lease. Existing grants can remain usable for up to ten
seconds from their local grant start under the documented clock assumptions.
Recovery requires a higher complete generation without the revoked key
and a conservative drain of incompatible outstanding
grants. Retain only uncompromised material. Already-authorized business IO
is not cancelled. Static generation zero is not an enrolled identity
authority. Stop older binaries before use; unmanaged embedders, root
compromise and rollback of the independent authority are outside the contract.

Source regressions are authored, unrun.

See [emergency revocation ADR](../../../../DECISIONS/2026-11-25-jwt-emergency-revocation.md),
[durable generation fence ADR](../../../../DECISIONS/2026-11-25-jwt-generation-fence.md),
[cluster lease barrier ADR](../../../../DECISIONS/2026-11-25-jwt-cluster-lease.md)
and [rolling replica admission ADR](../../../../DECISIONS/2026-11-25-jwt-rolling-admission.md).

### Clock discontinuity protection

Managed leases expire when either the ten-second monotonic or wall-clock
deadline elapses. Both clocks are checked before and after crypto. Local
and authority clocks are anchored for their lifetime; divergence over one
second latches a fault. Authority reads must finish within one second and
the sampled database timestamp must be within the host read interval plus
or minus 500 ms. Slow samples are refused; detected clock faults are sticky.
A diagnostic scrape can close the same gate, never renew it. A new generation
cannot clear a fault. Fix independent UTC synchronization and restart only
after the conservative lease window has elapsed on a reliable external clock.
Emergency revocation remains possible during an incident because it never
grants or expires authority; its wall-clock receipt is not elapsed-time proof.

This does not detect coherent failure of all clocks or a freshly started
host with a false time source. Independently synchronize hosts to UTC within
500 ms, keep the primary trusted and stop older binaries. Independent-authority rollback protection remains open. No migration is needed. See the
[clock discontinuity ADR](../../../../DECISIONS/2026-11-25-jwt-clock-discontinuity.md) for assumptions, recovery and
verification. Source regressions are unrun.

A lease that has been observed expired cannot reopen on a backward wall-clock
step. A fresh grant is required. Generation-zero replicas must present identical
canonical key metadata while any registered static grant is live; anonymous
outstanding grants also block static renewal until expiry.

### Independent JWT authority

Production requires an independently administered PostgreSQL database outside
the application backup/restore domain. Set `RUSTOK_JWT_AUTHORITY_DATABASE_URL`
and a stable, non-nil `RUSTOK_JWT_AUTHORITY_ID` UUID through trusted deployment
configuration. Partial configuration is refused. The UUID is authenticated
with the stable host MAC root and verified with the ledger on every authority
read and CAS write; callers must match the pinned root even at generation zero.
No host creates or reseeds the independent store automatically.
Development hosts may use the application DB only if both settings are absent;
that mode has no rollback protection. Revocation CLI always requires the
independent settings.

Cut over offline: apply existing source migrations, stop all older hosts
and CLI writers, wait for every lease and drain to expire, and back up the
trusted source. Create a new empty PostgreSQL database in a separate restore
domain; set the authority settings and `RUSTOK_JWT_REVOCATION_ROOT`, point
the CLI runtime at the trusted source, then run:

```sh
rustok-cli auth jwt-authority-init --confirm
```

Import preserves the authenticated ledger and all grant/horizon fields in
one destination transaction. Existing schemas and active source leases/drains
are refused. Never initialize from an old application snapshot during recovery.
The source remains historical evidence, not a runtime fallback. All hosts
and emergency commands must use the same pinned authority. Provision restricted
runtime credentials, verified TLS and independent backup/failover.

Restoring application data does not roll back JWT generation, key tombstones
or revocations. It does not protect restored sessions, refresh tokens, OAuth
or RBAC, or protect against loss/rollback of the independent witness itself. Static generation
zero has no enrolled key history. Do not resume traffic after loss of current
authority. See the [independent authority ADR](../../../../DECISIONS/2026-11-25-jwt-independent-authority.md).
No new application migration. Compilation and PostgreSQL restore/cutover
evidence remain pending.

### JWT rollback witness

The independent authority now also requires `RUSTOK_JWT_WITNESS_DATABASE_URL`
in a third restore domain. Both external PostgreSQL URIs require an explicit
TCP host/database and `sslmode=verify-full`; only `sslrootcert`, `sslcert`
and `sslkey` may be supplied additionally. Partial configuration is rejected.
The witness stores an HMAC-authenticated exact metadata checkpoint including
a monotonically increasing revision and every grant/horizon. All reads and
all five mutation paths verify it. Mutations lock authority then witness,
commit witness first and return success only after both durable commits.
Mismatch or uncertain write commit fences local JWT; recovery is offline.

For an existing independent authority, stop all older hosts and writers,
wait for all leases and drains to expire, provision an empty separate witness
database and run (with the stable root in `RUSTOK_JWT_REVOCATION_ROOT`):

```sh
rustok-cli auth jwt-witness-init --confirm
```

Fresh deployments must first run the independent authority import above.
Do not reimport an existing authority. After a rollback or interrupted commit,
stop all writers, wait for leases and independently verify the current witness;
then run `rustok-cli auth jwt-witness-recover --confirm`. This copies only the
authenticated checkpoint, preserves grants and refuses to lower the authority
revision. Never reset/reseed the witness or restore it from an older backup.
Restart fresh processes only after successful recovery. A joint rollback of the
authority and witness cannot be detected by this model. See the
[witness ADR](../../../../DECISIONS/2026-11-25-jwt-rollback-witness.md) for durability, failure-domain and runtime
verification requirements.

### Operational observability

The existing host-authorized metrics endpoint exports `rustok_auth_jwt_*`.
`independent_authority` reports whether an external pinned authority is configured.
`witness_configured` and `witness_fenced` report witness configuration and sticky
fault status; `JwtWitnessFenced` is a critical alert.
The Auth owner reads the same authority row and authenticates its ledger;
the read never renews a lease or modifies the database. The host bounds the
diagnostic query to two seconds. Failure exports `collection_status=0`
and NaN for unknown cluster gauges; local metrics remain available.
No key IDs, fingerprints, paths, replica UUIDs, tenant IDs or secrets are exported.

- `local_generation`, `local_ready`, `local_emergency_fenced`,
  `local_state_available` and `local_lease_remaining_seconds` sample the
  actual local crypto gate after DB IO. `local_clock_fenced` and
  `authority_clock_fenced` expose sticky detected clock faults.
- `durable_generation` and signed `generation_lag` compare independent
  database and local samples. Static mode has generation zero.
- `current_grants`, `stale_grants`, `current_grant_replicas` and
  `stale_grant_replicas` count unexpired conservative grants and distinct
  incarnations within each class. An incarnation may be in both classes.
  A grant is not proof of local installation or an online replica.
- `revoked_bindings` counts durable lifetime revocations (a gauge).
  `current_revoked` and `revoked_grants` identify current and outstanding
  material affected by revocation, including independent CLI operations.
- `drain_remaining_seconds`, `untracked_remaining_seconds` and
  `lease_horizon_remaining_seconds` use database time.
- `operations_total{operation,outcome}` has bounded labels for renewal,
  activation, grant denial, admission, ACK and reload. Counters are local,
  reset on restart, and can overlap (e.g. renewal denial and its reason).
- `ack_attempt_duration_seconds_sum/count` measures the immediate
  post-installation ACK attempt, including deferred/error results, not
  cluster-wide convergence. Later renewal cleanup is reflected in the
  outstanding grant gauges.
- `pending_generation` and `pending_seconds` track continuous local waiting
  for the same generation across drain, replica and grant retries. Clear on
  source loss, rejection, unchanged processed file or publication. This
  excludes pre-listener startup waiting.

Scrape each target with the deployment's existing host-read authority.
Use max, not sum, for the replicated database gauges; local gauges and
counters are per target. Sustained negative lag may indicate regression,
but transient negative values may be a sampling race. Monitor missing
targets and scrape failure via up and deployment inventory. Initial
Prometheus rules cover unavailable collection, closed gate, generation
lag, revoked current material and waiting rotation. They are operational
signals, not authorization. No new migration or public API is required.
Compilation, regressions, promtool and independent-replica runtime scrape
evidence remain pending.

## JWT principal and invite admission

Access JWT encoders and decoders enforce the canonical `AuthPrincipalKind`
classification. Direct JWTs require a non-nil session and no OAuth scopes or
registration ID. Delegated and service JWTs require non-nil client and durable
registration IDs, a nil session and an admitted OAuth access grant. The
`refresh_token` exchange still issues an `authorization_code` access grant.
Signature admission does not replace the host's live session or OAuth row
checks. Invite issuance and verification share the same custom-role predicate:
custom roles require the Customer baseline and a nonblank slug of at most 64
bytes. New invitation JWTs carry a random non-nil `jti` so independent issuances
with identical claims and timestamps cannot collide in the unique token-digest
ledger. Previously issued invitations without a `jti` remain admissible subject
to the same persisted exact-digest and one-use checks. Source regressions are
not yet executed.

## JWT and browser confirmation hardening

Both HS256 and RS256 host configuration require at least 32 non-whitespace
bytes of symmetric secret because browser confirmation and credential binding
still use it. JWT validation enforces `iat > 0`, `iat <= now`, `iat < exp`,
and `nbf` when present; checked duration construction rejects out-of-range TTL.
OAuth browser confirmation MAC v4 includes the direct session ID and rejects
expiries beyond its ten-minute window. Source regressions unrun.

## OAuth revocation client fence

RFC 7009 revocation rechecks the authenticated client snapshot under the app
mutation lock before inspecting the token. Concurrent secret or client policy
changes invalidate the preflight authentication; unknown tokens still succeed
for an authenticated client. Source regression unrun.

## OAuth issuance fences

Browser approval and authorization-code issuance share one transaction. The host
locks app, user and direct session in order, rechecks ownership, session liveness,
email policy and access credential expiry, then grants missing consent only for
explicit approval. Code insertion failure rolls back the consent change.

Re-granting consent invalidates pending authorization codes for the same app,
subject and tenant. Exchange rejects codes older than current consent and
rechecks the refresh grant under the app lock before persisting prepared refresh
credentials. OAuth refresh replay grace requires a nonnegative elapsed duration
less than 30 seconds. Source regression definitions are unrun.

## Direct session concurrency

The host uses user-before-session locking for direct refresh and bulk self-service
revocation, matching credential changes. Refresh revalidates the digest and subject
under the locks; historical replay locks the session only and never acquires a user
lock afterward. Bulk self-service revocation requires a still-active current session.
SQLite history-only replay acquires the writer before reading the history.

## Direct session refresh history

The host records each rotated refresh digest and its timestamp in `session_refresh_history`
inside the refresh transaction. The session remains the tenant and revocation authority;
all historical digests are rejected, and replay outside the 30-second per-token grace
revokes a live session. The previous-token columns are a last-rotation projection for
logout with a stale cookie, not the replay authority. History cascades on session deletion.
See [the ADR](../../../../DECISIONS/2026-11-25-session-refresh-history.md).

## GraphQL credential throttling

The Auth-owned rate-limit extension charges each distinct root credential mutation
response key in the selected operation. Different aliases are separate charges;
merged selections of the same field are not. Root fragment traversal is iterative
and each named fragment is visited once per operation, even before schema validation.
The normal GraphQL validator still rejects invalid documents. REST and GraphQL
share an IP-only Auth namespace budget; authenticated password change and logout
also consume it. Tenant, actor and mutation names do not partition the budget.
Backend-failure semantics remain unchanged.

## Responsibility Zone

`rustok-auth` owns authentication, OAuth, credential, token, and user-lifecycle
contracts. Host applications own transport extraction, persistence adapters,
and composition, but must not duplicate auth policy or token semantics.

## Integration

- depends only on `rustok-core` and common libraries, without dependency on `rustok-rbac`;
- used by `apps/server` for REST, GraphQL, session lifecycle and user-management flow;
- `apps/server/src/controllers/auth.rs` remains an HTTP adapter over auth-owned DTOs and lifecycle ports;
- `apps/server` checks registry wiring and GraphQL security hints against `AUTH_USER_PERMISSIONS`, so the host layer does not diverge from the auth-owned permission surface;
- `apps/server` implements ports on top of existing auth lifecycle/OAuth services and registers providers in shared runtime extensions; GraphQL and native `#[server]` adapters must consume one provider per boundary;
- publishes admin pages via `crates/modules/rustok-auth/admin` with `ui_classification = "admin_only"`; the separate [`ui` client capability](../ui/README.md) supplies one shared session boundary to both Leptos hosts without adding a storefront route;
- email delivery and transport wiring remain the responsibility of the host layer and adjacent modules.

## Config Lifecycle Surface

The canonical `AuthConfig` assembly is performed via `build_auth_config` /
`build_auth_config_with_env`: the host passes framework configuration, and
`rustok-auth` applies defaults, `AuthSettingsOverrides`, RS256 env key
resolution and validation. `apps/server` must not duplicate these rules, but
only map `AuthError` to a transport-specific error type.

For RS256, validation parses both PEM values and signs and verifies a probe at
startup. Malformed or mismatched key pairs therefore fail configuration assembly
before the server accepts traffic.

## Token Lifecycle Surface

The canonical set of auth-owned token helpers:

- access tokens: `encode_access_token`, `decode_access_token`;
- OAuth access tokens: `encode_oauth_access_token`;
- password reset tokens: `encode_password_reset_token`, `decode_password_reset_token`;
- email verification tokens: `encode_email_verification_token`, `decode_email_verification_token`;
- invite tokens: `encode_invite_token`, `decode_invite_token`.

The server password-reset consumer accepts only the credential-bound path. The
token subject includes a keyed fingerprint of the password hash present at
issuance, confirmation compare-and-swaps that exact hash, and all sessions are
revoked in the same transaction. There is no unbound reset execution fallback;
replay after the first successful password change fails closed.

Special-purpose tokens contain a strict `purpose` claim, use common JWT validation
`issuer`/`audience` and normalize email-subject to lowercase before issuance.
The host layer (`apps/server`) must publish transport endpoints only through these
helpers, so that invite/reset/verification flows remain auth-owned.

## Runtime Permission Set

The canonical set of permissions owned by the auth module:

- `users:create`
- `users:read`
- `users:update`
- `users:delete`
- `users:list`
- `users:manage`

When adding, removing or renaming permissions, update `AUTH_USER_PERMISSIONS`, `AuthModule::permissions()`, server registry/security tests and this document in a single increment.

## Incident Response

Primary owner for auth/JWT/RBAC incidents — Platform security/auth on-call. Escalation path: owner of `crates/modules/rustok-auth`, then owner of the server API surface.

On auth degradation:

1. Check `/health/ready`, `email_backend` and recent auth/API errors without logging secrets, reset/invite tokens or refresh tokens.
2. Verify effective `AuthConfig`: algorithm/key pairing, issuer, audience, TTL bounds and production policy.
3. If the issue relates to email reset/verification delivery, escalate also to the owner of host email transport.
4. If the issue relates to RBAC, verify `AUTH_USER_PERMISSIONS`, server registry/security hints and actual transport guards.
5. After rollback preserve evidence: artifact id, config snapshot without secrets, affected flows, health snapshot and list of revoked/rotated credentials if rotation was performed.

## Verification

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- targeted server tests for auth/RBAC contracts when changing runtime wiring

## Related Documentation

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Manifest Layer Contract](../../../../docs/modules/manifest.md)

# `rustok-auth` Documentation

`OAuthAppTranslationLifecycle` serializes as `active` or `archived`, matching
its canonical persisted spelling. Host translation snapshots consume this owner
type directly rather than defining another lifecycle enum.

`rustok-auth` is the core authentication module of the platform. It holds JWT lifecycle,
The `AuthLifecycleContext` carries the trusted optional `AuthPrincipalKind` classification; lifecycle self-service operations that depend on a direct user session must reject delegated OAuth and service principals rather than deriving principal kind from transport fields.
credential hashing, refresh/reset/invite/email-verification/email-change token flows and
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

- auth configuration, complete RS256 keyring-document validation, and host policy assembly/validation;
- encode/decode helpers for access/reset/invite/email-verification token flows; self-service email changes use 256-bit random one-time tokens stored only as SHA-256 digests; JWT encoders reject TTL values that cannot be represented safely as a checked `chrono` duration;
- password hashing, password verification and refresh-token helpers; refresh-token generation uses the OS RNG and propagates RNG failures as a typed `AuthError::RefreshTokenGenerationFailed` instead of panicking;
- auth-owned migrations, including tenant-scoped one-time email-change challenges with unique pending mailbox reservations and database-enforced `(tenant_id, user_id)` integrity (composite FK on PostgreSQL; insert/update and parent-move triggers on SQLite);
- auth-owned auth/OAuth/users REST DTO/OpenAPI schema surface in `rest.rs`, with host controllers
  limited to transport extraction, persistence adapters and response mapping;
- publication of permission surface `users:*` via `AUTH_USER_PERMISSIONS` and `RusToKModule::permissions()`.
- typed application boundaries `UserAdminMutationPort` and `OAuthAdminPort` for admin commands, OAuth reads and consent lifecycle without module crate dependency on host transport;
- `AuthUserBackfillDbReader` implements a tenant-scoped identity projection with a strict `1..=500` batch bound, checked SQL-limit conversion, deterministic `created_at,id` ordering, and redacted storage/row-read errors for owner-owned profile backfill.
- `AuthUserBootstrapDbWriter` owns tenant-scoped idempotent identity provisioning for installer workflows; email lookup is case-normalized, supported backends are `Postgres`/`Sqlite`, unsupported backends fail closed without panics, and storage/row-decoding failures are exposed only as stable owner errors. Caller-owned transactions remain supported for atomic identity+RBAC installation.
- owner-owned OAuth GraphQL query/mutation/types behind `graphql` feature; `apps/server` only implements the runtime port over the DB and connects roots into the common schema;
- self-service email change is exposed through `AuthLifecyclePort` and Auth GraphQL; request reauthenticates the direct user with the current password, and challenge lifetime starts after the locked issuance checks and cleanup, and issuance rejects an expired proving session or challenge before commit; confirmation consumes a short-lived SHA-256-digested opaque token before atomically updating the verified address and revoking other sessions/delegated grants;
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

Invite acceptance uses an unlocked ledger lookup only to locate the issuer,
then locks the issuer before the exact invitation and revalidates the digest,
account epoch and revocation under those fences. This avoids the inverse
issuer-delete/FK SET NULL lock order; committed consumption is rejected before
module/RBAC locks, with the unique receipt insert retained as final authority.
SQLite starts with the user write fence. PostgreSQL contention is unverified.

Invite acceptance and administrative revocation serialize on the same
`auth_invites` issuance row. PostgreSQL/MySQL use a locking read; SQLite acquires
its write fence before rereading the row. A revocation committed first makes
acceptance fail as invalid, an acceptance committed first cannot be retroactively
revoked, and repeated revocation does not emit duplicate lifecycle events. The
unique `auth_invite_consumptions.token_hash` constraint remains the independent
single-use replay fence.
Issuance locks and reloads the active issuer, reads persisted grants and
hierarchy, and requires both the authenticated request ceiling and current
persisted authority to allow the same target role before signing. Acceptance
shares the delegation validator. The issuance ledger binds the issuer's
account recovery revision; acceptance rejects missing or changed revisions
even after status, mailbox or password ABA. Stop old binaries before applying
the Auth migration: it revokes unconsumed legacy invitations without backfill,
preserves accepted receipts and prior revocations, and requires new invitations.
Invites also bind the RBAC-owned membership revision under the same user
fence. Membership removal/reassignment ABA cannot revive an older invitation.
The binding migration revokes previously unbound pending invites, preserving
consumption history. The ledger also binds the exact IDs and static grant revisions of every issuer
membership role and the destination role. Those roles are locked before grant
reads; missing or changed bindings deny acceptance. Missing built-in destinations
are initialized by RBAC at issuance without rewriting existing grants. Legacy
unbound invites are revoked at cutover. Invites also bind the static module policy digest, platform composition revision,
and tenant lifecycle cursor sequence under owner transaction locks. Issuance
and acceptance filter current grants using this uncached policy. The tenant-wide
sequence conservatively invalidates invites after unrelated tenant lifecycle
transitions, while composition changes affect all tenants. Artifact runtime
policy, out-of-contract writes bypassing owner cursors, and database rollback
remain outside this guarantee.

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

The executable server accepts exactly one JWT key source: the complete
generation-bearing document named by `RUSTOK_JWT_KEYRING_FILE`. This variable
is mandatory in development and production. Missing files, incomplete purpose
rings, non-RS256 rings, invalid RSA material, mismatched key pairs and invalid
history fail closed. The executable has no HS256/default-key fallback and does
not accept startup RSA, inline PEM or keyring environment overrides.

`settings.auth` is policy-only: `refresh_expiration`, `issuer`,
`audience` and `min_password_length`. Algorithm selectors and key-source
fields are rejected when the file source is selected; they are not silently
ignored. The host MAC root remains separate from JWT signing and verification
material.

The current unversioned document schema requires four entries: `access`,
`password_reset`, `email_verification` and `invite`. Each entry requires
a `keyring`, `rsa_private_key_pem` and `rsa_public_key_pem`; no algorithm
field is accepted. The four RSA moduli must be distinct. Each keyring accepts:

- `active_kid`: 1–64 ASCII letters, digits, hyphens or underscores.
- `verification_keys`: up to 16 objects with `kid`, public RSA PEM
  `material`, and exclusive Unix-second `verify_until`.
- No symmetric key material, algorithm override or fallback field is accepted.
  The independent MAC root is never used or HKDF-derived as JWT key material.

RSA keys must parse as actual public/private RSA material with a modulus of at
least 2048 bits. The signing boundary checks that the private key matches the
configured public key; purpose isolation uses canonical RSA moduli, not PEM
formatting. All four pairs are prepared as one immutable candidate before
admission. The MAC root is separately validated for minimum length and PEM
exclusion; those checks do not prove its entropy or independence.

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

For RS256 rotations, distribute the next public verification material to
all replicas first, then promote its matching private signer and retain the
outgoing public key until its JWTs expire. Verification history is public-only;
never reuse key IDs or RSA moduli across purposes or identities. The exclusive
cutoff is checked both during selection and after signature verification.
The mandatory generation-bearing file is the only executable host source and
supports atomic process-local hot reload. Auth request paths reuse prepared
encoding/decoding keys and indexed verification history; the JWT library still
constructs its internal crypto providers per operation. Unmanaged process-local
publishers share the durable ledger's terminal key-identity and non-increasing
retirement rules for their entire process lifetime. They do not preserve
history across restart or coordinate replicas; executable hosts still require
durable admission.

Focused regressions cover staging, promotion, retirement and exact key
selection with two independent RS256 key pairs, plus OAuth access and missing-ID
rejection. Compile and multi-replica execution evidence remain pending.
See [purpose keyring ADR](../../../../DECISIONS/2026-11-25-jwt-purpose-keyrings.md).

### RS256-only cutover

The accepted [RS256-only JWT architecture](../../../../DECISIONS/2026-11-25-rs256-only-jwt.md)
replaces algorithm migration. The executable host now accepts only complete
generation-bearing documents containing four independent RS256 purpose rings.
The keyring file is mandatory for every server environment; static/default
startup fallback is removed, and every startup candidate passes durable
generation admission before publication. Generation zero is no longer a
runtime mode.

The independent MAC root is not JWT material. It continues to serve host MAC
proofs, OAuth consent CSRF, credential binding and durable authority/witness
domains. HMAC-SHA256, HKDF-SHA256 and Argon2id remain in their non-JWT roles.
The root must be independently provisioned symmetric secret material; PEM
public/private keys and certificates are rejected at JWT preparation, host
MAC proof derivation and durable fence MAC derivation. Production additionally
requires at least 64 non-whitespace bytes and rejects known placeholders and
low-diversity values. These representation checks do not prove entropy or
independence of arbitrarily encoded material. Auth validates this policy
without exposing the root through the public AuthConfig API.

The Auth-owned database-free `validate-jwt-keyring` command uses the same
document parser and policy checks as the host. Full development Compose runs
it in the migration container before applying application migrations. The
browser-smoke and reference-artifact CI host jobs provision fresh isolated
RSA keyrings in runner temporary storage and remove them after use; private
keys are not placed in the repository, logs, artifacts or caches.

The durable authenticated metadata retains its fixed `RS256` assertion only
for state continuity; it is not a selector, and persisted `HS256` metadata is
rejected. Removing that marker requires a separately authorized authenticated
offline converter preserving tombstones, revoked identities, generation and
witness revision. No converter is delivered here. Never reset authority or
witness to bypass admission; block retained-state deployments until conversion
is available. Only explicitly disposable development state may be replaced
together with fresh keys and invalidation of old tokens.
The cutover does not authorize data deletion.

### Isolated development and test provisioning

On POSIX systems with Python 3.9+ and OpenSSL, explicitly create a new
generation-1 file for a **fresh, isolated** development/test security store:

```sh
install -d -m 0700 "$HOME/.local/share/rustok-dev"
python3 scripts/dev/provision-jwt-keyring.py --output "$HOME/.local/share/rustok-dev/jwt.json"
export RUSTOK_JWT_KEYRING_FILE="$HOME/.local/share/rustok-dev/jwt.json"
```

The full development Compose stack mounts the same dedicated directory
read-only into both the migration and server containers. Before any application
migration, the migration container runs `rustok-server validate-jwt-keyring`;
this invokes the Auth-owned parser and cryptographic validation rather than a
shell/Python duplicate. The dev startup script checks ownership and file
permissions before Compose starts and does not generate keys or reset security
state. Direct executable launches also require
`RUSTOK_JWT_KEYRING_FILE`; there is no static fallback.

The provisioning helper creates four independent RSA key pairs with distinct
key IDs and writes a complete file with mode `0600` outside the checkout. It
refuses replacement even if concurrent provisioning targets the same path.
File data and the published directory entry are synchronized before reporting
success; an uncertain directory sync must not trigger key regeneration. The
launcher rejects mounting HOME or a repository ancestor. It never generates or
modifies the independent MAC root. Supply that root separately. The file uses
the currently admitted document shape; no startup generation, production
fallback, or authority/witness reset is introduced.

Do not use this command as a rotation, recovery or conversion procedure for
an enrolled store. Use a different private output path for each isolated
test store. The server executable uses the generation-bearing RS256-only
keyring. No runtime algorithm selector or document-level algorithm field
remains. The fixed `RS256` assertion in authenticated durable authority
metadata is intentionally retained until an offline converter can preserve
tombstones, revoked identities, generation and witness revision; it is not a
runtime selector.

### Process-local hot reload

`RUSTOK_JWT_KEYRING_FILE` is mandatory and must identify a trusted
secret-manager-mounted JSON file. No startup algorithm, inline RSA PEM, keyring or environment-backed key
source is accepted; those fields are unknown and fail closed. Auth validates
the complete document as one candidate. `AuthSettingsOverrides` contains
policy fields only and is applied separately from key preparation. There is
no startup key environment resolver or alternate key source.
The file is limited to 256 KiB and requires a generation from 1 through 2^63-1 and a complete
set of four purpose entries. The document has one unversioned format;
obsolete schema tags and unknown fields are rejected. Generation is a
monotonic security-state identifier, not a format version.
See the [canonical keyring document ADR](../../../../DECISIONS/2026-11-25-jwt-canonical-keyring-document.md).
Example shape (replace every placeholder with a distinct, matching RSA key pair):

```json
{
  "generation": 1,
  "access": {
    "keyring": { "active_kid": "access-1" },
    "rsa_private_key_pem": "<access-private-pem>",
    "rsa_public_key_pem": "<access-public-pem>"
  },
  "password_reset": {
    "keyring": { "active_kid": "reset-1" },
    "rsa_private_key_pem": "<reset-private-pem>",
    "rsa_public_key_pem": "<reset-public-pem>"
  },
  "email_verification": {
    "keyring": { "active_kid": "verify-1" },
    "rsa_private_key_pem": "<verification-private-pem>",
    "rsa_public_key_pem": "<verification-public-pem>"
  },
  "invite": {
    "keyring": { "active_kid": "invite-1" },
    "rsa_private_key_pem": "<invite-private-pem>",
    "rsa_public_key_pem": "<invite-public-pem>"
  }
}
```

 Replace the
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

The Auth-owned fence migration inventory is shared by application schema
installation, the independent authority import and Auth regression fixtures.
The durable fence, authority, witness and observability regressions use fresh
independent RSA pairs per isolated ledger, with stable identity across its
generations and public-only history. Source checks require the fixed RS256
metadata assertion and reject HS256 snapshots. The marker is persisted only
for authenticated-state continuity; it never selects a verifier. Removing it
requires the separately authorized retained-state converter.
These source regressions have not been executed.
The three pending migration identities remain append-only until the
[consolidation admission guide](../../../../docs/guides/safe-database-migrations-and-squashing.md)
is satisfied. Do not infer permission to reset a retained security database
from the pre-release label. The implementation plan records the candidate
range and remaining verification gates.

Apply the Auth-owned migration before starting the host. Generation zero is
reserved exclusively for the empty/uninitialized ledger; no runtime can use
it or acquire a JWT lease at that generation. Every server startup requires
a complete file candidate and durable admission. The database singleton
authenticates canonical key metadata
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

There is no static-mode HTTP runtime: every host uses an admitted positive
generation. Keep the MAC root and database authority stable; do not restore
an older database or run an older host binary. Stop old writers before
enrollment. An unavailable database or unknown publication result terminates
the host during admission; a crash after durable commit but before local
publication can recover by restarting with the committed generation. A
rejected candidate retains the local snapshot.

### Cluster JWT lease and drain barrier

All HTTP hosts require a current database lease for JWT signing and
verification; there is no static mode. Renewal confirms the exact durable
generation and canonical metadata. Auth checks a ten-second local monotonic and wall-clock
deadline before and after crypto; the DB records a conservative fifteen-second
maximum lease horizon. Candidate activation checks the fixed source, immutable local policy,
cluster-managed publisher and monotonic generation before reserving a durable
replica grant; publication rechecks compatibility under its write fence.
The host supervisor runs every two seconds from
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
A database error terminates the host. A missing or denied lease fences JWT
operations; the mandatory file-backed host keeps polling for an admitted
successor while its previous lease is fenced after expiry. The host's critical `jwt_cluster_lease` readiness check samples the
same local gate after awaited dependency probes, returning 503 when the lease
is unavailable. Supervisor shutdown permanently closes the publisher's local
crypto gate; delayed renewals and candidate publication cannot reopen it.
In-flight business transactions already authorized are not cancelled.
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
and a conservative drain of incompatible outstanding grants. Retain only
uncompromised material. Already-authorized business IO is not cancelled.
Generation zero is reserved for the empty ledger and cannot represent an
enrolled runtime or JWT authority. Stop older binaries before use; unmanaged embedders, root
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
step. A fresh durable grant is required. Generation zero is not a runtime
generation and cannot obtain a JWT lease.

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
or RBAC, or protect against loss/rollback of the independent witness itself.
Generation zero represents only the empty ledger and cannot be used as a
runtime. Do not resume traffic after loss of current authority. See the [independent authority ADR](../../../../DECISIONS/2026-11-25-jwt-independent-authority.md).
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
Mismatch or uncertain write commit fences local JWT, including cancellation
of the asynchronous writer after entering the commit phase. The shared fault
latch remains closed until restart after offline recovery.

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
  database and local samples. Every HTTP host reports an admitted positive
  generation; generation zero represents only the empty ledger.
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
ledger. Legacy invitations without a `jti` are rejected and must be reissued
through the canonical issuer. Source regressions are not yet executed.

## JWT and browser confirmation hardening

The independent MAC root requires at least 32 non-whitespace bytes (64 in
production) because browser confirmation and credential binding use it. The
root is not JWT signing or verification material. JWT validation enforces `iat > 0`, `iat <= now`, `iat < exp`,
and `nbf` when present; checked duration construction rejects out-of-range TTL.
OAuth browser confirmation MAC v4 includes the direct session ID and rejects
expiries beyond its ten-minute window. Source regressions unrun.

## OAuth revocation client fence

RFC 7009 revocation rechecks the authenticated client snapshot under the app
mutation lock before inspecting the token. Concurrent secret or client policy
changes invalidate the preflight authentication; unknown tokens still succeed
for an authenticated client. Authorization-code and refresh-token replay
revocation revalidate the active client snapshot and grant under the same
app mutation lock, rejecting stale authentication before changing a family.
Source regressions unrun.

## OAuth issuance fences

Browser approval and authorization-code issuance share one transaction. The host
locks app, user and direct session in order, rechecks ownership, session liveness,
email policy and access credential expiry, then grants missing consent only for
explicit approval. The same locked session and access deadlines are rechecked
before code insertion and before commit; expiry or code insertion failure rolls
back the consent change. Code exchange admits only the interval
`created_at <= now < expires_at`, using one timestamp for the conditional
consumption and its recorded use time. Prepared access/refresh deadlines are
rechecked after insertion before commit. Service-token responses likewise
compute remaining durable access lifetime after commit instead of advertising
the original TTL. The Auth signer returns the precise issuance sample and
exact signed whole-second expiry; both service and delegated access rows store
that expiry, with the original refresh-family deadline applied before signing.
Responses derive remaining whole seconds after usage telemetry, and prepared
credentials dated after the current clock fail closed. Existing access rows
retain their stored expiry; validation still takes the earlier signed/durable
deadline. The live Auth runtime serializes credential wall-clock observations and
latches any observed backward step, even inside the JWT lease's divergence
tolerance. OAuth token admission, refresh replay classification, locked rotation,
recovery and pre-commit/response checks use that clock; access extraction samples
before signature validation and after asynchronous policy reads. The gate also
rechecks the managed lease and emergency/shutdown fences. Fixed embedders retain
their existing behavior. Explicit revocation is not disabled by this gate.
Browser confirmation issuance and verification use the same clock; the
verified MAC expiry is retained as an exclusive deadline through locked
session/consent checks and code persistence. New consent uses the sampled
admission time. The code's own expiry is checked before commit and after its
acknowledgement; no code is published if the gate closes after commit (the
committed grant cannot be rolled back at that point). Non-browser bearer
authorization also uses this gate without gaining consent-grant authority.
Application DB clock corroboration now lives in `rustok_auth::application_clock`.
OAuth code/refresh admission and time-based replay classification sample it
before deciding expiry; code issuance, standalone consent grants and all token
issuance/rotation/recovery writes recheck it on the same transaction before
mutation and commit. Recovery also rechecks the prepared access deadline after
insertion. The common access extractor samples the application DB after policy
resolution before returning authority, and rejects future-dated OAuth access
registrations. Credential timestamps remain host UTC; DB time corroborates
rather than extends them. Publication after commit rechecks the local latch
without another potentially failing DB round trip. Explicit revocations do not
require this clock check.

The application clock latch is shared with retention within one process,
assuming a single trusted application primary; it is independent of the
JWT authority latch. Slow/inconclusive DB observations deny the operation
without asserting a discontinuity; observed faults remain sticky. Committed application admission now persists host and database high-water
marks in the trusted application primary before selected operations. The
independent transaction survives domain rollback; in-domain observations
check but do not advance the durable floor. This is partial cross-restart
protection, not a proof against rollback of the primary or of time observed
only after the last committed admission. See the clock discontinuity ADR.
Direct registration/login/session creation and refresh/recovery now use the
same application DB observer and live credential clock at the mutation and
pre-commit boundaries. Login lockout admission uses checked time; failed
attempt bookkeeping corroborates DB time without requiring a signing lease.
Future session creation or rotation evidence cannot authorize refresh or
trigger a time-based replay revoke. New session timestamps share the checked
host timeline. Direct access signing returns its exact NumericDate deadline,
bounded by the persisted session expiry and absolute cap. Both response
lifetimes are measured after commit, and a closed local gate or elapsed
deadline suppresses publication. Other recovery mutations still require coverage.

Auth retention is now owned by `rustok_auth::retention`. Both the daily sweep
and session-only CLI corroborate fresh application DB wall time with bounded
host samples before any deletion and again before commit. They use one fixed
cutoff derived from the earlier observation minus two seconds; an inconclusive
sample aborts, a detected clock fault is sticky for the process, and any
pre-commit failure rolls back the full sweep. The CLI observes the same 30-day
session horizon as the worker, including cascading refresh evidence, and
rejects dry-run. The worker preserves its existing 120-day OAuth family/code
windows and complete-lineage predicate. The shared application clock guard
is independent of JWT signing lease availability: the CLI does not need signing
material.
The same bounded-clock assumptions apply; a fresh process with coherently
incorrect host and DB time is not protected. No durable time watermark exists.
Source regressions are authored, not executed.

Re-granting consent durably revokes existing access and refresh credentials
and invalidates pending authorization codes for the same app, subject and
tenant. Manual and first-party manifest policy changes invalidate the app's
tokens, pending codes and consents atomically; presentation edits and
equivalent policy sets do not. Restoring a prior policy cannot revive those
credentials. Source regressions remain unrun. Exchange rejects codes older than current consent and
rechecks the refresh grant under the app lock before persisting prepared refresh
credentials. The original 90-day refresh-family cap now bounds newly issued
access and refresh registrations; prepared deadlines are checked under lock,
and delegated token responses report remaining whole access-lifetime seconds.
OAuth refresh replay grace requires matching committed rotation timestamps,
valid creation/expiry ordering and nonnegative elapsed duration less than 30
seconds. Ordinary revocation is not rotation evidence. Source regression
definitions are unrun. Refresh descendants also require committed root
rotation evidence, matching consent/code identities and root-bounded scopes.
A forked family with multiple live refresh branches fails closed under the
mutation lock. Proof-bound OAuth refresh recovery is source-wired at the token endpoint.
Clients may retain the original refresh bearer and an independently generated
secret singleton `x-rustok-refresh-retry-key` (64 lowercase nonzero hex).
Retry the same request with the same scope and proof within 30 seconds.
Only a matching live successor is returned; a fresh access row is registered,
without another refresh row or expiry extension. Wrong proof, changed scope,
revoked successor or elapsed grace fail closed. An admitted recovery that
outlasts grace fails without revoking the lineage. The existing CORS
allowlist covers the header and the HTTP edge marks it sensitive before
tracing for both direct and OAuth refresh. SDK adoption, generated reference
verification, compilation and PostgreSQL
lost-response/concurrency evidence remain pending.

## Canonical OAuth family identity

The token model has a non-optional family UUID. The cutover migration
materializes old NULL self-roots, rejects missing/inconsistent roots or
multiple unrevoked refresh branches, then enforces immutable root identity
and a partial unique index. PostgreSQL uses NOT NULL and a deferred root FK;
SQLite uses equivalent write and root deletion guards. Fresh issuance inserts
the refresh root before its access registration in the same transaction.
Retention removes complete families only after the newest member's retention
deadline and keeps authorization-code replay evidence while linked rows exist.
Stop older hosts before applying the migration. Source regressions and the
registered backfill fixture are unrun; existing corrupt history requires
explicit operator repair, not automatic reassignment.

## Direct session concurrency

The host uses user-before-session locking for direct refresh (including history-only
recovery and replay) and bulk self-service revocation, matching credential changes.
Refresh revalidates the digest and subject under the locks. Bulk self-service
revocation requires a still-active current session. SQLite acquires its writer
at the user fence before transactional reads. Administrative and self-service
password replacements clear the old password's failed-login counter and
temporary lockout without changing account status. Password reset retains
the same lockout-clearing behavior. Administrative credential mutations and
password reset reuse the lifecycle's tenant/user-scoped bulk session
revocation; the shared writer updates both revocation and modification
timestamps and does not rewrite previously revoked rows.

## Account recovery revision

Before dispatching a recovery email, the tracked host task independently
admits application time, revalidates reset/verification JWTs using the live
keyring, or checks the committed email-change deadline. Expired credentials,
clock failures and invalid JWTs skip the sender and count as skipped sends.
Old-mailbox security notifications are not credential-gated. This is a
best-effort pre-sender check, not delivery-time validity or durable email
recovery. Before invoking the email sender, the host re-reads the active account and
checks the JWT's exact recipient, tenant, user and recovery revision. Reset
also checks its password-hash MAC; already-verified accounts skip verification
mail. Email-change dispatch reads the exact tenant/user/target/digest challenge
and its owner in one statement, reuses the confirmation binding, and never
extends the committed expiry. No account locks are held during SMTP; a change
after the read can still make a link stale. Confirmation remains authoritative.
Source regressions are unrun.

All JWT issuers, including reset, verification and invite, sample the live
credential clock before preparing claims and recheck it after RSA signing.
Reject backward time or an elapsed exclusive signed whole-second expiry;
this does not extend the deadline or replace application-clock admission.
No claim or key format changes; regression authored, unrun.

Password-reset and email-verification JWT subjects bind the exact account ID,
mailbox and database-owned monotonic `recovery_revision`. Password reset also
retains the existing password-hash MAC binding. A user-table trigger advances
the revision on changed email, password hash, status, tenant or removal of
mailbox verification; unchanged/profile updates and initial verification do
not advance it. Both issuance and confirmation use the persisted revision;
confirmation rechecks it under the user mutation lock and in its conditional
write. Old subjects without the revision fail closed. Stop older Auth writers,
apply the platform migration and request fresh links at cutover. See
[the recovery revision ADR](../../../../DECISIONS/2026-11-26-auth-recovery-revision.md).
Stored email-change challenges now bind the same revision, including the
pre-reauthentication snapshot comparison at issuance. Confirmation fails
closed on a missing or changed revision. The Auth migration removes legacy
pending requests instead of backfilling a revision they never proved.
Issuance reclaims expired or account-invalid reservations for the requested
mailbox under the previous owner's mutation fence, in a separate transaction
before the requester is locked. It re-reads the exact challenge identity;
live reservations are never removed. Concurrent replacement after preflight
can still cause a unique-index conflict and require retry. Broad expiry
cleanup remains with the retention worker. Reset, verification, email change and invite
acceptance recheck deadlines after transactional side effects and before
requesting commit; email-change confirmation also rechecks the proving
session. Recovery now uses the shared corroborated application clock and
commits an independent admission sample; later checks are not independently
durable and do not guarantee database commit-time deadlines.
Source regressions and PostgreSQL/SQLite migration execution remain pending.

## Direct session refresh history and recovery

The host records each rotated refresh digest and timestamp in
`session_refresh_history` inside the refresh transaction. The session remains
the tenant and revocation authority; replay outside the exclusive 30-second
per-token grace revokes the live lineage. The previous-token columns are a
last-rotation projection for recovery and logout with a stale cookie, not the
replay authority. History cascades on session deletion.

An optional secret `x-rustok-refresh-retry-key` header (one nonzero 256-bit
CSPRNG value in exactly 64 lowercase hex characters) is accepted by raw and
GraphQL HTTP refresh. Cookie REST instead derives the proof from the
Auth-issued HttpOnly envelope and rejects explicit retry headers. Auth uses the independent host
MAC root with the distinct HKDF/HMAC purpose `session-refresh-retry` to derive
one opaque successor per tenant, session and operation key. The host checks
the derived digest against current state and immutable rotation evidence.
Only the immediately previous credential with the matching proof may recover
the current successor during grace; a retry with the already-updated cookie
and the same proof also returns it. Both paths recheck the immutable rotation
time after history lookup, before commit preparation and after commit. An
admitted retry that outlasts grace conflicts without revocation; a new
historical presentation outside grace still revokes the lineage. Refresh cookie
lifetime is recomputed after commit; an already committed rotation may return
an expiry error if commit consumed its remaining lifetime. Recovery never changes rotation history
or extends the refresh deadline; it issues a fresh RS256 access token with
current authority. Missing or incorrect proofs do not allow historical
refresh. Proof reuse cannot resurrect a historical digest. Unrecoverable
duplicates return REST 409 / GraphQL CONFLICT. No proof or plaintext token is
stored, and the key must not be logged.

Clients must generate and retain an independent secret before the request,
reuse its proof only for that attempt, and discard it on resolution. A BFF
may retain an independent CSPRNG login seed in an HttpOnly envelope and
derive a separate generation-bound proof from that seed. The cookie-session
REST owner now issues and derives the proof from its token/seed envelope;
Next Admin relays it unchanged and direct Leptos CSR uses the same cookie
with a bounded origin-wide browser Web Lock and one ambiguous refresh retry.
The shared Leptos native/SSR BFF retains its raw REST proof scheme.
No browser-readable secret storage or new database state is introduced.
Standalone Storefront SSR composition, Next Storefront and mobile integration,
stale client projections and all mounted runtime verification remain pending.
See [the ADR](../../../../DECISIONS/2026-11-26-session-refresh-retry-proof.md).

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

The executable host assembles `AuthConfig` only through
`build_auth_config_from_document`, which consumes the mandatory complete
generation-bearing file. `AuthSettingsOverrides` is policy-only: issuer,
audience, refresh TTL and password policy cannot select or supply JWT keys.
There is no default AuthConfig constructor, algorithm selector, symmetric
JWT signer or alternate key source. Unknown algorithm and key-source fields
fail deserialization. The complete generation-bearing document is the sole
production source of JWT material.

 and signs and verifies a probe at
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
2. Verify effective `AuthConfig`: all four RSA key pairs and purpose isolation, issuer, audience, TTL bounds and production policy.
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

### Leptos client transition ownership (source only)

The shared Auth UI provider holds one browser lock through the full CSR/hydrate
transition and publishes one epoch-tagged user/session pair. Foreign browser
epoch changes invalidate projections, refuse stale cookie intents and suppress
native credential fallback. This is a client consistency fence, not backend
authorization or an expected-lineage server precondition. See [Auth UI](../ui/README.md)
and the [implementation plan](./implementation-plan.md). Mounted browser
verification, noncooperating response ordering and seamless peer refresh remain open.

### Expected session lineage (source only)

The optional singleton `x-rustok-expected-session-id` canonical non-nil UUID
is checked by raw/cookie REST refresh/logout and GraphQL HTTP refresh.
Mismatch is a conflict before mutation, including historical reuse revocation.
Expected logout requires possession of a digest of the exact tenant lineage
(current, previous or retained history), not merely knowledge of the ID.
Leptos retained-session transitions and Next Admin's Auth.js-bound session
forward the precondition. Startup discovery and delayed response ordering
remain separate concerns. See the [recovery ADR](../../../../DECISIONS/2026-11-26-session-refresh-retry-proof.md).

### Next Admin confirmed logout (source only)

The Next Admin host owns signout admission; the Auth.js catch-all never
forwards generic signout. Same-origin POST carries the expected lineage from
the displayed session. The owner confirms matching refresh revocation before
the private Auth.js cleanup helper and Rustok cookie clearing. The shared
client requires explicit success and keeps an ambiguous revocation retryable.
See the [recovery ADR](../../../../DECISIONS/2026-11-26-session-refresh-retry-proof.md).
Mounted Auth.js route/cookie behavior and late or partial response recovery
remain unverified; this is not distributed atomic cookie delivery.

## Witnessed authority clock floor

The existing authenticated rollback-witness checkpoint now includes a
nonnegative authority DB millisecond floor. Checked authority reads
commit its advance before returning; time-sensitive mutations recheck
under both locks, while emergency key revocation preserves the floor
without relying on clock health. The independent clock latch still
fences installed leases. Stop old binaries and drain leases before
first use of the extended checkpoint format. See the clock and witness
ADRs for legacy enrollment, recovery and remaining limits. This does
not persist every application or request-path observation. Source-only
regression evidence; WAL and contention costs remain unmeasured.

## Independently committed application observations

The host and session cleanup CLI configure a dedicated clock writer pool on
the same PostgreSQL primary; late verified observations commit separately
from domain transactions. SQLite optionally uses an offline-bound distinct
persistent file, never another writer to the application file. Unbound
SQLite and unmanaged embedders retain admission-only durability. See the
[clock ADR](../../../../DECISIONS/2026-11-25-jwt-clock-discontinuity.md) for
sidecar provisioning, fail-closed behavior and limits. Exact commit-time
expiry, full request-path JWT and WebSocket observation coverage, clock-store
rollback, throughput and runtime evidence remain open.

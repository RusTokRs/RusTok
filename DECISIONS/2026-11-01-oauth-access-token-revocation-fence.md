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

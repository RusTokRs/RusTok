# rustok-auth

## Purpose

`rustok-auth` owns authentication primitives for RusToK: password hashing, JWT lifecycle,
refresh-token helpers, invite-token helpers, auth config assembly/validation, and auth-related migrations.

The shared Leptos client lifecycle belongs to the Auth-owned [`rustok-auth-ui`](./ui/README.md)
FFA package and framework-neutral session models in `rustok-ui-auth`.

## Responsibilities

- Provide `AuthModule` metadata for the runtime registry.
- Own the shared browser/session FFA boundary in [`ui`](./ui/README.md), used by both hosts, and the admin pages in [`admin`](./admin/README.md).
- Expose auth primitives used by `apps/server` transport adapters and lifecycle services.
- Publish the typed `users:*` RBAC surface through `AUTH_USER_PERMISSIONS` and `RusToKModule::permissions()`.
- Own typed `AuthLifecyclePort`, `UserAdminMutationPort`, and `OAuthAdminPort` boundaries used by GraphQL and native adapters.
- Own the auth lifecycle and OAuth GraphQL query, mutation, input, and output surfaces behind the
  `graphql` feature. Its credential rate-limit extension uses shared `http` header types without a
  direct Axum dependency. It charges distinct credential mutation response keys in the selected
  operation and traverses root fragments without recursion or repeated expansion.
- Enforce tenant-composite integrity for OAuth apps, tokens, authorization codes,
  consents, and their user subjects at both database and query boundaries.
- Self-service email changes require current-password reauthentication, a short-lived
  one-time mailbox challenge, transactional identity/session updates, and a notice to
  the previous mailbox.
- Fail auth configuration fast for malformed or mismatched RS256 keys and keep
  OAuth code exchange and refresh rotation on one transactional execution path.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Used by `apps/server` for REST and GraphQL auth flows, session handling, and user lifecycle.
- Declares permissions via `rustok-core::Permission`.
- `apps/server` enforces those permissions through `RbacService`; `rustok-auth` itself does not depend on `rustok-rbac`.
- Human-readable RBAC ownership for the auth module is `users:*`.
- `apps/server` has registry and GraphQL security contract tests that compare host wiring against `AUTH_USER_PERMISSIONS`.

## OAuth application Translation boundary

OAuth application presentation copy is owner-localized in `oauth_app_translations` and is exposed to the neutral Translation control plane as `auth/oauth_application_copy`.

- Localizable owner copy is exactly `name` plus optional `description`.
- `slug`, `client_id`, secret material, icon/configuration, redirect URIs, scopes, grant types, delegated permissions, consent state, token state, usage timestamps, manifest provenance, and other operational/security fields stay outside Translation.
- Semantic resource and locale revisions hash only exact localized copy. Security/configuration-only mutations therefore do not invalidate Translation proposals or advance the owner copy cursor.
- Revoking/deactivating an OAuth application archives the Translation resource without changing its copy revision.
- Runtime authoring never uses the migration-only `und` locale. Legacy `und` rows remain storage provenance but are not exposed as source or target locales.
- Translation reads and applies retain the existing OAuth administration authority floor: `settings:manage`.
- OAuth application copy is treated as sensitive tenant data and is not eligible for AI export.
- The owner change journal is backfilled by the Auth migration and records create/copy/lifecycle changes for frozen ChangeCursor and aggregate progress evidence.
- `apps/server` owns the host adapter and exact-locale apply service; the Auth crate owns the storage contract, migration, semantic revision vocabulary, and lifecycle mapping.

## Entry points

- `AuthModule`
- `AUTH_USER_PERMISSIONS`
- `UserAdminMutationPort`
- `UserAdminMutationRuntime`
- `OAuthAdminPort`
- `OAuthAdminRuntime`
- `OAuthAppTranslationLifecycle`
- `oauth_app_translation_lifecycle`
- `oauth_app_translation_locale_revision`
- `oauth_app_translation_resource_revision`
- `AuthLifecyclePort`
- `AuthLifecycleRuntime`
- `graphql::{AuthQuery, AuthMutation, OAuthQuery, OAuthMutation}` (feature `graphql`)
- `AuthAdminMutationContext`
- `AuthAdminMutationError`
- `AuthConfig`
- `AuthSettingsOverrides`
- `build_auth_config_from_document`
- `validate_auth_config`
- `Claims`
- `encode_access_token`
- `decode_access_token`
- `encode_password_reset_token`
- `decode_password_reset_token`
- `encode_email_verification_token`
- `decode_email_verification_token`
- `encode_invite_token`
- `decode_invite_token`
- `generate_refresh_token`
- `hash_password`
- `verify_password`

## Docs

- [CRATE_API.md](CRATE_API.md)
- [Module docs](./docs/README.md)
- [Platform docs index](../../../docs/index.md)

## Current worker service contexts (2026-11-02; source only)

Auth now owns the narrow current authenticated read-context port used by the selected
[Inventory Index consumer](../rustok-distribution/docs/inventory-index.md). Its exact tenant/
consumer/correlation/budget request contains no credential, saved claims or event actor. The
server resolves canonical deployment SecretRef credentials and current OAuth service permissions;
it never provisions a service or adds grants. Human transport authentication is unchanged.

### Known Limitations / Pending Implementation

OAuth authorization rechecks the locked direct-session/access deadline after
consent and code writes, and code consumption rejects future-dated and expired
codes using one admission timestamp. Service and delegated token exchanges
recheck prepared expiry before commit. OAuth policy changes and consent re-grants now durably revoke previous
credentials at the source level. Compilation, focused regressions and
PostgreSQL contention/rollback evidence remain pending. OAuth refresh-family
proof-bound recovery is source-wired for the token endpoint; client adoption,
generated references and full OAuth lifecycle verification remain open. Newly issued
credentials are bounded by the original refresh-family deadline, and
rotation requires consistent consumed-root evidence and rejects multiple
live refresh branches. Pre-existing access registrations keep their stored
expiry. Canonical family identity, database uniqueness and family-wide retention are source-wired; migration execution and PostgreSQL verification remain pending.

Direct first-party refresh has source-level proof-bound recovery through the
raw and cookie REST endpoints and GraphQL HTTP transport. The cookie
REST owner issues an HttpOnly token/seed envelope and derives its proof;
Next Admin relays that envelope and direct Leptos CSR coordinates cooperating
tabs with Web Locks. Shared Leptos native/SSR BFF uses the raw proof scheme.
All use bounded ambiguous refresh retry. Standalone Storefront SSR composition,
Next Storefront, mobile/native proof handling, generated API header documentation, compilation
and multi-replica runtime evidence remain pending. See the
[proof-bound refresh ADR](../../../DECISIONS/2026-11-26-session-refresh-retry-proof.md).

JWT purpose keyrings, immutable prepared snapshots, fixed RS256 signing and
verification, exact key selection and retirement are source-implemented.
Mixed HS256/RS256 runtime branches, default key derivation, algorithm selectors
and the transitional keyring-document marker have been removed. Scoped
compilation, regression execution and coordinated replica rollout evidence
remain pending. Optional process-local file-backed hot reload, durable
generation fencing, identity tombstones, cluster lease/drain fencing,
rolling replica admission and emergency enrolled-key revocation are
source-implemented. The shared dev-only fixture generator supplies four
independent RSA pairs to Auth bootstrap, server special-token regressions,
durable fence, authority, witness, observability, local lease and immutable
snapshot rotation scenarios. Explicit isolated dev/test provisioning of four
independent RSA pairs is available through the
[keyring provisioning command](docs/README.md#isolated-development-and-test-provisioning).
File bootstrap constructs the selected complete key source directly without
instantiating default signing inputs. Candidate grant admission rejects local
policy mismatches and unmanaged publishers before durable grant mutation.
Do not reset non-disposable security state. See the
[cutover contract](docs/README.md#rs256-only-cutover).
All JWT purposes require an exact key ID; missing-ID access and raw-root
verification bridges are removed. Access grant type/scopes and invite nonce
are mandatory; previously accepted missing fields are rejected.
The keyring document uses one unversioned format with a bounded monotonic
generation and contains no algorithm field or alternate key source.
Password-reset credential binding and OAuth consent MAC proofs use one
unversioned canonical format. Zero-legacy cleanup remains open for other
unreleased token formats and broader config compatibility. Ambiguous or empty RSA key sources
are rejected.
The independent MAC root rejects PEM key material and weak values at JWT preparation, host-proof and durable fence derivation boundaries. Host root rotation remains open.
Clock discontinuity detection and dual-deadline leases are source-implemented;
independent clock fault-injection and suspend evidence remain pending.
Independent production JWT authority and explicit offline import are source-implemented;
a separately configured rollback witness detects authority-only rollback and
supports exact offline recovery. Joint-store rollback and application session
rollback remain separate. See [JWT rotation](docs/README.md#jwt-keyring-and-rotation).
Operational JWT metrics and initial alerts are source-implemented; see
[operational observability](docs/README.md#operational-observability).
Compilation, metrics regressions and live scrape/alert evidence remain pending.


Context/configuration and revocation-double definitions are uncompiled/unrun. Actual canonical
OAuth/module availability/secret rotation, denied/revoked grants, backend and deployment evidence
remain pending; absent bindings refuse instead of falling back to system/admin authority.
The [existing downstream task record](../rustok-inventory/docs/implementation-plan.md) tracks
recovery/cutover gates. No public transport/UI or new operator capability is exposed here.

The shared Leptos CSR/hydrate provider now source-wires origin-wide transition
admission, foreign epoch invalidation and atomic verified memory publication.
See [Auth UI](ui/README.md); server-side expected-lineage admission is
source-wired for raw/cookie REST and GraphQL HTTP refresh, with mounted browser
and cross-replica verification pending.

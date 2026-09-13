# rustok-auth

## Purpose

`rustok-auth` owns authentication primitives for RusToK: password hashing, JWT lifecycle,
refresh-token helpers, invite-token helpers, auth config assembly/validation, and auth-related migrations.

## Responsibilities

- Provide `AuthModule` metadata for the runtime registry.
- Expose auth primitives used by `apps/server` transport adapters and lifecycle services.
- Publish the typed `users:*` RBAC surface through `AUTH_USER_PERMISSIONS` and `RusToKModule::permissions()`.
- Own typed `AuthLifecyclePort`, `UserAdminMutationPort`, and `OAuthAdminPort` boundaries used by GraphQL and native adapters.
- Own the auth lifecycle and OAuth GraphQL query, mutation, input, and output surfaces behind the `graphql` feature.
- Enforce tenant-composite integrity for OAuth apps, tokens, authorization codes,
  consents, and their user subjects at both database and query boundaries.
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
- `build_auth_config`
- `build_auth_config_with_env`
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

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)

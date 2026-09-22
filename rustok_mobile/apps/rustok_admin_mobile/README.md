# RusTok Admin Mobile

Host Flutter admin application for RusTok mobile surfaces.

## Purpose

- Provides a mobile host shell for module-owned admin mobile packages.
- Keeps routing, auth/session bootstrap, locale/tenant context, and transport wiring at host level.
- Consumes backend UI contracts from `apps/server` (`/api/graphql`, `/api/graphql/ws`) rather than package-local data access conventions.

## Responsibilities

- Compose module entries from generated mobile manifest.
- Provide shared GraphQL client bootstrapping (including auth refresh bootstrap flow).
- Enforce host-level route/query and locale context contracts for module-owned UI.

## Interactions

- Depends on shared workspace packages in `rustok_mobile/packages/*`.
- Reads module metadata generated from `crates/*/rustok-module.toml`.
- Integrates with platform backend contracts documented in `docs/UI/*`.

## Entry points

- App bootstrap: `lib/main.dart`
- Auth + GraphQL bootstrap providers: `lib/app_shell/auth_bootstrap.dart`
- Route composition: `lib/routes/app_router.dart`

## Documentation

- Platform docs map: `docs/index.md`
- UI contract overview: `docs/UI/README.md`
- Flutter architecture research baseline: `docs/research/flutter.md`

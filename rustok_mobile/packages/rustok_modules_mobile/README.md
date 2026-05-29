# rustok_modules_mobile

Module-owned Flutter pilot package for the RusTok modules registry surface.

## Purpose

This package provides the first Phase 1 mobile pilot flow from
`docs/research/flutter.md`: a GraphQL-backed module registry list that is
mounted by the `rustok_admin_mobile` host shell.

## Responsibilities

- Own the mobile presentation for the modules registry pilot screen.
- Keep module data access behind `ModulesRepository` so the host supplies the
  shared GraphQL client and auth/tenant/locale context.
- Reuse the platform `moduleRegistry` GraphQL contract without introducing a
  Flutter-only API.
- Expose retryable loading/error/empty states for host-level E2E evidence.

## Interactions

- `apps/rustok_admin_mobile` overrides `modulesRepositoryProvider` with
  `GraphQlModulesRepository` built from the host `graphQlClientProvider`.
- The host resolves module detail navigation through the generated mobile
  manifest routes.
- This package does not create its own auth, tenant, locale, or GraphQL client
  fallback chain.

## Entry points

- `ModulesMobileScreen` — registry list UI.
- `ModulesRepository` — data boundary for tests and host injection.
- `GraphQlModulesRepository` — `moduleRegistry` GraphQL implementation.
- `modulesControllerProvider` — Riverpod async controller.

## Documentation

See the Flutter implementation plan in
[`docs/research/flutter.md`](../../../docs/research/flutter.md).

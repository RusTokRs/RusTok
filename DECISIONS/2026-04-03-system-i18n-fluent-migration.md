# Fluent migration path for system i18n bundles

- Date: 2026-04-03
- Status: Accepted

## Context

RusToK already has request-level locale negotiation and locale-aware domain read paths, but system strings still rely heavily on match-table translations in `rustok-core`. This is workable for a small catalog of messages, yet it scales poorly for pluralization, grammatical variants and tenant-aware expansion of system UX.

At the same time, HTTP runtime locale selection had to be stabilized immediately without waiting for a full translation engine migration.

## Decision

- Keep `RequestContext` as the canonical HTTP locale source of truth now.
- Standardize the HTTP locale chain as:
  - `query`
  - `x-medusa-locale`
  - locale cookie
  - `Accept-Language` with q-values
  - `tenant.default_locale`
  - `en`
- Constrain the effective locale through `tenant_locales` when tenant-specific locale policy exists.
- Emit `Content-Language` on locale-aware HTTP responses.
- Treat migration from `rustok_core::i18n` match tables to Fluent bundles as a separate architectural phase rather than bundling it into request-runtime hardening.
- First Fluent migration slice will cover auth, validation and system error messages.

## Consequences

- Runtime locale behavior becomes deterministic immediately, without blocking on a larger translation-system rewrite.
- The current match-table implementation remains temporarily acceptable for existing messages, but it is no longer the target architecture.
- CI and documentation must eventually add bundle completeness checks once the first Fluent message groups land.
- Module-owned translations can continue to evolve independently as long as they consume the canonical locale contract from `RequestContext`.

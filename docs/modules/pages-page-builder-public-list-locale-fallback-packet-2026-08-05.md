# Pages / Page Builder Public List Locale Fallback Packet

Date: 2026-08-05
Status: source-ready / execution-pending

## Problem

The public selected-page read and the public page list did not use the same locale chain.

The selected-page path already resolved:

```text
requested locale
  → tenant default locale
  → platform fallback locale
```

The list path resolved only:

```text
requested locale
  → platform fallback locale
```

For a tenant whose default locale is `ru`, a request in `fr` could therefore render the selected page in Russian while the list beside it returned empty title/slug fields or a different platform-locale translation.

## Retained source sequence

The owner now exposes:

```text
PageService::list_public_visible_with_locale_fallback
  → normalize requested locale
  → normalize explicit tenant fallback locale
  → retain published-only and channel visibility filters
  → resolve_translation_record(requested, tenant fallback)
  → platform fallback remains the final candidate
```

The legacy `list_public_visible` method remains and delegates with no explicit fallback. Existing callers that intentionally rely on platform fallback keep their behavior.

## Native storefront parity

The registered native storefront endpoint already derives both `requested_locale` and `fallback_locale` before the cache lookup. Its composite cache variant already binds both values.

After this slice:

```text
selected detail
  → get_by_slug_with_locale_fallback(requested, tenant default)

public list
  → list_public_visible_with_locale_fallback(requested, tenant default)
```

Channel/module admission, generation lookup, cache hit, owner reads, immutable artifact verification and best-effort cache fill retain their existing order.

## GraphQL public parity

The public GraphQL `pageBySlug` path already used `TenantContext.default_locale`.

The public `pages` helper now passes the same tenant default locale to the fallback-aware owner list. Authenticated/admin list behavior remains unchanged.

## Focused regression

`crates/rustok-pages/tests/page_locale_fallback.rs` retains:

```text
requested locale: fr
tenant default locale: ru
published page translations: ru only

public list
  → one item
  → Russian title
  → Russian slug
```

The focused test is:

- `public_list_respects_explicit_tenant_fallback_locale`.

The selected detail and public list now resolve the same translation for this scenario.

## Source evidence

- `crates/rustok-pages/contracts/evidence/pages-public-list-locale-fallback-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-public-list-locale-fallback.mjs`;
- `crates/rustok-pages/tests/page_locale_fallback.rs`;
- `crates/rustok-pages/src/services/page/read.rs`;
- `crates/rustok-pages/src/graphql/query.rs`;
- `crates/rustok-pages/storefront/src/transport/native_server_adapter.rs`.

## Boundaries

This slice changes the production Pages public list resolution behavior in the owner service, registered native storefront and unauthenticated GraphQL list.

It does not:

- change Page Builder or Fly behavior;
- change page translations, bodies, artifacts or bindings;
- change migrations, database schema, DTOs or GraphQL schema;
- change public routes;
- change cache namespaces, generation scopes, key shape, TTL or capacity;
- change channel visibility or module admission policy;
- change event delivery or optional external event infrastructure;
- add redirects, canonical URL policy or route aliases;
- claim tests, verifiers, Cargo, formatting, SQLite, native server-function, GraphQL, browser, workflow or CI execution;
- promote FFA or FBA.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-public-list-locale-fallback.mjs
cargo test -p rustok-pages --test page_locale_fallback -- --nocapture

node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs
```

Execution evidence remains pending.

# FORUM-24L localized category route identity owner

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24L introduces one transport-neutral owner for localized Forum category route identity:

```text
/{locale}/forum/c/{slug}
```

The route uses the locale-aware slug already stored with the selected category translation. Category UUIDs remain internal identity and are not emitted in the canonical path. Category hierarchy is deliberately not encoded in the URL: moving a category does not change this route.

Machine contract:

```text
crates/rustok-forum/contracts/forum-category-route-identity-owner.json
```

## Persistence contract

The existing `forum_category_translations` relation already enforces one category slug per tenant and locale through:

```text
idx_forum_category_translations_tenant_locale_slug
```

The route owner reuses that invariant. This slice adds no migration, route table, alias table, copied category identity or denormalized path column.

## Owner API

`ForumCategoryRouteService` exposes two operations:

```rust
canonical_descriptor(tenant_id, category_id, requested_locale, fallback_locale)
resolve(tenant_id, requested_locale, requested_slug, fallback_locale)
```

`canonical_descriptor` resolves one active category translation through the shared Forum locale precedence and emits the normalized path.

`resolve` performs bounded reverse lookup by the normalized category slug and applies this order:

1. requested locale;
2. explicit fallback locale;
3. platform fallback locale `en`;
4. first available locale only when every remaining active candidate belongs to the same category identity.

A noncanonical locale or slug resolves to `REDIRECT` and the owner-provided canonical descriptor. An exact canonical path resolves to `CANONICAL`.

## Ambiguity and lifecycle policy

Reverse lookup reads at most 64 matching translation rows. Persistence normally permits only one row per tenant, locale and slug, but the owner still fails closed if an exact locale contains more than one candidate or if residual first-available candidates belong to different categories.

An archived category is not a route candidate. If the exact requested locale and slug belongs to an archived category, resolution returns `FORUM_CATEGORY_ROUTE_NOT_FOUND`; it does not fall through to another category that happens to own the same readable slug in a fallback locale. This prevents lifecycle state from silently changing route identity.

The owner returns:

- `FORUM_CATEGORY_ROUTE_NOT_FOUND` for missing, archived or malformed public route identity;
- `FORUM_CATEGORY_ROUTE_RESOLUTION_CONFLICT` for inconsistent persistence or cross-category ambiguity.

## Authorization boundary

Route identity is not storefront authorization. This owner does not evaluate category audience inheritance, channel membership, module enablement or SEO publication eligibility.

FORUM-24M extends this owner with immutable localized slug history. FORUM-24N consumes current and historical decisions through additive GraphQL and native storefront transports and rechecks the canonical category through `ForumCategoryAudienceReadService` before disclosure. Host route mounting and SEO remain separate work.

Hierarchy moves need no route alias under this flat contract because parent identity is absent from the URL.

## Compatibility

FORUM-24L itself changes no existing category command, lifecycle command, GraphQL schema, REST endpoint, storefront route, topic route, semantic event, admin workflow, SEO metadata or hreflang output. FORUM-24M/N extend the owner with aliases and visibility-safe transport capability without changing the flat route shape or mounting a host URL.

The accepted Forum slug/locale decision remains authoritative: category slugs are locale-aware translation fields, and route lookup follows the shared locale fallback contract.

## Verification handoff

No tests, verifiers, formatting, Cargo commands, migrations, workflows, HTTP scenarios, browser scenarios or CI were executed while preparing this slice.

Maintainers can run:

```bash
node scripts/verify/verify-forum-category-route-identity-owner.mjs
cargo test -p rustok-forum services::category_route::tests -- --nocapture
cargo test -p rustok-forum --test category_route_identity_contract -- --nocapture
cargo test -p rustok-forum --test category_route_identity_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

## Remaining FORUM-24 scope after FORUM-24N

- Rust storefront category route mount and category-link cutover;
- private no-store redirect/not-found HTTP policy;
- canonical and hreflang document policy;
- Forum-specific SEO composition and matching schema.org semantics;
- Next storefront parity;
- maintainer SQLite, PostgreSQL, HTTP and browser evidence.

The canonical implementation plan remains the single roadmap. Its FORUM-24 ledger entry is not updated by these slices because the connected complete-file writer cannot safely retrieve and replace the full plan losslessly; this document records only the stable FORUM-24L contract and does not create a second backlog.

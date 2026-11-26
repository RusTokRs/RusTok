# rustok-taxonomy

## Purpose

`rustok-taxonomy` owns the scope-aware taxonomy dictionary for RusToK.

See [CRATE_API.md](CRATE_API.md) for the verified public crate API specification and platform capability bindings.

## Responsibilities

- Provide tenant-scoped taxonomy terms that can be either shared across modules or limited to one module.
- Keep canonical term identity separate from localized names and slugs.
- Own taxonomy storage (`taxonomy_terms`, `taxonomy_term_translations`,
  `taxonomy_term_aliases`, `taxonomy_translation_changes`) and migrations.
- Expose CRUD/list/lookup services for shared and module-local taxonomy terms.
- Provide transaction-aware helpers for domain modules that need to resolve or create module-local terms inside their own write transactions.
- Reuse the platform multilingual locale/fallback contract so blog/forum/pages-style locale handling stays consistent.
- Publish `TaxonomyTranslationTargetProvider` as the registered `taxonomy/term`
  exact-locale provider. It owns snapshots, field policy, validation,
  revision/CAS apply, exact progress, and an append-only owner change cursor.
- Use the generic Outbox receipt ledger under owner slug `taxonomy` for
  Translation-target apply while retaining Taxonomy authorization, validation,
  and business-write ownership.
- Mutations of global Tags publish a tenant-scoped `ReindexRequested` search event
  inside the same transaction, so downstream projections such as Blog Search cannot
  retain stale names or slugs after a shared Taxonomy Tag changes.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Uses `rustok-api::locale` for normalization and fallback. Events and owner-operation
  receipts use `rustok-outbox-storage` in the Taxonomy transaction.
- Depends on the Core Outbox receipt primitive but not on the Translation
  control-plane crate; Translation reaches Taxonomy only through
  `rustok-translation-targets`.
- Already backs forum topic tags through forum-owned `forum_topic_tags`.
- Already backs blog tags through blog-owned `blog_post_tags`.
- Already backs product tags through product-owned `product_tags`.
- Already backs profile tags through profile-owned `profile_tags`.
- Is the shared vocabulary layer for `blog`, `forum`, `product`, `profiles`, and future modules
  while leaving entity-term relation tables module-owned.

The `rustok-taxonomy-ports` package publishes the tenant-scoped name read port
used by Product for catalog projections. The in-process provider uses the
canonical Taxonomy locale resolution.

## Standalone term-name service

Run on an externally migrated PostgreSQL database with access to the Taxonomy
term and translation tables.

```sh
cargo run -p rustok-taxonomy --features service --bin rustok-taxonomy-service
```

Set `RUSTOK_TAXONOMY_DATABASE_URL`, `RUSTOK_TAXONOMY_TERM_NAMES_GRPC_BEARER_TOKEN`
and `RUSTOK_TAXONOMY_TERM_NAMES_GRPC_TENANTS` (one to 128 distinct tenant UUIDs).
Set `RUSTOK_TAXONOMY_SERVICE_TLS_CERT_PATH` and
`RUSTOK_TAXONOMY_SERVICE_TLS_KEY_PATH` together. The default bind is
`127.0.0.1:7446` and `RUSTOK_TAXONOMY_SERVICE_BIND` overrides it. For local
testing, `RUSTOK_TAXONOMY_SERVICE_ALLOW_INSECURE_LOOPBACK=true` permits HTTP
on an IP loopback address. The credential grants only tenant-scoped term names.
Product selects the service in its standalone catalog configuration.
[Product service configuration](../rustok-product/README.md#standalone-service-configuration)

## Entry points

- `TaxonomyModule`
- `TaxonomyService`
- `TaxonomyTranslationTargetProvider`
- `dto::*`
- `entities::*`
- `migrations::*`

See also `docs/README.md`.

### Transaction-preserving Forum host adapter

The joint Forum+Taxonomy server distribution now registers an in-process adapter
at the composition root. Taxonomy does not depend on Forum. The adapter delegates
sync/owned alias history, hierarchy mutations and tag writes to owner APIs inside
the supplied transaction. `resolve_term_route_for_module_in` also reads route and
alias provenance on that exact connection; the strict module parent reader rejects
missing/wrong-scope Categories or missing placements rather than inventing roots.
See [CRATE_API.md](CRATE_API.md#transactional-module-route-and-strict-hierarchy-readers).

### Known Limitations / Pending Implementation

The new strict-reader regression and server real-owner adapter tests are authored,
not compiled or executed in the current environment (Cargo absent). Full migration,
PostgreSQL contention and mounted Forum/SEO/Search/native paths remain acceptance
checks. The adapter is shared-database/in-process only; it supplies no remote
provider or distributed transaction protocol. Track this follow-up in Forum's
[implementation plan](../rustok-forum/docs/implementation-plan.md).

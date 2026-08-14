# Taxonomy route-registry recovery

## Purpose

Use this runbook when localized Taxonomy content and `taxonomy_term_route_keys`
disagree. The registry is the only route-ownership authority for public and
owner-module lookup. Translation and alias rows are localized content; they are
not a second lookup authority.

This procedure is intentionally conservative. Diagnose with read-only queries,
repair through the Taxonomy owner/service mutation path, and let the same
transaction reconcile route reservations. Do **not** insert, update, or delete
`taxonomy_term_route_keys` directly in production.

## Identity tuple

Always reason about the complete route identity:

`tenant_id + kind + scope_type + scope_value + locale + route_key`

Never repair from a slug alone. Module and global scopes, tenants, and locales
are independent namespaces.

## Incident classes

### Missing reservation

A row in `taxonomy_term_translations.slug` or `taxonomy_term_aliases.slug`
exists for a term, but the matching registry tuple is absent. Public route
lookup may return no result even though localized content is present.

### Cross-term collision

Localized content for term A wants a route tuple currently owned in
`taxonomy_term_route_keys` by term B. Do not choose a winner from table order,
creation time, translation-versus-alias precedence, or operator preference.
The conflicting owner decision must be resolved at the owning domain level.

### Stale reservation

A registry row exists for a localized slug/alias that is no longer desired by
its owning term. A normal localized Taxonomy mutation reconciles desired keys
before releasing stale reservations in the same transaction.

## Read-only diagnosis

Run diagnostics against the same database/tenant that serves the affected
request. The retained psql script is:

`crates/rustok-taxonomy/docs/sql/route-registry-drift.sql`

Invoke it with an explicit tenant UUID:

```bash
psql "$DATABASE_URL" --set=ON_ERROR_STOP=1 \
  --set=tenant_id='00000000-0000-0000-0000-000000000000' \
  --file=crates/rustok-taxonomy/docs/sql/route-registry-drift.sql
```

The diagnostic is one read-only `WITH ... SELECT` statement. It normalizes
same-term translation/alias representations into one desired route before
comparing them with the registry, then separately scans registry rows that have
no matching localized route for their recorded owner. Every returned row is
classified as one of:

- `missing_reservation` — localized content wants the tuple but no registry row
  exists;
- `cross_term_collision` — the tuple exists but belongs to another term;
- `stale_reservation` — a registry-only tuple is no longer represented by the
  recorded owner's localized translation or aliases;
- `consistent` — localized content and registry owner agree.

This second, registry-driven pass is required: a diagnostic that starts only
from translation/alias rows cannot see a stale registry-only reservation.
`scripts/verify/verify-taxonomy-route-registry-recovery.mjs` pins that property,
requires tenant scoping, and rejects mutation SQL in the retained diagnostic.

For one reported route, also resolve it through the normal Taxonomy service.
The service result is the user-visible authority; raw translation/alias rows do
not override a different registry owner.

## Repair a missing or stale reservation

1. Record the complete identity tuple, affected `term_id`, current localized
   translation, aliases, and current registry rows.
2. Confirm there is no different registry owner for the intended tuple.
3. Re-save the intended localized Taxonomy state through the normal
   `TaxonomyService::update_term`/owner mutation path. Supplying the current
   normalized slug and aliases is acceptable when the goal is reconciliation;
   the mutation is auditable and advances the normal Taxonomy revision/change
   evidence.
4. The service transaction calls localized route reconciliation before durable
   change evidence is committed. Missing reservations are inserted and stale
   reservations are released atomically with the localized mutation.
5. Re-run the read-only diagnostic and the normal route lookup. The registry
   owner must now be the intended `term_id` and the public route must resolve to
   it.

`tests/route_key_registry.rs::owner_service_update_repairs_missing_route_reservation`
keeps the missing-reservation recovery path executable. For a stale reservation,
re-save the authoritative current translation/alias set for that locale; route
reconciliation releases registry keys absent from that desired set.

## Repair a cross-term collision

Do **not** retry by deleting term B's registry row. The database uniqueness key
is protecting a real route-identity conflict.

1. Identify both terms and their owning module/domain usage.
2. Inspect the localized translation/alias intent for both terms and the owner
   attachment records that reference them.
3. Decide the correct domain outcome: rename/remove one localized alias/slug,
   merge domain usage onto one term where the owning module supports that, or
   deliberately move owner attachments through that module's API/migration.
4. Apply the domain decision through owner-module and Taxonomy service APIs.
5. Re-run the diagnosis query and public lookup.

A repair attempt for the losing term must fail rather than steal the key;
`tests/route_key_registry.rs::owner_service_repair_refuses_cross_term_route_collision`
proves that fail-closed behavior.

## Owner attachments and categories

Taxonomy does not own generic attachment or category hierarchy storage. When a
term identity is replaced or consolidated, inspect and repair attachments in
the owning module:

- Blog: `blog_post_tags`
- Forum: `forum_topic_tags`
- Product: `product_tags`
- Profiles: `profile_tags`

Use those modules' APIs/migrations for attachment changes. Do not create a
Taxonomy `owner_type/owner_id` table to centralize the repair.

Category parent/child relations remain domain-owned as well. Blog and Forum own
their category hierarchy/translation tables; Product owns its category
hierarchy and closure storage. Route-registry recovery never moves category
hierarchy into Taxonomy and never introduces a Taxonomy `parent_id`.

## Hard delete

A Taxonomy term has no soft-deprecated lifecycle. `delete_term` is a hard
identity removal. The term foreign-key cascade releases its route reservations;
a later replacement may claim the same route identity. Do not reconstruct a
deleted term by inserting registry rows. Create or reuse the intended current
term through the normal owner/service contract and update owner attachments in
their owning module.

## Verification

For repository changes that affect this procedure, run:

```text
node scripts/verify/verify-taxonomy-route-registry-recovery.mjs
cargo test -p rustok-taxonomy --test route_key_registry
cargo test -p rustok-taxonomy --test localized_route_lookup
node scripts/verify/verify-taxonomy-ownership-boundary-self-test.mjs
node scripts/verify/verify-taxonomy-ownership-boundary.mjs
```

The path-filtered `Taxonomy Lookup Contract` workflow runs the read-only
recovery source guard before both Rust integration binaries, independently of
unrelated workspace CI failures. PostgreSQL route contention and
translation-target evidence remain covered by `Taxonomy PostgreSQL Evidence`.

# Explicit Immutable Artifact Binding Replacement

Date: 2026-08-07  
Status: `source-ready / maintainer-validation-pending`

## Purpose

Pages can now retain a verified append-only rebuild candidate without changing public output. This slice adds the separate explicit activation command that can replace one exact localized published binding with that rebuilt artifact.

The command is intentionally narrower than publish or rollback. It activates one rebuild receipt for one locale only.

## Input authority

The command is scoped to one exact:

```text
tenant
page
page_artifact_rebuild_operations.id
expected page version
expected current artifact id
idempotency key
```

It requires tenant-wide `pages:manage` (`PermissionScope::All`). Owner-scoped Manage is insufficient.

The expected current artifact must be the source artifact recorded by the selected rebuild receipt. The live locale binding must still point to that exact artifact when the owner transaction acquires its lock.

## Replacement verification

Before changing the binding, Pages verifies:

- the selected rebuild receipt has valid tenant, page, locale, source and replacement identities;
- the rebuild receipt uses the operation-bound `rebuild:<operation-id>` instance key;
- the current locale binding still points to the expected source artifact;
- the replacement row belongs to the same tenant, page and locale;
- its instance key, artifact hash and materialization hash match the rebuild receipt;
- the complete stored static artifact and materialization evidence pass the existing Page Builder integrity verifier inside `bind_existing_body_in_tx`.

The damaged source payload is not required to pass full artifact integrity. Requiring that would make recovery from a detected corruption impossible. Its exact binding identity remains fenced by both the caller and rebuild receipt.

## Atomic activation

The owner transaction:

1. locks the page;
2. resolves exact idempotent replay;
3. checks the expected page version and published state;
4. locks the selected rebuild receipt and current locale binding;
5. verifies the replacement artifact;
6. updates one `page_published_landing_artifacts` row;
7. advances the page version once and keeps the page published;
8. writes `NodeUpdated` and `NodePublished` to the transactional event bus;
9. stores one immutable binding-replacement receipt;
10. commits.

Cache generations are not mutated inline. Existing Pages lifecycle handling observes committed events and owns subsequent route/page/artifact generation effects.

## Receipt and replay

`page_artifact_binding_replacement_operations` retains:

- tenant, page, rebuild operation, body and locale identities;
- normalized idempotency key and deterministic request hash;
- expected page version and previous artifact id;
- replacement artifact id, artifact hash and materialization hash;
- result page version and timestamps.

An exact replay returns the stored result without changing the binding, version or events again. Reusing the idempotency key for another request is rejected. A rebuild operation may receive only one activation receipt, preventing a later unrelated lifecycle state from silently reusing the same repair decision.

## Preserved boundaries

This command does not:

- sanitize or compile a Page Builder project;
- create another rebuilt artifact;
- read the mutable current body;
- update or delete the damaged source artifact;
- mutate the rebuilt artifact;
- replace more than one locale binding;
- publish an unpublished page;
- select a rebuild automatically from audit findings;
- add GraphQL, HTTP, OpenAPI, admin UI or worker transport;
- promote FFA or FBA.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-rebuild.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance.mjs
cargo test -p rustok-pages --test explicit_artifact_binding_replacement_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```

SQLite/PostgreSQL migration, tenant-wide authorization, stale version/current-binding rejection, replacement corruption rejection, lifecycle/cache observation and exact replay evidence remain pending.

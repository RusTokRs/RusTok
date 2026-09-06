# Explicit Immutable Artifact Binding Replacement

Date: 2026-08-07  
Status: `source-ready / postgres-harness-source-ready / maintainer-validation-pending`

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

It requires tenant-wide `pages:manage` (`PermissionScope::All`). Under the current request permission bridge effective Pages Manage resolves to `All` when present and `None` when absent; there is no current Pages Manage `Own` request state. The owner service still requires `PermissionScope::All` before input validation or writes as defense in depth.

The expected current artifact must be the source artifact recorded by the selected rebuild receipt. The live locale binding must still point to that exact artifact when the owner transaction acquires its lock.

## Replacement verification

Before changing the binding, Pages verifies:

- the selected rebuild receipt has valid tenant, page, locale, source and replacement identities;
- the retained provenance source still has a valid deterministic provenance hash;
- the rebuild receipt matches that provenance source across publish operation, tenant, page, body, locale, source artifact, review, artifact and materialization identities;
- the rebuild receipt uses the operation-bound `rebuild:<operation-id>` instance key;
- the current locale binding belongs to the retained provenance body and still points to the expected source artifact;
- the replacement row belongs to the same tenant, page and locale;
- its instance key, artifact hash and materialization hash match the rebuild receipt;
- the complete stored static artifact and materialization evidence pass the existing Page Builder integrity verifier inside `bind_existing_body_in_tx`.

The damaged source payload is not required to pass full artifact integrity. Requiring that would make recovery from a detected corruption impossible. Its exact binding identity remains fenced by the caller, retained provenance, rebuild receipt and locked binding.

## Atomic activation

The owner transaction:

1. locks the page;
2. resolves exact idempotent replay;
3. checks the expected page version and published state;
4. locks and validates the selected rebuild receipt and retained provenance source;
5. locks the current locale binding;
6. verifies the replacement artifact;
7. updates one `page_published_landing_artifacts` row;
8. advances the page version once and keeps the page published;
9. writes `NodeUpdated` and `NodePublished` to the transactional event bus;
10. stores one immutable binding-replacement receipt;
11. commits.

Cache generations are not mutated inline. Existing Pages lifecycle handling observes committed events and owns subsequent route/page/artifact generation effects.

## Receipt and replay

`page_artifact_binding_replacement_operations` retains:

- tenant, page, rebuild operation, body and locale identities;
- normalized idempotency key and deterministic request hash;
- expected page version and previous artifact id;
- replacement artifact id, artifact hash and materialization hash;
- result page version and timestamps.

An exact replay returns the stored result without changing the binding, version or events again. Reusing the idempotency key for another request is rejected. A rebuild operation may receive only one activation receipt, preventing a later unrelated lifecycle state from silently reusing the same repair decision.

## PostgreSQL source packet

Marker:

```text
explicit-artifact-repair-postgres-harness-source-ready
```

`crates/rustok-pages/tests/explicit_artifact_repair_postgres.rs` applies the real Outbox and Pages migrations in an isolated PostgreSQL schema and executes activation through this owner command after a reviewed publish and append-only rebuild.

When executed, the packet requires:

- stale expected-current-artifact rejection to leave the binding unchanged and create no activation receipt;
- successful activation to switch exactly the retained locale binding to the selected rebuilt artifact;
- exactly one page-version increment while retaining `published` state;
- both damaged source and rebuilt artifact rows to remain present;
- one durable `NodeUpdated` and one durable `NodePublished` envelope for the activated page;
- exact replay to return the same operation/version without another receipt or page-version increment;
- another idempotency key targeting an already-consumed rebuild receipt to be rejected;
- the migration-owned unique `(tenant, page, rebuild_operation_id)` constraint to reject a second activation receipt;
- a preceding page-version marker and outbox event in that failed PostgreSQL transaction to disappear after rollback.

Cache handler execution remains a separate cursor: this packet retains durable lifecycle envelopes but does not claim generation rotation until those committed envelopes are processed by the existing Pages lifecycle handler.

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
- promote FFA or FBA.

Bounded GraphQL/HTTP/OpenAPI transports now expose activation separately from rebuild. No transport combines audit, rebuild and activation into an automatic action.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-rebuild.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance.mjs
cargo test -p rustok-pages --test explicit_artifact_binding_replacement_sqlite -- --nocapture
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test explicit_artifact_repair_postgres -- --nocapture
cargo check -p rustok-pages --all-targets
```

SQLite execution plus PostgreSQL execution, Manage present/absent authorization, stale-version, provenance-mismatch, invalid replacement, unpublished-page and cache-generation evidence remain pending. The PostgreSQL stale-current/success/replay/reuse/lifecycle/receipt-rollback packet is source-ready but unvalidated.

# Explicit Immutable Artifact Rebuild

Date: 2026-08-06  
Status: `source-ready / maintainer-validation-pending`

## Purpose

Pages can already audit retained immutable Page Builder artifacts and new reviewed publications retain one immutable sanitized source row per locale. This slice adds the first explicit rebuild command over that provenance.

The command creates a new immutable artifact row only. It does not decide that the new row should become public.

## Input authority

The command is scoped to one exact:

```text
tenant
page
page_publish_rebuild_sources.id
expected provenance hash
idempotency key
reviewed runtime context
```

It requires tenant-wide `pages:manage` (`PermissionScope::All`). Owner-scoped Manage is insufficient.

The mutable current Pages body is never read. The retained sanitized project is re-sanitized through the canonical Page Builder policy and its sanitized hash and provenance hash are recomputed before compilation.

## Runtime reauthorization

The historical complete runtime context is deliberately not persisted as rebuild authority.

The caller must submit a fresh `ReviewedPagePublishRuntimeInput`. The command requires:

- exact reviewed `review_hash` equality;
- exact runtime scenario equality;
- exact runtime-context hash equality with the retained materialization identity;
- successful canonical runtime materialization;
- exact source, artifact and materialization hash reproduction;
- byte-for-byte equality of materialization identity and runtime snapshots.

A source, runtime or renderer drift is a rejection, not a best-effort repair.

## Append-only storage

`page_static_landing_artifacts` now carries an internal `instance_key`:

```text
canonical
rebuild:<rebuild-operation-uuid>
```

Existing rows and ordinary reviewed publication use `canonical`. The original deterministic build/materialization identity remains unchanged. A rebuild uses a unique operation-bound storage instance so the same deterministic artifact can be retained as a second immutable row without overwriting or deleting the damaged row.

The forward-only migration replaces the old five-column unique key with the same identity plus `instance_key`.

## Receipt

`page_artifact_rebuild_operations` stores one replayable receipt containing:

- tenant, page, source and source publish operation identities;
- locale and source artifact id;
- normalized idempotency key and deterministic request hash;
- expected provenance and reviewed runtime hashes;
- rebuilt artifact id and storage instance key;
- exact rebuilt artifact and materialization hashes;
- creation timestamp.

An exact replay returns the stored result. Reusing the idempotency key for another source, provenance or reviewed runtime is rejected.

## Preserved boundaries

This command does not:

- mutate or delete the source artifact;
- update a published binding;
- advance the page version;
- emit `NodeUpdated` or `NodePublished`;
- rotate route, page or artifact cache generations;
- publish, rollback or unpublish a page;
- read the mutable current draft;
- add GraphQL, HTTP, OpenAPI, admin UI or background worker transport;
- automatically react to an audit finding;
- promote FFA or FBA.

Binding replacement remains a separate idempotent tenant-admin command. Only that later command may create lifecycle/cache effects after it proves the replacement artifact and active binding state.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-rebuild.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit.mjs
cargo test -p rustok-pages explicit_artifact_rebuild -- --nocapture
cargo check -p rustok-pages --all-targets
```

SQLite/PostgreSQL migration, authorization, exact replay, source corruption, runtime mismatch, append-only insertion and unchanged-binding evidence remain pending.

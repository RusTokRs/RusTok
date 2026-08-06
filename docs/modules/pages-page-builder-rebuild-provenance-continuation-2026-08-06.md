# Pages / Page Builder Rebuild Provenance Continuation

Date: 2026-08-06  
Status: source-ready / explicit-artifact-rebuild-source-ready / execution-pending
Scope: Pages immutable artifact audit, reviewed publication provenance, append-only rebuild and the next explicit binding-replacement boundary

## Rechecked parity cursor

Current `main` before this slice already contains:

- provider-owned reviewed static publish resource limits from PR #3099;
- the bounded Pages immutable artifact integrity audit from PR #3103;
- tenant-admin GraphQL, HTTP and OpenAPI audit adapters from PR #3107;
- one immutable sanitized rebuild source per locale for new reviewed publications from PR #3112.

The audit can identify invalid, partial or oversized retained artifacts. Rebuild provenance supplies the exact historical sanitized source and reviewed artifact/runtime identities without treating the mutable current draft as authority.

The remaining source gap was an explicit command that can retain a second verified immutable copy without overwriting the damaged row or changing what is public.

## Continued source slice

Marker:

```text
explicit-artifact-rebuild-source-ready
```

Pages now exposes a service-layer `rebuild_immutable_artifact` command around one exact:

```text
tenant
page
page_publish_rebuild_sources.id
expected provenance hash
idempotency key
reviewed runtime context
```

The command requires tenant-wide `pages:manage`. It recomputes the retained provenance hash, re-sanitizes the stored sanitized source and requires an explicitly reviewed runtime context whose review hash, scenario and context hash match the retained materialization identity.

Compilation must reproduce the retained source hash, static artifact hash, materialization hash, materialization identity and runtime snapshots exactly. Drift is rejected rather than normalized or repaired heuristically.

## Append-only artifact storage

The prior storage uniqueness contract allowed only one row for an exact tenant/page/locale/build/materialization identity. A deterministic rebuild would therefore have selected or collided with the damaged row instead of creating a replacement candidate.

`page_static_landing_artifacts` now carries an internal storage identity:

```text
instance_key = canonical
instance_key = rebuild:<rebuild-operation-uuid>
```

Ordinary publication remains `canonical`. A rebuild inserts a new row with the same verified deterministic content identity and a unique operation-bound storage instance.

`page_artifact_rebuild_operations` stores the idempotent rebuild receipt. Its source and rebuilt artifact ids are retained as opaque identities rather than cascade dependencies on those artifact rows, while the exact provenance source remains the command authority.

The audit record hash now binds `instance_key` and accepts only the canonical or operation-bound rebuild shape.

## Preserved public boundary

The rebuild command stops after the new artifact row and receipt commit.

It does not:

- update or delete the source artifact;
- read the mutable current draft;
- modify `page_published_landing_artifacts`;
- advance the page version;
- publish, rollback or unpublish a page;
- emit `NodeUpdated` or `NodePublished`;
- rotate route, page or artifact cache generations;
- add GraphQL, HTTP, OpenAPI, admin UI or worker transport;
- start automatically from an audit finding.

The currently published binding continues to reference its prior artifact id until a later explicit binding-replacement command is authorized and committed.

## Parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Reviewed static publish resource budgets | Source-ready | Real-project and publish execution pending |
| Bounded immutable artifact audit | Source-ready | SQLite/PostgreSQL execution pending |
| GraphQL/HTTP/OpenAPI audit transports | Source-ready | Transport execution pending |
| New-publish immutable source provenance | Source-ready | Migration and publish execution pending |
| Legacy provenance backfill/import | Open | Not designed |
| Explicit append-only repair/rebuild command | Source-ready | Database/runtime execution pending |
| Rebuild tenant-wide authorization | Source-ready | Scope execution pending |
| Rebuild exact idempotent receipt | Source-ready | Replay/conflict execution pending |
| Canonical/rebuild artifact instance identity | Source-ready | Migration/duplicate identity execution pending |
| Explicit binding replacement | Open | Not implemented |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Source ownership

- Pages owns provenance, tenant/page fencing, rebuild authorization, artifact storage instances and receipts.
- Page Builder owns sanitizer, runtime materialization and deterministic artifact identity.
- The rebuild caller owns the fresh reviewed runtime context; retained hashes constrain it but do not replace authorization.
- A future binding-replacement command will own the explicit public activation decision and its lifecycle/cache effects.
- Audit remains read-only and never schedules rebuild.

## Next cursor

1. Execute the provenance and explicit-rebuild source guards.
2. Retain SQLite/PostgreSQL migration evidence for canonical defaults and operation-bound duplicate identities.
3. Prove tenant-wide Manage succeeds and owner-scoped Manage rejects before source compilation or writes.
4. Prove exact replay returns one receipt/artifact while a reused idempotency key with another source, provenance or runtime rejects.
5. Prove source/provenance corruption, runtime scenario/context mismatch and renderer/materialization drift reject atomically.
6. Prove a successful rebuild appends one valid artifact and receipt while the original row, binding, page version, events and cache generations remain unchanged.
7. Define Explicit binding replacement around one rebuild receipt, expected current binding artifact, expected page version and a separate idempotency key.
8. Require the later switch to verify both source and rebuilt artifacts, update one exact locale binding, advance the owner version and emit lifecycle/cache effects only after commit.
9. Keep automatic audit-to-repair behavior absent.
10. Complete retained audit, provenance, static-publish, browser and tenant evidence before FFA/FBA promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-rebuild.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-transport.mjs
cargo test -p rustok-pages --test explicit_artifact_rebuild_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo, formatting, migrations, database/runtime scenarios, workflows and CI were intentionally not run.

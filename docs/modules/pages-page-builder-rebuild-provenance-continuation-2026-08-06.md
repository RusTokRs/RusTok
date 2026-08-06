# Pages / Page Builder Rebuild Provenance Continuation

Date: 2026-08-07  
Status: source-ready / explicit-artifact-rebuild-source-ready / explicit-artifact-binding-replacement-source-ready / execution-pending
Scope: Pages immutable artifact audit, reviewed publication provenance, append-only rebuild, explicit activation and the next tenant-admin repair transport boundary

## Rechecked parity cursor

Current `main` before this slice contains:

- provider-owned reviewed static publish resource limits from PR #3099;
- the bounded Pages immutable artifact integrity audit from PR #3103;
- tenant-admin GraphQL, HTTP and OpenAPI audit adapters from PR #3107;
- one immutable sanitized rebuild source per locale for new reviewed publications from PR #3112;
- the explicit append-only immutable artifact rebuild command from PR #3115.

The audit can identify invalid, partial or oversized retained artifacts. Rebuild provenance supplies the exact historical sanitized source and reviewed artifact/runtime identities without treating the mutable current draft as authority. Rebuild can retain a second verified immutable copy without overwriting the damaged row or changing what is public.

The remaining source gap was the separate explicit decision that may activate that rebuilt copy.

## Explicit append-only rebuild

Marker:

```text
explicit-artifact-rebuild-source-ready
```

Pages exposes `rebuild_immutable_artifact` around one exact tenant, page, provenance source, expected provenance hash, idempotency key and explicitly reviewed runtime context.

It requires tenant-wide `pages:manage`, re-sanitizes retained immutable source, verifies runtime scenario/context and reproduces the source, artifact, materialization identity and snapshots exactly.

Artifact storage separates deterministic content identity from storage identity:

```text
instance_key = canonical
instance_key = rebuild:<rebuild-operation-uuid>
```

The rebuild stops after a new immutable artifact row and `page_artifact_rebuild_operations` receipt commit. It does not touch public bindings, page version, events or cache generations.

## Explicit binding replacement

Marker:

```text
explicit-artifact-binding-replacement-source-ready
```

Pages now exposes `replace_rebuilt_artifact_binding` around one exact:

```text
tenant
page
page_artifact_rebuild_operations.id
expected page version
expected current artifact id
idempotency key
```

The command requires tenant-wide `pages:manage` and a currently published page. The selected rebuild receipt must be valid and its source artifact must equal the caller's expected current artifact. The locked locale binding must still reference that exact source id.

The replacement row must belong to the same tenant, page and locale and match the rebuild receipt's operation-bound instance key, artifact hash and materialization hash. `bind_existing_body_in_tx` then applies the existing complete Page Builder static artifact and materialization verifier before updating the binding.

The damaged source payload is not required to pass full integrity because that would make recovery from detected corruption impossible. Its exact identity remains fenced by the request, rebuild receipt and locked binding.

## Atomic activation and lifecycle effects

The owner transaction:

1. locks the page;
2. resolves exact idempotent replay;
3. checks the expected version and published state;
4. verifies the rebuild receipt;
5. locks the exact locale binding;
6. verifies the replacement artifact;
7. updates one binding only;
8. advances the page version once and retains published state;
9. writes `NodeUpdated` and `NodePublished` through the transactional event bus;
10. stores `page_artifact_binding_replacement_operations`;
11. commits.

Cache generations are never mutated inline. Existing committed lifecycle processing owns route/page/artifact generation effects after activation commit.

An exact replay returns the stored result without repeating the binding, version or events. One rebuild receipt may receive only one activation receipt, preventing silent reuse after later publish or rollback state changes.

## Preserved boundaries

The activation command does not:

- sanitize, compile or rebuild a project;
- read the mutable current body;
- update or delete the damaged source artifact;
- mutate the replacement artifact;
- replace more than one locale binding;
- publish an unpublished page;
- run automatically from an audit finding;
- add GraphQL, HTTP, OpenAPI, admin UI or worker transport;
- promote FFA or FBA.

## Parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Reviewed static publish resource budgets | Source-ready | Real-project and publish execution pending |
| Bounded immutable artifact audit | Source-ready | SQLite/PostgreSQL execution pending |
| GraphQL/HTTP/OpenAPI audit transports | Source-ready | Transport execution pending |
| New-publish immutable source provenance | Source-ready | Migration and publish execution pending |
| Legacy provenance backfill/import | Open | Not designed |
| Explicit append-only repair/rebuild command | Source-ready | Database/runtime execution pending |
| Rebuild tenant-wide authorization and replay | Source-ready | Scope/replay execution pending |
| Canonical/rebuild artifact instance identity | Source-ready | Migration/duplicate identity execution pending |
| Explicit binding replacement | Source-ready | Database/fence/lifecycle execution pending |
| Binding replacement tenant-wide authorization | Source-ready | Scope execution pending |
| Binding replacement exact idempotent receipt | Source-ready | Replay/conflict execution pending |
| Tenant-admin repair transports | Open | Not implemented |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Source ownership

- Pages owns provenance, tenant/page fencing, rebuild authorization, storage instances, bindings, lifecycle state and receipts.
- Page Builder owns sanitizer, runtime materialization, renderer and deterministic artifact integrity.
- The rebuild caller owns the fresh reviewed runtime context; retained hashes constrain it but do not replace authorization.
- The activation caller owns the explicit public switch decision and optimistic current-state fences.
- Existing Pages lifecycle handlers own cache generation effects after committed activation events.
- Audit remains read-only and never schedules rebuild or activation.

## Next cursor

1. Execute the provenance, explicit-rebuild and explicit-binding-replacement source guards.
2. Retain SQLite/PostgreSQL migration evidence for canonical defaults, operation-bound duplicate identities and activation receipt constraints.
3. Prove tenant-wide Manage succeeds and owner-scoped Manage rejects before rebuild or activation writes.
4. Prove rebuild exact replay/conflict, provenance corruption, runtime mismatch and byte-for-byte reproduction behavior.
5. Prove rebuild appends one artifact while binding, page version, events and cache generations remain unchanged.
6. Prove Explicit binding replacement rejects stale version, stale current artifact, reused rebuild, missing/invalid replacement and unpublished state atomically.
7. Prove successful activation changes one locale binding, advances one version, retains both artifact rows and produces one lifecycle pair plus one receipt.
8. Observe committed event processing and route/page/artifact generation changes only after activation commit.
9. Design bounded tenant-admin repair transports for explicit rebuild and activation with static public errors, current-tenant fencing and no raw provenance/runtime payload exposure.
10. Keep automatic audit-to-repair behavior absent and complete browser/tenant evidence before FFA/FBA promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-rebuild.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-transport.mjs
cargo test -p rustok-pages --test explicit_artifact_binding_replacement_sqlite -- --nocapture
cargo test -p rustok-pages --test explicit_artifact_rebuild_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo, formatting, migrations, database/runtime scenarios, lifecycle/cache observation, workflows and CI were intentionally not run.

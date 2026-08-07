# Pages / Page Builder Rebuild Provenance Continuation

Date: 2026-08-07  
Status: source-ready / explicit-artifact-rebuild-source-ready / explicit-artifact-binding-replacement-source-ready / explicit-artifact-repair-transport-source-ready / request-contract-harness-source-ready / execution-pending
Scope: Pages immutable artifact audit, reviewed publication provenance, append-only rebuild, explicit activation, bounded tenant-admin repair transports and request-level evidence

## Rechecked parity cursor

Current `main` contains:

- provider-owned reviewed static publish resource limits from PR #3099;
- the bounded Pages immutable artifact integrity audit from PR #3103;
- tenant-admin GraphQL, HTTP and OpenAPI audit adapters from PR #3107;
- one immutable sanitized rebuild source per locale for new reviewed publications from PR #3112;
- the explicit append-only immutable artifact rebuild command from PR #3115;
- the explicit rebuilt-artifact binding activation command from PR #3126;
- bounded explicit rebuild/activation transports from PR #3128;
- a generated GraphQL/OpenAPI transport contract harness from PR #3133.

The audit can identify invalid, partial or oversized retained artifacts. Rebuild provenance supplies the exact historical sanitized source and reviewed artifact/runtime identities without treating the mutable current draft as authority. Rebuild can retain a second verified immutable copy without overwriting the damaged row or changing what is public. Activation can switch one exact locale binding only after explicit current-state fences.

The current source cursor is request-level authorization and static-error evidence for those bounded transports.

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

Pages exposes `replace_rebuilt_artifact_binding` around one exact:

```text
tenant
page
page_artifact_rebuild_operations.id
expected page version
expected current artifact id
idempotency key
```

The command requires tenant-wide `pages:manage` and a currently published page. The selected rebuild receipt and retained provenance must be valid and mutually consistent. Its source artifact must equal the caller's expected current artifact. The locked locale binding must still reference that exact source id and retained body.

The replacement row must belong to the same tenant, page and locale and match the rebuild receipt's operation-bound instance key, artifact hash and materialization hash. `bind_existing_body_in_tx` then applies the existing complete Page Builder static artifact and materialization verifier before updating the binding.

The damaged source payload is not required to pass full integrity because that would make recovery from detected corruption impossible. Its exact identity remains fenced by the request, retained provenance, rebuild receipt and locked binding.

## Atomic activation and lifecycle effects

The owner transaction:

1. locks the page;
2. resolves exact idempotent replay;
3. checks the expected version and published state;
4. verifies the rebuild receipt and retained provenance;
5. locks the exact locale binding;
6. verifies the replacement artifact;
7. updates one binding only;
8. advances the page version once and retains published state;
9. writes `NodeUpdated` and `NodePublished` through the transactional event bus;
10. stores `page_artifact_binding_replacement_operations`;
11. commits.

Cache generations are never mutated inline. Existing committed lifecycle processing owns route/page/artifact generation effects after activation commit.

An exact replay returns the stored result without repeating the binding, version or events. One rebuild receipt may receive only one activation receipt, preventing silent reuse after later publish or rollback state changes.

## Bounded tenant-admin repair transports

Marker:

```text
explicit-artifact-repair-transport-source-ready
```

`PagesMutation` mounts two separate explicit mutations:

```text
rebuildPageArtifact
activateRebuiltPageArtifact
```

The Pages Axum router mounts:

```text
POST /api/admin/pages/{id}/artifacts/rebuild
POST /api/admin/pages/{id}/artifacts/activate
```

GraphQL checks tenant module enablement before resolver authorization. Both transport families enforce current-tenant identity and an effective `pages:manage` grant before owner delegation. The owner services independently retain the authoritative `PermissionScope::All` check before writes.

The adapters delegate once to the owner commands. They do not import entities, query artifact/provenance/binding tables, emit events or touch cache generations. Public errors use fixed codes and messages and never copy `PagesError` text.

HTTP accepts the existing explicit owner inputs. GraphQL mirrors those command fields. Bounded transport results expose receipt, page, locale, relevant artifact identities, verified artifact/materialization hashes, replay state and timestamps. They omit provenance source id, source publish operation id, internal storage instance key, idempotency keys, runtime context, materialization identity JSON and runtime snapshots.

`PagesApiDoc` registers both routes and bounded result schemas.

The transports do not select repair inputs for the caller, chain audit into rebuild, chain rebuild into activation or add a combined repair endpoint.

## Authorization actualization

Marker:

```text
explicit-artifact-repair-pages-manage-all-none-actualized
```

The earlier cursor incorrectly asked for an `owner-scoped pages:manage` request case. Under the current request bridge and RBAC scope function, Pages Manage is binary:

```text
pages:manage present  -> PermissionScope::All
pages:manage absent   -> PermissionScope::None
```

`PermissionScope::Own` is not currently representable for Pages Manage. The owner `PermissionScope::All` checks remain defense in depth for direct/internal callers and future authorization-model changes, but request evidence must test Manage present/absent rather than claim a nonexistent owner-scoped grant.

The request-level harness source is tracked in:

```text
docs/modules/pages-page-builder-repair-request-contract-continuation-2026-08-07.md
```

## Preserved boundaries

The repair path still does not:

- read mutable current body as rebuild authority;
- update or delete the damaged source artifact;
- mutate the replacement artifact;
- replace more than one locale binding;
- publish an unpublished page;
- run automatically from an audit finding;
- expose raw provenance, sanitized project or runtime payloads;
- add admin UI or worker scheduling;
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
| Bounded tenant-admin repair transports | Source-ready | GraphQL/HTTP/OpenAPI execution pending |
| Generated GraphQL/OpenAPI transport contract harness | Source-ready | Maintainer execution pending |
| Request-level tenant/Manage/static-error harness | Source-ready | Maintainer execution pending |
| Pages Manage `All`/`None` semantics | Source actualized | Maintainer execution pending |
| Static public repair errors and bounded results | Source-ready | Response-shape execution pending |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Source ownership

- Pages owns provenance, tenant/page fencing, rebuild authorization, storage instances, bindings, lifecycle state and receipts.
- Page Builder owns sanitizer, runtime materialization, renderer and deterministic artifact integrity.
- The rebuild caller owns the fresh reviewed runtime context; retained hashes constrain it but do not replace authorization.
- The activation caller owns the explicit public switch decision and optimistic current-state fences.
- GraphQL/HTTP adapters own only current-tenant/effective-permission fencing, command conversion and static public errors.
- Existing Pages lifecycle handlers own cache generation effects after committed activation events.
- Audit remains read-only and never schedules rebuild or activation.

## Next cursor

1. Execute the generated transport contract and request-level repair contract harnesses; retain schema/OpenAPI and request-response evidence.
2. Run provenance, rebuild, activation, transport and request-contract source guards.
3. Retain SQLite/PostgreSQL migration evidence for canonical defaults, operation-bound duplicate identities and activation receipt constraints.
4. Prove Manage present reaches owner validation and Manage absent/current-tenant mismatch fail closed through both GraphQL and HTTP.
5. Retain rebuild exact replay/conflict, provenance corruption, runtime mismatch and byte-for-byte reproduction evidence.
6. Prove rebuild appends one artifact while binding, page version, events and cache generations remain unchanged.
7. Prove activation rejects stale version, stale current artifact, reused rebuild, missing/invalid replacement and unpublished state atomically.
8. Prove successful activation changes one locale binding, advances one version, retains both artifact rows and produces one lifecycle pair plus one receipt; observe cache generations only after committed events.
9. Keep automatic audit-to-repair behavior absent before browser/tenant evidence and FFA/FBA promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
cargo test -p rustok-pages --test explicit_artifact_repair_transport_contract -- --nocapture
cargo test -p rustok-pages --test explicit_artifact_repair_request_contract -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport-contract.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-request-contract.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-transport.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-rebuild.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit.mjs
node crates/rustok-pages/scripts/verify/verify-pages-immutable-artifact-integrity-audit-transport.mjs
cargo test -p rustok-pages --test explicit_artifact_binding_replacement_sqlite -- --nocapture
cargo test -p rustok-pages --test explicit_artifact_rebuild_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo, formatting, migrations, GraphQL/HTTP/OpenAPI execution, database/runtime scenarios, lifecycle/cache observation, workflows and CI were intentionally not run.

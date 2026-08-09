# rustok-blog canonical implementation cursor

Status: `canonical_source_cursor_actualized_through_slice_100`.

This document is the canonical **current** source cursor for `rustok-blog`.
`crates/rustok-blog/docs/implementation-plan.md` remains the long historical baseline and embedded implementation log, but its inline `Current state`, completed-slice list, and `Next results` stop before the later continuation series and must not be used as the live cursor without this file.

The continuation series is authoritative for source work after the historical baseline. Slice 101 records this actualization; the latest production/source behavior slice before this plan-only correction is slice 100.

## Re-audit basis

Fresh `main` was re-audited after slices 98–100. The retained continuation artifacts show that several phrases still present in the historical baseline are stale as current instructions:

- the remote Comments transport is no longer an unimplemented source item;
- the cached public Comments snapshot is no longer merely planned;
- the storefront comment-form fallback is not an implementation target because the active storefront has no public Comments write surface;
- Blog category Translation PostgreSQL migration/concurrent-CAS/change-cursor evidence source is already retained and waits for maintainer execution.

These are planning corrections only. They do not promote runtime evidence.

## Current source tracks

### Comments remote transport and host composition

The remote Comments source implementation exists. The retained continuation chain covers the typed transport boundary, `TcpJsonCommentsTransport`, TCP server/listener and host selection, user delegation and authorization, key/keyring lifecycle, schedule persistence/audit, canonical event admission, source retry/dead-letter/recovery ownership, restart/ambiguous-commit evidence sources, and the canonical `rustok-outbox` relay evidence source.

Canonical interpretation:

`remote_comments_transport = source_implemented_maintainer_execution_pending`

Do **not** interpret historical `remote transport remains pending` text as a request to implement another transport, listener, retry lane, or relay.

The latest audit/relay source boundary is slice 97:

`canonical_outbox_relay_postgres_evidence_source_ready_maintainer_execution_pending`.

Source-row and immutable recovery-audit retention remain intentionally gated: do not advance that source work before retained maintainer execution of slices 95–97.

### Blog category Translation target

The `blog/category` target production source is present. Slice 98 adds the isolated PostgreSQL evidence source for:

- real migration `up -> down -> up`;
- concurrent same-revision CAS with one winner and one conflict;
- change-cursor recovery across provider/owner reconstruction and delete lifecycle.

Canonical interpretation:

`category_translation_postgres = source_ready_maintainer_execution_pending`

Do **not** reopen PostgreSQL migration, concurrent CAS, or ordinary cursor-recovery source scaffolding. After maintainer execution, record the result in the active Translation readiness view. Broader production enablement remains a separate Translation-owner decision.

### Storefront Comments fallback

Slice 99 implements one Blog-owned cached public Comments snapshot policy shared by GraphQL and native SSR. Successful approved public reads refresh the bounded cache best-effort. Only `ExternalService` and `Timeout` may consume an exact valid stale snapshot; stale hits preserve `UNAVAILABLE` / `TIMEOUT` and expose `cachedSnapshot=true`.

Canonical interpretation:

`cached_public_comments_snapshot = source_ready_maintainer_execution_pending`

Slice 100 re-audits the storefront write surface and proves that the active package is read-only. There is no public comment form, textarea, submit handler, GraphQL storefront mutation, or native create-comment server function.

Canonical interpretation:

`comment_form_fallback = not_applicable_no_storefront_write_surface`

The legacy `hide_comment_form` token remains compatibility vocabulary in the existing FBA registries; it is not authorization to invent a new storefront write surface.

## Remaining execution-owned results

The source re-audit identifies no independent production source gap inside the tracks above. The remaining concrete results are maintainer execution or explicitly execution-gated follow-ups:

1. Execute the retained Comments transport/composition, PostgreSQL, restart/ambiguity, canonical relay, and cached-snapshot evidence at an exact revision.
2. Execute slices 95–97 before defining terminal Blog source-row and immutable recovery-audit retention.
3. Execute slice 98 PostgreSQL evidence before advancing the Blog category Translation readiness result.
4. Execute category CRUD/Search refresh/canonical navigation/mounted rate-limit evidence already retained by the historical plan.
5. Execute the Blog article richtext cutover/backfill/browser evidence already retained by the historical plan.

A future autonomous source slice must start from a fresh repository audit and identify a genuinely new independent source gap. It must not manufacture work by reopening a source-complete or not-applicable cursor.

## Superseded historical cursor phrases

The following phrases may remain in the historical baseline as records of earlier state, but they are superseded as live instructions:

- `remote transport remains pending`;
- `cached snapshot and comment-form fallback remain planned`;
- `PostgreSQL migration, concurrent CAS, and change-cursor recovery evidence are still required before production inventory enablement`;
- `then implement the remote network transport`.

The continuation slice files and machine evidence remain the source of detailed ownership/non-claim history. This file only defines the current planning cursor.

## Validation boundary

No tests, Cargo commands, Node verifiers, PostgreSQL/Redis/TCP scenarios, browser targets, formatting, Clippy, builds, workflows, CI, HTTP execution, runtime validation, or production validation were executed by the implementation agent while producing this actualization.

## Next cursor

No independent production source gap is claimed by this actualization.

Continue only after a fresh repository audit finds a new source gap outside the execution-gated tracks above, or after maintainers provide execution results that unlock one of their explicit follow-ups.

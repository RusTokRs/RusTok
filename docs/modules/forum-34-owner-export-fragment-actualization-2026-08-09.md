# FORUM-34D owner-response export fragment actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked / locale-enumeration-open`

## Cursor

FORUM-34A through FORUM-34C established runner-neutral NodeBB source mapping, bounded dependency inspection and source-local category-cycle rejection.

Fresh `main` before this slice was `2b8f569a7a9297b5bfd77a4b8940707c165fb403`. The only commit after FORUM-34C was Commerce/Payment owner-port work; no Forum source overlapped. Fresh repository search still finds no neutral shared import/export runner, `rustok-import` owner crate, generic `ImportRunner`, `ImportAdapter`, `ImportJob`, checkpoint or receipt API suitable for Forum composition.

## Export boundary selected

The existing cursor read model is bounded and RBAC-aware, but it is intentionally presentation-oriented. In particular, `ReplyReadModel` contains only `content_preview`, so it cannot be used as an authoritative export source.

Forum already exposes full owner responses through its owner services:

- `CategoryResponse`;
- `TopicResponse`;
- `ReplyResponse`.

Those responses are produced only after the existing owner read authorization and localization paths execute. FORUM-34D therefore adds a pure mapper over **already-authorized full owner responses** instead of introducing a second database read path.

The mapper does not call those services itself. Enumeration, pagination, RBAC context and retry behavior remain future runner/host concerns.

## Public source contract

`rustok-forum::export_mapping` publishes:

- `FORUM_EXPORT_SCHEMA_V1 = "rustok.forum.export.v1"`;
- `MAX_FORUM_EXPORT_OWNER_VIEWS_PER_FRAGMENT = 512`;
- `ForumExportOwnerViewBatch`;
- `ForumExportCategoryRecord`;
- `ForumExportTopicRecord`;
- `ForumExportReplyRecord`;
- `ForumExportUserRef`;
- `ForumExportFragment`;
- `ForumExportMappingError`;
- `ForumOwnerExportMapper`.

`ForumOwnerExportMapper::map_fragment` preserves caller order and rejects a fragment above 512 total category/topic/reply owner views.

## Localization semantics

One full owner response represents one resolved locale view, not an entire multilingual resource.

The export record therefore uses `effective_locale`, never `requested_locale`. This matters for fallback reads: a request for `de` that resolves to stored `en` content exports locale `en`, not a synthetic German translation.

The mapper rejects duplicate `(resource id, effective locale)` views within one fragment. Two different requested locales that both fall back to the same effective locale cannot silently create duplicate exported translations.

FORUM-34D does **not** claim multilingual export completeness. Category and Topic owner responses expose available-locale metadata, but `ReplyResponse` currently does not. A complete export runner must gain a bounded owner contract for exact locale enumeration before it can claim that every reply translation was exported.

## Canonical versus presentation fields

The export records retain current owner state needed for a future migration adapter:

- category identity, parent, ordering, moderation and localized presentation fields;
- topic identity/category/author reference, localized title/slug, canonical `RichTextDocument`, metadata, status, tags, channel slugs, solution relation, pin/lock state and timestamps;
- reply identity/topic/author/parent relation, localized canonical `RichTextDocument`, status and timestamps.

The mapper deliberately drops presentation/viewer-derived values:

- rendered rich-text HTML;
- plain-text/preview derivatives;
- `current_user_vote`;
- aggregate vote score;
- subscription state;
- category/topic/reply counters;
- reply `is_solution` duplicate state, because the topic `solution_reply_id` relation is the exported current solution authority.

Votes/reputation remain separate FORUM-34 resources when their owner export contracts exist; they are not reconstructed from aggregate scores.

## Identity boundary

`author_id` becomes `ForumExportUserRef { user_id }` only as a source identity reference.

FORUM-34D does not declare that a source RusTok user UUID can be reused in another installation. Import-side identity matching remains a shared auth/Profile-owner resolution concern and must not be implemented by Forum through private user/profile persistence.

Category/topic/reply UUIDs are native Forum source identities inside the export format. A future importer/runner owns durable source-to-target identity receipts and replay semantics.

## Ownership and side-effect boundary

The new mapper imports no SeaORM type and performs no database read/write, transaction, async call, runtime registration, owner service construction, filesystem operation or network call.

FORUM-34D adds no:

- generic or Forum-only runner;
- pagination loop;
- checkpoint, receipt, replay or audit persistence;
- migration;
- CLI/admin/GraphQL/REST transport;
- identity/Profile lookup;
- Media access;
- attachment export;
- Search rebuild execution;
- automatic locale enumeration.

The existing owner services remain the only authority for what may be read. This mapper only transforms values already returned by those authorities.

## Remaining FORUM-34 scope

Still open after FORUM-34D:

- neutral shared import/export runner contract and host composition;
- durable cursor/checkpoint/receipt/replay/audit semantics;
- bounded exact locale enumeration, especially for replies;
- an operator-authorized bounded export reader/composer over the owner services;
- external-user resolution through the proper owner boundary;
- cross-batch import dependency resolution;
- candidate-to-existing Forum owner command adapter;
- revisions/history export/import policy;
- votes/reputation export/import through their owner contracts;
- attachment/media mapping after FORUM-14 provides Forum-owned relations;
- URL alias export/import where owner route contracts permit it;
- runner-level dry-run/reconciliation/search-rebuild orchestration;
- CLI/admin transport only after RBAC/idempotency/audit admission exists;
- retained SQLite/PostgreSQL/restart/lost-response evidence.

The large canonical implementation plan is not replaced wholesale through the GitHub contents API; this dated packet records the FORUM-34D execution cursor without overwriting unrelated concurrent roadmap edits.

## Maintainer validation

Per maintainer instruction, no test, Cargo command, Node verifier, formatter, migration, DB scenario, CLI, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-owner-export-fragment-source.mjs
```

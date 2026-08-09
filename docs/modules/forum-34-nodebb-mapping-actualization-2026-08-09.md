# FORUM-34A NodeBB mapping boundary actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor transition

Fresh `main` before this slice was `bd70dda0ab61aadf41a2f15c59d454962f12b1ac`.

FORUM-33 has no additional truthful source slice on that baseline:

- no persisted Forum-owned attachment relation/source-revision authority exists, so attachment reconciliation remains blocked on FORUM-14;
- posting-policy evaluation still has no mounted production enforcement call site, so spam-outcome telemetry remains blocked;
- Moderation still exposes no public application-operation read/status contract suitable for Forum lag/status diagnostics.

The canonical Forum ledger places `FORUM-34` next and describes it as the Forum import/export adapter plus NodeBB mapping over a shared runner.

## Shared-runner recheck

Fresh repository search found no neutral shared import runner/framework contract, no `rustok-import` owner crate, and no generic `ImportRunner` / `ImportAdapter` / `ImportJob` API suitable for Forum composition.

FORUM-34A therefore does **not** create a Forum-only runner, job table, receipt table, scheduler, CLI, transport, migration, database writer or orchestration loop. Those responsibilities stay blocked until the shared runner exists.

Forum can still truthfully own the source-specific mapping/validation boundary required by the canonical ownership rules. This slice starts that boundary only.

## Public mapping contract

`rustok-forum::import_mapping` now exposes a runner-neutral, side-effect-free NodeBB mapping surface:

- `NodebbExportBatch`;
- `NodebbCategoryRecord`;
- `NodebbTopicRecord`;
- `NodebbPostRecord`;
- `NodebbForumImportMapper`;
- bounded Forum import candidate DTOs and external references;
- fixed structural mapping errors.

The mapper accepts at most `MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH = 512` source records across categories, topics and posts.

It performs no database access and does not call Forum owner write services. A future shared runner must decide transaction, receipt, replay, checkpoint and recovery semantics before any candidate reaches owner commands.

## Identity boundary

NodeBB numeric identities are preserved only as external source references:

```text
nodebb/category:{cid}
nodebb/topic:{tid}
nodebb/post:{pid}
nodebb/user:{uid}
```

The DTO representation stores `source = "nodebb"`, a fixed entity kind and the external key separately.

The mapper never manufactures RusTok UUIDs. In particular, NodeBB `uid` is not treated as a RusTok user/profile identity. Positive user IDs become external author references; non-positive/guest-style values remain unresolved (`None`). A future shared runner plus the proper identity/Profile owner composition must resolve those references explicitly.

## Structural normalization

The source-specific mapper currently retains the minimum NodeBB facts needed for later Forum owner commands:

- category: `cid`, optional parent, name, description and order;
- topic: `tid`, category, external author, title, optional slug, optional `mainPid`, timestamp, pinned and locked state;
- post: `pid`, topic, external author, content, timestamp and deleted state.

Root/non-positive `parentCid` is normalized to no parent. Non-positive optional `uid` / `mainPid` values are treated as absent rather than converted into domain identities. Negative timestamps are not promoted as candidate timestamps.

Required category names and topic titles are trimmed and must remain non-empty. Category/topic/post owner source IDs must be positive and unique within their entity kind for the current batch.

This is import-candidate validation only. It does not replace the final Forum owner validation that must run before persistence.

## Topic-body versus reply classification

NodeBB topic body identity can depend on `mainPid`. FORUM-34A deliberately avoids cross-page guessing.

For a post whose topic is present in the same bounded batch:

- matching `mainPid` -> `TopicBody`;
- other post -> `Reply`.

If the topic is outside the current batch, the role is `Unresolved`. The mapper does not silently treat that post as a reply. A future shared runner can resolve the topic/main-post relationship using its own bounded source/checkpoint state.

## Ownership and safety

The mapping module imports no SeaORM/database types, `Uuid`, Media storage API, Profiles persistence, Notifications, Search or Moderation storage. It has no create/update/delete operation and no runtime-extension registration.

That keeps FORUM-34A below the orchestration boundary:

1. source-specific mapping and structural validation belong to Forum;
2. external identity resolution belongs to the relevant shared owner/composition;
3. generic run/checkpoint/retry/receipt/audit orchestration belongs to the future shared import runner;
4. final category/topic/reply persistence must enter existing Forum owner commands rather than writing tables directly.

## Remaining FORUM-34 scope

Still open:

- neutral shared import runner contract and host composition;
- durable runner checkpoint/receipt/replay/audit semantics;
- explicit external-user resolution policy;
- candidate-to-existing-Forum-owner command adapter;
- cross-batch category/topic/post dependency resolution;
- attachment/media mapping after FORUM-14 provides Forum-owned attachment relations;
- import reconciliation and dry-run reporting;
- Forum export adapter;
- admin/CLI transport only after runner/RBAC/idempotency boundaries exist;
- runtime, PostgreSQL/SQLite and restart/lost-response evidence.

The canonical implementation plan is intentionally not replaced wholesale through the GitHub contents API because it is large, concurrent and currently carries unrelated roadmap edits. This dated actualization records the FORUM-34A execution cursor without pretending the canonical ledger's `planned` status is already fully reconciled.

## Maintainer validation

Per maintainer instruction, no tests, Cargo command, Node verifier, formatter, migration, database scenario, CLI, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-nodebb-import-mapping-source.mjs
```

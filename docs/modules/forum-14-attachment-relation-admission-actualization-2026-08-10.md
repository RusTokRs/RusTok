# FORUM-14A attachment relation admission actualization — 2026-08-10

Status: `source-ready / admission-only / persistence-blocked-on-media-lifecycle / maintainer-execution-open`

## Fresh cursor

This slice was rechecked from `main@2dcffcd7c20c9deee58e6912d7b18ea761e149c5` after FORUM-34Q, Pages #3426 and Commerce #3428.

The FORUM-34 bounded mapping/application chain is source-ready through deleted-reply owner persistence, but a genuinely shared owner-data migration runner still does not exist. `rustok-modules` now has durable artifact-data upgrade/checkpoint primitives, but those contracts are explicitly tied to artifact installation identity (`target_installation_id`, `ModuleInstallationScope`, installation revision CAS) and are not a neutral Forum owner-data migration runner.

FORUM-33 source work is already materially ahead of the stale canonical summary: subscription and mention reconciliation, Search/Notifications shared-owner diagnostics, unread/activity observations and the bounded locale baseline already exist. FORUM-33K correctly stops further blanket locale instrumentation. Its remaining attachment cursor is blocked because FORUM-14 has no persisted Forum-owned attachment authority yet.

The canonical Forum implementation-plan ledger still lists FORUM-14 as `planned` and parts of FORUM-33/FORUM-34 behind the merged source cursor. This dated packet records the truthful slice without replacing the large canonical file wholesale through the contents API.

## Ownership boundary

Media remains authoritative for:

- upload sessions and binary bytes;
- blob/storage identity and drivers;
- MIME/dimensions/renditions;
- quarantine and deletion lifecycle;
- public delivery/proxy decisions;
- Media-owned reconciliation.

Forum may own only the relation between one Forum content revision and a Media asset together with Forum usage/order/caption semantics.

FORUM-14A therefore adds no upload command, storage key, URL, blob metadata, Media deletion marker, quarantine copy or Media reconciliation logic.

## Public admission contract

The crate now exposes `attachment_relation` and re-exports its public DTOs.

The source-ready entrypoint is:

```rust
ForumAttachmentRelationPreparer::prepare(
    ForumAttachmentRelationAdmissionRequest {
        tenant_id,
        target,
        source_revision,
        locale,
        attachments,
    },
)
```

`target` reuses the existing typed `ForumContentTarget`, so only Forum Topic or Reply identities can be supplied.

`source_revision` is the logical Forum content revision used by existing current-revision owner contracts: initial content is revision `1`, and captured topic/reply history advances the logical revision monotonically. It is deliberately not a Media revision and not a relation-row database identity.

A future persistence boundary must independently fence this admitted revision against the current Forum owner revision before committing relations. FORUM-14A performs no database read and makes no claim that a supplied historical revision is currently writable.

## Relation facts

Each admitted relation contains only:

```text
media_id
usage = inline | attachment
position
caption
```

The batch is bounded to 32 relations per Forum content revision.

Rules:

- tenant and target UUIDs must be non-nil;
- source revision must be positive;
- locale is normalized through the existing Forum/platform locale contract;
- every Media UUID must be non-nil;
- positions are unique and contiguous from zero;
- repeated use of the same Media asset at different positions is allowed;
- captions are trimmed, empty captions become absent, and retained captions are single-line/control-free UTF-8 up to 512 bytes;
- input order is not authoritative: the prepared result is sorted by admitted position;
- an empty relation set is valid and means that content revision carries no admitted attachments.

The preparer creates no UUIDs and performs no external call.

## Why persistence remains blocked

Fresh Media public-port recheck found `MediaAssetReadPort` exposes asset metadata, listing, image descriptors and translations, while `MediaAssetWritePort` owns upload completion/deletion/reconciliation.

The current `MediaItem` read DTO still does not publish the quarantine/deletion lifecycle facts needed for Forum to prove that a relation is safe to persist. The existing Forum category-cover code already documents the same boundary: persistent Media references remain disabled until those owner states are published.

FORUM-14A therefore does **not** call Media merely to check that an asset ID currently resolves. Existence alone is weaker than the lifecycle contract required by the ownership plan and would turn a partial Media DTO into invented attachment admission semantics.

A later FORUM-14 persistence slice must first consume a public Media owner contract that can establish the required lifecycle state, then fence tenant/asset identity and the exact Forum source revision before writing Forum-owned relation rows.

## FORUM-33 impact

This slice establishes the stable in-process relation shape but does not yet unblock attachment reconciliation because there is still no persisted Forum attachment authority to scan.

After actual relation persistence lands, FORUM-33 may add a bounded read-only diagnostic over Forum-owned attachment rows and source revisions. It must not reconcile Media-private asset state by direct table access.

## Deliberately unchanged

FORUM-14A adds no:

- table, entity or migration;
- owner write service;
- GraphQL/HTTP/admin/storefront transport;
- Media dependency or lifecycle copy;
- upload/session/delete/reconcile command;
- event/outbox receipt;
- attachment repair worker;
- lockfile/dependency change;
- runtime evidence claim.

Text-only Forum remains independent of Media composition.

## Next cursor

Recheck Media for a public bounded asset-lifecycle admission fact covering at least tenant identity plus deletion/quarantine eligibility. If that fact appears, the next safe slice is FORUM-14B: owner persistence fenced by exact Forum content revision and Media lifecycle admission.

If Media still lacks that fact, do not create attachment persistence from `get_asset` existence alone. Continue another independently authorized Forum plan slice instead.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-attachment-relation-admission-source.mjs
```

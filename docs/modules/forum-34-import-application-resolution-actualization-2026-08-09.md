# FORUM-34K bounded import application resolution actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor and recheck

FORUM-34A through FORUM-34J are merged before this slice. Fresh `main` for 34K is `bc7fec764589b64b257340572cb0a1611c27db65`; the commit after 34J is Commerce/Fulfillment-only and does not overlap Forum import/export source.

Repository recheck still finds no neutral shared `ImportRunner` / `ImportJob` / migration-runner contract suitable for Forum checkpoint, receipt, replay or recovery ownership. The canonical Forum implementation-plan ledger still labels `FORUM-34` as `planned`; this dated packet records the truthful 34K cursor without replacing the large roadmap wholesale.

## Why 34K resolves before it writes

The existing Forum owner create APIs are correct interactive commands, but they are not yet a migration application port:

- category/topic/reply create paths generate fresh UUIDs internally;
- topic/reply authors are derived from `SecurityContext` rather than an admitted source-user binding;
- the interactive commands do not preserve imported timestamps/deleted state as caller-supplied owner facts;
- NodeBB source IDs must never be converted directly into RusTok IDs.

Writing through those APIs now would lose the explicit external-to-owner identity map or misrepresent the importing operator as the source author. 34K therefore adds a side-effect-free resolution boundary first.

## Public in-process contract

`ForumImportApplicationResolver::resolve_batch(...)` accepts `ForumImportApplicationResolutionRequest` containing:

- one non-nil tenant ID;
- one explicit import locale, normalized through the existing content locale normalizer;
- one already-inspected `NodebbForumImportInspection`;
- bounded explicit `ForumImportIdentityBinding` values.

The request and resolved application types are in-process only and add no serde/transport contract.

The binding cap is `MAX_FORUM_IMPORT_RESOLUTION_BINDINGS_PER_BATCH = 1024`, derived as twice the existing 512 source-record bound. A bounded batch can require at most one owner identity per source record plus one external user binding per topic/post source record.

## Typed identity admission

A binding contains the original `ForumImportExternalRef`, a typed owner target kind and a non-nil UUID. Target kinds are deliberately explicit:

- category source -> `Category` UUID;
- topic source -> `Topic` UUID;
- reply-classified NodeBB post -> `Reply` UUID;
- external NodeBB user -> `User` UUID.

The resolver never calls `Uuid::new_v4` and never derives a RusTok UUID from NodeBB numeric IDs.

Bindings fail closed on duplicate source refs, nil target UUIDs, target-kind mismatches, unused bindings, missing required bindings and same-kind target collisions. Same-kind collision rejection prevents two distinct source identities from silently collapsing onto one category/topic/reply/user identity without a future explicit merge policy.

## Topic-body identity boundary

A NodeBB `post` classified as `TopicBody` does **not** receive a `Reply` UUID. Forum stores the topic body as localized topic content, not as a separate reply entity.

34K therefore consumes the confirmed `mainPid` body post into `ForumResolvedImportTopic.body` and preserves its `body_source` external reference. That source can later be recorded as an alias/reference to the owning topic by the shared identity ledger; the resolver does not invent a standalone Forum reply identity for it.

Reply-classified posts do require explicit `Reply` target UUIDs and become `ForumResolvedImportReply` application facts.

## Dependency admission

34K deliberately accepts only a structurally self-contained bounded application batch.

The only unresolved inspection issue admitted past 34C is `AuthorUser / ExternalOwnerResolution`, and every such source user must have an explicit `User` binding.

The resolver independently rechecks candidate structure instead of trusting a forgeable inspection object blindly:

- category parents must be present in the same bounded category set;
- category parent chains must remain acyclic;
- topic categories must be present in the same batch;
- topic main posts must be present in the same batch;
- post topics must be present in the same batch;
- all source refs must remain in the `nodebb` namespace with the expected entity kind;
- duplicate candidate source refs are rejected;
- `ForumImportPostRole::Unresolved` is rejected.

Cross-batch category/topic/post resolution remains runner-owned. An identity binding alone is not enough to prove absent source content or topic-body classification.

## Topic body and author integrity

Every topic must carry an explicit body-post source, and the referenced post must:

- be present in the bounded batch;
- be classified as `TopicBody`;
- point back to the same source topic;
- not be marked deleted.

The topic author and its main-post author must resolve to the same optional owner user ID. 34K does not guess between conflicting source author facts.

Guest/no-author facts remain `None`; positive NodeBB user refs require explicit external owner resolution.

## Resolved application facts

The resolver emits deterministic source-order `ForumResolvedImportApplicationBatch` facts:

- categories with admitted owner ID and resolved parent owner ID;
- topics with admitted owner ID, category ID, optional resolved author, source title/slug/body/timestamp/pin/lock facts and the exact body source ref;
- replies with admitted owner ID, topic ID, optional resolved author, body/timestamp and deleted flag.

The normalized import locale is carried once on the batch because the current NodeBB source mapping has no per-record locale fact.

These are resolution/application facts, not persistence DTOs. 34K does not invent missing category icon/color/moderation policy, rich-text conversion policy, imported status defaults, or owner-event semantics.

## Storage and ownership boundary

`import_resolution.rs` imports no SeaORM/database type, owner service, event bus, transport or migration runtime. It performs no create/update/delete operation and no UUID generation.

The next persistence slice must still enter a Forum-owned command boundary that can preserve admitted identities/authors and imported lifecycle facts without bypassing Forum invariants. Generic retry/checkpoint/receipt/audit semantics remain blocked on the absent shared runner.

## Current FORUM-34 chains

Export is source-ready for one live bounded page through:

`34J page composer -> 34I candidate IDs -> 34H exact locales -> 34F owner reads -> 34D export mapping`.

Import is now source-ready through identity/dependency resolution for one self-contained bounded batch:

`34A NodeBB mapping -> 34B/34C inspection -> 34K explicit owner identity + application fact resolution`.

## Next FORUM-34 cursor

The next safe slice should inspect and define the smallest Forum-owned **import write port** capable of consuming `ForumResolvedImportApplicationBatch` with caller-admitted IDs/authors while preserving owner invariants. It must not reuse interactive create APIs if doing so would replace imported identity/author/timestamp/deleted facts, and it must not add runner checkpoints or receipts locally.

Cross-batch source assembly and durable identity-ledger persistence still belong to shared composition/runner work.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-import-application-resolution-source.mjs
```

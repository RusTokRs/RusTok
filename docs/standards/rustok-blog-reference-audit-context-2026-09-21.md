# RusTok Blog Reference Audit — Current Handoff Context

Status: active engineering audit
Repository: `RusTokRs/RusTok`
Fresh `main` snapshot: `fce141df0746c836e9af2a06a72dd5d2bae04553`
Snapshot commit time: `2026-09-21T06:17:16Z`
Review ledger at snapshot: `125 / 218` components audited

## Purpose

`rustok-blog` is the canonical/reference native module for RusTok. Its source layout,
ownership boundaries, tenant/security contracts, lifecycle semantics, multilingual
behavior, error mapping, concurrency rules, and integration boundaries are intended
to be the template for future module migrations.

This file is an audit handoff, not a replacement for the canonical Blog implementation
cursor. The canonical live Blog source cursor remains:

`crates/modules/rustok-blog/docs/implementation-plan-current.md`

Historical `implementation-plan.md` and `implementation-plan-slice-*.md` files do not
override that cursor.

## Already completed reference work

PR #4071 — native module physical layout:
- merged 2026-09-18;
- merge commit: `0aef65063f7b8d77444a5292c06128ee343c7172`;
- establishes Blog as the strict native-module layout reference.

PR #4072 — canonical reference architecture certification:
- merged 2026-09-18;
- merge commit: `45ff1479942a9e6aa08e484062f89fc83b001973`;
- PR head: `561dfcef05e6aed3961ff979e0b202de4c314176`.

The canonical reference hardening covers:
- locale write provenance and exact-locale semantics;
- no read-fallback provenance leakage into writes;
- no locale-copying when creating a new translation locale;
- Patch<T> nullable edit semantics;
- version/CAS protection;
- explicit lifecycle transitions including Archived -> Draft restore;
- derived Comments counters without mutating Blog business version/updated_at;
- current-tenant authority;
- one redacted Blog public-error boundary;
- private persistence/integration boundaries;
- deterministic list ordering and fail-closed missing translation handling;
- stable comment command identity across retries;
- physical module source-layout separation;
- one Blog post permission namespace;
- invariant classification for storage/projection corruption;
- typed Taxonomy dependency failures;
- fail-closed native admin/storefront host-context lookup;
- composition-time classification of runtime extension registration failures.

Do not treat this as runtime/test certification. Maintainer-owned compile, test, database,
browser, transport, restart, and production-style evidence is a separate activity.
The audit assistant does source inspection and code review; the repository owner runs
validation.

## Canonical Blog Category ownership

The Blog Category migration is source-complete through TAXONOMY-CAT-12 and its
owner-scoped documentation cleanup is complete through CAT-17.

Canonical ownership:
- Taxonomy owns Category identity, localized canonical copy, route history, hierarchy,
  and canonical presentation.
- Blog owns `blog_categories` membership/settings/revision and Blog-specific command
  policy.
- Blog Category create/update/move/delete commands synchronize canonical Taxonomy state
  through transaction-aware owner APIs.
- Blog reads and mutation responses consume canonical Taxonomy projections.
- The former `blog/category` Translation provider is retired.
- The former Blog Category translation change journal and live donor translation mirror
  are retired.
- The historical donor entity exists only for upgrade/backfill provenance.
- `blog_categories` is explicitly `excluded` / `not_registered` in the central
  Translation surface registry.
- `taxonomy/term` remains the canonical registered Taxonomy Translation provider.

Important: registry/documentation exclusion is not itself a runtime authorization
boundary. See finding #4102 below.

Do not recreate the retired Blog Category Translation provider or its deleted
PostgreSQL harness merely because historical slice-98 text mentions it.

## Confirmed open findings

### #4102 — generic Taxonomy Translation bypasses module-owned term authorization

Scope:
- `crates/modules/rustok-taxonomy/src/translation_target.rs`
- Taxonomy service exact-translation write path
- Blog-owned module-scoped Taxonomy terms, including Blog tags/categories

Confirmed behavior:
- `TaxonomyTranslationTargetProvider` is registered for `taxonomy/term`.
- `list_resources` is tenant-scoped but does not exclude module-owned terms.
- `parse_identity` validates only the generic Taxonomy term identity shape.
- `apply_patch` authorizes `Resource::Taxonomy` / `taxonomy:update`.
- `apply_exact_translation_in_tx` performs tenant/revision checks but does not require
  owner-module authorization for a module-scoped term.
- Therefore a module-owned Blog term can be translated through generic Translation
  without Blog Tags/Category authorization and without Blog-specific projection side
  effects such as reindex publication.

Why it matters:
- breaks owner-boundary semantics;
- creates a path around Blog resource permissions;
- can leave Blog Search stale after a successful owner-data mutation.

Expected direction:
- add an explicit owner authorization/capability hook at the Taxonomy Translation
  boundary;
- keep owner-specific side effects with the owner;
- do not hard-code Blog checks into Translation.

### #4103 — Blog lifecycle existence probes still depend on fallback locale

Scope:
- `crates/modules/rustok-blog/src/services/tag.rs`
- `crates/modules/rustok-blog/src/services/category.rs`
- Taxonomy owner-read helpers

Confirmed behavior:
- Tag update/delete preflight probes use `PLATFORM_FALLBACK_LOCALE`.
- Category existence validation similarly uses a localized owner reader with the
  platform fallback locale.
- A valid module-owned term/category that exists only in a non-English locale can
  therefore appear missing during locale-neutral lifecycle commands.

Expected direction:
- identity/ownership checks must be locale-independent;
- localized presentation may use request/default locale;
- lifecycle authorization must use tenant + kind + module scope + term_id, not
  translation presence in a particular locale.

### #4099 — Blog is missing its hard Profiles module dependency

Scope:
- `crates/modules/rustok-blog/Cargo.toml`
- `crates/modules/rustok-blog/rustok-module.toml`
- `crates/modules/rustok-blog/src/module.rs`
- Blog GraphQL author/profile loading
- `crates/modules/rustok-modules/src/policy.rs`

Confirmed behavior:
- Blog directly depends on `rustok-profiles`.
- Blog GraphQL uses ProfileService / ProfileSummaryLoader / ProfilesReader.
- The module manifest and `BlogModule::dependencies()` omit `profiles`.
- Module enable/disable policy relies on declared dependency topology.

Expected direction:
- declare `profiles >=0.1.0` in the Blog module dependency contract;
- add `profiles` to `BlogModule::dependencies()`;
- update the corresponding metadata contract test.

### #4090 — hard-deleted Blog posts leave orphaned Comments threads

Scope:
- Blog post hard delete
- `rustok-comments` polymorphic thread ownership

Confirmed behavior:
- Blog hard-deletes non-published posts and publishes `BlogPostDeleted`.
- Comments threads are keyed by tenant + target type + target id without an FK to the
  Blog post.
- Comments has no target-deletion lifecycle cleanup path.
- The orphaned thread/comment state remains after the Blog post disappears.

Expected direction:
- owner-side target lifecycle cleanup in Comments;
- generic source lifecycle contract preferred;
- cleanup must be tenant/source/kind/subject scoped and replay/idempotent.

### #4094 — hard-deleted Blog posts leave orphaned Reactions subject state

Scope:
- Blog post deletion
- Reactions subject/catalog/actor/aggregate state

Confirmed behavior:
- Reactions stores subject state independently of Blog posts.
- Blog post hard-delete emits `BlogPostDeleted`.
- Reactions currently has no corresponding source-subject cleanup lifecycle.

Expected direction:
- Reactions-owned cleanup driven by the authoritative source deletion lifecycle;
- do not let Blog issue direct DELETE statements against Reactions tables.

### #4095 — Blog Search author projection is stale after user name changes

Scope:
- Blog Search projection
- user update lifecycle

Confirmed behavior:
- Blog search documents materialize `users.name` as `payload.author_name` and in
  searchable text.
- The canonical user update path does not emit `UserUpdated` for the relevant change.
- Search ingestion has no independent `UserUpdated` -> Blog author reindex path.

Expected direction:
- publish `UserUpdated` transactionally;
- perform targeted Blog author reindex;
- rebuild full affected Blog search documents so the DB search vector is refreshed.

### #4097 — deactivated Blog authors remain in public Search projection

Scope:
- Blog Search author materialization
- account deactivation lifecycle

Confirmed behavior:
- account deactivation retains the users row;
- Blog Search joins `users` and materializes the retained name;
- the Blog projection path needs an independent handling of `UserDeleted` / deactivation
  semantics to redact the public author presentation.

Current related work:
- draft PR #4098 proposes the source-level fix but is not merged.

### #4091 — Blog channel visibility is not enforced by storefront Search filtering

Scope:
- Blog Search documents contain channel visibility facets;
- storefront Search channel filtering is implemented by a helper scoped to product
  documents.

Expected direction:
- enforce Blog channel visibility in the Search owner/query path;
- keep server-side filtering authoritative.

### #4089 — CommentsThreadPort durable idempotency is incomplete

Scope:
- `rustok-comments` write contract
- Blog FBA Comments integration

Confirmed behavior:
- the contract requires durable idempotency for write operations;
- Blog provides a stable `CreateCommentInput.command_id`;
- Comments does not currently persist/deduplicate that write identity;
- nonce/delegation replay protection is not equivalent to durable command idempotency.

Expected direction:
- durable owner-scoped idempotency receipt or equivalent owner storage;
- identical command + payload replays the original result;
- concurrent duplicates execute once;
- same key with different payload conflicts.

## Open draft fixes that are not in main

PR #4087:
- `fix(blog): avoid system reread in public SEO listing`
- open draft;
- not merged into current `main`.

PR #4098:
- `fix(search): redact deactivated Blog authors from public search`
- open draft;
- not merged into current `main`.

Do not treat either PR as part of the current production code until merged.

## Current important source paths

Blog:
- `crates/modules/rustok-blog/docs/implementation-plan-current.md`
- `crates/modules/rustok-blog/rustok-module.toml`
- `crates/modules/rustok-blog/src/module.rs`
- `crates/modules/rustok-blog/src/services/category.rs`
- `crates/modules/rustok-blog/src/services/category_command.rs`
- `crates/modules/rustok-blog/src/services/category_name_projection.rs`
- `crates/modules/rustok-blog/src/services/category_owner.rs`
- `crates/modules/rustok-blog/src/services/tag.rs`
- `crates/modules/rustok-blog/src/graphql/query.rs`
- `crates/modules/rustok-blog/src/services/post/commands.rs`

Taxonomy:
- `crates/modules/rustok-taxonomy/src/translation_target.rs`
- `crates/modules/rustok-taxonomy/src/services.rs`
- `crates/modules/rustok-taxonomy/src/module_term_mutation.rs`
- `crates/modules/rustok-taxonomy/src/owner_category_read.rs`
- `crates/modules/rustok-taxonomy/src/owner_category_sync.rs`

Translation:
- `crates/modules/rustok-translation-targets/src/lib.rs`
- `crates/modules/rustok-translation/src/workflow.rs`
- `crates/modules/rustok-translation/src/inventory.rs`
- `crates/modules/rustok-translation/src/progress.rs`
- `crates/modules/rustok-translation/src/interchange.rs`
- `crates/modules/rustok-translation/src/graphql/query.rs`
- `crates/modules/rustok-translation/src/graphql/mutation.rs`

Search:
- `crates/modules/rustok-search/src/blog_projector.rs`

Host composition:
- `apps/server/src/services/module_event_dispatcher.rs`

Module lifecycle policy:
- `crates/modules/rustok-modules/src/policy.rs`

Central Translation registry:
- `docs/modules/translation-surfaces.json`

## Audit rules for continuation

1. Always start from a fresh `main` SHA. Parallel agents move `main` rapidly.
2. Read current source before trusting issue descriptions or older audit notes.
3. Issue bodies may contain stale "Fresh main" SHAs. Treat the issue number/finding as
   useful evidence, but revalidate against current `main`.
4. Do not reopen retired CAT-1..CAT-17 work or recreate the retired Blog Category
   Translation provider.
5. Separate source correctness from maintainer/runtime validation. Do not claim tests,
   DB runs, browser runs, workflow runs, or production evidence unless explicitly
   observed.
6. Prefer narrow owner APIs over direct cross-module persistence access.
7. For locale-sensitive code, distinguish exact requested locale, effective fallback
   locale, and locale-independent identity/ownership.
8. For provider-driven Translation, verify both registry/documentation state and the
   actual authorization/mutation path.
9. For lifecycle projections, check deletion, update, retry, replay, and out-of-order
   delivery semantics.
10. Search/projection findings must trace the authoritative source event through the
    complete consumer path, not only the final SQL projection.

## Recommended next continuation

Start with #4102 because it crosses the newly canonical Taxonomy ownership boundary
and the Translation control plane. Trace:
- who is allowed to translate global vs module-owned Taxonomy resources;
- how owner-module authorization is represented at provider level;
- how owner-side reindex/projection side effects are triggered;
- whether list/read/apply expose a consistent ownership model;
- whether the same flaw affects Forum or other module-scoped Taxonomy terms.

Then re-check #4103 and #4099 against the same fresh `main`.

No runtime claim should be attached to this source-level handoff until the repository
owner performs the corresponding tests/workflows.

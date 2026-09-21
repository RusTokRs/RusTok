# RusTok Blog Reference Audit — Current Handoff Context

Status: active engineering audit
Repository: `RusTokRs/RusTok`
Fresh `main` snapshot: `90ebb9ac6394be4ad580f77cd79c205074ba6efb`
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

## Source-fixed findings

All findings below have source fixes directly committed to `main`. Their GitHub issues may
remain open until runtime validation/maintainer closure.

- #4102 — Taxonomy Translation owner boundary — fixed by `0ffa5a104fd6e21896fbd764849083701ffc9518`.
- #4103 — Tag locale-independent lifecycle — fixed by `8ccbbba5f036c74849f8ac91015ffd161755236e`.
  Category half was rechecked and is not currently reproduced as an identity defect.
- #4099 — Profiles hard dependency — fixed by `288861490f48ea582020258e13847c68f5d17b03` and `bd6c62634300fc7bbae6e33dcd7b728bab47eea8`.
- #4090 — Comments target cleanup — fixed by `d5793857266cecd289d7dcfcff49069d76e7f050`.
- #4094 — Reactions subject cleanup — fixed by `ca60ad4bdb40e2728accc70fa9630ca1b311f95e` and corrective wiring `a940938094b2fee2f3b6064f7f2b43cc695ca3f2`.
- #4095/#4097 — Blog author Search freshness/redaction — fixed by `c446dc32f25798b6bfc3a57c416406a9963cd731` and `794bfdc455ac03b7312bfa8c7f53f92f591412e9`.
- #4091 — Blog channel visibility in storefront Search — fixed by `8ad2a7e9df45f231efa19fac1a66f4451d39d2a5`.
- #4089 — durable Comments port idempotency — fixed by `bb95cb78a91caffc83f3c21ffc4123e3e7b2a759` and corrective error mapping `1d52ee7bc24d133361379f615430e94dfd7012aa`.
- #4087 — Blog SEO public listing system reread — fixed by `90ebb9ac6394be4ad580f77cd79c205074ba6efb`.

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

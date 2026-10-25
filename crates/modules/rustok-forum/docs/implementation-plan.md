---
id: doc://crates/modules/rustok-forum/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-forum
last_reviewed: 2026-09-24
---

# `rustok-forum` canonical implementation plan

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence: `scripts/verify/verify-forum-admin-boundary.mjs` locks the forum fast boundary guardrail, including category interactive drag-and-drop owner-command integration.

## Canonical-source policy

This file is the single source of truth for Forum product scope, Forum-owned
implementation work, shared-capability integration work, task status, execution
order and release gates.

The exact pre-correction snapshot remains at
`docs/archive/implementation-plan-2026-08-06.snapshot` for audit only. It is not
authoritative. Do not copy ownership or task status from it.

Every pull request that changes a task below must update this plan when it
changes status, remaining scope, ownership, verification, compatibility,
migration or degraded-mode behavior. A task is `done` only after implementation,
integration, migration/backfill, tests, public contracts, documentation and
required runtime evidence are complete. Source-ready slices remain
`in_progress`.

## Scoped functional remediation — 2026-10-17

Source changes on `arena/d189c701-rustok`, based on main
`f8dd78dc807e93cae21bea608f43bc8c2c9f9d5a`; not integrated and not runtime-verified:

- Leptos composer state and command construction now belong to framework-independent `core.rs`.
  Topic/parent/mode switches preserve separate in-memory drafts, category selection preserves the
  current text or restores an occupied draft, and pending submissions freeze editor/target changes.
  Six pure Rust regression tests cover isolation, restoration, request capture, failed retry,
  successful cleanup and incomplete drafts.
- Next-admin topic selectors now consume every owner `nextCursor` page, preserve tenant/auth/locale,
  deduplicate overlapping topic identities and reject stalled/incomplete cursor streams. The obsolete
  `first: 100` candidate cap is removed from all five operation-page callers; topic and reply pages
  consume the same complete list. Five executable JS transport tests cover multi-page selection,
  overlaps, empty lists, cursor failures and upstream errors.
- Standalone topic/reply quote replacement now applies the canonical author edit window. REST and
  GraphQL runtime composition inject the existing settings provider rather than constructing an
  unconfigured quote service. A SQLite owner regression covers both targets, expired/open/unlimited
  windows, moderator exemption, settings failure and no revision writes on rejection.

Second packet (findings 5, 7, 10 and 11) is also source-ready:

- Next reply drafts are persisted and validated against the exact `(topicId, parentReplyId)`
  within the existing tenant/viewer namespace. Top-level and distinct parent replies no longer
  overwrite each other, and success removes only the submitted target. Destination reads are
  validated before changing editor state; corrupt/unavailable storage blocks switching and
  reports a localized error, including when the composer is closed. Old topic-only reply keys
  are left untouched and are not auto-restored: their missing parent cannot be inferred safely.
- Next composer preview reuses the same canonical discussion richtext frame with dynamic
  read-only state and no toolbar, rather than flattening nested nodes or rendering quote
  relations twice. This is editor preview, not a replacement for server-owned public projections
  (see `DECISIONS/2026-07-22-richtext-capability-boundary.md`).
- Both thread retry controls now retry categories, thread content/member cards and a failed
  read receipt. Category request generations reject stale completions. Receipt state distinguishes
  pending/succeeded/failed: effects never loop on failure, concurrent clicks cannot duplicate a
  pending write, and only explicit retry repeats a failure. Viewer/tenant response isolation remains.
- Next-admin reply success clears the submitted document while retaining its content locale;
  failure preserves input and releases the retry lock. It reads current frame input rather than a
  stale render snapshot, rejects double submissions, preserves genuinely newer input and ignores
  completions from an unmounted topic/locale/tenant/auth editor session.

Third packet (findings 8 and 9) is source-ready:

- Inline `ThreadPanel` and full-page topic navigation now share a framework-independent cursor
  walker and a React DOM-position adapter. “Jump to end” follows the existing audience-scoped
  reply read, not an unloaded DOM index or the topic aggregate count. Each explicit click reads
  at most 20 additional pages; a partial walk keeps its cursor, focuses the loaded frontier and
  displays a localized continuation notice rather than claiming it reached the end. Ordinary
  Load more still reads one page. Cursor cycles and transport errors remain visible failures.
- A synchronous pending lock covers both load-more and jump actions. Snapshot refreshes invalidate
  the old walk; viewer/tenant/topic changes stop it after the pending page, before another read or
  stale commit. Scroll runs after the new DOM is committed, once per explicit jump, and cannot
  replay on an ordinary append. The opening-link `?reply=` budget/contract remains unchanged.
- Reading position is measured inside the mounted thread root, with frame-coalesced scroll/resize
  updates and logarithmic post-position lookup, including tall posts. Same-length replacement
  snapshots rebind DOM nodes. The inline timeline no longer hardcodes post 1. Cursor exhaustion
  supplies the exact visible denominator; before then the owner count is bounded below by the
  loaded prefix. Busy controls and accessible progress values reflect the actual navigation state.
- The English/Russian continuation message is catalog-owned. Message declarations were regenerated
  with the host's pinned next-fluent CLI (`14f870bb900069762054dc882427c654a69822be`), not edited by hand.

Fourth packet (finding 6) has source changes under
[`2026-10-17-forum-topic-feed-selection`](../../../../DECISIONS/2026-10-17-forum-topic-feed-selection.md)
(Accepted, implementation In progress):

- The canonical topic filter now carries search/order/solved. Top orders stored reply counts
  before the existing Latest keys; search uses the actual hydrated locale title/slug and Solved
  uses the canonical solution relation. Sparse selections refill from bounded server candidate
  batches before audience/unread output pagination; no browser-wide scan or invented count.
- Cursors carry rank and selection/context scope; query/order/solved, tenant/viewer, locale/fallback,
  category/status, channel/surface mismatches are rejected. Page size is excluded. The separate
  read-model owner rejects scoped/ranked cursors. Old opaque tokens have no compatibility decoder.
- REST and all three GraphQL list variants delegate to the same owner fields. Native and GraphQL
  Leptos adapters carry/retain the same typed selection across initial and continuation reads;
  current Leptos UI still requests Latest. Migration 39 adds global/category Top keyset indexes;
  production index creation needs the deployment owner's lock/maintenance-window assessment.
- Next controls now request owner-selected pages, reset feed identity on every selection change,
  debounce text input and ignore stale results/errors, while preserving owner order. The loaded-only
  warning and local search/Top/Solved transforms are retired; empty search/unread/error states remain
  distinct. Catalog-owned search-clear accessibility text was generated with the pinned Fluent CLI.
- Six new API and ten selection/lifecycle JS regressions execute production functions. Rust source
  regressions cover a sparse match beyond 100 candidates, hidden matches, Top ties/paging, Solved,
  displayed-locale-only matching/fallback, invalid context, failures, DTO normalization/cursors,
  schema/native projections and unread/read-model composition. **These Rust tests have not run.**

Requested recheck (same branch/base; no phase completion) found and corrected seven further
frontend defects in these changes/adjacent touched flows:

- A delayed receipt/post could call an old feed callback, increment the shared request generation
  and discard the current selection's pending result, leaving its loader stuck. An old detail
  callback could likewise cancel another thread's current detail/reply request. Both entrypoints
  now reject obsolete scope before touching generations, pending flags or loading state.
- Whitespace-only search edits changed callback/effect dependencies even though the feed key and
  owner selection were unchanged; this unnecessarily replaced appended pages with a first page.
  The callback, debounce and key now use the same trimmed input, while the input keeps raw text.
- QuoteTooltip optimistically assumed `openReply` succeeded even when destination draft restoration
  failed. `openReply` now acknowledges a successful switch with a boolean; pending/stale-viewer,
  save and restore failures return false without editor mutation. No lookup/quote append follows
  a rejected switch. Tooltip unmount also invalidates pending quote results and their DOM cleanup,
  preventing an old selection from modifying a still-live composer after its tooltip is gone.
- Next composer title and quote-removal controls remained editable during a write although the
  frame/category were frozen; successful cleanup could discard those newer edits. They now share
  the same pending lock. A synchronous submit ref additionally rejects duplicate form submissions
  before React commits the pending state, and releases on failure for retry.

The failing-before regressions were executed before fixes (including a separate stale-detail case
and duplicate reply/topic submissions). Twelve new tests now cover ten production-callback/draft
scenarios plus two source-wiring assertions. **First recheck evidence: 106 scoped Forum JS + 14
ownership-verifier tests pass (120 total); host scripts separately pass 96 storefront and 12 admin
tests.** The scoped TS 6.0.3 semantic check and six source guards pass; `git diff --check` is clean.
All 42 modified Rust files parsed through WASM rustfmt without edits; that is not Rust compilation,
macro expansion, migration execution or an owner runtime test. Cargo remains unavailable, and
PostgreSQL, mounted React/browser and sparse-search performance evidence remain open. No product
dependency change or temporary node_modules link remains.

Continued user-requested inspection (same branch/base, 2026-10-17) reproduced and corrected
three further Next storefront draft defects:

- Closing a scoped editor ignored a failed final save and could replace fresh in-memory text
  with an older stored draft on reopening. A failed flush now keeps the editor active/open;
  successful saves and unscoped memory-only closing still work.
- Saved richtext validation checked only the root, admitting malformed nested nodes and unsafe
  link marks. Draft parsing now reuses `@rustok/richtext`'s canonical discussion-profile validator
  with `allowEmpty: true`. Invalid destinations fail before replacing an editor or mutating
  category draft storage. No parallel schema/profile or product dependency was added.
- Post-success `removeItem` failures were swallowed, allowing already-posted content to be
  restored. The composer now reports cleanup failure with a cleanup-only retry action. A private
  per-session pending-key set prevents restoration until deletion succeeds; both direct opening
  and category switching retry cleanup before reading that key. A successful fresh save supersedes
  pending cleanup for its key. This set is temporary recovery control state, not another persisted
  draft store. **Blocked browser storage cannot be durably repaired across reload, account/session
  remount or another tab**; the warning explicitly requires retrying cleanup before reloading.
  This is not server mutation idempotency or cross-tab conflict resolution.

Four new tests (including the category-switch bypass) were executed failing before their respective
fixes. They exercise production callbacks and actual shared richtext validation/profile code;
closed cleanup-alert/retry wiring also has source assertions. **Draft-storage-pass evidence: 110 scoped Forum
JS + 14 ownership-verifier tests pass (124 total); host scripts pass 100 storefront / 12 admin.**
Scoped TS 6.0.3, six source guards, three i18n verifiers and `git diff --check` pass; existing i18n
baselines remain warnings, not browser proof. These callback tests are not mounted React tests.
No additional Rust/runtime evidence, product dependency changes, phase completion or acceptance
of the original 11 findings is implied. Browser, Rust/PostgreSQL and sparse-search scale checks
remain outstanding.

The next user-requested continuation (same branch/base, 2026-10-17) corrected three adjacent
asynchronous composer defects:

- A quote lookup checked only mode/target IDs and open state. Closing and reopening the same
  target could therefore accept an old exact-revision quote or its text-only fallback. The
  composer now owns a private editing-lifetime generation and exposes `captureEditorSession`.
  Successful open/target replacement, category change, close and post-success reset invalidate
  captured lifetimes. Rejected transitions and same-category/minimize/expand actions do not.
- The form's synchronous duplicate-submit lock did not cover context mutations before React's
  pending render. The context now owns a synchronous submission lock, with `isSubmitting` as its
  render projection. Opening/replacing targets, category selection, title/document updates and
  quote append/removal cannot modify a pending submission's editor; controlled updates also
  reject inactive/obsolete viewers. Releasing a failed write permits editing/retry again.
  Quote completions consult the captured owner guard, not only the last rendered pending flag.
- A late quote's final DOM cleanup could clear a newer selection and hide its quote action,
  including after editor reopening. Cleanup now requires the same editor lifetime/target and
  the same selection-change generation. Its in-flight lock still releases when cleanup is skipped;
  a valid original quote continues to use its captured text, not the later selection.

Twelve additional tests cover these paths: nine new cases reproduced failures without their fixes,
plus three controlled-edit/lifetime/retry checks. Existing source-wiring assertions require the
owner lock/guard and selection counter to remain connected. **Asynchronous-composer-pass evidence: 122 scoped Forum
JS + 14 ownership-verifier tests pass (136 total); host scripts pass 112 storefront / 12 admin.**
The scoped TS 6.0.3 check, six source guards, three i18n verifiers and `git diff --check` pass with
the existing i18n baselines. Tests execute production callback bodies with injected lifecycle/DOM
bindings, not a mounted React/browser lifecycle. No persistence schema, server idempotency,
product dependency or accepted architectural boundary changed. Browser, Rust/PostgreSQL and
sparse-search performance evidence are still open; this is not phase completion or runtime
acceptance of the original findings.

Continued frame inspection (same branch/base, 2026-10-17) corrected two related defects:

- A queued richtext update could replace a different draft after a successful target/session
  switch. `ComposerState.editorSession` is now an explicit render projection of the existing
  composer-owned generation. Each committed lifetime updates that projection and keys its own
  `RichTextEditor`; the document callback also captures the lifetime before updating state.
  Normal keystrokes/read-only toggles do not advance it. Existing rejected-transition and
  cleanup/retry lifetime checks now verify the render projection as well.
- The shared controller still accepted queued document/focus/error callbacks and handshake
  callbacks after disposal. The fix belongs to `packages/richtext`, not a Forum protocol fork:
  destroyed controllers now ignore both entrypoints. Its canonical build regenerated tracked
  bundles and immutable frame assets/manifest. See the shared richtext plan for build limits.

Six new checks include three failing-before behavioral regressions, a failing-before frame-key
wiring assertion and two positive editing cases. **Frame-pass evidence: 125 scoped Forum JS + 14
ownership-verifier tests (139), plus 13 shared-richtext tests, all pass (152 combined).** Host
scripts pass **115 storefront / 12 admin**. Scoped Forum TS and the full richtext package semantic
TS 6.0.3 check, richtext generated-contract verification, six source guards, three i18n verifiers
and `git diff --check` pass. The shared package's canonical `npm test` build passes, using the
existing frozen dependency graph with forced peer resolution outside the checkout; normal peer
resolution is blocked by the pre-existing mixed Tiptap versions, not repaired in this lifecycle
slice. No product dependency or lockfile change. Callback/channel tests are not mounted browser
evidence, and Rust/PostgreSQL/performance and blocked-storage durability limits remain open.

The subsequent user-authorized dependency pass resolves the earlier installation conflicts:
Tiptap now shares a `3.30.5` cohort; both host React/render/type pairs, ESLint/TypeScript and admin
parser/AJV peers are compatible. Storefront Next is `16.3.8`, matching admin. npm locks and tracked
richtext artifacts were regenerated; four dependency/build tests prevent the previous drift.
Both hosts now pass their **full native `typecheck` and `lint` commands**, plus explicit Forum
module lint, without scratch aliases. Ordinary clean npm installs and installed dependency-tree
checks pass without forced peer resolution; the central richtext plan records CA/prune details.
Dependency-pass scoped combined evidence is **139 Forum/ownership + 17 richtext = 156 tests**, with host
scripts still **115 storefront / 12 admin**. The TypeScript-7 build-ignore workarounds are removed;
admin's Bun lock/Docker path is cut over to npm with correct root/local-package/asset wiring.
Docker is unavailable, so this image path is source-reviewed, not runtime accepted. The prior
whole-host TS/lint and Tiptap peer blockers are resolved; Rust, PostgreSQL, browser, sparse-search
scale and blocked-storage durability limits remain. This does not mark FS-03 or the original
findings runtime-accepted, nor assert that every transitive advisory is fixed.

The next functional continuation (same branch/base, 2026-10-17) fixes two further storefront
composer defects:

- The form submit callback checked pending state and viewer identity, but not its captured editor
  lifetime. A retained callback could send the old topic/reply target and document after closing
  or replacement; after successful cleanup the released local lock could also permit that old
  callback to post again. Submit entry now uses the existing `isEditingSession` guard before
  validation, error updates, locking or any mutation request. Failure/retry behavior for the
  still-current editor is unchanged. This is client lifetime isolation, not server idempotency
  for ambiguous network outcomes or multi-tab retries.
- Composer headings, the fallback topic label and the quotes heading were hardcoded English.
  They now use the English/Russian Fluent catalogs (`replyingTo` with typed `title`, `topic`,
  `quotes`, and the existing `newTopic`). Host message declarations were regenerated by the
  installed pinned Fluent CLI rather than edited manually.

Five new checks cover four production submit-callback scenarios (reply/topic replacement and
post-success invalidation) plus catalog-owned label wiring. The scenarios fail without the
entry guard; the label assertion failed before the catalog cutover. **Current evidence: 144
Forum/ownership + 17 shared-richtext = 161 tests**, with host scripts **120 storefront / 12 admin**.
Native storefront typecheck, host/module lint, six source guards, three i18n verifiers and diff
whitespace checks pass. Callback tests inject lifecycle bindings and are not mounted React/browser
proof. No dependency/storage/protocol change was needed. Docker, browser, Rust/PostgreSQL,
sparse-search scale and blocked-storage durability limits remain open; no audit phase is closed.

PR #4619 integration reconciliation: fetched main at `d3f4c5f8c`, merged it into the fixed
session branch, retained both ADR index additions and the upstream native continuation cleanup.
Upstream dependency updates are retained alongside the verified React/ESLint/TypeScript/Tiptap
cohorts; Next and its ESLint config now align at `16.4.0` on both hosts rather than restoring
main's older storefront Next pin. npm locks were regenerated normally. Both native host
`typecheck`, `lint`, Forum scripts and the combined **161-test** scoped suite pass again.
Base-owned migration/API/release/worker infrastructure approval evaluators report unchanged
protected infrastructure. GitHub checks and merge acceptance remain separate from local proof;
the Rust/PostgreSQL/browser/Docker limitations above still apply.

Fourth-packet evidence (before the requested recheck): **94 scoped Forum JS tests plus 14
ownership-verifier tests pass (108 total)**. Host scripts separately pass **84 storefront** tests
(including the two shared GraphQL tests) and **12 admin** tests. Six scoped source guards and three
i18n verifiers exit successfully (their existing baselined i18n warnings are not new green proof).
The scoped TypeScript 6.0.3 check, including the feed, selection projection and API imports, passes
against exact Next 16.3.6 and real dependencies outside the checkout. Temporary node_modules links
were removed. No product dependency/lockfile change. SQLite executed the **literal migration 39
SQL** twice up/down and used both covering indexes on toy EXPLAIN plans, preserving an unrelated
index; that is not the SeaORM migration chain, owner runtime, PostgreSQL or scale evidence.
The ADR verifier still fails on exactly the same two pre-existing 2026-10-16 ADRs as base HEAD
(reproduced from `git archive HEAD`); the new ADR adds no governance errors.

Earlier packet evidence (retained for provenance): 78 scoped Forum JS tests pass, including 22 new second-packet and 27 new third-packet
regressions; 14 Blog/Forum ownership-verifier tests pass. Host `test:forum` commands pass separately:
68 storefront tests (including two shared GraphQL transport tests, using temporary external Apollo
4.3.1 test dependencies) and 12 admin tests. No product dependency/lockfile changes were needed.
Forum admin/storefront boundary, storefront cursor paging, reply-Reactions storefront composition
and Blog/Forum UI ownership guards pass. The ownership guard and its negative fixtures require
submission from the captured current document; no guard was weakened to accept an old path.
UI catalog parity, UI key verification and the i18n contract verifier pass.

The six third-packet TS/TSX source roots and their imports pass a scoped semantic TypeScript 6.0.3
check using real declared dependencies/owner sources and the host-generated i18n declarations,
with a temporary external compiler config (no stubs or suppressions). This is not the whole-host
Next typecheck. Preview wiring tests and deterministic navigation hook/DOM bindings are not
mounted React/browser rendering evidence.

Modified Rust files were parsed/formatted using WASM rustfmt, not compiled.
`cargo test -p rustok-forum --lib` and `cargo test -p rustok-forum-storefront --lib` remain required
in a Rust-enabled environment. The architecture dependency guard and remediation gate cannot
complete without `cargo`. The installed host environments still lack the `next-fluent` and `eslint`
executables: the whole-host typecheck/lint commands cannot run there. The pinned CLI used for
catalog generation and the scoped compiler/dependencies live outside the checkout; full host
installation, Next integration checks and browser E2E remain pending.
The previous packet established six additional guards failing identically at base `HEAD`:
quote commands (stale runtime/doc markers), mention runtime proof (stale PostgreSQL marker),
and merge/fork/split/slug-rename admin UI (stale Leptos markers/contracts).
These failures are not green evidence for this change.

All eleven original findings now have source changes, not complete runtime acceptance. Required
fourth-packet checks attempted here stop at `cargo: command not found` (exit 127): owner check/lib
and scoped SQLite/schema tests, storefront lib tests and native `--features ssr` check. Maintainers
must also check hydrate/CSR profiles and PostgreSQL owner/migration/EXPLAIN behavior. Sparse search
has bounded memory but can scan many server batches; production latency/buffer evidence is open.
Browser E2E, Rust compilation/owner tests and transport runtime verification remain pending. This
is not completion of the Forum audit, ADR implementation or a sequential audit phase.

Additional host dependency observation (outside these eleven functional findings): installing the
exact declared Next 16.3.6 in isolated verification tools produced npm audit advisories including
high-severity `GHSA-cjq9-62q9-8jv4` (Image Optimization SSRF) and cache/disclosure advisories fixed
in 16.3.8. Runtime applicability was not assessed. The host dependency/lockfile upgrade is not part
of this module navigation packet and requires host-owner remediation before release.

## Current state

Forum owns canonical topic/reply storage, transports, revisions, projections,
and module UI packages. Canonical Forum Category identity, localized copy, route
history, hierarchy/order and presentation are Taxonomy-owned after the verified
CAT-5 cutover; Forum retains the typed same-ID binding plus Forum policy, counters
and lifecycle state. Shared richtext storage and server projection are live;
Leptos and Next topic create/edit plus reply composition use the shared frame,
while storefront topic/reply reads use the editor-free shared `RichTextHtml`
boundary. Leptos reply writes select a native server function for SSR/hydrate
and the canonical GraphQL mutation for CSR/headless use without fallback. Dirty
locale switching is now fail-closed for Forum admin category/topic editors:
unsaved owner-localized content and reply drafts block switching, fallback-only
localized content is scrubbed before writable target-locale state is exposed,
and unchanged Taxonomy tag attachments are preserved. Full mounted save/reload
evidence and the broader product/runtime gates listed below remain open. Previously retained baseline evidence includes native Forum storefront checks, a whole-host Next
typecheck and ownership verifiers; that evidence does not verify the unintegrated scoped
remediation above.
Manifest dependencies between `modules.toml`, `rustok-module.toml`,
`RusToKModule::dependencies()` and `docs/modules/registry.md` are aligned to
`["content", "media", "taxonomy"]. Taxonomy owns the canonical Forum Category persistence boundary.
The WASM storefront check still stops in the pre-existing
`Resource::new_blocking`/non-`Send` GraphQL future and missing direct `web_sys`
dependency path, outside the richtext renderer.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Richtext UI evidence: the shared Next/Leptos topic and reply adapters now
  receive the owner-selected content locale separately from host UI messages,
  propagate derived direction and spellcheck, and follow dynamic form
  busy/read-only state. Package typecheck/unit tests, the Chromium frame harness,
  native Forum admin check, SSR/hydrate checks, and the focused Forum admin
  locale-switch contract cover this slice. Owner-copy i18n and full mounted
  save/reload/browser parity evidence remain open.

## Attachment relation concurrency contract

Forum attachment relations have a current-state relation set whose concurrency identity is
separate from Forum content revisions and from the immutable mention/quote relation
projection revisions.

- `expected_relation_revision` is the compare-and-swap token supplied by the caller.
- `0` means that no attachment relation set has ever been committed for the target.
- A write is accepted only when the expected token exactly matches the current committed
  token; `0 -> 1` is the initial set creation and every later replace advances monotonically.
- Replacing the set with zero attachments is still a committed revision. This preserves
  an explicit tombstone for the empty current set and prevents a stale writer from
  recreating attachments after a clear-all operation.
- `source_revision` remains the Forum topic/reply content revision captured as provenance
  for the relation mutation. Attachment-only mutations must not manufacture or consume
  content revisions.
- The immutable `forum_relation_revisions` stream remains dedicated to mentions/quotes;
  attachment state must not reuse it as a mutable head or CAS row.
- Attachment relation persistence now uses the Forum-owned CAS head and bounded relation rows. A Media admission result alone does not reserve an asset against deletion; the writer acquires durable Media owner references before the Forum commit and releases removed references only after commit. Ambiguous commits and failed post-commit releases intentionally preserve conservative holds for later reconciliation.

Persistence now combines the owner-side CAS with Media durable reference retention. New Media
holds are acquired before the Forum transaction commits; removed holds are released only after
a successful Forum commit. Stable consumer-owned reference IDs are derived from tenant, target,
locale, position and media identity, while usage/caption changes retain the same reference.
Ambiguous commits and failed post-commit releases intentionally preserve conservative holds for
later owner reconciliation. Exact-state retries re-establish Media holds before returning the
already committed set.

## Product model

Forum is an installable domain application composed from platform modules. It
must not recreate common social-platform capabilities inside `rustok-forum`.
Each module owns one capability and Forum contributes only Forum-specific state,
policy, adapters, semantic events and UI composition.

Product inclusion does not imply Forum persistence ownership. Forum may present
profiles, media, reactions, reputation, moderation, notifications, search, SEO,
translation and realtime behavior while consuming those owners through public
contracts.

## Ownership rules

### Forum-owned

`rustok-forum` owns only:

- category membership/binding plus Forum policy, counters and lifecycle state;
- localized topic/reply content, lifecycle, revisions and route identity;
- topic kinds, accepted-solution semantics, subscriptions and Forum read state;
- Forum drafts and bookmarks unless a proven neutral owner replaces them;
- Forum attachment relations, usage, order, caption and source revision;
- Forum trust, posting policy and Forum-local enforcement state;
- Forum subject adapters for reactions and moderation;
- Forum visibility, source authorization, semantic events and projections;
- Forum statistics/activity projections and module-owned admin/storefront UI.

Forum tables may reference another owner by typed tenant-scoped identity, but
must not copy that owner's source-of-truth data or read its private tables.

### Shared owners

| Capability | Authoritative owner | Forum responsibility |
| --- | --- | --- |
| Login identity and sessions | `auth/users` | Carry trusted actor identity only. |
| Public handle, display name, biography, locale and profile privacy | `rustok-profiles` | Batch summaries and compose Forum statistics. |
| Avatar/banner and all binary asset lifecycle | `rustok-media` | Store typed media references and attachment policy only. |
| Follow/block relationship facts | `rustok-social-graph` and profile privacy ports | Request exact bounded facts; never read relation tables. |
| Reaction catalog, actor reactions and aggregate reaction counts | `rustok-reactions` | Publish Forum subject adapter and authorization; consume permitted semantic facts and compose UI. |
| Cross-domain reputation ledger and achievements/badges | planned `rustok-reputation` / achievement capability | Publish semantic facts and display permitted projections. |
| Reports, cases, queues, decisions, appeals and cross-domain audit | `rustok-moderation` through `rustok-moderation-api` | Report subjects and apply validated effects to Forum state. |
| Notification inbox, fan-out, grouping, preferences, digests and deliveries | `rustok-notifications` | Publish source events/providers and authorize current targets. |
| Translation workflow | `rustok-translation` | Publish exact Forum-owned topic/reply targets only when explicitly onboarded; Category copy uses Taxonomy's registered `taxonomy/term` provider. |
| Canonical Category identity, hierarchy, localized copy, routes/aliases and presentation | `rustok-taxonomy` | Keep the typed same-ID binding plus Forum policy/counters/lifecycle; consume canonical Taxonomy Category state and never register a duplicate `forum/category` Translation target. |
| Search storage and retrieval | `rustok-search`; generic indexing in `rustok-index` | Publish visibility-safe projections and repair sources. |
| SEO aggregation and host head composition | `rustok-seo` | Publish canonical Forum targets and structured semantics. |
| Durable event delivery | `rustok-outbox` / `rustok-events` | Commit Forum state and semantic events atomically. |
| Realtime transport | shared host/runtime capability | Publish revisions/cursors and reload canonical owner state. |
| Import orchestration | shared bounded import framework when available | Provide Forum mapping, validation, receipts and reconciliation. |

### Non-duplication gates

Without a platform ADR and ownership migration, the following are forbidden:

- copied Profile identity, avatar or a second member directory;
- Forum-owned uploads, storage keys, delivery URLs, quarantine or Media deletion;
- Forum-owned report/case/appeal queues or a second cross-domain moderation log;
- Forum-owned notification inbox, preferences, grouping, digests or delivery;
- a Forum-specific reaction catalog after `rustok-reactions` composition;
- a duplicate Forum Category Translation provider, change cursor or localized donor mirror after the accepted Taxonomy cutover;
- a reusable Forum-only reputation ledger or universal badge catalog;
- transport-local visibility/reaction/moderation policy;
- direct reads of another module's persistence tables.

Optional capabilities must expose explicit unavailable/degraded behavior. Their
absence must not break unrelated Forum owner commands.

## Current verified baseline

Forum already provides categories, localized topics/replies, lifecycle,
revisions/tombstones, bounded reads, subscription levels, accepted solutions,
tags, Forum statistics, transactional events, visibility-aware Search/SEO,
shared rich text, module-owned admin/storefront packages and Page Builder
contracts.

Topic and reply storefront reads now render the typed server-derived
`RichTextView` through the shared `RichTextHtml` boundary with the effective
content locale. Forum UI no longer owns direct `inner_html` sinks, and the
editor/Tiptap runtime remains absent from read-only rendering. This contract is
locked by `verify-forum-storefront-boundary.mjs` and its negative fixture.

Forum Page Builder contribution metadata, Fly identity, owner-preview and
owner-property source are now source-ready. Canonical `rustok-module.toml` owns
two complementary version-pinned admin contributions: `rustok.forum.widget-catalog`
admits the `forum.topic_list`, `forum.topic_detail` and `forum.reply_stream` blocks
plus owner-schema-reference property editors under `tree + properties`, while
`rustok.forum.widget-preview` admits only their renderer contracts under
`preview`. `rustok-forum-admin/build.rs` consumes the shared platform contribution
normalizer, and the Forum admin package registers real Fly component/block
identities plus a `ContributionAdapter` without reading Forum owner data.

Owner preview data remains Forum-owned end to end. `ForumWidgetPreviewService`
first normalizes configuration through the existing widget contract, then applies
Forum visibility/RBAC and bounded owner reads for all three widget types. The
HTTP owner route `/api/forum/widgets/preview`, SSR-only Forum admin transport and
provider-neutral Page Builder contribution host compose that source into the real
Pages admin route only when Forum is tenant-enabled and its exact manifest
permission is effectively granted. `preview_off` filters the renderer
contribution without removing `tree + properties`.

Owner-backed property editing is also source-ready. Contribution metadata keeps
only `forum_widget_owner_schema_ref_v1` pointers; the Forum admin property
transport verifies the exact generated descriptor, loads the current schema body
from `ForumWidgetContractService::catalog`, and validates candidate configuration
through `ForumWidgetContractService::validate_props`. The provider-neutral Page
Builder property panel supports the current bounded Forum schema subset and
applies only valid owner `normalized_props` through the ordinary Fly
`EditorCommand::Patch` history path. It rechecks selection/schema identity around
asynchronous validation and never writes Forum owner data, tenant identity or actor
identity into the document. Retained runtime/browser evidence and the observed
tenant Wave remain open; no source may treat source-ready host composition as
observed rollout proof.

Profiles, Media, Social Graph, Reactions, Moderation, Notifications,
Translation, SEO, Search/Index, Outbox/Events, Taxonomy, Workflow, Comments,
Groups and Channel are separate platform capabilities. New Forum work must
integrate them instead of cloning their data models.

The Reactions owner now has neutral bounded API contracts, unique source
provider/factory registries, PostgreSQL/SQLite-compatible tenant-composite
persistence, immutable catalog snapshots, shared Outbox command receipts,
atomic actor-state/aggregate updates, sealed semantic reaction events and
bounded aggregate reconciliation. Forum publishes a source-ready `topic`/`reply`
provider factory with exact active-state, visibility and current-revision
authorization plus a bounded single-`like` v1 catalog. Blog now supplies the
second real producer through the same neutral SPI using Blog-owned publication,
channel visibility and owner version, with a Blog+Reactions composition profile
in source. Optional owner selection, host materialization and executable source evidence for all three optional profiles are source-ready; retained execution evidence remains pending. A bounded manifest-composed GraphQL read/write
transport over the neutral Reactions ports and the separate module-owned
`rustok-reactions-storefront` reaction controls are source-ready. Forum exposes
generic visibility-gated `forumStorefrontTopicCurrentRevision` and
`forumStorefrontReplyCurrentRevision` owner facts and carries them through a
dual-path storefront transport facade: native server functions for SSR/hydrate
and GraphQL for headless/CSR. The Forum storefront publicly exposes those
generic facade functions as neutral host-extension facts. The storefront host
now composes one exact Forum reaction target at a time: the selected canonical
topic by default, or the one valid explicitly selected reply when the canonical
topic route carries `?reply={reply_id}`. In both cases the host combines only the
Forum identity/current-revision fact with `ReactionSubjectUiRef` and mounts the
separate module-owned `ReactionBar` when Reactions is enabled. It never fans out
revision requests across the visible reply list. Forum owner/storefront packages
still do not depend on the Reactions owner or presentation package. Rust
Playwright source now covers the observable selected-topic and selected-reply
composition markers against maintainer-supplied real storefront URLs; browser
execution is still pending. The current lockfile contains the
`rustok-reactions-storefront` package entry; event-digest generation and retained
owner/provider/schema/runtime/UI evidence remain pending.

Forum now also publishes source-ready `rustok-moderation-api` subject adapter
factories for `forum_topic` and `forum_post` without depending on the Moderation
owner crate. FORUM-19 adds dedicated tenant-scoped Forum moderation subject
revision clocks for topic/reply core, content, lifecycle and local enforcement
state instead of reusing the narrower Reactions/content revision. The bounded
application source reuses the shared Outbox owner-operation receipt ledger,
fences the active subject plus that revision clock, accepts only trusted
service/system application callers and now supports `NoDomainMutation` for both
subject kinds, permanent topic lock, exact `SetVisibility(Hidden)`, exact
`SetVisibility(Removed)` and exact `RejectPublication` for Forum replies.
Approved-to-hidden and approved-to-rejected use the established
topic/category/author public counter accounting and every changed lifecycle
mutation writes the canonical `ForumReplyStatusChanged` event; already-hidden
and already-rejected are no-ops. Removed reuses the complete
`ReplyService::remove_in_tx` owner path for accepted-solution cleanup,
soft-delete/tombstone capture, public/solution accounting and the canonical
deleted status event/projection in the same fenced receipt transaction.
`SetVisibility(Unpublished)` remains fail-closed and distinct from
`RejectPublication`; it is not approximated as `ReplyStatus::Rejected`.

The optional server host source-materializes the neutral Moderation adapter
registry when `mod-moderation` and `ModerationModule` are both selected. A
Moderation-only profile materializes an empty registry; Forum+Moderation
materializes exactly the topic/reply adapters; Forum without Moderation remains
valid. The Moderation owner has source-ready durable application operations,
one-attempt dispatch, shared module-work scheduling, atomic application/case
audit lifecycle and bounded replay-safe operator recovery. Human operator
requeue is limited to `rejected`/`operator_review`, preserves the immutable
decision UUID and returns work to the existing scheduler/dispatcher; `applied`
can never be requeued. Pre-audit terminal rows can be reconciled to Moderation
case state without invoking Forum or fabricating historical lifecycle facts. A
true stale-revision re-review is explicitly a new case/new immutable decision on
a fresh producer-supplied revision, never a retargeted old decision. Authorized recovery GraphQL transport/RBAC and fresh-revision re-review are now
present, and the repository retains the PostgreSQL/SQLite, scheduler, host-composition,
lost-response and Forum concurrency/accounting evidence. Deployment-dependent promotion
is deferred to the final production-validation phase.

## Program ledger

| Task | Status | Current result and remaining deliverable |
| --- | --- | --- |
| `FORUM-00` | `done` | PostgreSQL/SQLite runtime baseline. |
| `FORUM-01` | `done` | Tenant-composite integrity and locale width. |
| `FORUM-02` | `done` | Typed topic/reply lifecycle and revisions. |
| `FORUM-03` | `done` | Atomic category writes and translations. |
| `FORUM-04` | `done` | Tree, placement, policy, subtree lifecycle and admin DnD. |
| `FORUM-05` | `done` | Serialized publication-aware counters. |
| `FORUM-06` | `done` | Locked topic and moderation publication semantics. |
| `FORUM-07` | `done` | Monotonic reply positions. |
| `FORUM-08` | `done` | Revisions, tombstones and owner soft-delete paths. |
| `FORUM-09` | `done` | Versioned Forum event catalog and journal. |
| `FORUM-10` | `done` | Bounded cursor reads. |
| `FORUM-11` | `done` | Subscription levels and participation policy. |
| `FORUM-12` | `in_progress` | Mention/quote relations plus the topic-created, topic-replied and user-mention notification sources exist: reply fan-out resolves topic-level watchers, then the owning category's watchers, with a per-recipient visibility recheck and a reply deep link (landing on the topic; reply anchoring remains under the Search/SEO/deep-link work). Runtime execution, profile/block privacy, moderator audience, localized templates (`NOTIFY-06`) and final Notifications evidence remain. |
| `FORUM-13` | `in_progress` | Forum-specific Category presentation ownership is retired: canonical icon, color and typed Media presentation references are owned by Taxonomy under TAXONOMY-CAT-3/CAT-5. Forum must not reintroduce a duplicate presentation validator, storage model or Category-cover owner. Remaining mounted/runtime evidence belongs to the Taxonomy/CAT-5 production-validation gate. |
| `FORUM-14` | `in_progress` | Forum attachment relations over Media-owned assets are implemented through a dedicated CAS head, bounded ordered rows and durable Media owner reference retention. `source_revision` is content provenance only; attachment-only changes do not advance Forum content revisions. Runtime integration/reconciliation evidence remains open; FORUM-33 now audits conservative orphan holds through the public Media owner-reference listing contract. |
| `FORUM-15` | `in_progress` | Profiles supplies `ProfilesReader`; FORUM-15A through 15E provide member-card owner service, user stats, GraphQL/native transport, and privacy-aware storefront UI composition. Retain live runtime evidence. |
| `FORUM-16` | `in_progress` | Read state, unread projections, bounded bulk owners and transports exist. Visibility-scoped storefront bulk commands and PostgreSQL evidence remain. |
| `FORUM-17` | `planned` | Forum drafts/bookmarks with optional Notifications reminders and Media references. |
| `FORUM-18` | `in_progress` | Neutral API, optional owner registration/selection, tenant-composite persistence, shared receipts, atomic actor aggregates, semantic reaction events, bounded aggregate reconciliation, Forum topic/reply provider, Blog second producer, host materialization, composition-test source, bounded Reactions GraphQL transport, separate module-owned Reactions storefront controls, dual-path generic visibility-gated Forum topic/reply current-revision transport, bounded selected-topic/selected-reply host UI composition and Rust Playwright browser-evidence source are ready. Retain the browser execution plus event-digest, owner/event/repair/Forum+Blog/GraphQL/UI/runtime evidence and release lockfile verification; Forum votes remain separate and no reaction ownership moves into Forum. |
| `FORUM-19` | `done` | Bounded Forum/Moderation integration is complete: neutral topic/reply adapters, Forum-owned revision fencing and exact supported effects, Moderation-owned durable dispatch/scheduler/audit/recovery, dedicated `moderation_cases` RBAC, authorized recovery GraphQL transport, replay-safe fresh-revision new-case/new-decision re-review, and retained PostgreSQL/SQLite/host/concurrency/lost-response/accounting evidence are present. `Unpublished`, temporary restrictions and broader neutral effects remain separate future feature scope rather than lossy approximations. Deployment/promotion evidence is deferred to `PROD-FORUM-19`; Moderation keeps cases, decisions, appeals and audit. |
| `FORUM-20` | `in_progress` | FORUM-20A-G provide rich visibility and recipient-aware source/inbox slices. Reply anchoring on the Next.js surfaces is delivered through the `?reply=<uuid>` route contract (`DECISIONS/2026-10-11-forum-reply-anchor-route-contract.md`); remaining: an owner-side read for the page containing a reply, remaining reads, reconciliation, delivery and PostgreSQL evidence. |
| `FORUM-21` | `in_progress` | A-X provide move/merge/split/fork/range owners, transports and UI. Retained runtime evidence remains. |
| `FORUM-22` | `planned` | Forum-owned Q&A/wiki/announcement kinds and scheduled lifecycle. |
| `FORUM-23` | `in_progress` | Search projection/filtering, ordering, owner revisions and repair exist. PostgreSQL/Iggy and cross-module evidence remain. |
| `FORUM-24` | `in_progress` | A-S plus descriptor correction provide category/topic routes, transports, mounts, SEO/hreflang and Search URLs. Execute registered-host, HTTP, browser and reindex evidence. |
| `FORUM-25` | `done` | Category backend/Translation implementation is complete: canonical Category copy/routes/hierarchy are Taxonomy-owned, the duplicate Forum provider/cursor/progress runtime and legacy donor storage are retired, and Arabic/admin/storefront locale boundaries are source-ready. Mounted multilingual/RTL browser execution is deferred to the final production-validation phase and does not keep FORUM-25 open; topic/reply UGC translation remains opt-in rather than an automatic provider. |
| `FORUM-26` | `in_progress` | Forum trust/posting facts exist. Add Moderation/Reputation facts, persistence/enforcement, shared rate limits, transports/UI and evidence. |
| `FORUM-27` | `planned` | Compose Profiles directory/profile with Forum stats/activity and permitted reputation/achievements. |
| `FORUM-28` | `in_progress` | Canonical storage, renderer, projections, shared editor runtime, Leptos and Next topic create/edit and reply composition, native SSR/hydrate plus GraphQL CSR reply writes, owner content-locale/direction/spellcheck propagation and dynamic read-only state are implemented. Dirty locale switching is now fail-closed for category/topic editors, fallback-only localized form state is scrubbed before writes, unsaved reply drafts block locale changes, and unchanged Taxonomy tag attachments are preserved. Complete owner-copy i18n and mounted save/reload/browser parity evidence. |
| `FORUM-29` | `planned` | Shared realtime transport with Forum cursors/revisions and canonical reload. |
| `FORUM-30` | `planned` | Complete Forum admin by composing Forum and shared owners. |
| `FORUM-31` | `planned` | Complete Forum storefront by composing Profiles, Media, Reactions, Notifications and Search. |
| `FORUM-32` | `in_progress` | Generated Forum Fly blocks/renderers/property contracts, Forum-owned preview service/HTTP/native transport, provider-neutral Pages host composition and owner-backed schema/validation property editing are source-ready. Retained runtime/browser evidence and observed Page Builder Wave evidence remain. |
| `FORUM-33` | `in_progress` | Bounded snapshot-consistent owner counter and accepted-solution reconciliation with independent keyset cursors, strict operator GraphQL/owner admission and baseline platform telemetry are source-ready, with mention, subscription and attachment-hold reconciliation slices implemented. Retain SQLite/PostgreSQL execution evidence; write repair remains blocked on dry-run/audit/idempotent job state and CLI integration awaits a synchronized dependency/lock update. |
| `FORUM-34` | `in_progress` | Forum import/export adapter and NodeBB mapping; FORUM-34A through 34Q (export inventory/planner/reader/mapping, import inspection/relation-prep/resolution/tombstone/write) are source-ready, awaiting shared migration runner integration. |
| `NOTIFY-00` | `in_progress` | Neutral API/runtime composition and Forum providers exist; executable distribution evidence remains. |
| `NOTIFY-01` | `in_progress` | Persistence/source inbox exist; final commands, migrations, retention and reconciliation remain. |
| `NOTIFY-02` | `planned` | Preferences, quiet hours and digests. |
| `NOTIFY-03` | `in_progress` | Source acceptance/candidate materialization exist; policy, final rows, delivery and lease evidence remain. |
| `NOTIFY-04` | `planned` | Owner inbox/read APIs and reconciliation of existing slices. |
| `NOTIFY-05` | `planned` | Delivery provider SPI. |
| `NOTIFY-06` | `planned` | Localized semantic templates. |
| `NOTIFY-07` | `planned` | Privacy, blocking and target-open authorization. |
| `NOTIFY-08` | `planned` | Notification UI and parity evidence. |
| `NOTIFY-09` | `planned` | FBA/degraded profiles. |
| `LINK-FORUM-01` | `planned` | Forum-to-Notifications proof. |
| `LINK-FORUM-02` | `planned` | Profiles/Media/Forum proof. |
| `LINK-FORUM-03` | `planned` | Forum/Search/Index ordering and visibility proof. |
| `LINK-FORUM-04` | `planned` | Capability profiles and startup validation. |
| `LINK-FORUM-05` | `planned` | Waiver-free production release gate. |

## Corrected task boundaries

### FORUM-13/FORUM-14: Media and Category ownership

Media owns upload, blobs, MIME, dimensions, renditions, quarantine, deletion,
delivery and reconciliation. Forum stores typed tenant-scoped attachment
relations, Forum usage/order/caption and source revision. Canonical Category
identity, localized copy, hierarchy, routes, aliases and presentation are
owned by Taxonomy.

#### FORUM-13 status

Forum-specific Category presentation ownership was superseded by
TAXONOMY-CAT-3 and retired during the CAT-5 Forum Category cutover. Canonical
icon, color, image/cover Media identities, presentation revision and their
validation boundary belong to Taxonomy. Forum must not reintroduce a duplicate
presentation validator, persistence model, Media capability error surface or
Category-cover owner command.

The remaining multilingual/RTL browser packet is production-validation
evidence for the Taxonomy-owned Category surface, not additional Forum
implementation scope.

#### Delivered in the retired pre-cutover FORUM-13 scope

The historical FORUM-13A/FORUM-13B Category presentation implementation is no
longer part of the active runtime contract. Its old cover candidate,
Media owner-port resolver and Forum-local icon normalizer were removed when
Category ownership moved to Taxonomy.

#### Delivered in `FORUM-14B`

- Forum persists a tenant/target/locale attachment relation head separately from Forum content revisions.
- The head advances with exact compare-and-swap semantics; an initial write creates revision 1 and a clear-all mutation still commits a new revision.
- Relation rows are Forum-owned and store only relation identity, Media identity, usage, order and caption.
- The requested content provenance is validated before Media retention and revalidated after the Forum target row is locked inside the relation transaction.
- Removed Media references are released only after Forum commit; failures leave a conservative hold and do not roll back the already committed Forum state.
- Stable Media reference IDs are deterministic across request retries and do not change when usage or caption changes.
- Database constraints enforce tenant ownership, valid target kind, positive revisions, monotonic relation revisions, immutable relation rows, bounded positions and unique per-target ordering.
- FORUM-33 now provides a read-only attachment-hold diagnostic that enumerates Media-owned `forum` holds through the public owner contract and flags orphan or Media-ID-mismatch state without reading Media persistence directly.

### `FORUM-15`/`FORUM-27`: Profiles

Profiles owns public member identity and directory. Forum adds bounded stats,
trust and activity. Author lists use batched `ProfilesReader` calls or a
revisioned reconciled projection. Forum never stores copied profile source data.

### `FORUM-18`: votes, reactions, reputation and achievements

Existing Forum votes remain Forum semantics and must be hardened independently.
`rustok-reactions` owns reusable reaction catalogs, actor state, shared-receipt
command execution, aggregate projections, semantic reaction events and bounded
aggregate repair. Forum owns topic/reply existence, current revision, visibility
and reaction-policy authorization through its provider.

The Forum provider factory supports `topic` and `reply`, checks tenant/source/
kind, active soft-delete state, approved/open lifecycle and the existing rich
audience visibility service, and returns one bounded single-selection `like`
catalog. The `topic`/`reply` provider factory current revision is
`latest captured Forum revision id + 1`, so the identity advances with captured
topic translation/metadata and reply-body edits. Missing and denied targets
share one `Unavailable` result; revision conflict is returned only after current
visibility succeeds. Delegated service/system access uses the existing exact
recipient-context port rather than inventing profile or authority storage.

Forum registers only the neutral provider factory and depends only on the API
crate through an explicit path. It does not depend on the Reactions owner and
does not add Reactions to Forum module dependencies. The optional `mod-reactions`
feature selects the owner independently in the distribution/server, remains outside
defaults, and materializes the Forum provider only after host audience and
recipient-context facts exist. This is the Optional owner selection and host materialization boundary.

The Reactions-disabled Forum composition remains valid: Forum commands and reads
continue without owner storage or a materialized reaction registry. Reactions
without Forum or Blog materializes an empty source registry; Forum with Reactions
materializes the `forum` source with `topic` and `reply` kinds. Blog is the
second real producer and materializes the `blog` source with `post` while using
Blog-owned positive version, `published` lifecycle and typed channel visibility.
Forum and Blog both depend only on the neutral API, so no producer implies the
Reactions owner. Executable source evidence exists for these profiles and the
selected-feature/missing-owner failure, but retained execution evidence remains
pending.

A changed Reactions command commits actor state, aggregate deltas, one sealed
`reactions.actor_state.changed` event and its completed owner receipt atomically.
No-op and receipt replay do not publish another fact. Reactions repair is bounded
to one exact stored subject and rebuilds only aggregate projection rows from
valid actor selections under the immutable current catalog. Missing/corrupt
catalog or actor-state corruption blocks repair; the repair never mutates Forum
content, visibility, lifecycle, votes or producer-private state.

The first Reactions transport slice is manifest-composed GraphQL over the
existing neutral read/write ports. Tenant comes only from `TenantContext`;
subject source/kind/UUID/revision remain caller input, while actor identity is
never accepted from the caller. Anonymous reads request public state only,
authenticated human reads may include their actor state, and writes require a
human-user principal. The mutation command UUID is also the owner idempotency
key. Producer visibility, lifecycle, revision and catalog decisions remain in
Forum/Blog providers, and GraphQL does not read producer-private storage.
Positive revisions and aggregate counts are exposed as decimal strings to avoid
GraphQL integer-width truncation. This is source-ready transport foundation, not
a frozen presentation contract; retained schema/runtime evidence remains.

The presentation owner is also separate. `rustok-reactions-storefront` provides
module-owned neutral controls over the Reactions GraphQL transport and has no
Forum/Blog/private-owner dependency. Forum does not copy those controls. Instead,
Forum exposes `forumStorefrontTopicCurrentRevision` and
`forumStorefrontReplyCurrentRevision` as generic Forum owner facts after the
same tenant/channel/audience gates as the corresponding storefront reads. The
Forum storefront facade selects native server functions for SSR/hydrate and
GraphQL owner fields for headless/CSR, preserving FFA transport parity while
returning only positive decimal revisions derived from Forum revision history.
Those generic facade functions are publicly exported for neutral host
extensions; neither adapter creates reaction subjects, catalogs, actor state or
commands. `apps/storefront` owns the bounded cross-module presentation: it routes
Forum rendering through `ForumStorefrontComposition`, checks the tenant's
`reactions` module enablement and consumes only the generic Forum revision
facades. An explicit valid reply selection on a Forum topic route takes
precedence over the topic target, so the host requests at most one current
revision and mounts at most one separate `ReactionBar`. With no valid selected
reply it composes the selected topic through
`fetch_storefront_topic_current_revision`; with a valid selected reply it uses
`fetch_storefront_reply_current_revision` and constructs
`ReactionSubjectUiRef("forum", "reply", ...)` only in the host. The Forum reply
list and `ReplyCard` stay reaction-agnostic and no revision request is fanned out
across visible replies. Disabled Reactions, unavailable producer revision or
invalid subject construction leaves Forum rendering intact without host-owned
Reactions error presentation.

The Rust E2E crate now includes
`tests/e2e-rust/tests/leptos_storefront_forum_reactions.rs`. Maintainer execution
supplies one canonical visible topic URL and the same topic route with one valid
`?reply=<uuid>` selection. The browser harness requires the topic document to
render only `data-storefront-composition="forum-topic-reactions"` and the reply
selection to render only `data-storefront-composition="forum-reply-reactions"`.
It observes the mounted host output only; it does not call Reactions transport or
Forum revision fields directly, seed fixtures or bypass producer authorization.
Source presence does not count as retained browser execution evidence.

Reputation and achievements remain separate shared capabilities consuming
semantic facts. Forum trust remains Forum-owned because it controls Forum
posting policy.

### `FORUM-19`: Moderation

Moderation owns reports, cases, queues, immutable decisions/effects, appeals,
application orchestration/retries and cross-domain audit. Forum publishes
subject adapters and applies validated effects through
`ModerationSubjectCommandPort`, retaining only Forum subject revision/local
enforcement state and bounded shared-receipt provenance.

Forum registers neutral factories for `forum_topic` and `forum_post` while
depending only on `rustok-moderation-api`. The adapter reuses
`rustok-outbox::idempotency` rather than creating a Forum application receipt
table. The Moderation decision UUID must equal the trusted write `PortContext`
idempotency key; shared receipt admission binds the full command and happens
before subject reads so replay does not re-evaluate a changed Forum subject.
Direct user callers are rejected; only service/system orchestration callers may
enter the application port.

The optional server host materializes the neutral subject-adapter registry only
when `mod-moderation` selects the owner and `ModerationModule` is present in the
supplied `ModuleRegistry`. Materialization happens after `HostRuntimeContext`
exists and after Forum host facts are composed. Moderation without Forum produces
a valid empty registry; Forum+Moderation materializes `forum/forum_topic` and
`forum/forum_post`; Forum without Moderation remains available with
unmaterialized neutral factories. Missing owner, factory build, duplicate-key and
factory-key mismatch failures remain startup errors.

The server's existing generic module-work bootstrap owns background task
lifecycle. `ModerationModule` contributes one
`rustok_runtime::ModuleWorkRegistration` for worker slug
`moderation_decision_application`; no Forum-specific switch and no bespoke
Moderation `tokio::spawn`/polling interval are introduced. The bootstrap starts
registered module work only in runtime modes that run background workers and
uses the deployment `StopHandle` to stop future claims while allowing already
claimed work to finish.

The Moderation owner persists one durable `moderation_application_operations`
row per typed immutable decision. New decision + typed effect + pending operation
+ `case_decided` event + Moderation command receipt share one owner transaction.
Upgrade backfill creates pending operations only for decisions that already have
`moderation_decision_effects`; historical `effect: None` decisions remain
non-dispatchable. The operation snapshots decision hash plus exact reviewed
subject identity and supports bounded due reads, fresh UUID lease-token CAS claim,
expired-lease reclaim, explicit retry scheduling, rejected/operator-review
terminal states and exact `ModerationDecisionApplication` evidence before
`applied`. This state belongs exclusively to Moderation; Forum neither stores nor
reads it.

The Moderation one-attempt dispatcher is source-ready. It claims at most one
exact due tenant/decision operation, reconstructs the immutable command from
Moderation decision/effect/case state, validates exact decision hash and reviewed
subject, looks up only the stored module/kind adapter and invokes it as trusted
service `rustok-moderation`. The domain `PortContext.idempotency_key` remains the
immutable decision UUID across all lease attempts; the lease token only varies the
attempt correlation. Missing adapter and retryable neutral errors schedule
bounded retry. Non-retryable `Conflict` (including stale reviewed revision) and
`InvariantViolation`, corrupt immutable command state and mismatched successful
application evidence require operator review. Other non-retryable neutral port
errors are rejected. A successful result becomes applied only after exact
evidence and live-lease validation.

The shared scheduler source discovers at most one earliest-due Moderation
candidate per pass and does not create the durable lease. Its generic
`ModuleWorkItem.lease_token` is envelope identity only. The handler delegates to
`dispatch_application_operation_once`, which repeats the canonical due predicate
and performs the sole authoritative Moderation CAS before any domain adapter is
called. Two hosts may discover the same candidate, but only one can win that CAS;
the loser performs no domain mutation. Generic module-work completion is a no-op
because `moderation_application_operations` remains the sole durable outcome
source.

Moderation operation state, case lifecycle and the existing `moderation_events`
owner audit ledger advance atomically below the dispatcher for claims/finalizers
executed after this source is active. The first winning claim moves a `decided`
case to `applying_decision` and increments its revision; retries/reclaims while
already applying do not bump the case revision again. Retryable outcomes keep the
case applying. Accepted matching application evidence moves the operation to
`applied` and the case to `closed`, sets `closed_at`, releases the active
deduplication key and writes `application_applied` plus `case_closed` in the same
owner transaction. Rejected and operator-review remain distinct application
outcomes but both fail closed at case level by moving to `escalated` with the
matching application audit event plus `case_escalated`. Escalated cases retain
their active identity for future operator recovery/report attachment; only closed
cases release it. These are internal Moderation owner audit events, not a newly
frozen typed cross-domain event family.

If the operation CAS, case revision CAS or audit insert fails, the Moderation
transaction rolls back instead of partially advancing operation/case state. A
crash after a committed claim leaves the case applying until the existing lease
is reclaimed. Lost-response retries still call Forum with the immutable decision
UUID idempotency key, so an already committed Forum mutation replays its shared
receipt before Moderation records application success and closes the case.

Moderation now also owns replay-safe application recovery commands. A human-user
operator may requeue only a `rejected` or `operator_review` operation. The command
binds an idempotency receipt, exact expected case revision and bounded reason,
validates immutable decision/hash/subject/case identity, moves the operation to
`retryable` due now and advances `escalated` (or legacy pre-audit `decided`) to
`applying_decision`. It never requeues `applied` and never calls a Forum adapter
directly; the existing scheduler/dispatcher remains the next execution path with
the same immutable decision UUID domain idempotency key.

Pre-audit terminal application rows can now be reconciled without manufacturing
history. Exact stored terminal truth maps `applied -> closed` and
`rejected|operator_review -> escalated`; an already-consistent case is a no-op.
Applied reconciliation requires stored applied revision/time and closes the case
at the **current reconciliation time**, releasing the active case key. The owner
writes only present-time `application_legacy_terminal_reconciled` and
`case_legacy_terminal_reconciled` audit facts. It does not emit fake historical
application/case lifecycle events and does not invoke Forum/domain mutation.

A true stale-revision re-review remains a new review, not a recovery mutation.
The reviewed revision is immutable decision identity, so Moderation must use a
new case and new immutable decision built from a freshly authorized producer
revision. The old escalated case/decision remains historical truth; no recovery
path may retarget it.

The existing Reactions/current-revision clock is intentionally not reused for
Moderation because it does not advance on every lifecycle/enforcement mutation.
Migration `m20260807_000027_add_forum_moderation_subject_revisions` introduces
`forum_topic_moderation_subject_revisions` and
`forum_reply_moderation_subject_revisions`: tenant-scoped Forum-owned current
revision rows, backfilled for existing subjects and maintained by PostgreSQL and
SQLite triggers over topic/reply core, content, lifecycle and local enforcement
changes. They are not case/decision/audit/application-attempt storage and are not
Reactions state.

The adapter fences both the exact active Forum subject and its dedicated
moderation revision row before comparing `ModerationSubjectRef.revision`.
PostgreSQL uses a serializable owner transaction plus row locks; SQLite reserves
the writer through the dedicated revision row without issuing a no-op update on
the subject. A stale decision fails without mutation and is never retargeted.
Successful domain mutation and completed shared receipt commit atomically.
Non-retryable failures may be stored as terminal shared receipts, while retryable
storage/serialization failures leave the processing lease reclaimable rather
than freezing transient failure into replay.

The bounded effect set now supports `NoDomainMutation` for topic/reply, permanent
topic lock through `TopicService::set_locked_in_tx`, exact reply
`SetVisibility { state: Hidden }`, exact reply
`SetVisibility { state: Removed }`, and exact reply `RejectPublication`.
A real lock/hide/reject/removal mutation must advance the dedicated moderation
subject revision in the same transaction and the adapter returns that
post-application value as `ModerationDecisionApplication.applied_revision`; a
true no-op retains the reviewed revision. Missing or non-advancing revision state
fails as an invariant rather than guessed success.

Reply `Hidden` maps only to the established `ReplyStatus::Hidden` lifecycle, and
`RejectPublication` maps only to the established Forum moderator rejection
action with target `ReplyStatus::Rejected`. Both use the same bounded non-public
status owner primitives: an already-target reply is a no-op, each changed
transition must pass the existing state machine, and an Approved source reply
decrements topic/category/author public reply accounting. Other valid
non-public-to-non-public transitions change lifecycle state without public
counter deltas. Every changed hide/rejection writes the canonical
`ForumReplyStatusChanged` root event, already a Forum Search full-scope source,
and category projection invalidation is emitted when public counters changed.
Status, counters/statistics, event/projection, moderation revision and shared
receipt therefore commit or roll back together.

Neutral `SetVisibility { state: Unpublished }` is a different Moderation effect
from `RejectPublication`. Forum has an exact existing rejection lifecycle but no
separate exact unpublished lifecycle state, so `Unpublished` remains fail-closed
and must not be mapped to `ReplyStatus::Rejected`.

Reply `Removed` is not a status-only mapping. Direct delete and Moderation removal
share `ReplyService::remove_in_tx`, which claims the active reply, validates the
existing transition to `ReplyStatus::Deleted`, removes an accepted-solution
relation when present, performs the established `status = deleted` plus
`deleted_at` soft delete so delete revisions/tombstone history are captured, and
applies the existing public-reply and solution-stat accounting. The adapter then
writes the same canonical `ForumReplyStatusChanged` fact with `new_status =
deleted` and category projection invalidation when public counters changed.
Soft-delete/tombstone, solution cleanup, accounting, event/projection, moderation
revision and completed receipt commit or roll back together. Receipt replay
happens before subject reads; a fresh attempt against an already removed subject
is unavailable rather than re-applied.

Temporary locks fail closed because Forum does not yet own expiry-safe moderation
enforcement state. The remaining Moderation effect catalog stays pending. The
remaining FORUM-19 orchestration/product gap is authorized Moderation admin
transport/RBAC for the source-ready recovery commands and an explicit
fresh-producer-revision -> new case -> new immutable decision re-review flow. It
must not move recovery state into Forum or bypass the existing shared-scheduler +
one-attempt dispatch path.

## `LINK-FORUM-03` — index and search

**Status:** `planned` (source-ready proofs only; maintainer execution pending)

Prove publish, translation, moderation approval, move, hide/delete, ACL change,
attachment and reader-state transitions end to end: Forum owner commit, outbox event,
Search inbox and projector, owner checkpoint, and the storefront read that must not
return a document the owner no longer exposes. The reconciliation queries
(`forum_search_projection_reconciliation_report`) and the versioned-invalidation
delivery proofs are the source side of that link; they are `source_ready` and explicitly
do not claim PostgreSQL, Iggy, restart, DLQ, ACL or LINK-FORUM-03 runtime evidence, and
no slice in this plan closes the link. `FORUM-23B2G2B3D19` owns the promotion packet
that may move this row once a maintainer retains the aggregate runtime artifact.

## `FORUM-23` — search/index integration

**Status:** `in_progress`

Forum publishes projection documents and owns every authorization decision that a
Forum-scoped Search result needs; Search owns querying, ranking and pagination. The
split is enforced by the host policy registered in the generic
`SearchAudienceRegistry`, never by Search reading Forum tables.

> **Current state (2026-10-25).** The dedicated Forum storefront Search entrypoint
> (`forumStorefrontSearch`), its Search-side executor and its two storefront adapters
> were deleted in Phase 4. The milestones below are kept as delivered history; the live
> mechanism is `execute_audience_eligible_search` + `SearchAudiencePolicyPort` (host:
> `apps/server/src/services/forum_search_result_eligibility.rs`) + the module-neutral
> `StorefrontSearchCategoryScopePort`, guarded by `npm run verify:search:audience`.

### Delivered history

The delivered `FORUM-23B*` slice notes and the `FORUM-23B2G2B3*` aggregate
boundary moved verbatim to
`crates/modules/rustok-forum/docs/archive/forum-23-search-index-integration.snapshot`.
They are history, not active work, and they still name the retired Forum-only
storefront executor and projector, so the canonical plan does not restate them.
Read that snapshot for slice detail; read this section only for the live
mechanism and the remaining scope.

### Delivered slices

`FORUM-23B1` (exact category filter), `FORUM-23B2A` (owner category scope),
`FORUM-23B2B` (category audience scope), `FORUM-23B2C` (projection invalidation),
`FORUM-23B2D` (eligibility boundary), `FORUM-23B2E1` / `FORUM-23B2E2` (profile
events), `FORUM-23B2F1`-`FORUM-23B2F4` (current channel filter),
`FORUM-23B2G1` / `FORUM-23B2G1A` (durable PostgreSQL ingest sequence),
`FORUM-23B2G2A` / `FORUM-23B2G2A1` / `FORUM-23B2G2B1` / `FORUM-23B2G2B2` (owner
revisions) and `FORUM-23B2G2B3A`-`FORUM-23B2G2B3D19` (versioned invalidation and
retained evidence). Each slice's own contract and note carry its status; the
per-slice prose lives only in the archive snapshot.


### Remaining scope of `FORUM-23`

Search filters include
category subtree, author, tag, locale, date, solved, kind, channel/group and
attachment-presence families; kind and attachment-presence have no owner model yet.
The plan keeps owner-issued monotonic projection revisions as the remaining ordering
work: they must replace the remaining clock-based projection ordering, reconciliation
and ACL/deletion cleanup need runtime evidence, and
the maintainer-executed PostgreSQL, restart, retry and LINK-FORUM-03 evidence is not
retained. The 51 `forum-search-*.json` contracts in this directory state an
execution-pending status of their own - commonly
`source_complete_execution_pending` or `source_ready_maintainer_execution_pending` -
and none of them claims completion.

## `FORUM-24` — localized routes

**Status:** `in_progress`

Forum 24 owns the localized canonical, redirect, hreflang and metadata routes built
from one route resolver, and the Search-facing boundary is that Search never
reconstructs a Forum slug or short id: Forum projects the canonical path and absolute
URL, and Search validates the projected payload identity and the route locale before it
emits either. The registered-host SSR evidence for the resolver is
`docs/evidence/forum-forum24-registered-host/` and the per-slice records are the
`FORUM-24A` through `FORUM-24S` rows in `docs/standards/continuous-review-ledger.md`.

Two Search-owned follow-ups stay open as executable packets rather than completed work.
`crates/modules/rustok-forum/contracts/forum-search-canonical-route-cutover.json`
(status `source_ready_maintainer_execution_pending`) describes cutting the storefront
Search result links over to `/{locale}/forum/c/{slug}` and
`/{locale}/forum/t/{short_id}/{slug}` with a `?reply={reply_id}` deep link, which is why
the Leptos and Yew storefront rows still render `/forum/t/{id}` and
`/forum/t/{id}?reply={reply_id}`;
`crates/modules/rustok-forum/contracts/forum-search-canonical-route-reindex-harness.json`
(status `executable_no_run`) provides the PostgreSQL-only reindex and eligibility
harness that a cutover needs before links are republished, with
`RUSTOK_SEARCH_TEST_DATABASE_URL` falling back to `DATABASE_URL` and a unique schema
first in `search_path`. Both packets read this section for the canonical route cutover
and full-reindex status rather than trusting the ledger row, and until they run no
route is claimed as cut over. `verify-forum-search-canonical-route-cutover.mjs`,
`verify-forum-search-canonical-route-reindex-harness.mjs` and
`verify-forum-canonical-route-seo-policy.mjs` pin the boundary.

## `FORUM-32` — Page Builder and widget evolution

**Status:** `in_progress`

Forum contribution discovery, Fly contract registration, owner-backed preview
and owner-backed property editing are source-ready. Canonical
`rustok-module.toml` owns the exact three widget identities and splits
authoring/property admission from preview admission: `rustok.forum.widget-catalog`
requires `tree + properties`, while `rustok.forum.widget-preview` requires only
`preview`. This preserves property access when the provider is in `preview_off`
degraded mode.

Forum admin build generation uses the shared platform normalizer and exports the
version-pinned manifest plus stable component ids. The Forum Fly adapter registers
component/block identities and resolves renderer/property-editor contracts but
never imports Forum owner services or persistence. The Fly document stores only
versioned widget configuration in `props`.

Owner preview is explicit instead of reusing an incomplete generic topic
transport. `ForumWidgetPreviewService` first runs the existing widget contract
normalizer, applies Forum visibility/RBAC and executes bounded owner reads. Topic
list `activity/newest/top`, `include_pinned`, category filtering and pagination
are applied before result materialization; topic detail uses the existing exact
owner facade; reply-stream moderator mode requires `forum_replies:moderate` and
excludes deleted tombstones. `/api/forum/widgets/preview` and the Forum admin
server-function adapter expose this owner source without moving tenant/actor
selection into widget props.

The generic Page Builder admin package owns only provider-neutral extension ports:
external manifest, Fly registry installer, async preview, and owner property
schema/validation. `apps/admin` composes the Forum extension on the real Pages
route only when Forum is tenant-enabled. Contribution RBAC is resolved
server-side through `has_effective_permission`, so `manage -> read` semantics
remain identical to the platform permission model and the browser never receives
an unrelated permission snapshot. Selected-component owner preview remains
explicit Refresh-only and bounded to a 16 KiB JSON summary; owner data is never
persisted into Fly.

Property schema and validation also remain Forum-owned. The generated property
editor descriptor carries only `forum_widget_owner_schema_ref_v1`; the Forum admin
transport requires an exact descriptor match before loading the schema body from
`ForumWidgetContractService::catalog`, and candidate props are normalized only by
`ForumWidgetContractService::validate_props`. The generic property panel accepts
only the bounded current schema subset, rejects unsupported/additional fields,
rechecks the selected component around async validation, and executes
`ComponentPatch::set_field("props", normalized_props)` only for a valid object
response. Raw browser form state and Forum topic/reply/category data are never
persisted as a second owner authority.

Observed browser/runtime and tenant Wave evidence remain open. Page Builder stays
optional; Forum routes and owner state must remain available when builder,
properties or preview capabilities are disabled.

Replace the synthetic Wave packet with an observed tenant control-plane run only
after the `pages` reference-consumer gate. The observed run must retain the
existing all-on/publish-off/preview-off/builder-off profiles and correlate
`builder_write -> forum_publish -> storefront_read`; source-ready host composition
is not observed rollout evidence.

Verification cursors include:

```bash
node scripts/verify/verify-forum-page-builder-contribution-metadata.mjs
npm run verify:page-builder:consumer:forum
npm run verify:forum:wave-evidence-freshness
cargo xtask module validate forum
```

These commands remain maintainer-run in this source slice.

## `FORUM-33` — analytics, observability and reconciliation

**Status:** `in_progress`

FORUM-33A/B provide the read-only `ForumCounterReconciliationService`; FORUM-33C
adds a sibling read-only `ForumSolutionReconciliationService`. Both are exposed
through the same operator GraphQL query object, alongside subscription, mention and attachment-hold diagnostics:

```text
forumCounterReconciliationReport(
  limit: Int,
  topicAfter: UUID,
  categoryAfter: UUID
)

forumSolutionReconciliationReport(
  limit: Int,
  solutionAfter: UUID,
  solutionStatAfter: UUID
)

forumAttachmentHoldReconciliationReport(
  limit: Int,
  mediaAfter: UUID,
  relationAfter: UUID
)
```

Tenant identity comes only from trusted request `TenantContext`. The transport
rejects auth/tenant mismatch and requires both effective
`forum_categories:manage` and `forum_topics:manage` permissions. Both owner
services independently reauthorize the same two Manage scopes through the
canonical Forum RBAC helper before opening a database transaction.

Counter reconciliation checks `forum_topics.reply_count` against approved
replies, `forum_categories.topic_count` against current topic rows, and
`forum_categories.reply_count` against approved replies across category topics.
Topic/category continuation is independent and uses strict UUID `id > cursor`
keysets with default 100 / hard 500 rows per shape.

Accepted-solution reconciliation treats `forum_solutions` as authority. The
selected reply must remain the exact same-tenant/topic reply and must still be
`approved`; non-public or missing replies produce `accepted_reply_eligibility`
drift. `forum_user_stats.solution_count` remains only a Forum projection. The
solution report detects a missing user-stat row for an approved accepted-solution
author and exact stored-vs-expected `solution_count` drift, including stale
positive counts when no accepted solution remains. Solution rows and user-stat
rows have independent strict UUID keyset cursors.

Every report call is snapshot-consistent within its page. PostgreSQL uses
`REPEATABLE READ READ ONLY`; SQLite keeps all report reads in one transaction.
Multi-page scans are deliberately page-local and are not a cross-request
serializable repair fence. `clean` is page-local; whole-tenant clean requires
exhausting every relevant cursor chain with every page clean.

The services reuse platform module-entrypoint/span/error metrics rather than
adding duplicate Forum metric families. Attachment-hold reporting records the same
entrypoint/span/error classes and remains read-only. Source-ready reporting does not
claim runtime observability evidence.

FORUM-33 remains `in_progress`. Retain SQLite and PostgreSQL execution evidence
for counter and accepted-solution clean/drift pages, independent multi-page
cursor traversal, exhausted-one-side behavior and concurrent page-local snapshot
semantics. The attachment reconciliation slice is now source-ready as a read-only bidirectional owner diagnostic. It uses independent bounded keysets for Media-owned holds and Forum-owned relation rows, exact bulk Media lookup for the reverse relation page, and fails closed on tenant/owner/identity, ordering, cursor or response-bound violations. It reports orphan Media holds, missing Media holds and stable-reference Media-ID mismatches; runtime PostgreSQL evidence remains open. Automatic repair stays blocked by the existing FORUM-33 write-repair gate.
The canonical machine-readable contract is `crates/modules/rustok-forum/contracts/forum-attachment-hold-reconciliation.json`, guarded by `scripts/verify/verify-forum-attachment-hold-reconciliation.mjs`. Add only
non-duplicative operational metrics for moderation, notification/search lag,
unread/activity, locale fallback and spam outcomes. Any write repair remains
blocked until it has explicit operator RBAC, dry-run behavior, durable audit,
idempotent job/receipt state and bounded recovery. A platform CLI adapter must be
added only with its synchronized workspace dependency and `Cargo.lock` update.

### `FORUM-30`/`FORUM-31`: UI composition

Forum packages own Forum workflows. Shared modules own reusable profile, media,
moderation, reaction, notification, search, translation and SEO components.
Hosts register/mount packages and do not absorb policy.

## Execution order

### Track 1 — shared capabilities

1. Reactions neutral API/optional module foundation: source-ready, maintainer verification pending.
2. Reactions owner persistence/atomic aggregates: source-ready, lockfile and runtime evidence pending.
3. Forum `topic`/`reply` provider factory and disabled Forum profile: source-ready, maintainer verification pending.
4. Optional distribution/server selection and host materialization after Forum facts: source-ready, maintainer verification pending.
5. Executable composition profiles, sealed semantic reaction events and bounded aggregate reconciliation: source-ready; retain execution, rollback, replay and repair evidence.
6. Second producer and neutral-contract review: Blog `post` source and Blog+Reactions composition profile are source-ready; retain provider/host execution evidence before freezing shared presentation contracts.
7. Bounded Reactions GraphQL transport, separate module-owned Reactions storefront controls, dual-path generic visibility-gated Forum topic/reply current-revision transport, bounded selected-topic/selected-reply neutral host composition and Rust Playwright browser-evidence harness are source-ready. Execute and retain the browser/runtime evidence without adding Reactions functionality to Forum.
8. Introduce Reputation/Achievements only after at least two producers agree.
9. Forum `rustok-moderation-api` topic/reply factories, dedicated Forum moderation subject revision clocks, shared receipt/revision fencing, permanent topic lock, exact reply Hidden/Removed/RejectPublication application, optional server host registry materialization, Moderation-owned durable application operations, bounded one-attempt exact adapter dispatch, shared `ModuleWorkScheduler`, atomic application/case audit lifecycle, rejected/operator-review same-decision requeue and no-domain legacy-terminal reconciliation are source-ready. Retain selected-owner/missing-owner, scheduler/stop/multi-host convergence, recovery receipt/CAS/no-adapter behavior, operation migration/lease/dispatch/lost-response and Forum mutation/replay/concurrency evidence. Keep Unpublished distinct; the next product milestone is authorized Moderation admin transport/RBAC plus explicit fresh-revision new-case/new-decision re-review, never Forum case queues/audit.

### Track 2 — close existing Forum work

1. Execute FORUM-24 SQLite registered-host evidence.
2. Add mounted shared-storefront HTTP evidence.
3. Add browser navigation evidence for category/topic/reply Search links.
4. Execute PostgreSQL and deployment reindex evidence.
5. Execute retained FORUM-16/21/23/26 evidence.

### Track 3 — Profiles/Media and Forum product

1. Category cover and attachment relations over Media; attachment relation persistence, durable Media reference retention/control, and bidirectional orphan/missing-hold reconciliation are source-ready. Category-cover owner write semantics remain a separate product slice; attachment runtime evidence remains open.
2. Batched Profiles member composition.
3. Topic kinds, drafts/bookmarks, read-state bulk completion and trust enforcement.
4. Full admin/storefront assembly and release integrations.

### Track 4 — Forum Page Builder contribution continuation

1. Keep canonical Forum contribution metadata on the shared module-tooling boundary and keep Forum widget persistence/visibility/validation authoritative.
2. Generated Fly component/block/renderer/property identities plus provider-neutral Pages host composition: source-ready.
3. Forum-owned preview service/HTTP/native transport with explicit selected-component Page Builder preview: source-ready.
4. Forum-owned property schema/validation transport plus generic normalized-`props` editor: source-ready.
5. Retain all-on/preview-off/properties-off/Forum-disabled runtime/browser evidence before replacing synthetic Wave evidence with an observed run.

## Compatibility and migration

- Existing Forum votes remain source-compatible. Do not reinterpret them as
  reactions without explicit semantic mapping and migration.
- Forum statistics are projections, not a reputation ledger.
- Forum reconciliation is read-only in FORUM-33A/B/C. Counter, accepted-solution
  and solution-stat traversal use independent strict UUID keyset cursors and
  page-local snapshots; they must not be treated as a cross-request serializable
  repair fence. `forum_solutions` remains accepted-solution authority while
  `forum_user_stats.solution_count` remains only a projection. No transport may
  repair owner counters/solution projections until the write path has explicit
  RBAC, dry-run, audit, idempotent job/receipt state and bounded recovery.
  Shared-owner reconciliation must use public owner contracts and must not read
  another module's private tables.
- Forum moderation state remains authoritative Forum state while Moderation
  decisions are applied idempotently through an adapter.
- Forum depends only on `rustok-moderation-api`, not the Moderation owner crate;
  reports, cases, queues, decisions, appeals and cross-domain audit remain outside
  Forum persistence.
- `mod-moderation` remains optional and is not implied by `mod-forum`. Selecting
  the owner feature without `ModerationModule` is a startup configuration error;
  Forum without the owner remains valid with unmaterialized neutral factories.
- Moderation application-operation persistence, one-attempt dispatch, shared
  scheduler registration, case/application audit lifecycle and operator recovery
  belong only to `rustok-moderation`. Forum must not copy pending/applying/retry/
  lease/applied state, recovery commands, case revisions, owner audit rows,
  reconstruct Moderation decisions, publish its own worker, or invoke the owner
  dispatcher/recovery directly.
- Shared module-work discovery is only a scheduling hint. The existing Moderation
  operation CAS must remain the sole durable claim before a Forum adapter call;
  generic scheduler envelope tokens must never become Forum receipt keys,
  Moderation operation lease tokens or domain idempotency keys.
- Moderation's first successful operation claim may advance its own case from
  `decided` to `applying_decision`; retries/reclaims must not repeatedly bump the
  case revision. This is Moderation owner state and never Forum lifecycle state.
- Only matching accepted application evidence may close the Moderation case and
  release its active deduplication key. Rejected/operator-review outcomes remain
  not-applied and escalate the Moderation case; they must not be reported as
  Forum mutation success.
- Moderation operation/case transitions and internal `moderation_events` audit
  facts must commit atomically for transitions executed by the lifecycle source.
  Forum must not mirror those events into a second moderation audit log or treat
  the internal audit ledger as a public event bus.
- Same-decision operator requeue may only move Moderation `rejected` or
  `operator_review` back to retryable work. `Applied` can never be requeued, and
  Forum is not called by the recovery command itself; any later domain call still
  comes only from the canonical scheduler/dispatcher with the same decision UUID.
- Pre-audit terminal Moderation operations are reconciled by Moderation owner
  recovery, not by replaying a Forum mutation or inventing historical case/audit
  facts. Forum must not participate in or store that reconciliation state.
- A stale reviewed decision must never be retargeted by recovery. A true re-review
  requires a new case and new immutable decision built from a fresh authorized
  producer revision; Forum only supplies its normal subject/revision facts.
- Moderation one-attempt dispatch must keep the immutable decision UUID as the
  domain idempotency key across lease attempts. The lease token is attempt/
  correlation identity only; it must never create a second Forum receipt key.
- Moderation dispatch must select exactly the stored subject module/kind adapter.
  Missing or retryable adapters/errors remain retryable; there is no fallback to
  another producer or subject kind and no guessed applied result.
- Non-retryable Moderation `Conflict`/`InvariantViolation`, including stale
  reviewed revision, and mismatched successful application evidence require
  operator review; they must not be silently retried or collapsed into ordinary
  rejection/applied success.
- Legacy decisions without typed effects remain non-dispatchable; a stale worker
  lease must not bypass the Forum domain receipt or exact reviewed revision.
- FORUM-19 owns only subject-local moderation revision clocks and local enforcement
  state. Those revision rows must not contain reports, decisions, audit payloads,
  application-attempt state or copied Reactions state.
- The moderation subject revision is distinct from the Reactions/content revision,
  timestamps and global Forum event offsets; no integration may silently substitute
  one of those narrower/unrelated clocks.
- Moderation decision application uses the shared Outbox owner-operation receipt
  ledger. Forum must not create a second application receipt table or retarget a
  reviewed decision to a newer subject revision.
- Reply `SetVisibility(Hidden)` must preserve the exact Forum lifecycle, public
  counter/statistics and event/projection semantics; already-hidden must not
  duplicate counters or events.
- Reply `RejectPublication` must preserve the exact established Forum
  `ReplyStatus::Rejected` moderator lifecycle, public counter/statistics and
  event/projection semantics; already-rejected must not duplicate counters or
  events.
- Neutral `SetVisibility(Unpublished)` is distinct from `RejectPublication` and
  must not be mapped to `ReplyStatus::Rejected` without a separate explicit Forum
  lifecycle contract.
- Reply `SetVisibility(Removed)` must reuse `ReplyService::remove_in_tx`; no
  Moderation adapter may bypass the full Forum accepted-solution cleanup,
  soft-delete/tombstone capture, public/solution accounting and canonical
  status-event/projection semantics with a status-only approximation.
- Temporary moderation effects must fail closed until Forum owns explicit
  expiry-safe enforcement state; permanent owner state must not be used as a
  lossy approximation of an expiring restriction.
- Profile, Media, Notifications, Reactions, Search and SEO integrations are
  additive; private-table fallbacks are forbidden.
- Reactions remains optional and outside server defaults and `default_enabled`
  until persistence, adapters and degraded profiles are executable and verified.
- Reactions owner tables contain no Forum routes, content, visibility or copied
  profile data.
- Forum's neutral Reactions API dependency does not make the Reactions owner a
  required Forum module dependency; the same boundary is now proven by Blog as
  a second producer.
- Selecting `mod-reactions` without registering `ReactionsModule` is a startup
  configuration error, not an implicit empty owner.
- Reactions GraphQL derives tenant and actor scope from trusted request context;
  it does not accept caller-supplied tenant/actor identity or duplicate producer
  authorization in transport code.
- Forum current-revision reads remain generic Forum owner facts. They must not
  construct reaction subjects, import Reactions UI/owner crates or duplicate
  reaction state/commands inside Forum.
- Forum current-revision storefront adapters must preserve native/GraphQL path
  parity and the same visibility/status gates before exposing a revision.
- Cross-module Reactions presentation composition belongs to the storefront
  host. The host may import both producer and Reactions UI packages, but Forum
  owner/storefront packages must remain free of the Reactions owner/UI dependency.
- Selected-reply Reactions composition is bounded to one explicit Forum reply
  query target and must never fan out current-revision lookups across the visible
  reply list; a valid selected reply replaces the topic ReactionBar rather than
  mounting a second ambiguous control.
- Reactions semantic event envelope identity is the admitted owner-operation
  UUID; it is not a Forum route/revision/vote identity.
- Reactions bounded reconciliation repairs aggregate projection only and cannot
  mutate Forum-owned subjects or reinterpret Forum votes.
- Forum Fly component/block/renderer identities and Page Builder host composition
  may expose only versioned configuration plus owner contract references. They
  must not copy Forum topic/reply/category data, schemas, visibility facts or
  authorization into Fly or the generic Page Builder package.
- Forum widget preview must normalize through `ForumWidgetContractService`, use
  Forum owner visibility/RBAC, and accept no caller-supplied tenant or actor in
  widget props. Moderator reply preview must exclude deleted tombstones.
- Page Builder contribution discovery may use server-resolved effective permission
  strings for admission, but every Forum owner transport must independently
  reauthorize the request. A `manage` grant may satisfy an exact `read` manifest
  requirement only through the platform `has_effective_permission` semantics.
- Forum widget property editing is owner-contract driven: schema must be loaded
  from the Forum owner catalog, candidate props must pass Forum owner validation,
  and only owner-normalized Fly `props` may be persisted. It must not create a
  second Forum data authority.

## Verification

```bash
node scripts/verify/verify-forum-counter-reconciliation-source.mjs
node scripts/verify/verify-forum-page-builder-contribution-metadata.mjs
node scripts/verify/verify-forum-shared-capability-ownership.mjs
node scripts/verify/verify-forum-moderation-subject-adapter.mjs
node scripts/verify/verify-moderation-host-composition.mjs
node scripts/verify/verify-moderation-application-operation.mjs
node scripts/verify/verify-moderation-application-dispatch-once.mjs
node scripts/verify/verify-moderation-application-work-scheduler.mjs
node scripts/verify/verify-moderation-application-audit-lifecycle.mjs
node scripts/verify/verify-moderation-application-operator-recovery.mjs
node scripts/verify/verify-reactions-foundation.mjs
node scripts/verify/verify-reactions-owner-persistence.mjs
node scripts/verify/verify-forum-reaction-subject-provider.mjs
node scripts/verify/verify-blog-reaction-subject-provider.mjs
node scripts/verify/verify-reactions-host-composition.mjs
node scripts/verify/verify-reactions-composition-profiles.mjs
node scripts/verify/verify-reactions-events-reconciliation.mjs
node scripts/verify/verify-reactions-storefront-ui.mjs
node scripts/verify/verify-forum-storefront-topic-current-revision.mjs
node scripts/verify/verify-forum-storefront-reply-current-revision.mjs
node scripts/verify/verify-forum-storefront-current-revision-transport-parity.mjs
node scripts/verify/verify-forum-topic-reactions-storefront-composition.mjs
node scripts/verify/verify-forum-reply-reactions-storefront-composition.mjs
node scripts/verify/verify-forum-reactions-storefront-browser-evidence.mjs
cargo test -p rustok-events reactions
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo test -p rustok-reactions --features graphql graphql
cargo test -p rustok-reactions-storefront
cargo test -p rustok-forum reaction_subject
cargo test -p rustok-forum moderation_subject -- --nocapture
cargo test -p rustok-blog reaction_subject
cargo test -p rustok-moderation
cargo test -p rustok-e2e-rust --test leptos_storefront_forum_reactions -- --nocapture
cargo check -p rustok-reactions --features graphql --all-targets
cargo check -p rustok-reactions-storefront --all-targets
cargo check -p rustok-forum --all-targets
cargo check -p rustok-moderation --all-targets
cargo check -p rustok-distribution --features "mod-forum mod-reactions"
cargo check -p rustok-server --no-default-features --features mod-reactions
cargo check -p rustok-server --no-default-features --features "mod-forum mod-reactions"
cargo test -p rustok-server --no-default-features --features "mod-blog mod-reactions" --test reactions_composition_profiles blog_with_reactions_materializes_post_provider
cargo check -p rustok-server --no-default-features --features mod-moderation
cargo check -p rustok-server --no-default-features --features "mod-forum mod-moderation"
cargo test -p rustok-server --no-default-features --features mod-moderation --test moderation_composition_profiles
cargo test -p rustok-server --no-default-features --features "mod-forum mod-moderation" --test moderation_composition_profiles
cargo run -p rustok-events --example event_contract_digests -- --write
cargo xtask module validate forum
npm run verify:page-builder:consumer:forum
npm run verify:forum:admin-boundary
npm run verify:forum:storefront-boundary
git diff --check
```

Tests, lockfile/event-digest generation and runtime evidence are maintainer-run.
Source contracts, Moderation adapter/migration/materialization/application-operation/
one-attempt-dispatch/shared-scheduler/application-audit/operator-recovery source,
Forum Page Builder Fly/owner-preview/owner-property host source and browser harness
source do not promote runtime status.

## Final production validation — DEFERRED

Deployment-dependent evidence is collected after the remaining Forum source/backend product queue is
exhausted. A live tenant, maintainer credential, mounted browser fixture or observed rollout does not
keep an otherwise complete Forum implementation task open. Production-only checks are appended here
instead of being used as intermediate ledger blockers.

Repository-executable engineering gates are **not** deferred: focused source/runtime tests, SQLite/
PostgreSQL migration evidence, composition profiles, schema checks and browser harnesses that can run
against isolated test fixtures remain part of the task that changes their boundary.

### PROD-FORUM-25 — Category multilingual/RTL mounted evidence — DEFERRED

Execute the retained `Forum Category Taxonomy Browser Evidence` `workflow_dispatch` from `main` only
after a prepared Taxonomy-backed Forum admin/storefront fixture and authenticated GitHub environment
exist. The mounted multilingual/RTL job must execute rather than skip and its successful run URL/ID
plus exact `main` head SHA must be retained. Until then the browser packet remains production evidence
pending without reopening FORUM-25 or TAXONOMY-CAT-5 implementation.

### PROD-FORUM-19 — Moderation composed deployment/promotion validation — DEFERRED

After the remaining Forum source/backend queue is exhausted, retain deployment-level proof for the selected production distribution: Moderation owner/runtime selection, real background-worker lifecycle, recovery operator access and representative Forum decision application under the deployed host configuration. Capture the exact deployment/release provenance required by the final release gate.

This item does **not** absorb repository-executable engineering gates. SQLite/PostgreSQL migration/operation tests, host composition profiles, scheduler concurrency/stop tests, lost-response replay and Forum revision/effect/accounting tests remain implementation evidence and are already retained in the repository. Pending deployment promotion does not reopen FORUM-19 unless it exposes a genuine implementation regression.

## Release gates

Forum is not production-ready while there are cross-tenant references, copied
owner data, duplicate capability owners, partial owner mutations, private data
leaks, transport policy bypasses, optional-capability outages, unbounded work or
runtime claims without retained executable evidence.

## Decisions requiring an ADR to reopen

1. Profiles remains the sole public member identity and directory owner.
2. Media remains the sole binary asset and delivery-lifecycle owner.
3. Moderation remains the cross-domain report/case/decision/appeal/audit owner.
4. Notifications remains the inbox/fan-out/preferences/delivery owner.
5. Reactions and cross-domain reputation/achievements are separate shared owners.
6. Forum trust remains Forum-owned and is not equivalent to reputation.
7. Search, SEO, Translation, Outbox/Events and realtime remain shared.

## Immediate next action

For FORUM-32, retain browser/runtime evidence for Forum-enabled Pages block
insertion, owner schema loading, invalid diagnostics, owner-normalized `props`,
undo/redo, explicit owner preview, Forum-disabled composition,
`preview_off`/`properties_off`, hidden-category filtering, moderator reply preview
and effective `manage -> read` contribution admission. Source composition is now
complete for metadata, Fly identity, owner preview and owner properties; do not
add another Forum-local schema/data authority. Replace synthetic Wave evidence
only after the existing Pages reference-consumer execution gate and the Forum
executable packet are accepted.

For FORUM-33, SQLite execution evidence is retained and verified for
clean/drifted counter pages (`counter_reconciliation_sqlite.rs`), accepted-solution
pages (`solution_reconciliation_sqlite.rs`), subscription pages
(`subscription_reconciliation_sqlite.rs`), and mention pages
(`mention_reconciliation_sqlite.rs`). GraphQL schema SDL, RBAC authorization,
tenant scope isolation, unauthenticated rejection, and operator execution evidence
are retained and verified across the Forum reconciliation reports
(`reconciliation_graphql_contract.rs`), including attachment-hold owner isolation. This covers relation eligibility, missing
solution-author stats, stale/mismatched `solution_count`, independent multi-page
cursor traversal, exhausted-one-side behavior, target existence, merge redirect
source subscription detection, muted preferences integrity, positive revision
checks, child-source agreement, and projection fingerprint validation. Architectural
cleanup removed `#![allow(dead_code)]` and wired `ugc_translation_apply` as a first-class
service module. The remaining FORUM-33 scope includes PostgreSQL runtime evidence, attachment-hold execution evidence, and permitted shared-owner projections. Attachment diagnostics are page-local across the two owner boundaries and never release or retain Media references. Do not add write
repair until operator RBAC, dry-run, durable audit, idempotent job/receipt state and
bounded recovery are designed together. Add a Forum CLI adapter only with the
synchronized workspace dependency and `Cargo.lock` update.

Regenerate the event-contract digests and retain release verification for the
current `Cargo.lock`, then retain SQLite and PostgreSQL evidence for changed/
no-op/replayed reaction event cardinality, rollback on event failure, concurrent
actor writes, clean/blocked/drift bounded aggregate reconciliation and repair
receipt replay. Retain Blog provider and Blog+Reactions composition evidence plus
manifest-composed Reactions GraphQL schema/runtime evidence for anonymous/
authenticated reads, human-user writes, tenant mismatch, idempotent replay and
stale/denied subjects. Retain the separate Reactions storefront source/runtime
evidence plus Forum topic/reply current-revision GraphQL/native transport parity,
then execute and retain the bounded selected-topic/selected-reply Playwright
browser harness from
`tests/e2e-rust/tests/leptos_storefront_forum_reactions.rs`. Do not add reaction
catalogs, state, commands, aggregate ownership or copied Reactions presentation
to Forum.

FORUM-19 implementation is complete. Do not reopen it for deployment-only promotion; track deployed-host/runtime/release proof under `PROD-FORUM-19`. Unsupported `SetVisibility(Unpublished)`, temporary restrictions and other neutral effects remain fail-closed until a separately accepted feature slice defines exact Forum owner semantics. The broader Moderation admin UI, public event family and additional producer adapters remain Moderation-platform work rather than Forum-owned recovery or queue logic.

## `FORUM-20` — ACL and visibility inheritance

**Status:** `in_progress`  
**Priority:** P0  
**Dependencies:** RBAC, channel/group capability contracts

### Scope

Model typed category visibility and create/reply/moderate audience rules:
public, authenticated, roles, trust level, channel members, group members and
explicit allow/deny. Child categories inherit unless explicitly overridden.
A topic may narrow but cannot broaden parent visibility without a privileged
command.

Forum reads, notifications, search, SEO and deep links must call the same
visibility policy. Do not place ACL policy in arbitrary JSON.

## `FORUM-35` — Collaborative Documents Integration & Universal Entity Reference

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** `rustok-docs`, `rustok-content-orchestration`, `rustok-api`

### Scope

1. **EntityCardResolverPort Adapter:** Expose typed resolution of `EntityRef` (`urn:rustok:<tenant>:forum:topic:<id>`) across the 4 projection views:
   - Inline Pill (`@Topic: <title>` with status badge and reply count);
   - Compact Preview Card (Author, excerpt, category, tags, activity metrics);
   - Interactive Live Widget (Embedded discussion thread preview with in-place reply composer and reaction buttons);
   - Database Rollup (Computed columns for view count, reply count, solved status).
2. **Reverse Backlinks Panel:** Expose a read-only accordion on the Topic Detail view querying `docs_entity_links` to display: *"Referenced in X Documentation Pages"*.
3. **Cross-Domain Content Promotion:** Provide handler integration with `rustok-content-orchestration` for `promote_topic_to_doc`: promoting selected RFC / brainstorm threads directly into structured specifications in `rustok-docs`.
4. **RBAC & Audience Alignment:** Topic resolution enforces `ForumAclPolicy` and tenant isolation; unauthorized viewers in shared documents receive a masked placeholder.

## Verification checklist

- [x] Contract tests cover the current public use-cases.
- [ ] FORUM-35: EntityCardResolverPort unit tests and reverse backlink read-path tests.

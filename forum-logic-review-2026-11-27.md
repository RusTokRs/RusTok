# Forum Module Logic & Code Review — 2026-11-27

Scoped source review of `crates/modules/rustok-forum` (and touched server/Next.js
surfaces), performed on branch `arena/9dced585-rustok` at base
`b16abc0ad91421cecf4b55070177f9003152b956`.

## Verification context and honest limitations

- **This is a scoped development review, not the `audit` ledger trigger.** No
  audit ledgers were activated; ordinary scoped-development rules from
  `AGENTS.md` §2 apply. No workspace-wide build was started.
- **`cargo` is not available in this environment** (`which cargo` → not found,
  no `~/.cargo`; `rustc`/`rustfmt` also absent). Therefore **no Rust code was
  compiled, no Rust test was executed, and no Rust linter/formatter ran.**
  Every Rust statement below is a *textual* reading of the source. Items tagged
  **[CONFIRMED]** are directly readable from the code shown; items tagged
  **[CANDIDATE]** are plausible defects whose runtime impact depends on behavior
  I could not exercise (no test run, no DB). The repo's own implementation plan
  admits the same gap for most of its regressions ("authored, unrun").
- **After the fixes in Part 4, the repository's Node/Python source verifiers
  were executed** (see "Verification performed" there): `scripts/verify/
  verify-forum-*.mjs` (non-test), all verifiers referencing the edited files,
  six `node --test` suites, and both `verify-forum-*.py` suites. All failures
  observed were established as pre-existing by a `git stash` baseline
  comparison; the fixes introduced **zero new verifier failures**. Rust-side
  checks (`cargo check/test/clippy/fmt`, `canonical_route_seo_policy_contract`,
  `seo_audience_targets_sqlite_tests`) remain **not run**.
- Cross-reference of the prior dated audit (`forum-audit-report-2026-11-07.md`,
  base `f40c54b`) was re-checked against the current code rather than trusted.

---

## Part 0 — Prior-audit re-verification (status vs. current code)

| Prior § | Finding | Current status |
|---|---|---|
| §2.1 | Mass `include!()` in `mod.rs` | **FIXED** — `include!` returns 0 hits in src. |
| §3.1 | State machine missing `Pending` / incomplete transition matrix | **FIXED** — `state_machine.rs` defines `Pending` and a full documented transition matrix incl. `Open↔Closed`, `Closed→Archived`, `Pending→Open/Archived/Rejected`. |
| §4.1 | `Scope::None` bypass in `enforce_author_deletion_policy` | **FIXED** — returns `Forbidden` for `PermissionScope::None`. |
| §5.1 | Swallowed rollbacks in write paths | **FIXED** — no `let _ = …rollback` swallowing found. |
| §5.2 | Dead `target_topic_id` code | **FIXED** — `let _ = target_topic_id` 0 hits; `posting_rate` callers are exactly `topic.rs:1586`, `reply_owner.rs:1502`. |
| §5.3 | Dual posting-rate vs. canonical posting-policy | **APPLIED** — see **D1** below (Rust verification pending). |
| §7.1 | Mention prepare TOCTOU | **FIXED** — `mention_relation.rs::persist_in_tx` re-validates resolved profiles in-tx via `revalidate_resolved`. |
| §7.3 | Cross-tenant quote references | **FIXED** — `validate_quote_targets_in_tx` filters `tenant_id`+kind+id on `forum_relation_revision`. |
| §9.2 | Search N+1 tag loading | **FIXED** — batch `load_topic_tags_for_topics`. |
| §13.2 | `From<DbErr>` mapping leaks trigger text | **FIXED** — maps via SQLSTATE `P0001` + `TRIGGER_MSG_*`; no unwrap/expect in `error.rs`. |

---

## Part 1 — New defects found this round

Grouped by the six Universal Logic Dimensions from `AGENTS.md`.

### A. Preconditions & Bounds

**A1 [CANDIDATE, P2] — `find_topic` / `find_topic_in_tx` return soft-deleted rows.**
`crates/modules/rustok-forum/src/services/topic.rs:414` and `:426` issue
`forum_topic::Entity::find_by_id(...).filter(tenant_id)` with **no**
`deleted_at IS NULL` predicate. Only the *write-lock* helpers
`find_topic_for_update_in_tx` (`:448`) and `claim_topic_update_in_tx` (`:486`)
carry `AND deleted_at IS NULL` and distinguish `TopicDeleted` from
`TopicNotFound`. The plain helpers instead collapse "deleted" and "missing" into
`TopicNotFound`.

Impact: every one of the ~25 call sites (reply_owner, moderation_subject,
topic_merge, topic_fork, topic_split, topic_owner, topic_audience*,
read_tracking, etc.) must independently re-guard on status/visibility, or it
will operate on / mutate counters of an `archived`+`deleted_at`-set tombstone.
The established callers mostly do guard (engagement guard rejects `Archived`;
owner-read condition excludes `Pending`/`Archived`; storefront filters
`Open`), so this is currently a **latent trap** rather than a demonstrated leak.
The helper's contract is "unsafe by default." Recommend a deleted-filtered
variant (or a default `deleted_at IS NULL` with an explicit
`find_topic_including_deleted` for the few legitimate callers).

### B. Branch Completeness

**B1 [CONFIRMED, P2] — `map_forum_error` catch-all leaks `Display` text and
mis-classifies unmapped errors.** `controllers/mod.rs` `map_forum_error` (arms
at `:259`–`:298`) enumerates 7 `match` arms. `ForumError::Validation(..)` is in
**none** of them, so it falls through to:

```rust
error => HttpError::bad_request(code, error.to_string()),   // controllers/mod.rs ~:296
```

Consequences:
1. Any **new `ForumError` variant not added here** is silently rendered as HTTP
   **400** with the raw `error.to_string()` in the body — i.e. internal/DB text
   can leak to clients instead of becoming a 500. This is a maintainability and
   a potential information-disclosure defect.
2. `Validation` itself maps to 400, which is arguably correct *for that variant*
   (its `Display` is the user-facing message), but the absence is accidental,
   not deliberate — the catch-all is the only safety net.

Recommend: add an explicit `ForumError::Validation(_) => 400` arm and replace the
open `error =>` catch-all with an explicit `error => 500` that does **not** echo
`to_string()` (only the stable code).

### C. State Transitions

No new defect. The state machine (`topic.rs` / `reply_owner.rs` transitions via
`state_machine.rs`) is now complete (§3.1 fixed). `enforce_topic_engagement`
(`topic_engagement_guard.rs:33`) correctly maps `Archived→TopicArchived`,
`Pending→TopicAwaitingModeration` (for vote/reply/restore), `Closed→TopicClosed`
(only for create/restore reply), and locked-topic checks. No gap observed.

### D. Side-Effect Completeness

**D1 [P1, APPLIED; original finding below] — Canonical posting-policy evaluator is never wired
into any write path (continuation of prior §5.3).**
`ForumPostingPolicyEvaluator` / `ForumPostingPolicyRules` are re-exported
(`lib.rs:228`, `services/mod.rs:185`) and have a full fact/port tree
(`posting_policy*.rs`), but there is **zero** caller in `topic.rs` or
`reply_owner.rs`. The only enforcement actually reached on write is the legacy
`posting_rate::enforce_new_topic_rate_in_tx` / reply equivalent. The crate's own
tests *assert the evaluator is absent from the write code*:

```rust
// crates/modules/rustok-forum/src/services/posting_policy.rs:573-575
assert!(!topic.contains("ForumPostingPolicyEvaluator"));
assert!(!reply.contains("ForumPostingPolicyEvaluator"));
```

The implementation plan (`docs/implementation-plan.md:886`) still lists
"Wire the canonical posting policy evaluator" as **P2, outstanding**. Effect:
the richer policy (windows, approved-post facts, precedence rules) is dead code
from a runtime standpoint; only raw rate limits are enforced. This is a
real behavioral/architectural gap, not just dead code, because the documented
policy semantics are not applied.

**D2 [CANDIDATE, P2] — SEO category targets use `SecurityContext::system()`
even for `PublicRoute`, exposing hidden/restricted categories.**
In `seo_targets.rs`, the **topic** loader correctly discriminates on
`SeoTargetLoadScope`:
- `PublicRoute` → `get_public_storefront_visible_with_locale_fallback` + an
  extra `status != OPEN || !channel_visible` → `None` (`:259`, `:270`).
- `Authoring` → `SecurityContext::system()` (legitimately needs held topics).

The **category** loader does **not** branch: `load_target` (`:88`),
`list_bulk_summaries_page` (`:137`), `sitemap_candidates` (`:187`),
`load_category_summary` (`:459`), `load_category_sitemap_candidate` (`:489`) all
call `get_with_locale_fallback(..., SecurityContext::system(), ...)` with no
visibility gate. The in-code comment at `:335` ("the system context would list
held topics") is attached to the *topic* bulk path, which uses `public_read()` —
yet the category bulk/sitemap path uses `system()`. Because the storefront hides
categories via `hidden_category_ids` and the visibility gate, an SEO summary /
sitemap built with `system()` would **index hidden/restricted categories** that
the storefront never shows. Runtime impact [CANDIDATE] — needs a DB-backed check
that `load_category_summary`/`map_category_response` don't filter, which my read
shows they don't. Recommend mirroring the topic `PublicRoute` branch for
categories.

### E. Calculation Correctness

**E1 [CANDIDATE, P3] — Imported `Closed` topic keeps a +1 `category.topic_count`
but is never storefront-listed.**
`finalize_import_topic_in_tx` (`topic.rs:236`) inserts the temporary row as
`Open` and counts `+1`, then subtracts `-1` **only when
`status == Archived`**. An import that lands as `Closed` retains the `+1`. The
storefront list filters `Status == Open` (`topic.rs:2041`, `:2346`,
`topic_visibility.rs:211`), so a `Closed` imported topic inflates
`category.topic_count` without ever appearing in the public list. Whether
`topic_count` is meant to be "all topics" or "public topics" is undocumented; if
the latter (consistent with the storefront gate), this is a counter
inconsistency. [CANDIDATE — verify intent.]

**E2 [CONFIRMED as designed, no action] — Pending topics are counted at create
(unconditionally `+1`) and `ForumTopicCreated` is emitted only on first
publication.** Not a defect; documented anti-double-emit via owner journal
(`moderation_owner.rs` set-status path). Archived/restore reconciles whole-thread
counters; reply un-publish writes one `UserStatsService` update with
`solution_delta`. Counter semantics are internally consistent.

### F. Concurrency / TOCTOU

No new defect in this dimension. The write-path invariant holds everywhere
read this round: audience re-checked inside the txn, category-tree lock acquired
before the topic-row lock, rate limits enforced in-tx, quote/mention persist
chain revalidates profiles in-tx (fixes prior §7.1), and
`From<DbErr>` maps trigger errors without leaking (fixes prior §13.2).

---

## Part 2 — Architectural gaps

**G1 — Closed-topic public visibility is intentionally hidden but its owner
read/edit/vote semantics are inconsistent with that intent.**
`topic_audience_visibility.rs:126` states missing/foreign/**closed**/category-denied
topics "all resolve to `false`" for the anonymous/storefront visibility gate, and
`apply_storefront_visibility` filters `Status == Open` only. Yet:
- Owner read (`topic_owner_read_condition`, `pending_visibility.rs:55`) and
  `read_model_owner.rs:471` explicitly surface `Closed` topics to their owner.
- `enforce_topic_engagement` permits **votes, edits, and reply updates** on
  `Closed` topics (only *new* replies / restores are blocked).

So a closed topic is invisible to the public but still votable/editable by the
owner, and listed in the owner's own list. This may be exactly the intended
product model, but it is **undocumented** outside that one comment, and there is
no DECISION file defining closed-topic storefront behavior (the
`2026-11-02-forum-remove-unwired-closing-and-digest-settings.md` only removes
unwired *author self-close*). Recommend a one-paragraph DECISION confirming
"closed = public-hidden, owner-readable, votes/edits allowed, new replies
blocked."

**G2 — The two-tier `find_topic` helper family encodes an easy-to-miss invariant
(see A1).** "Plain" helpers (`find_topic`, `find_topic_in_tx`) ignore
`deleted_at`; only the `_for_update_in_tx` / `claim_*` variants honor it. A
reviewer adding a new caller is one missing guard away from mutating a
tombstone. Consolidate on a single deleted-filtered default.
*Addressed by the A1 fix in Part 4 (default filtered, deleted-inclusive
variants explicitly named); the local helpers listed there remain to migrate.*

**G3 — Verification gap (environment).** `cargo` is absent, so none of the
forum's ~192k LOC of Rust (incl. tests) was compiled or executed this session.
The crate's own `docs/implementation-plan.md` similarly records most regressions
as "authored, unrun." Findings D1/B1 are read-confirmed; A1/D2/E1 were
textual candidates at review time and are now applied (Part 4), but their Rust
tests remain unrun — only the repository's Node/Python source verifiers were
executed (baseline-compared, no new failures).

---

## Part 3 — Suggested logic-hygiene pattern additions

If the repo's `docs/standards/logic-hygiene-patterns.md` is maintained for
recurring defects, the following two patterns are worth capturing (following the
existing `P-*` format):

- `P-FORUM-ERROR-MAP-CATCHALL-LEAK` — HTTP error mappers must not end in an open
  `error => HttpError::bad_request(code, error.to_string())`; unmapped variants
  must default to 500 without echoing `Display`. Detect:
  `rg -n 'error\.to_string\(\)' crates/modules/rustok-forum/src/controllers`.
- `P-FORUM-DELETED-ROW-HELPER-TRAP` — shared entity lookups must filter
  `deleted_at IS NULL` by default; deleted-inclusive variants must be
  explicitly named. Detect: `rg -n 'find_by_id\(topic_id\)' crates/modules/rustok-forum/src/services/topic.rs`.

---

## Part 4 — Fixes applied this session (2026-11-27)

Scope decision (user): apply **A1, B1, D2, E1, G1**, then include **D1** in
the same PR (superseding the initial deferral). All Rust edits are
textual/uncompiled (no cargo).

**B1 — APPLIED.** `controllers/mod.rs::map_forum_error` now has an explicit
`ForumError::Validation(msg) => HttpError::bad_request(code, msg)` arm, and the
open catch-all returns a generic 500 (`"The forum operation could not be
completed"`) without echoing `error.to_string()`. Unmapped variants can no
longer surface as 400 with internal text.

**E1 — APPLIED.** `finalize_import_topic_in_tx` (`services/topic.rs`) now
decrements category and user `topic_count` by `-1` when the imported status is
`Archived` **or** `Closed`, matching the storefront `Status == Open` list gate
(see the delete/restore counter comments in `topic_owner.rs`, which treat the
same "counted but never listed" invariant).

**G1 — APPLIED (documentation).** `DECISIONS/2026-11-27-forum-closed-topic-visibility.md`
records the intended semantics: `TopicStatus::Closed` is orthogonal to
soft-delete (`deleted_at`) and to `Archived`; closed = public-hidden,
owner-readable, votes/edits allowed, new replies blocked; counters release only
on `Archived`-class transitions.

**A1 — APPLIED, caller audit completed.** `services/topic.rs` now reads topics
through `find_topic_raw(db, tenant, id, exclude_deleted)` (raw SQL, because
`forum_topic::Model` does not model `deleted_at`):
- `find_topic` / `find_topic_in_tx` → `exclude_deleted = true`
  (`AND deleted_at IS NULL`);
- new `find_topic_including_deleted` / `find_topic_in_tx_including_deleted`
  (`exclude_deleted = false`);
- `get_with_locale_fallback` routes through `find_topic_including_deleted`, so
  owner/export/SEO `get` responses keep their pre-change behaviour and still
  deliver `is_deleted = true`;
- `topic_owner::restore` routes through `find_topic_in_tx_including_deleted`.

Caller audit results (no legitimate flow now hits an unintended
`TopicNotFound`):
- **delete** (`topic_owner.rs`): preflight `find_topic` (row not yet
  tombstoned on first delete) → `claim_topic_delete_in_tx` (guards
  `deleted_at IS NULL`, returns `TopicDeleted`) → in-tx `find_topic_in_tx`
  succeeds; `mark_topic_thread_deleted_in_tx` sets `status='archived'` +
  `deleted_at` inline and does **not** call `adjust_reply_count_in_tx`.
- **restore**: fully tombstone-aware (including-deleted read, explicit
  `is_topic_deleted` check, un-tombstone SQL before reply restoration).
- **update / slug-rename / pin-lock-status / moderation / reply writes**:
  all take `find_topic_for_update_in_tx` first, which already returns
  `TopicDeleted` for tombstones; the subsequent `find_topic_in_tx` then reads a
  row the lock proved live.
- **`adjust_reply_count_in_tx`**: every caller holds the topic lock or operates
  on a freshly inserted provisional import topic; any hypothetical
  tombstoned-row call now errors inside the caller's transaction (rollback),
  it cannot persist a counter mutation.
- **import finalize / reply import**: read rows created in the same
  transaction; never tombstoned.
- **Error-type deltas (deliberate, error paths only):** operations against an
  already-deleted topic may now surface `TopicNotFound` (HTTP 404) where they
  previously surfaced `TopicDeleted`/`TopicArchived` (HTTP 409): double `delete`
  (preflight fails first), moderation `Lock` effect on a deleted topic, and
  author `update` preflight on a tombstone. No previously-successful call now
  fails; only the error variant/status of already-failing calls shifts.
- **Not covered by A1 (pre-existing, unchanged):** the *local* `find_topic_in_tx`
  free functions in `topic_split.rs:752`, `topic_merge.rs:802`,
  `topic_fork/implementation.rs:1389` and the local `find_topic` helpers in
  `topic_audience.rs:327` / `topic_reply_create_audience.rs:377` are typed
  `find_by_id` reads with no `deleted_at` predicate (Model lacks the column).
  Split/merge/fork still reject tombstones indirectly because delete forces
  `status='archived'` and those flows reject `Archived` sources; the audience
  helpers are Manage-scoped. Defence-in-depth follow-up, not a live leak.

**D2 — APPLIED (severity refined during implementation).** Discovery: the
*registered* provider is `seo_audience_targets::ForumCategorySeoTargetProvider`
(`module.rs:88`), an outer wrapper that already gates public category SEO with
`ForumPublicDiscoveryService` across `load_target`, `resolve_route`,
`list_bulk_summaries_page` and `sitemap_candidates` (tests
`seo_audience_targets_sqlite_tests.rs:248-340`). The inner crate-private
`seo_targets::ForumCategorySeoTargetProvider` (used directly only through that
wrapper) was therefore hardened as **defence in depth / parity with the topic
mapper**, not as a live external leak:
- `load_target`: `SeoTargetLoadScope::PublicRoute` returns `None` unless
  `seo_category_is_publicly_visible` (audience-visibility service, public
  viewer, taxonomy port + settings port wired like the topic reads);
  `Authoring` intentionally bypasses.
- `list_bulk_summaries_page`: the raw system-context keyset now drops hidden
  categories **and refills** — it loops `list_keyset_with_locale_fallback` until
  `limit` visible summaries are collected or the keyset is exhausted, so it
  never returns an empty page with `next_cursor = None` while rows remain (the
  outer `'scan` loop and its `limit = 1` interleaved-hidden test rely on cursor
  progress); the cursor advances only when another raw row is known to exist.
- `sitemap_candidates`: each offset-page item passes the same public gate
  before `load_category_sitemap_candidate`.
- Verified against the pinned source contracts: the settings-chaining literal
  count in `seo_targets.rs` stays at exactly 2 (category gate binds the
  settings port via a local, per `verify-forum-anonymous-discovery.mjs`).

**D1 — APPLIED after follow-up request.** Topic/reply create paths now prepare
configured window facts before the transaction and enforce the pure decision
inside it, before mutation. REST/GraphQL carry the host composer through shared
service builders. Defaults are inert; configured rules fail closed when context
or facts are unavailable. See the posting-policy window-enforcement ADR.
The former absence assertions now check positive wiring markers.

Follow-up hardening found and corrected two D1 delivery gaps:
- Rules prepared before the transaction are now compared with transactional
  settings before mutation. Enabling, changing or disabling a rule mid-attempt
  rejects the write instead of evaluating stale policy, including an initially
  inert preparation. This is not a reservation of the observed create count.
- Both window objects are declared in `rustok-module.toml` with required bounded
  integer properties and no defaults, making them configurable through the
  platform settings owner. README now documents the live composition.

### Verification performed (after fixes)

- `scripts/verify/verify-forum-*.mjs` (non-test, 243 scripts): **164 pass / 79
  fail** after D1, versus a `git stash -u` baseline of 163 / 80 — set diff
  shows **zero new failures** and exactly one flip to green:
  `verify-forum-posting-policy-contract.mjs` (26C), whose `write_path_cut_tests`
  were honestly rewritten from a negative "no evaluator in write paths" marker
  to positive wiring markers now that D1 landed.
- Earlier session rounds: the same sweep at 163/80 with all 65 verifiers
  referencing the pre-D1 edited files failure-identical to baseline (24
  pre-existing, 0 new), including `verify-forum-public-discovery-seo.mjs` (D2
  markers) and `verify-forum-anonymous-discovery.mjs` (settings count — passed
  after the local-variable adjustment).
- `node --test scripts/verify/verify-forum-*.test.mjs
  scripts/verify/forum-source-verification.test.mjs`: **886 tests passed,
  0 failures** across 29 test files — includes taxonomy-wiring,
  wave-plan-sync, structural-taxonomy, create-audience-guards and
  owner-boundary suites that read the edited service sources.
- `python3 scripts/verify/verify-forum-reply-restore.py` (10 tests OK) and
  `verify-forum-topic-repair.py` (4 tests OK).
- ADR governance: both new decisions are registered; the verifier retains
  pre-existing repository failures (including registry ordering). No new
  ADR diagnostics remain relative to the stashed baseline.
- **Not run:** `cargo check` / `cargo test` / `cargo clippy` / `cargo fmt`
  (toolchain absent), the Rust contract test
  `tests/canonical_route_seo_policy_contract.rs`, and the SQLite test module
  `seo_audience_targets_sqlite_tests.rs` — the keyset-refill behaviour of the
  inner category bulk path is reasoned-through and contract-verified but
  **not executed**. D1 likewise ships **uncompiled**: the new
  `posting_policy_enforcement` module (12 authored tests), the three settings
  tests, and the manifest/DTO bound-parity test in `tests/contract_surface.rs`
  remain unexecuted. Coverage includes topic/reply threshold boundaries,
  retryable/nonretryable unavailable facts, missing provider, configuration
  drift and old settings JSON. No Rust compiler ran.
- Three additional **authored, unexecuted** service tests in
  `tests/posting_limits_sqlite.rs` use real topic/reply window providers:
  initial admission, threshold denial with cooldowns disabled, author isolation,
  expired persisted creates, and missing composer/context/provider. Every denial
  compares all fixture table contents before/after, including outbox, counters
  and `next_reply_position`. Run
  `cargo test -p rustok-forum --test posting_limits_sqlite posting_policy_`.
- A Tree-sitter syntax-only parse of all 18 changed Rust files passed. This is
  not a compiler, type check, integration-test run or formatter check.
- Python `tomllib` successfully parsed the manifest and checked both optional
  object schemas and their required integer bounds. This is structural evidence,
  not execution of the Rust settings normalizer.
- The final stash-baseline comparison again found zero new forum failures and
  identical ADR diagnostics. `git diff --check` passed. Graph tooling was not
  available; impact and caller review used manual source inspection.

### Follow-ups left open

1. **D1 — closed in this branch** (originally deferred, then requested for the
   same PR): create transactions now compose and evaluate configured window
   rules beside the cooldown — see
   `DECISIONS/2026-11-27-forum-posting-policy-window-enforcement.md`. Follow-up
   for the maintainer: run `cargo test -p rustok-forum posting_policy` (no
   toolchain in this pass).
2. **Entity↔DB desync:** `forum_topic::Model` and `forum_reply::Model` omit
   `deleted_at` although migration `m20260713_000009` adds it; A1 therefore
   uses raw SQL. Adding the column to the Models (and auditing every
   `ActiveModel` literal) is the clean fix, deferred because it cannot be
   compile-checked in this environment.
3. Split/merge/fork and audience local `find_topic*` helpers should adopt the
   shared deleted-filtered reader (A1 pattern) for defence in depth.

---

## Priority summary

| ID | Dimension | Priority | Status |
|---|---|---|---|
| D1 | Side-effect | P1 | **APPLIED** (opt-in window rules → pre-txn compose + in-txn pure decide on topic/reply creates; 26C `write_path_cut_tests` rewritten to positive wiring markers — `verify-forum-posting-policy-contract.mjs` flipped fail→pass; uncompiled) |
| A1 | Preconditions | P2 | **APPLIED** (raw-SQL tombstone filter + `*_including_deleted` variants; caller audit clean) |
| B1 | Branch completeness | P2 | **APPLIED** (explicit `Validation`→400; generic 500 catch-all, no `Display` echo) |
| D2 | Side-effect | P2 | **APPLIED** (inner category SEO gated on PublicRoute, bulk keyset refills, sitemap gated; outer wrapper already enforced — severity refined) |
| G1 | Architectural | P2 | **APPLIED** (decision record written) |
| E1 | Calculation | P3 | **APPLIED** (`Closed` import releases `topic_count`) |

No P0 defect was found in the current code that is not already covered by the
prior audit (which is itself mostly fixed). All five applied findings
(A1, B1, D2, E1, G1) plus **D1** (posting policy write-path enforcement, wired
for opt-in window rules with fail-closed semantics) are now source-ready on
this branch; none of them could be cargo-verified in this environment.

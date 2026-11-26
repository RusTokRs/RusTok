---
id: doc://crates/modules/rustok-forum/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-forum
last_reviewed: 2026-11-26
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

## Forum correctness remediation — 2026-11-26

Branch `arena/336e29d4-rustok`, base `710fb31ee330cadfa80aad304e99e951b1a7f27b`.
This is ordinary scoped remediation, not advancement of an audit ledger.
**Status: in_progress; remediation packets, including trusted moderation effects, are source-ready. Rust compilation/service/concurrency, full migration-chain and mounted-browser validation remain open.**

- Reply edit re-pending, moderation and removal share post-status publication effects; accepted
  reply/solution member statistics change in one write. Archived reply removal does
  not double-release category/member counts; delete/restore is in the shared regression.
  Category/topic projection
  invalidations are atomic with re-pending.
- Direct topic/reply deletion no longer relies on a preflight-only author policy.
  It rechecks locked source ownership and transactional tenant settings before
  snapshot/destructive writes. Reply direct deletion and internal moderation retain
  one shared locking/effects core without applying author policy to moderation.
  The [delete admission decision](../../../../DECISIONS/2026-11-26-forum-delete-policy-admission.md)
  replaces only the earlier accepted preflight ordering. Ten cases across both
  source kinds are authored for each SQLite/PostgreSQL service matrix, covering
  differing static/transactional policy, capability failure, missing transactional
  reader, staff/system reasons and foreign ownership. Refusal checkpoints cover
  source/bodies/revisions/solution/statistics, snapshot counts and outbox; positive
  cases assert snapshot reasons. These Rust cases remain unrun. Eight supplemental
  source-guard fixtures pass and reject loss of transactional checks or shared core.
- Reply restore now rejects archived parents under the topic lock, before replaying
  the tombstone/snapshot. The previous owner path admitted ordinary replies there
  but failed on the database solution-target guard for accepted replies. The active
  [parent admission decision](../../../../DECISIONS/2026-11-26-forum-reply-restore-parent-admission.md)
  makes reopen -> restore explicit for both cases. The shared SQLite/PostgreSQL
  source regression checks exact persisted checkpoints on rejection and repeated
  restoration, plus original solution metadata after reopen/retry. REST/GraphQL
  already delegate to this owner; the existing HTTP conflict mapping is unchanged.
- Whole-topic restoration of an already-archived accepted thread previously failed
  on the same active-solution guard. Unlike a standalone reply restore, its deleted
  parent cannot first be reopened. Migration 47 admits only the original historical
  solution INSERT named by the tenant/topic and approved child delete snapshots;
  ordinary archived INSERTs and target UPDATEs remain forbidden. The owner acquires
  the solution scope after restoring/locking the parent and before child writes;
  archived snapshot restoration skips public author contributions. Shared SQLite /
  PostgreSQL service regression sources cover archived restore with/without a
  solution, original marker metadata, no publication, no revived independently
  deleted replies, snapshot consumption, repeated restore and explicit reopen.
  See the active [topic replay decision](../../../../DECISIONS/2026-11-26-forum-topic-restore-solution-replay.md).
- Migration 46 restores SQLite archived-category predicate parity and rebuilds prior
  counter/last-reply drift without rewriting content or lifecycle history. Its topic
  repair excludes immutable soft-deleted rows, not live archived rows. An unscoped
  repair was reproduced failing on PostgreSQL's tombstone guard and corrected;
  SQL regressions execute that guard on both backends. Quiesce writers for migration
  and rebuild Forum search projections before ranking reads.
- Repair and edit-window regression setup uses typed ORM UUID bindings, not text
  UUID interpolation that matches no SQLite BLOB rows. The shared repair regression
  includes a real deleted thread, repeated repair, unchanged content/history and
  rejection of subsequent tombstone mutation on SQLite and PostgreSQL.
- Slug rename uses the configured GraphQL runtime owner and transactional lifecycle /
  edit-window admission. Standalone quote commands recheck source/parent state and
  the window under the write fence; ordinary content commands recheck the window too.
- Leptos category/selection changes drive the resource source; loaded pages are not
  filtered/sorted locally. Unread mode is an owner predicate on both transports and
  every page. Controls survive empty/reloading results, stale page responses are
  fenced by the entire resource key for both topics and replies, including while a
  previous Suspense result is retained, and composer draft state survives reloads.
- Native/headless category GraphQL reads `pageInfo.totalCount`. Schema tests consume
  the actual embedded operations; category DTO, selection and reactive-source tests
  supplement existing owner integration tests.
- Cursor verification follows the current topic owner and tenant ranking defaults;
  discovery verification includes the typed SEO error mapping rather than an obsolete
  chain spelling. Mention persistence verification now pins live CRATE_API transaction /
  identity guarantees and the actual module registration instead of deleted rollout
  notes. Slug owner/GraphQL guards and the Rust source-contract test now require the
  configured runtime owner and live CRATE_API/README contracts rather than direct
  construction or deleted handoff notes; four negative/positive guard tests pass.
  Quote/mention integration guards now follow the current ordinary owner modules and
  the consolidated relation tests instead of deleted inline fragments and handoff
  records. Nine guard fixtures include negative cases for lost REST/GraphQL settings,
  skipped transactional admission, private transport persistence and a removed CAS
  source lock. The existing quote-window Rust matrix now supplies both settings
  readers, and a new matrix admits preflight but rejects a stricter or failed
  transactional policy for topic and reply commands, with no added relation revision.
  These Rust tests are source-only until the toolchain checks below run.
  Source checks remain supplemental to behavior tests.

Local evidence: 137 scoped Next client tests and 65 source-verifier fixture tests
passed, as did Forum owner/storefront/admin boundary, cursor paging, anonymous
discovery, mention persistence/integration/events, quote command composition,
delete-policy admission, event/runtime static inventories and UI i18n catalog parity checks. The generic i18n-contract guard now passes: its multilingual registry
followed an obsolete foundation path after the MCP migration moved to its owner.
The registry now names the existing MCP-owned migration without closing its
localization gap or changing migration identity; four guard regression tests pass
(`npm run test:verify:i18n:contract`). UI key validation also passes (3,539 references
across 73 packages). Global docs/ADR verification remains red on unchanged registry /
metadata/heading defects in November 24–25 Fly, Pages and JWT decisions, plus the
Pages implementation plan's broken decision link; no Forum or multilingual-contract
failure was reported. These unrelated documentation defects are not fixed here.
The full-key continuation admission has reactive Rust regression sources, not mounted
browser evidence: its Rust tests still require the toolchain checks below.
The remediation gate's diff checks passed; its architecture dependency check could
not run without Cargo. An in-memory Python SQLite harness executed the repository trigger/repair
SQL for withdrawal and open/archived reply removal with/without solution, prior split-statistic failure, archive /
restore, repair idempotence and content preservation. WASM rustfmt parsed/formatted
changed Rust files; this is not compilation. The environment has no Cargo toolchain.
A later scoped check ran **3 SQLite + 3 PostgreSQL 17.6 SQL regressions** in
`scripts/verify/verify-forum-topic-repair.py`: the script extracts the actual topic
repair statement and lifecycle/counter guards, verifies the pre-fix failure,
checks repeated repair of live/open and live/archived projections, preserves deleted
topics and canonical reply state, and proves the tombstone guard remains enabled.
PostgreSQL used an isolated temporary schema on a local Unix-socket-only test server
(provisioned from `@embedded-postgres/linux-x64@17.6.0-beta.15`, not an application
dependency). This is SQL evidence on a minimal fixture, **not** the full Rust
migration chain, service tests, lock-contention evidence or mounted-browser proof.
`scripts/verify/verify-forum-reply-restore.py` now passes **10 SQLite checks +
7 PostgreSQL 17.6 checks**. Three SQLite checks reproduce standalone-reply parent
admission; the other seven scenarios execute the migration-47 SQL templates on
both databases. They reproduce the pre-fix whole-topic failure, admit only exact
snapshot target INSERTs, reject 15 source/snapshot mismatch cases, reject archived
target UPDATEs, close admission on snapshot consumption, retain normal non-archived
writes, and verify strict down/corrected up behavior. The repair matrix was also
rerun green on both databases after extracting the optional PostgreSQL connection
helper shared by the two scripts. These remain minimal, isolated extracted-SQL
fixtures, not Rust MigrationTrait/service execution or concurrency evidence.
Reproduce without Cargo:

```bash
python scripts/verify/verify-forum-reply-restore.py
python scripts/verify/verify-forum-topic-repair.py
# Install pg8000 in a test virtualenv; set RUSTOK_FORUM_TEST_DATABASE_URL.
python scripts/verify/verify-forum-reply-restore.py --postgres
python scripts/verify/verify-forum-topic-repair.py --postgres
```

### Trusted moderation effects — source-ready, runtime validation open

The follow-up in `src/moderation_subject.rs` now routes approved → hidden/rejected
through the shared publication effects after persisting status. It removes accepted
solutions, changes reply/solution statistics in one guarded write, and avoids a second
category/member withdrawal under archived topics. Hide/reject invalidates the topic
projection; removal invalidates it when the reply was public or a solution was removed.
Category invalidation and status events remain in the same transaction. Exact receipt
replay and same-target hide/reject no-ops keep their existing no-write semantics.

Admission now acquires category → parent topic → solution scope → subject/revision,
and revalidates a reply's discovered parent. SQLite reserves its writer through the
revision table before discovering the parent, without invoking source update triggers.
PostgreSQL keeps Serializable transactions and reviewed-revision fencing. This is a
source-level lock-order correction, not runtime contention/deadlock evidence.

The shared `tests/support/moderation_effects.rs` matrix is mounted by
`logic_review_remediation_sqlite`: hide/reject/remove × ordinary/accepted reply ×
open/archived topic on each backend (12 scenarios each). It asserts aggregates,
solution cleanup, status events, PostgreSQL projection-ledger changes, immutable
receipt replay, same-target no-ops and rollback after an injected status-outbox failure.
Rust execution remains blocked by missing Cargo. Existing PostgreSQL content-edit
concurrency tests must also be rerun before integration.

Unintegrated migration 46 now removes invalid solution relations targeting nonpublic
or deleted replies before rebuilding aggregates; approved solutions under live archived
topics survive. Rollback does not resurrect invalid relations. This amends this branch's
new migration, not a previously deployed migration. Run with writers quiesced and
rebuild search projections as already required. The extracted repair SQL/guard harness
passes **4 SQLite + 4 PostgreSQL 17.6 cases**; this is not full migration-chain evidence.
The supplemental effects guard has **11 passing positive/negative Node fixtures**;
three additional lost-response guard fixtures pin rustfmt-tolerant replay assertions.
The scoped combined regression run passes **46 Node tests**, 12 source contracts
(event inventory in `--static-only` mode), i18n and `git diff --check`. The remediation
gate passes diff invariants but cannot run the architecture metadata guard because
Cargo is absent; it is not reported green. No audit phase has been advanced.

```bash
npm run verify:forum:moderation-effects
npm run test:verify:forum:moderation-effects
cargo test -p rustok-forum --test logic_review_remediation_sqlite trusted_moderation_effects
cargo test -p rustok-forum --test moderation_effect_contract_postgres
cargo test -p rustok-forum --test moderation_revision_concurrency_postgres
cargo test -p rustok-forum --test moderation_terminal_concurrency_postgres
```

### Fresh recheck — 2026-11-26

A fresh source review found and corrected three remaining admission/identity risks:

- Topic edits now CAS-claim the source **before** reading transactional edit-window
  settings/time, then reload and recheck ownership and archived status. A writer
  wait cannot consume the remaining window while preserving an earlier admission.
  `tests/support/edit_policy.rs`, mounted by `logic_review_remediation_sqlite`,
  adds SQLite/PostgreSQL service cases: preflight allows 60 minutes, transactional
  policy allows 10, and a 30-minute-old source must be rejected. The reader asserts
  that the source claim already happened; the persisted checkpoint must be unchanged.
  These cases are authored/formatted, **not executed** without Cargo. The supplemental
  bounded ordering guard has nine passing positive/negative fixtures.
- Standalone reply quote admission explicitly locks the reply after its parent and
  before evaluating ownership/settings/time; the later relation-persistence lock
  cannot retroactively protect an earlier time check. A negative guard fixture
  detects replacing this admission lock with a plain read (10 quote fixtures pass).
- Initial Leptos resources return their originating key together with the data;
  `ForumShowcase` uses that key for continuation identity instead of reading the
  newest key when a retained result mounts. Stale initial results cannot update
  read-state availability. Four paging-guard fixtures cover the current wiring and
  removal of these protections. Mounted-browser verification remains open.

The recheck also repaired the read-model verifier's reference to a removed source
file: shared ordering lives in `src/services/topic.rs`, while call-site checks stay
bounded to `read_model_owner.rs`. The storefront now reexports `ForumTopicSelection`
and `ForumTopicFeedOrder`, so callers of the public fetch API can name its selection
argument; a Rust public-crate-path regression is authored but unrun.

Latest executed evidence supersedes earlier local-run counts above:

- **169/169 Node source-verifier fixture tests**, including the generic multilingual
  registry regressions. These are not Rust/service or mounted-UI evidence.
- **121/121 scoped Next client tests** via `npm --prefix apps/next-frontend run test:forum`.
  The first attempt was blocked by missing `@apollo/client`; a separate ignored
  test dependency install resolved the environment without changing project manifests.
- **15 Forum source contracts**, including static-only read-model verification;
  generic i18n contract and its four fixtures; `git diff --check`.
- Extracted repository SQL rerun: repair **4/4 SQLite + 4/4 PostgreSQL 17.6**,
  restoration **10/10 SQLite + 7/7 PostgreSQL 17.6**. The temporary Unix-socket-only
  PostgreSQL server was stopped afterward. This remains minimal SQL-fixture evidence,
  not full migration-chain or Rust transaction/concurrency evidence.

Reproduce the supplemental checks:

```bash
npm run verify:forum:edit-policy
npm run test:verify:forum:edit-policy
npm run test:verify:forum:cursor-paging
node --test scripts/verify/*forum*.test.mjs scripts/verify/verify-db-multilingual-contract.test.mjs
```

### Configured settings capability — source-ready, runtime validation open

A subsequent review found that `module_settings_in_tx` silently used defaults when
only the static settings reader was configured. That could admit preflight against
stored policy and then replace its window with `0` (unlimited), or bypass configured
pre-moderation/vote policy. The shared provider now rejects this incomplete host
configuration with `FORUM_TRANSACTIONAL_SETTINGS_UNAVAILABLE`. Read-only consumers
still use the static reader. Explicit no-reader default-only services and paired
readers returning no stored row retain their existing defaults. The accepted
[capability decision](../../../../DECISIONS/2026-11-26-forum-transactional-settings-capability.md)
partially supersedes the earlier missing-reader fallback, not policy values or scopes.

Provider unit regressions cover all four reader configurations and nontransactional
reads. Transaction-only providers now refuse ordinary and transactional policy reads
with `FORUM_STATIC_SETTINGS_UNAVAILABLE`, including engagement-mode reads (which
previously read the static field directly). The SQLite/PostgreSQL edit-policy matrix
covers a stricter transactional window and either incomplete reader pair, comparing the
source/translation/history/aggregate/outbox checkpoint after refusal. Topic/reply
quote regressions assert no added relation revision. Direct-delete matrices add
static-only staff/system refusal alongside author refusal (12 cases/source/backend)
and assert the typed capability error instead of an incidental false-policy denial.
These Rust cases are authored and WASM-formatted, not compiled or executed.

Boundary review found runtime assembly erasing incomplete capability configurations.
REST (one), GraphQL (one), native composers (two), initial read (one), and continuation
(two) now pass both optional readers unchanged through the shared
`ForumSettingsProviders::from_optional_readers` constructor. Native admin already
requires both capabilities. Supplemental fixtures mutate each capability at every
composition file and verify constructor retention, static/transactional admission,
and both stable codes. No transport may turn a partial pair into default-only services.
REST host-runtime Rust regressions cover both incomplete pairs and HTTP 503;
a GraphQL source-error regression retains both codes and nonretryability.
These runtime regressions remain unexecuted.

The scoped source fixtures remain supplemental to the required Rust service/concurrency
checks below. No migration, client-specific policy or audit ledger advancement is involved.

The same pass fixed two Leptos feed regressions: mode-only selection changes no longer
reset unsubmitted search text (the effect observes an equality-checked memo of the
applied query), and solved badges resolve `forum.topic.solved` from all three catalogs.
Two negative source fixtures reject restoring the draft reset or hardcoded badge;
a localized badge Rust test is authored but unrun. Mounted-browser behavior remains open.

Executed follow-up evidence: **196/196 Node source-fixture tests**; **15 scoped
owner/transport guards**, read-model/runtime-baseline static modes; generic i18n contract;
**3,540 UI-key references across 73 packages**; `git diff --check`. No Rust execution
is claimed. Global ADR verification still fails on the previously recorded Fly/Pages/
JWT registry/metadata defects; it reports no diagnostic for the new capability ADR.
The earlier Next and extracted-SQL results above were not rerun in this follow-up,
which changes neither Next code nor database SQL.

### Fresh recheck — projection/SEO verification reconciled

The two previously recorded source-verifier failures are now corrected against the
current owners, not bypassed. `verify-forum-projection-invalidation.mjs` reads the
actual category/topic/reply modules and requires the reply publication call to carry
the event bus, caller transaction, tenant, actor and topic. Public discovery checks
both SEO provider registration calls in `module.rs`, retaining the `lib.rs` module
and export checks. The missing canonical projection note is restored at its existing
contract path; its event name is `index.reindex_requested`. Contract outbox paths
now point to `crates/libs/rustok-outbox-storage`, matching the executable owner.
Fifteen new positive/negative fixtures reject missing publication, transaction
binding, module declaration, provider registration, event identity, outbox paths and
contract documentation. Run `npm run verify:forum:projection-ownership` and
`npm run test:verify:forum:projection-ownership`.

Executed projection/SEO recheck evidence (before the subsequent GraphQL fix below):

- **211/211 Node source-fixture tests**, no failures/skips;
- **17 scoped Forum owner/transport guards**, plus read-model/runtime-baseline
  `--static-only` checks;
- **121/121 scoped Next frontend tests**, no failures/skips;
- extracted repair SQL: **4 SQLite + 4 PostgreSQL** cases;
- extracted restore/solution SQL: **10 SQLite + 7 PostgreSQL** cases;
- generic i18n contract and **3,540 UI-key references across 73 packages**;
- WASM Rust formatting: **36 changed/new Rust files**, no formatting differences;
- `git diff --check`.

The PostgreSQL fixtures used an isolated local test schema; an initial connection
attempt used the nonexistent `postgres` role, then the successful run used the
cluster's existing `user` role. The database process was stopped afterwards. These
are extracted-SQL checks, not Rust migrations, the complete migration chain or
owner/concurrency execution. Temporary Next dependency wiring was removed after
its test run. No new production-logic defect was confirmed in this recheck.

Global ADR verification was rerun and still fails outside these Forum decisions
(Fly/Pages/JWT/Dioxus registry/metadata); it reports no Forum decision diagnostic.
Cargo remains unavailable (the targeted engagement-mode test exits 127), so Rust
compilation, service/concurrency matrices and mounted native/headless UI validation
remain required. WASM formatting and passing source fixtures do not satisfy those
gates. No audit ledger or integration status is advanced.

### GraphQL settings-error provenance — source-ready, runtime validation open

A further boundary review found eight GraphQL default-setting reads converting
`ForumError` to `async_graphql::Error::new(error.to_string())`. That erased the
precise capability code and retryability/retry hint; aliased fields could not even
use the legacy Forum-path fallback. Topic order and topic/reply page-size defaults
now propagate the typed error via `?` in ordinary queries, audience topic lists and
storefront unread/reply lists. Policy values, source reads and payloads are unchanged.
The capability ADR and CRATE_API now explicitly cover this transport invariant.

`graphql_settings_error_contract` mounts the real Forum query schema and all eight
branches, with field aliases, a transaction-only host and a transient static-reader
failure. It asserts exact codes, `retryable`, `retryAfter`, diagnostic redaction and
the reader call count. The fixture has no Forum source/outbox tables, so admission
must fail before an owner can proceed. This Rust regression is authored and
WASM-formatted, **not compiled or executed**: its targeted Cargo command exits 127.
Eleven new source fixtures cover the three resolver files and mutation of every
one of the eight default accessors. The guard rejected the original code before
its string conversions were removed.

Current executed evidence: **222/222 Node source-fixture tests**, **17 scoped Forum
owner/transport guards**, read-model/runtime-baseline static modes, generic i18n
contract, WASM formatting of **40 changed/new Rust files**, and `git diff --check`.
The **121 Next tests** and **25 extracted-SQL cases** recorded above are historical
for the immediately preceding pass, not rerun for this GraphQL-only change. Rust
services/full migrations/concurrency and mounted native/headless UI remain open.

### Selected GraphQL reads: typed domain failures — source-ready, runtime validation open

The follow-up removed eight more source-erasing conversions: selected category/topic
catch-all branches, topic-route resolution/visibility/disclosure, and member-card
normalization/composition/fallback reads. The transport now preserves `ForumError`
through `?`/`into()` under aliases, without changing existing absence-to-`null`
branches or foreign owner error mappings. The settings regression schema fixture
was factored for reuse and extended with four aliased selected-read/route storage
failure cases and nil-user member-card validation. These Rust cases are authored
and WASM-formatted, **not compiled or executed**; the targeted Cargo command still
exits 127 (`cargo: command not found`). Route fixture identity length and permission
admission were checked against the source, not against a running schema.

The new bounded `verify-forum-graphql-error-provenance.mjs` guard and **13 positive/
mutation fixtures** cover all eight sites and retained absence branches; package
scripts expose both commands. Latest executed evidence: **235/235 Node fixtures**
(Forum plus multilingual-contract fixtures), **18 scoped source guards** (event
contract in `--static-only` mode), read-model/runtime-baseline static modes, generic
i18n, **42 Rust files** with zero WASM formatting differences, and `git diff --check`.
Running the event guard without `--static-only` also stopped at its unavailable
Cargo formatting step; only its static result is claimed. Earlier Next and
extracted-SQL results remain historical, not rerun in this follow-up.

Native/headless feed-order inspection found the same explicit `ForumTopicSelection`
order serialized by GraphQL and mapped by native `selected_topic_filter`; both use
`Latest` as the explicit UI default. That is not evidence of a missing native
settings-default lookup, so no policy change was made. This was source inspection,
not mounted UI/runtime parity evidence. Also corrected stale README text: enabled
`use_reactions` with missing/disabled Reactions fails with the existing capability
code; it does not silently fall back to internal votes.

No audit ledger/integration status is advanced. Rust services/full migrations/
concurrency and mounted native/headless UI checks remain required.

### Category-route error provenance follow-up — runtime and wiring blockers remain

The category-route resolver had three additional `ForumError` erasures hidden behind
`internal_error(error.to_string())`: route resolution, audience-context validation,
and exact category visibility. They now propagate typed errors with `?`/`into()`.
The generic helper is retained for foreign Channel errors, not Forum failures.
The existing category/route-not-found-to-`null` branches remain unchanged.

The bounded provenance guard now covers this resolver and its original helper
wrapper. **20/20 focused fixtures** pass (seven added positive/mutation cases).
The real-schema Rust regression adds invalid-locale validation under aliases for
category and both topic route fields, plus malformed-identity `null` cases that
terminate before owner reads. Those cases are authored and WASM-formatted, **not
compiled or executed**: Cargo remains unavailable. They must not be represented
as successful resolution or authorization of real Taxonomy-backed routes.

Latest executed evidence: **242/242 Node fixtures**, **18 scoped source guards**
(event contract static-only), read-model/runtime-baseline static modes, generic
i18n, **43 changed/new Rust files** with zero WASM formatting differences, and
`git diff --check`. No Next, extracted-SQL, mounted UI, full migration or service
runtime evidence was produced by this follow-up.

**Finding at that pass; consumer propagation repaired below:** GraphQL
`category_route_query.rs` and native `storefront_category_route_native` constructed
`ForumCategoryRouteService::new` without `with_port`, installing Noop whose route
lookup always returns `None`. GraphQL runtime can receive an optional
`Arc<dyn ForumTaxonomyPort>`, but it did not forward that capability to route and
category audience owners. The subsequent trace also confirmed that `apps/server`
at that pass had no concrete provider implementation/registration. This was a
separate blocker (host source implemented in the continuation below): changing error shape or forwarding an absent port cannot make
Taxonomy-backed production routing available. No audit ledger/integration status
is advanced.

### Injected Taxonomy read capability propagation — evidence before host adapter implementation

Configured providers now reach GraphQL category routes and category audience reads,
REST selected/list/tree reads, native category data/route reads, Search projection
canonical routes and public discovery, and server Search category subtree expansion.
`ForumCategoryAudienceReadService::with_taxonomy_port` forwards to both metadata and
visibility composition; the base category floor and richer inherited audience layers
now use the injected hierarchy rather than the standalone root-only fallback.
Provider-backed hierarchy reads retain the caller's borrowed connection; visibility
policy operations also retain their transaction. No-op standalone behavior remains
unchanged and does not produce a canonical category route.

SEO now carries a generic cloned `ModuleRuntimeExtensions` snapshot in
`SeoTargetRuntimeContext.runtime_extensions`, preserving the acyclic ownership seam.
The host SEO service forwards it to providers; Forum's category mapper, route and
public-discovery compositions retrieve their own typed port from that snapshot.
All workspace context struct literals were updated, including Pages fixtures. The
Taxonomy ADR and affected API/README contracts record this context addition. Forum's
remaining production Taxonomy dependency was moved to dev-dependencies, matching
the accepted port boundary; the remaining source imports are test-only.

`tests/taxonomy_read_wiring.rs` adds **eight authored Rust regressions** using a
read-only injected provider and Forum-only tables: canonical/alias routes, provider
failure at route/hierarchy/snapshot stages, inherited authenticated/richer audience
refusal, standalone route absence, direct audience metadata/failure, cross-tenant
refusal before provider invocation and unknown-route absence. Unexpected fake writes
panic. A separate SEO unit regression checks provider capability identity and
snapshot independence. These Rust tests are WASM-formatted, **not compiled or
executed**; `cargo test -p rustok-forum --test taxonomy_read_wiring` exits 127 because
Cargo is absent. Native/SEO/Search runtime parity is not established by source pins.

Executed evidence: **302/302 Node fixtures**, including **60 new Taxonomy wiring/
route-guard fixtures** over **28 bounded composition edges**; **20 scoped source
guards** (event contract static-only); read-model/runtime-baseline static modes,
generic i18n, WASM formatting of **65 changed/new Rust files**, and `git diff --check`.
The existing SEO source guard now follows the injected route factory rather than
requiring a bare constructor in the wrapper, and still checks canonical registration.
The affected category-route transport source guard/Rust contract test also referred
to removed include slices and a removed per-slice document. They now inspect the
live owner/adapter files and canonical docs index; FORUM-24N historical compatibility
flags remain historical, not a claim that later route mounting never happened.
Positive and missing-factory/owner fixtures exercise the refreshed Node guard;
the Rust source contract remains unexecuted.
Next/extracted-SQL results above remain historical, not rerun here.

**Still open:** implement/register the concrete `ForumTaxonomyPort` adapter in an
acyclic host composition, including transaction-preserving hierarchy/category/tag
mutations and error mapping. The accepted ADR already marks this cutover incomplete.
The fake provider proves no actual Taxonomy storage integration, and consumer
propagation must not be called production route readiness. Full Rust compilation,
provider-backed migration/service tests and mounted native/headless checks remain
required. No audit ledger or integration status is advanced.

### Deep re-review of Taxonomy read wiring — three confirmed defects corrected

The re-review traced the changed host/runtime/SEO/Search composition into category
metadata, base/richer visibility, list/tree and transactional policy owners, then
reviewed the new fixtures and source guards. This was a targeted code review of the
affected boundary, not an audit-ledger pass or full-crate runtime certification.

1. **Least-privilege failure:** the storefront selected-category owner admitted
   `forum_categories:list` but the shared metadata path demanded `read` again.
   The now-reachable provider-backed route therefore rejected list-only readers.
   A private `CategorySelectedReadAccess` mode carries either owner-read or
   storefront-list admission through both owners. Ordinary selected reads remain
   read-gated; no permission snapshot is augmented and read-only does not grant list.
2. **Unvalidated provider membership:** parent maps were trusted without checking
   all requested local IDs or parent references. An empty map could produce no
   hidden IDs on a public list, and extra IDs could enter ancestor traversal.
   `category_hierarchy::validate_category_parent_projection` now validates exact
   tenant-local membership, all parent references, cycles and the depth limit before
   overrides or visibility filtering. All four whole-tenant parent-map consumers
   (base visibility, richer audience, direct Search subtree and read-tracking subtree)
   use it. Direct Search scope also forwards its port to its visibility owner.
3. **Tree panic:** a same-length projection with duplicate/foreign IDs passed the
   old cardinality check, leaving a missing canonical map entry at `.expect` during
   sorting. The tree mapper now checks every identity and duplicate insertion, then
   validates the complete parent graph before sorting.

`taxonomy_read_wiring` now has **12 authored integration tests** (four added test
functions, including permission combinations, malformed hierarchy/list/Search,
malformed tree metadata and a provider failure on the second hierarchy read after
policy persistence). The failure fixture requires `Transaction` connections and
asserts that the policy write rolls back before success publication. Three new pure
Rust unit tests cover empty/valid graphs, missing/foreign/cyclic maps and the exact
depth boundary. **None of these Rust tests were compiled or executed**: the scoped
Cargo command still exits 127. The fixture/source inspection is not rollback proof.

Latest executed evidence: **336/336 Node fixtures**, including **94 Taxonomy source
fixtures** over **45 bounded wiring/integrity/admission pins**, **20 scoped source
guards** (event contract static-only), read-model/runtime-baseline static modes,
generic i18n, WASM formatting of **71 changed/new Rust files**, and `git diff --check`.
Those source fixtures would not replace the newly added behavioral regressions.
No new SQL/Next/mounted-UI evidence was produced; earlier numbers remain historical.
At that pass the missing concrete server Taxonomy adapter and unexecuted Rust/runtime
checks were explicit blockers. The continuation below implements the adapter source;
its runtime checks remain pending. No audit ledger or integration status is advanced.

### Real in-process Forum Taxonomy host adapter — implementation, runtime pending

The continuation implements the previously missing concrete provider at
`apps/server/src/services/forum_taxonomy.rs`. The composition root depends on both
modules; neither Forum nor Taxonomy gains a new production dependency on the other.
All 12 port operations delegate to Taxonomy owner APIs; category reads and
category/tag mutation scopes are fixed to `forum`. There are no direct foreign entities/table reads in this host adapter.

Host registration runs at the start of `module_event_dispatcher.rs` host-provider
assembly, before any source capability materialization. It is feature-gated and
requires both modules in the distribution
registry, not merely compiled Cargo features. It occurs before Search/SEO/GraphQL/
native extension snapshots. Forum-only registries preserve the isolated Noop
profile. Category sync retains `forum-category-{uuid}` canonical identity and
Taxonomy-owned append-only alias history; hierarchy/tag writes keep the consumer
transaction. Reads dispatch `Connection` and `Transaction` unchanged. `None` fails
with `FORUM_TAXONOMY_CONNECTION_REQUIRED`, never a fresh pool read.

Two missing owner-API capabilities were added in Taxonomy rather than accessing
its tables from the host:

- `resolve_term_route_for_module_in`: route lookup and alias provenance both use
  the supplied connection; the pool method now delegates to it without changing
  its locale/scope precedence. The Forum adapter refuses global-scope fallbacks.
- `load_module_category_parents_in_strict`: selected IDs must all be Categories in
  the requested tenant/module scope with explicit placements. No translation is
  needed. Missing terms/placements fail, rather than inheriting the generic legacy
  reader's synthetic root default. Whole-tenant graph validation remains Forum-owned.

Six host tests are authored using actual Taxonomy migrations and owner APIs:
transaction-local sync/canonical+alias routes/localized copy/tags and rollback;
committed tenant/scope/empty-selection behavior; hierarchy commands and cycle
refusal; missing connection/storage errors; joint-registry selection; and typed
redacted provider-error mapping. A separate Taxonomy regression removes a placement
and verifies that the strict parent reader rejects it. These seven tests are
**uncompiled and unexecuted** in this environment; source inspection is not rollback
or full-migration evidence. No production runtime certification is claimed.

The source guard now includes host registration/dispatch, strict scope/placement,
owner alias sync and transaction-aware route/alias reads. Executed evidence for
this continuation: **364/364 Node fixtures**, including **122 Taxonomy source
fixtures** over **59 bounded pins**; **20 scoped source guards** (event contract
static-only); read-model/runtime-baseline static modes; generic i18n; **77 changed/
new Rust files** with zero WASM formatting differences; and `git diff --check`.
These are source/format checks, not execution of the seven new Rust regressions.
`cargo test -p rustok-server --no-default-features --features mod-forum --lib
services::forum_taxonomy::tests` exits 127 (`cargo: command not found`).
The remediation gate's diff invariants passed, but its mandatory architecture
check could not run: `cargo metadata` is unavailable (Cargo absent). This gate is
**blocked**, not green. No manifest cycle is introduced by the source dependency
inspection, but Cargo must confirm it. PostgreSQL/full mounted transports and
remote-provider/distributed transaction behavior remain outside this evidence;
the delivered adapter is explicitly shared-database/in-process.

#### In progress: inherited policy capability propagation

The confirmed topic base/richer inheritance gap has a partial source repair:

- `ForumTopicVisibilityService`, richer topic/reply readers and lists receive the
  selected hierarchy provider. Generic policy composition passes the actual pool
  or transaction connection; topic-policy management retains its transaction.
- GraphQL/REST/native and SEO/Search/discovery factories, Reactions/Notifications,
  widget preview and storefront unread/read-state composites forward the provider.
- Vote and topic-subscription write gates use the selected provider inside their
  existing category-tree writer transaction. Report audience preflights use it
  before revision/intake access. Blind subscription clears remain non-probing.
- Category topic/reply-create owners, topic-local reply-create policy and both
  create-authorization owners receive the selected provider and actual connection.
  Context-aware and contextless public creation use the same configured admission
  owner; missing context no longer selects an unconfigured raw command path.
- Native storefront composer and native admin reply creation forward the hierarchy
  provider; the admin write now also forwards host audience facts and the exact
  authenticated native `PortContext`. GraphQL/REST keep their configured facade.
- Split/fork/reply-range services and GraphQL adapters carry the provider through
  both source and target visibility/reply-create policy comparisons in the writer
  transaction. Reply-range comparison also checks the inherited base public/auth
  floor. There is one canonical reply-create loader, not a Noop compatibility copy.
- Four additional Forum-only Rust regressions cover create-policy transactions,
  local clearing/inheritance, denied and allowed admission, context/contextless
  writes and rollback after a post-write provider failure. Three range-move unit
  regressions cover uncommitted base/richer/reply-create policy comparisons,
  unrestricted equality and provider error propagation. All remain unexecuted.
- The anonymous transactional owner gate now enforces the inherited base floor.
  A dedicated unit regression writes an ancestor restriction inside an uncommitted
  transaction, checks denial, rolls back and checks visibility is restored; a
  second regression checks provider failure. These tests are authored, not run.
- Forum-only topic/reply regressions cover inherited base and richer denials,
  local rules that cannot broaden ancestors, read/discovery/Search denial before
  content hydration, selected-provider failures and topic-policy transactions.
  Engagement/read-state/report regressions assert exact denied-target errors
  without mutation/content tables. These tests are likewise uncompiled/unexecuted.

- Richer category policy management, moderation category management/authorization
  and the public `ModerationService` now carry the selected hierarchy. Canonical
  moderation policy loaders require a provider and retain the actual pool/writer
  transaction; errors propagate without standalone fallback. GraphQL/HTTP
  moderation factories forward the provider alongside optional facts. Exact
  topic-author solution authorization remains separate from moderator admission.
- Six additional Forum-only regressions cover managed local clearing/inheritance,
  post-replacement provider failure/rollback, ancestor denial and positive
  authorization, uncommitted transaction policy, denied command effects, and
  configured GraphQL/mounted HTTP factories. They are **authored, not executed**;
  no mounted transport runtime evidence is claimed. No client schema, UI or
  migration changed. Trusted `moderation_subject` effects are a distinct workflow.
- Corrected the stale moderation contract wording: authorization belongs inside
  the locked writer transaction, before domain effects, not before transaction
  creation. The source guard checks the selected canonical loader; mutation
  fixtures reject a missing provider gate or owner.

#### Deep re-review corrections

- **P1 — invalid ORM FK declarations in the new consumer fixtures:** all 24
  category/topic audience relation entities pointed only to `category_id` or
  `topic_id`, although the policy PK is `(tenant_id, subject_id)`. SeaORM 2.0.4's
  schema builder emits those declared FKs. An isolated SQLite execution with
  foreign keys enabled reproduced `foreign key mismatch`; disabling FK checks
  would conceal the defect and break policy-clear cascades. Corrected both FK
  sides to the composite tuple. Production migrations already have this shape;
  no migration or client/DTO change is required. Six added Rust schema regressions
  are authored, not executed.
- **P2 — transactional require contract returned a denied success:** both create
  audience `require_in_tx` methods returned `Ok(allowed: false)` from the evaluator.
  Current raw writers compensated with additional checks, so this review does
  **not** establish an existing write bypass. Moved typed denial conversion into
  a shared private `require_allowed`, used by pool and transactional require, and
  removed duplicate writer conversions. Two private Rust regressions exercise
  inherited denial/rollback recovery and topic-local reply narrowing; unexecuted.
- Strengthened the moderation local-clear regression: it now creates a real
  child policy and checks two layers before clearing to one inherited layer.
  Previously its moderation half cleared an already-empty child. Removed the
  unnecessary `unused_variables` suppression from the shared fake and explicitly
  named unused write arguments instead.
- New relation guards inspect all 24 declarations, reject loss of either tenant
  column, and execute a projection of the FK tuples under SQLite. Their passing
  result does not substitute for the six SeaORM-generated schema regressions.

#### Structural/lifecycle and tombstone continuation

- Move/merge selected-provider builders now preserve inherited target-policy
  reads in their writer transaction; both GraphQL merge commands forward the
  runtime provider. Manager move/merge may change audience, unlike range movement;
  no new equality restriction is imposed. Exact receipt replay remains ahead of
  current category/provider checks and preserves immutable receipts/events.
- The common placement/restore lifecycle gate no longer hardcodes Noop or requests
  localized category snapshots. It uses the selected parent-only projection,
  validates complete tenant membership and bounded graph integrity, and retains
  the caller transaction. The previous Noop snapshot path rejected nonempty
  trees as incomplete; this is not evidence of a lifecycle authorization bypass.
  Topic create, category create/move, split/fork/move/merge/range and topic/reply
  restore all forward their selected provider. Reply facade retains it in the
  restore owner; category parent wrappers preserve typed capability/storage errors.
- Deeper helper review confirmed that tenant-operation lock helpers already took
  the same PostgreSQL hierarchy key implicitly; the later split/fork calls were
  redundant. This is **not** a demonstrated prior deadlock/race. Separated receipt
  serialization from the shared hierarchy scope: all five commands now call the
  canonical tree-lock owner explicitly after immutable replay and before counter/
  topic rows. Removed duplicated inline hierarchy SQL from operation helpers.
  The ordering guards pin this ownership, not a PostgreSQL concurrency proof.
- Delete snapshots use selected base visibility and richer audience with the
  actual writer transaction. Errors abort the transaction; existing immutable
  history remains unchanged. The sealed boolean read side needs no new provider.
- Six private Rust regressions in `services/structural_taxonomy_tests.rs` cover
  selected lifecycle/uncommitted ancestor/rollback recovery, incomplete projection
  and typed failure, post-write move/merge rollback (including moved reply state),
  tombstone base/richer ancestor privacy and provider errors. They are authored,
  **not compiled or executed**. Their Forum-only entity schema is not production
  migration or host-provider evidence.
- Repaired all three stale visibility/tombstone guards against live owners,
  selectors, consolidated native adapter route scope and canonical docs, without
  recreating deleted modules. Move guard now pins selected transaction calls.
  Isolated mutation fixtures reject missing owner files and missing owner calls.

Local supplemental evidence: the taxonomy wiring guard checks 259 bounded composition
edges and five structural lock orders; all 522 wiring fixtures and 32 additional
structural-order/live-guard fixtures pass. Creation guards follow the live raw
owners and the single mounted REST command route instead of deleted include files
or handlers; 30 base/richer/create/moderation guard fixtures check current source,
removed gates and missing owners. Category/topic policy guards inspect the shared
bounded storage and live Rust modules instead of removed helper/include files.
The broader Forum/DB-multilingual Node suite passes 900/900 tests, including
74 relation-schema guard cases and SQLite execution of declaration-projected FK
tuples (not SeaORM-generated DDL or services). All 31 additional targeted Forum
source guards pass; `git diff --check` passes. These are not Rust/runtime evidence:
they detect source-contract drift and lost wiring, not Rust type errors, runtime
registration or real owner storage behavior.

The scoped remediation gate passes diff invariants but is **blocked** in its
architecture dependency phase: `cargo metadata` cannot start because Cargo is
missing. `cargo test -p rustok-forum --lib services::structural_taxonomy_tests`
likewise exits 127. Do not treat this as a passing gate or compiled verification.

**Remaining mandatory repair/verification (release blocker):**

1. Broaden positive/negative split/fork/range and move/merge consumer/transport
   regressions beyond these source guards, private transaction tests and current
   range-move helper tests. Run the authored regressions, fixing any compiler or
   runtime failures rather than treating static success as service evidence.
2. Run Forum and host Rust suites, real-owner PostgreSQL concurrency, migration
   DDL and mounted transport acceptance. No Rust build, concurrency or mounted-client
   evidence exists for this continuation because Cargo is absent. No UI/wire
   schema change is introduced by provider wiring.

No adapter-only/source result closes this boundary. Inheritance acceptance
remains in progress; the README limitations mirror the unresolved evidence gates.

Required maintainer checks before integration:

```bash
cargo test -p rustok-server --no-default-features --features mod-forum --lib services::forum_taxonomy::tests
cargo test -p rustok-taxonomy --test owner_category_read strict_module_parent_projection
cargo test -p rustok-forum --lib category_hierarchy
cargo test -p rustok-forum --test taxonomy_read_wiring
cargo test -p rustok-forum --test audience_relation_schema
cargo test -p rustok-forum --lib create_audience_authorization_tests
cargo test -p rustok-forum --lib structural_taxonomy_tests
cargo test -p rustok-forum --lib topic_audience_visibility::tests
cargo test -p rustok-forum --lib topic_reply_range_move::policy_tests
cargo check -p rustok-forum-admin --features ssr
cargo check -p rustok-forum-storefront --features ssr
cargo test -p rustok-seo --lib target_context_keeps_the_same_injected_provider_capability
cargo check -p rustok-seo-targets --features server
cargo check -p rustok-server --features mod-forum
cargo test -p rustok-forum --lib standalone_quote_commands
cargo test -p rustok-forum --lib services::engagement_mode
cargo test -p rustok-forum --lib static_only_host_settings
cargo test -p rustok-forum --lib transaction_only_host_settings
cargo test -p rustok-forum --lib preserves_transactional_settings_capability_code
cargo test -p rustok-forum --test graphql_settings_error_contract
cargo test -p rustok-forum --test logic_review_remediation_sqlite
cargo test -p rustok-forum --test storefront_category_connection_graphql_contract
cargo test -p rustok-forum --test topic_slug_rename_sqlite
cargo test -p rustok-forum --test topic_slug_rename_graphql_contract
cargo test -p rustok-forum --test mention_quote_runtime_postgres
cargo test -p rustok-forum --test soft_delete_revision_postgres
cargo test -p rustok-forum-storefront --lib
cargo check -p rustok-forum-storefront --features ssr
cargo check -p rustok-forum-storefront --target wasm32-unknown-unknown --features hydrate
```

Set `RUSTOK_FORUM_TEST_DATABASE_URL` for the PostgreSQL cases (including the case
in `logic_review_remediation_sqlite`); a skipped database test is not green evidence.
Run mounted native/headless category -> search/Top/Solved/Unread -> load-more tests,
including an empty result, selection change during an outstanding page, and an open
composer draft. Measure tenant policy-lock contention and sparse-feed scan/query
amplification before changing the serialization or completeness contracts; neither
risk is claimed eliminated by these correctness changes.

## Scoped forum audit remediation — 2026-10-28

Branch: `fix/forum-audit-2026-10-28`, base `main` at
`35ceddafb6fec742d4361dc89e44a09328a9235d`.
**Status: in_progress, source changes only; no local cargo, database or browser runtime.**
Do not mark complete or integrate until the maintainer runs the scoped checks and
explicitly authorizes a merge. No competing audit ledger is created.

The remediation follows the canonical Forum owner, existing ADRs and `AGENTS.md`.
Findings are ordered by risk and by dependency (owner correctness before transport/UI):

1. **P1, source-ready in this branch:** Publish `ForumTopicCreated` only on
   the first transition to public visibility (direct `Open` creation,
   `Pending -> Open` approval, or a first-ever `Archived -> Open` restore
   after `Pending -> Archived` rejection). Consult the immutable Forum
   creation/status journal inside the moderation transaction. Re-archiving and
   restoring a previously published thread never notifies again.
   Notification audience is reauthorized at fanout.
2. **P1/P2, source-ready in this branch:** Archive/reopen adjusts category
   topic **and approved reply** totals and recomputes author topic/reply/solution
   counts in the same transaction. Solution mark/clear re-checks archived state
   under the locked topic row, eliminating its stale pre-transaction status
   acceptance. Preserve `forum_topics.reply_count` as the approved-reply
   projection. Emit category and topic invalidations. Preexisting drift still
   needs a separately reviewed reconciliation.
3. **P1, outstanding (CI-specific authorization required):** The
   taxonomy-category-binding workflow references retired paths and a missing
   runtime test. `AGENTS.md` forbids workflow edits unless the maintainer
   explicitly requests CI/CD changes. The attempted workflow edit was
   reverted; once authorized, update the workflow to the live Taxonomy
   owner files and canonical read-cutover test.
4. **P1, partially implemented (remaining external owner contract):** Forum
   category/topic audience is evaluated from the write transaction after
   acquiring the category-tree writer lock, including reply-parent lookup.
   Topic subscription updates use the same boundary; blind clears remain
   possible after archival. Reply votes now lock/check the parent topic and
   refuse writes after archival, matching topic vote behavior.
   This serializes Forum-owned policy changes with writes, but host-owned
   trust/channel/group facts remain outside the database snapshot and the
   vote/subscription route channel is still unmodeled. Design a host-owned
   revision/fencing contract before claiming full cross-owner atomicity.
   SQLite committed-revocation and PostgreSQL policy-lock overlap regressions
   are source-ready; host-fact and route-channel races still need owner contracts.
5. **P2, source-ready in this branch:** Reply first publication is derived
   from the Forum append-only creation/status journal before the status
   mutation, including `Rejected -> Approved` and `Hidden -> Approved`.
   Repeat visibility cycles must not duplicate `ForumTopicReplied`. SQLite
   and PostgreSQL semantic regression sources are ready; PostgreSQL concurrency,
   restart and import paths remain unverified.
6. **P2, source-ready in this branch:** Topic/reply vote set/clear and topic
   solution mark/clear acknowledge successful owner writes without a subsequent
   audience-selected topic/reply read. Both vote clears delete only the
   actor's own row without probing the target. Topic clearing remains possible
   after archival; no phantom projection event is emitted for no-op clears.
   REST returns
   `204 No Content`; GraphQL returns `Boolean!`. Owner authorization,
   tenant checks and write-failure propagation are unchanged; mutations
   do not leak hidden content. Confirm
   clients, schema and authorization races in the maintainer runtime run.
7. **P2, outstanding:** Wire the canonical posting policy evaluator and its
   window facts into create transactions (not only cooldown). Verify
   concurrent writers, temporal boundaries, failure modes and actor exemption.
8. **P2, outstanding:** Add report rate limits through the Moderation owner,
   category/topic/reply report UI, and operator-friendly feedback. Avoid
   duplicating automatic Moderation policy in Forum.
9. **P2, partially source-ready:** `default_topic_sort` now resolves
   omitted REST, GraphQL owner, storefront and unread sort orders from
   tenant settings (`latest_reply` or `top`). Explicit ordering wins;
   invalid stored values produce a typed validation error. The module
   manifest now lists only `latest_reply` and `top`; any previously saved
   unsupported value needs explicit operator correction, not a silent remap.
   `allow_user_topic_closing` was removed as an unwired setting by
   `DECISIONS/2026-11-02-forum-remove-unwired-closing-and-digest-settings.md`; author
   self-close returns only as a complete write-authorization and UI feature.
10. **P2/P3, outstanding:** Separate faithful owner widget preview from live
    storefront preview, batch audience checks, and avoid a hard failure over
    1000 candidates.
11. **P3, outstanding:** Exact reply anchors beyond the 20-page discovery
    budget need a viewer-scoped owner-backed cursor/location lookup.
12. **P3, outstanding:** Complete topic move/revision/attachment transports
    and UI; benchmark sparse unread/Search/Top/Solved reads on PostgreSQL.
    Do not assert performance without measurements.

Maintainer verification before merge:

- `cargo fmt --all -- --check`
- `cargo check --locked -p rustok-forum --lib`
- `cargo test --locked -p rustok-forum --lib`
- `cargo test --locked -p rustok-forum --test topic_sort_settings`
- `cargo test --locked -p rustok-forum --test topic_publication_lifecycle_sqlite`
- `cargo test --locked -p rustok-forum --test moderation_semantics_sqlite`
- `cargo test --locked -p rustok-forum --test moderation_semantics_postgres`
- `cargo test --locked -p rustok-forum --test moderation_revision_concurrency_postgres`
- `cargo test --locked -p rustok-forum --test votes`
- `cargo test --locked -p rustok-forum --test vote_subscription_audience_concurrency_postgres`
- `cargo test --locked -p rustok-forum --test graphql_stats_and_vote_tenant_scope_sqlite`
- `cargo test --locked -p rustok-forum --test counter_reconciliation_sqlite`
- `cargo test --locked -p rustok-forum --test category_taxonomy_read_cutover`
- `node scripts/verify/verify-forum-taxonomy-category-binding.mjs`
- Review `git diff --check`, GitHub Actions status and deployed outbox recipient
  behavior (pending approval once, archive restore never).

SQLite source regressions in this packet are **written but not executed** here.
PostgreSQL concurrency, migration/backfill effects, frontend integration,
outbox delivery/notification deduplication and the maintainer's acceptance
remain unverified. Keep this packet separate from future `main` integration.

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
and the canonical GraphQL mutation for CSR/headless use without fallback. Delivered through `FORUM-21X`; FORUM-21X composes the bounded reply-range move command with no transport fallback. Dirty
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
| `FORUM-16` | `in_progress` | FORUM-16A-F add tenant-scoped monotonic topic read state, bounded unread projections, resumable category-subtree/all-read owner commands, REST/GraphQL contracts, authenticated visible-topic storefront composition and a source-ready PostgreSQL concurrency/query-plan proof; maintainer execution/output capture and visibility-scoped storefront bulk commands remain. |
| `FORUM-17` | `planned` | Forum drafts/bookmarks with optional Notifications reminders and Media references. |
| `FORUM-18` | `in_progress` | Neutral API, optional owner registration/selection, tenant-composite persistence, shared receipts, atomic actor aggregates, semantic reaction events, bounded aggregate reconciliation, Forum topic/reply provider, Blog second producer, host materialization, composition-test source, bounded Reactions GraphQL transport, separate module-owned Reactions storefront controls, dual-path generic visibility-gated Forum topic/reply current-revision transport, bounded selected-topic/selected-reply host UI composition and Rust Playwright browser-evidence source are ready. Retain the browser execution plus event-digest, owner/event/repair/Forum+Blog/GraphQL/UI/runtime evidence and release lockfile verification; Forum votes remain separate and no reaction ownership moves into Forum. |
| `FORUM-19` | `done` | Bounded Forum/Moderation integration is complete: neutral topic/reply adapters, Forum-owned revision fencing and exact supported effects, Moderation-owned durable dispatch/scheduler/audit/recovery, dedicated `moderation_cases` RBAC, authorized recovery GraphQL transport, replay-safe fresh-revision new-case/new-decision re-review, and retained PostgreSQL/SQLite/host/concurrency/lost-response/accounting evidence are present. `Unpublished`, temporary restrictions and broader neutral effects remain separate future feature scope rather than lossy approximations. Deployment/promotion evidence is deferred to `PROD-FORUM-19`; Moderation keeps cases, decisions, appeals and audit. |
| `FORUM-20` | `in_progress` | FORUM-20A-G provide rich visibility and recipient-aware source/inbox slices; FORUM-20A-AO provide the notification inbox/group owner plane with authenticated storefront ports and native/GraphQL transport parity; FORUM-20A-AS provide exact topic-create audience enforcement with shared Channel, Groups and trust facts adapters; FORUM-20A-AU provide the inherited category reply-create layer and its owner enforcement; FORUM-20A-AZ provide inherited category moderation audience, existing solution-route composition and the synchronized ledger through FORUM-20BA. Reply anchoring on the Next.js surfaces is delivered through the `?reply=<uuid>` route contract (`DECISIONS/2026-10-11-forum-reply-anchor-route-contract.md`); remaining: an owner-side read for the page containing a reply, remaining reads, reconciliation, delivery and PostgreSQL evidence. |
| `FORUM-21` | `in_progress` | A-X provide move/merge/split/fork/range owners, transports and UI. Retained runtime evidence remains. |
| `FORUM-22` | `planned` | Forum-owned Q&A/wiki/announcement kinds and scheduled lifecycle. |
| `FORUM-23` | `in_progress` | Search projection/filtering, ordering, owner revisions and repair exist; maintainer PostgreSQL/Iggy plus LINK-FORUM-03 runtime evidence remain. |
| `FORUM-24` | `in_progress` | A-S plus descriptor correction provide category/topic routes, transports, mounts, SEO/hreflang and Search URLs. Execute registered-host, HTTP, browser and reindex evidence. |
| `FORUM-25` | `done` | Category backend/Translation implementation is complete: canonical Category copy/routes/hierarchy are Taxonomy-owned, the duplicate Forum provider/cursor/progress runtime and legacy donor storage are retired, and Arabic/admin/storefront locale boundaries are source-ready. Mounted multilingual/RTL browser execution is deferred to the final production-validation phase and does not keep FORUM-25 open; topic/reply UGC translation remains opt-in rather than an automatic provider. |
| `FORUM-26` | `in_progress` | FORUM-26A-J provide authoritative Forum trust state/facts, posting-policy contracts, evaluation/composition, account-age, topics-read, approved-post and topic/reply create-window facts, plus pre-enforcement author/query-plan hardening. Active flags/moderation history, reputation, edit windows, bump age, policy persistence, owner enforcement, shared rate-limit execution, duplicate hashing, optional scoring, transports, UI and maintainer runtime evidence remain. |
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
| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |
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
**Dependencies:** FORUM-20/23

Prove publish, translation, moderation approval, move, hide/delete, ACL change,
attachment and reader-state transitions, out-of-order events and search-disabled behavior
end to end: Forum owner commit, outbox event, Search inbox and projector, owner checkpoint,
and the storefront read that must not return a document the owner no longer exposes. The reconciliation queries
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
boundary represent delivered history, not active work, and they still name the retired Forum-only
storefront executor and projector. Read this section only for the live
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


### Aggregate boundary of `FORUM-23`

`FORUM-23B2G2B3A` carries the versioned-invalidation wire contract
(`crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-wire.json`,
`contract_frozen_implementation_pending`), `FORUM-23B2G2B3B1` the caused-publication API and
`FORUM-23B2G2B3B2` the Forum-owner publisher
(`crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-causation-api.json`,
`source_complete_family_publisher_pending`), and `FORUM-23B2G2B3C` the persistent
consumer (`crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-consumer.json`,
`source_complete_runtime_evidence_pending`). None of them closes `FORUM-23`. The
family that pins this boundary is
`verify-forum-search-versioned-invalidation-wire.mjs`,
`verify-forum-search-versioned-invalidation-causation-api.mjs`,
`verify-forum-search-versioned-invalidation-publisher.mjs`,
`verify-forum-search-versioned-invalidation-consumer.mjs` and
`verify-forum-search-versioned-invalidation-runtime-evidence.mjs`.

`FORUM-23B2G2B3D0`
(`crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json`)
is the retained runtime-evidence protocol and stays
`source_ready_maintainer_execution_pending`: the maintainer harness must execute and retain every D0 PostgreSQL/Iggy scenario
- normal delivery, the legacy and typed
duplicate deliveries, acknowledgement failure with restart, multi-process delivery,
raw and semantic poison quarantine, missing-delivery repair, snapshot-only backfill,
search-disabled recovery and the deletion/ACL ordering case - and only the assembled
`LINK-FORUM-03` cross-module runtime proof can advance the aggregate. The two per-slice
handoffs `crates/modules/rustok-forum/docs/forum-23b2g2b3d7-normal-delivery-proof.md`
and `crates/modules/rustok-forum/docs/forum-23b2g2b3d8-deletion-acl-ordering-proof.md`
carry the exact `cargo test --ignored` invocations and the PostgreSQL prerequisites; no
agent may substitute a static guard for that run. `FORUM-23B2G2B3D1` is the canonical
plan-sync slice and is satisfied by the two subsections above; the read-only
`node scripts/audit/forum_plan_requirements.mjs` reports which markers the verifier
family demands of this section.

The owner-revision boundary itself is pinned by
`verify-forum-search-owner-revision-ledger.mjs`,
`verify-forum-search-owner-revision-counter-hardening.mjs`,
`verify-forum-search-owner-revision-source.mjs` and
`verify-forum-search-owner-revision-checkpoint.mjs`. The versioned family is
`FORUM-23B2G2B3A` (frozen wire contract), `FORUM-23B2G2B3B1` (caused-publication
API), `FORUM-23B2G2B3B2` (owner publisher), `FORUM-23B2G2B3C` (persistent
consumer) and `FORUM-23B2G2B3D0` / `FORUM-23B2G2B3D1` (retained runtime
evidence); each keeps its own execution-pending status, and only the
`LINK-FORUM-03` cross-module runtime proof can close them.

### Remaining scope of `FORUM-23`

Search filters include
category subtree, author, tag, locale, date, solved, kind, channel/group and
attachment-presence families; kind and attachment-presence have no owner model yet.

The ordering work is source-complete but not evidence-complete. `FORUM-23B2G1`
replaced wall-clock and event-UUID execution ordering with a PostgreSQL-issued
`ingest_sequence`; `FORUM-23B2G2A`-`FORUM-23B2G2B2` added the owner-issued
monotonic projection revisions, their counter hardening, the owner publisher and
the Search checkpoint; and `FORUM-23B2G2B3A`-`FORUM-23B2G2B3D19` added the
versioned invalidation wire contract, the persistent consumer and the retained
runtime-evidence protocol.

No slice closes `FORUM-23`: owner-issued revision reconciliation stays open only
as maintainer runtime evidence, because the PostgreSQL/Iggy restart, retry,
poison, multi-process and ACL/deletion runs are not retained, and the
`LINK-FORUM-03` aggregate proof cannot be assembled from source alone. The
source side does establish the contract those runs must confirm: owner/index
revisions reconcile through one shared identity
(`forum_projection_revision_ledger.event_id` = `search_projection_inbox.event_id`
= `ContractEventEnvelope.causation_id`) and one `ingest_sequence` order, so a
replay can only move forward.

Tag resolution for the Search projection is still one round-trip pair per topic
(`forum_topic_tags`, then Taxonomy term names): `project_topic` reads each topic
through `get_public_topic_with_locale_fallback`, which resolves its tags itself.
Finding 9.2 of `forum-audit-report-2026-11-07.md` asks for one batch per page. Two
`load_topic_tags_for_topics` helpers were added for it in `24c41d87bc` without a
caller and were removed as dead code; the batch needs a page-level read in the
projection source that hands the resolved tags to `project_topic`, and a reindex
round-trip count as its evidence. Open.

The `forum-search-*.json` contracts in this directory state an execution-pending
status of their own - commonly `source_complete_execution_pending`,
`source_complete_runtime_evidence_pending` or
`source_ready_maintainer_execution_pending` - and none of them claims completion.

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
properties or preview capabilities are disabled. Page Builder stays optional;
forum routes must not depend on provider availability.

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

The counter, solution, subscription and mention reports share one
`ReconciliationExecutor` (`services/reconciliation_executor.rs`): it owns the
operator fence (`ForumCategories::Manage` and `ForumTopics::Manage`), the
entry-point, span and error metrics, and the page bound. Each report declares a
single `EXECUTOR` constant with its bounds and its three metric labels and keeps
no copy of those rules. The labels are explicit because they are three published
series: the entry-point counter uses `<report>_reconciliation_report`, the module
error counter uses `<report>_reconciliation`, and the span metrics use
`forum.<report>_reconciliation_report`. The first executor revision reused the
span operation for all three, which renamed two series per report; the labels are
back to their published values. `scripts/verify/lib/forum-reconciliation-executor.mjs`
states this contract for the three reconciliation source guards, and
`forum-reconciliation-executor.test.mjs` next to it mutates the live sources one
rule at a time. The attachment-hold report still carries its own fence, metrics
and clamp and has not moved onto the executor.

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

## `FORUM-16` — read tracking and unread state

**Status:** `in_progress`  
**Priority:** P1  
**Dependencies:** FORUM-07/10

### Scope

Persist monotonic per-user topic read position/revision. Expose unread count,
last-read position and unread filters on bounded topic projections. Add mark
topic/category/all read commands.

Use `GREATEST(existing, incoming)` or equivalent compare-and-set semantics.
Anonymous page views do not create read rows. Cache and realtime updates may
accelerate the badge but database position/revision remains canonical.

### Delivered in `FORUM-16A`

- PostgreSQL and SQLite add `forum_topic_read_states` keyed by tenant, topic and
  user, with a tenant-composite topic foreign key, nonnegative high-water marks
  and database triggers that reject direct position or revision regression;
- `ForumTopicReadStateService` exposes authenticated owner get/mark operations,
  while anonymous reads return an implicit zero state and anonymous marks fail
  without creating persistence;
- mark validates the incoming position against the latest approved reply and the
  incoming revision against the latest topic revision before independently
  advancing both high-water marks through insert-or-no-op and conditional owner
  updates in one transaction;
- stale devices become no-ops instead of moving either cursor backwards, and a
  SQLite owner scenario covers future-bound rejection plus the direct database
  regression guard.

### Delivered in `FORUM-16B`

- `ForumReadModelService::list_topics_with_unread` exposes a separate
  authenticated owner projection without changing the existing public topic
  list contract;
- each bounded cursor page is enriched through one aggregate owner query with
  explicit read-state presence, last-read position/revision, approved-reply
  unread count, topic-revision unread state and a canonical `is_unread` result;
- a missing read-state row keeps a newly discovered topic unread even when it has
  no replies, while an explicit zero row records that an empty topic was opened;
- `unread_only` is applied before cursor pagination and includes unseen topics,
  approved replies after the high-water mark and newer immutable topic revisions;
- hidden, rejected and deleted replies are excluded by the approved-public
  predicate, and the SQLite scenario covers visibility changes plus two-page
  unread cursor behavior without N+1 reply reads.

### Delivered in `FORUM-16C`

- `ForumTopicReadStateService::mark_category_read` marks a root category and its
  current descendant subtree, while `mark_all_read` covers the tenant scope;
- each command processes at most 100 topics in one transaction and returns an
  exact `(snapshot_at, created_at, topic_id)` continuation cursor bound to the
  tenant, authenticated user and command scope;
- the first page fixes the topic-creation snapshot, so topics created later are
  excluded from that operation and picked up by a subsequent idempotent pass;
- category traversal reuses the existing 512-node owner bound and fails closed
  for an oversized, missing or cyclic tenant tree;
- one aggregate query per bounded page resolves the latest approved-reply
  position and latest immutable topic revision before monotonic owner upserts;
- equal non-stale high-water marks refresh the read snapshot so a late moderator
  approval below an already-read position can be acknowledged, while stale lower
  markers cannot regress state or refresh the snapshot;
- SQLite owner scenarios cover subtree pagination, snapshot exclusion, cursor
  scope isolation, tenant-wide replay, bounds, anonymous rejection and late
  approval acknowledgement.

### Delivered in `FORUM-16D`

- additive REST routes expose the authenticated unread cursor page, topic
  read-state get/mark, category-subtree mark and tenant-wide mark-all commands;
- additive GraphQL query and mutation fields expose the same owner contracts
  with tenant-scope rejection, permission checks and opaque cursor passthrough;
- both transports call `ForumReadModelService` and
  `ForumTopicReadStateService` directly, so unread calculation, monotonic
  updates, bounds and cursor validation remain owner policy;
- the legacy offset-based topic endpoints remain unchanged, and the unread/read
  state contracts do not open anonymous or realtime paths;
- OpenAPI and GraphQL SDL contract coverage locks the route and field names plus
  the shared request and response shapes.

### Delivered in `FORUM-16E`

- `ForumReadModelService::summarize_topic_ids` reuses the canonical unread
  aggregate for an authenticated caller-supplied set capped at 100 exact topic
  IDs, so storefront composition does not duplicate reply/revision policy;
- `TopicService::get_storefront_visible_with_locale_fallback` centralizes the
  single-topic open/channel visibility check used before a storefront mutation;
- authenticated GraphQL and native server-function adapters enrich only the
  topic IDs already selected by the storefront-visible owner list, while the
  public category/topic/reply feed remains the anonymous compatibility path;
- authentication or permission absence degrades explicitly to the public feed,
  but network, HTTP, persistence and domain failures remain visible instead of
  becoming synthetic anonymous state;
- the selected visible topic can be marked at the current approved-reply and
  immutable-revision high-water marks after visibility is rechecked, and content
  published after that owner snapshot remains unread;
- the module-owned storefront renders unread badges and a topic mark-read control
  without creating anonymous rows; GraphQL SDL and SQLite owner-composition tests
  lock the visible-topic-only contract and keep storefront bulk mutations closed.

### Delivered in `FORUM-16F`

- `topic_read_state_postgres` is source-ready in the isolated PostgreSQL Forum
  fixture without changing production runtime APIs, migrations or transports;
- two independent PostgreSQL connections mark the same tenant/user/topic with
  separate reply-position and topic-revision advances, requiring durable state
  to converge to the component-wise maximum and the database regression trigger
  to reject a later direct downgrade;
- a production-sized fixture creates 128 topics, 8,192 approved replies and 512
  topic revisions, then validates the canonical owner summary across a bounded
  100-topic page containing reply-unread, revision-unread, read and unseen rows;
- a natural `EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, FORMAT JSON)` proof requires
  exactly 100 aggregate rows, no per-topic `SubPlan` and all four owner relations;
- a separate `enable_seqscan = off` plan proves index capability for read-state,
  reply-position and topic-revision access without claiming a latency threshold
  or requiring the production planner to choose one fixed plan;
- the machine contract, source verifier and proof record remain explicitly
  source-ready; no successful PostgreSQL execution or captured plan output is
  claimed until the maintainer runs `topic_read_state_postgres`.

### Compatibility and degraded mode

No migration or backfill is required: an absent row means position/revision zero
and preserves unseen-topic identity. Existing public, offset-based and storefront
topic reads remain source-compatible; storefront DTOs add only defaulted optional
unread fields. Authenticated requests enrich the existing visibility-filtered
page, while anonymous requests retain the public feed and never create read rows.
Authentication/permission absence is the only personalization fallback; real
transport or owner failures remain explicit. REST and GraphQL pass opaque cursors
and owner DTOs without duplicating unread policy. Category membership is
revalidated on every resumed owner bulk page; a subsequent idempotent pass
converges after a concurrent category move. Storefront category/all-read controls
remain closed because the current owner bulk scope is tenant/category based and
cannot yet narrow to the channel-visible topic set. FORUM-16F adds only
source-ready tests, contracts and documentation; it changes no persistence or
degraded-mode behavior. Cache and realtime accelerators remain optional and
never replace database owner state.

### Remaining scope

- add visibility/channel-scoped category and all-read storefront commands only
  after the shared ACL/visibility policy can produce an exact bounded owner scope;
- record successful maintainer PostgreSQL execution and capture the natural and
  index-capability `EXPLAIN JSON` output for the source-ready proof.

### Definition of done

Concurrent devices cannot move read state backwards, deleted/hidden replies do
not inflate unread counts, and category/all-read commands are resumable and
bounded.

### Verification

```bash
cargo test -p rustok-forum --test topic_read_state_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_unread_projection_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_bulk_read_state_sqlite -- --nocapture
cargo test -p rustok-forum --test read_state_transport_contract -- --nocapture
cargo test -p rustok-forum --test storefront_read_state_contract -- --nocapture
cargo test -p rustok-forum --test storefront_read_state_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_read_state_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum-storefront
node scripts/verify/verify-forum-read-state-runtime-proof.mjs
cargo xtask module validate forum
npm run verify:forum:admin-boundary
npm run verify:forum:storefront-boundary
```

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

### Delivered in `FORUM-20A`

- `ForumTopicVisibilityScope` and `ForumTopicVisibilityService` centralize the
  current storefront topic rule: an open topic is visible when it has no channel
  restriction or when the exact normalized request channel is present;
- exact caller-selected topic IDs are bounded to 100 before deduplication,
  filtered in one tenant-scoped owner query and returned in first-occurrence
  order without discovering additional topics;
- missing, foreign-tenant, closed and nonmatching-channel targets all resolve as
  absent, so the exact evaluator does not expose an existence oracle;
- the single-topic storefront facade preserves its existing owner read/RBAC
  check before visibility evaluation, while the storefront list rechecks the
  exact selected page and fails closed if the compatibility SQL prefilter drifts
  from the owner scope;
- storefront unread composition continues through the guarded topic facade, so
  it cannot enrich or mark a topic outside the exact current scope;
- `forum-topic-visibility-scope.json`, `topic_visibility_sqlite` and
  `verify-forum-topic-visibility-scope.mjs` lock bounds, tenant isolation,
  order-preserving deduplication, public/exact-channel behavior and the residual
  full-ACL scope;
- this slice adds no migration, transport field, endpoint or UI and does not
  claim category inheritance, roles, groups, trust or explicit allow/deny.

### Delivered in `FORUM-20B`

- `ForumCategoryVisibility` defines the typed effective category audience floor
  currently supported by Forum: `public` and `authenticated`;
- `forum_category_policies.visibility_override` is nullable and stores only the
  narrowing `authenticated` override; PostgreSQL and SQLite reject an explicit
  `public` row, so a descendant cannot broaden an authenticated ancestor through
  direct persistence;
- `ForumCategoryVisibilityPolicyService` resolves the complete tenant category
  hierarchy within the existing 512-node and depth-16 bounds, reports the exact
  category that supplies an inherited authenticated floor, and fails closed for
  missing, foreign, cyclic or over-bound trees;
- the owner `set` command is guarded by `forum_categories:manage`; requesting
  `public` clears a local override only beneath an effective public parent, while
  requesting `authenticated` narrows the category and all descendants;
- the existing `allows_topics` policy shares the row without overwriting the new
  visibility field, and existing categories require no backfill because absence
  remains the public root default;
- `forum-category-visibility-policy.json`,
  `category_visibility_policy_sqlite` and
  `verify-forum-category-visibility-policy.mjs` lock inheritance, tenant
  isolation, monotonic narrowing and database enforcement;
- the static audit also repairs the already-merged FORUM-20A storefront drift
  error to construct the typed `rustok_core::Error` required by
  `ForumError::Internal`;
- this slice adds no transport, storefront, topic/reply read composition, role,
  trust, group or explicit allow/deny behavior.

### Delivered in `FORUM-20C`

- storefront topic exact and paginated reads compose the inherited category
  floor with the existing open/exact-channel policy through the Forum owner;
- public viewers see public categories only, while authenticated viewers may see
  both public and inherited authenticated categories without page/count drift;
- category exclusions are applied before storefront count and pagination and the
  exact selected page is rechecked by the bounded visibility evaluator;
- `topic_authenticated_visibility_sqlite` and the cumulative storefront contract
  preserve non-oracular missing/denied behavior and tenant-scoped channel checks.

### Delivered in `FORUM-20D`

- exact topic reads authorize, evaluate the inherited category floor and only
  then hydrate localized content; denied public targets resolve as
  `TopicNotFound`;
- ordinary topic pages exclude inherited authenticated categories before count
  and pagination, while authenticated owner reads preserve existing behavior;
- exact reply reads evaluate their parent topic before body/vote enrichment and
  denied public targets resolve as `ReplyNotFound`;
- reply pages reject a denied parent as `TopicNotFound` before reply count and
  pagination;
- `forum-owner-read-visibility.json`,
  `topic_reply_owner_visibility_sqlite` and
  `verify-forum-owner-read-visibility.mjs` lock the owner-read order and residual
  scope.

### Delivered in `FORUM-20E`

- exact category reads authorize and evaluate the inherited category floor
  before localization/subscription hydration; denied public targets resolve as
  `CategoryNotFound`;
- flat category pages exclude inherited authenticated categories before count
  and pagination;
- the bounded canonical category tree removes authenticated subtrees for public
  viewers and recomputes visible `total_nodes`, `max_depth`, `children_count` and
  `has_children` metadata;
- monotonic inheritance guarantees that pruning a hidden node cannot orphan a
  visible descendant, while authenticated tree reads retain the complete bounded
  hierarchy;
- `forum-category-owner-read-visibility.json`,
  `category_owner_visibility_sqlite` and
  `verify-forum-category-owner-read-visibility.mjs` lock exact, page and tree
  composition.

### Delivered in `FORUM-20F`

- `ForumAudienceConstraints` models additional role, minimum-trust,
  channel-member, group-member and explicit user allow/deny selectors after the
  inherited public/authenticated floor; positive selectors form a union and
  explicit deny always wins;
- raw input is capped before normalization at four roles, 32 channel candidates,
  32 group candidates and 100 allow plus 100 deny user IDs; trust is bounded to
  0..100 and channel slugs to 128 characters;
- `ForumAudienceFactsRequest` and `ForumAudienceFacts` form an exact tenant- and
  actor-scoped request/response pair, rejecting cross-identity or unrequested
  trust, channel or group facts;
- `ForumAudienceFactsPort` is a transport-neutral optional owner boundary with
  mandatory read-deadline semantics and no direct Forum dependency on Channel,
  Groups or their private tables;
- `ForumAudienceFactsResolver` binds the port context to the requested tenant and
  user, skips the optional provider when local deny/allow/role facts already
  decide the union, returns empty facts for public or non-user actors, and
  otherwise fails closed with typed capability errors;
- `ForumAudienceEvaluator` validates exact facts and returns an explainable
  decision reason for unrestricted, deny, allow, role, trust, channel, group,
  authentication-required or no-match outcomes;
- `forum-audience-capability-ports.json`, `audience_capability_contract` and
  `verify-forum-audience-capability-ports.mjs` lock bounds, precedence, exact
  subsets, identity/deadline semantics and residual composition work;
- this slice adds no storage, migration, owner-read composition, write policy,
  transport, provider adapter or cross-consumer behavior.

### Delivered in `FORUM-20G`

- five normalized Forum-owned tables persist one optional local category layer
  plus typed role, channel, group and explicit allow/deny relations without JSON;
- every policy and relation is tenant/category scoped through composite foreign
  keys, with typed checks for supported roles, trust `0..100`, canonical channel
  slugs, non-nil identities and allow/deny effects;
- direct channel/group/user inserts are bounded to the FORUM-20F limits and
  policy and relation updates are rejected; owner replacement uses delete plus
  insert;
- `ForumCategoryAudiencePolicyService` exposes managed policy inspection and
  atomically replaces a local layer under `forum_categories:manage` and the
  tenant category-tree lock;
- an empty constraint set deletes only the local layer and restores inheritance;
- effective policy is an ordered root-to-target conjunction of every non-empty
  local layer, preserving the local union/deny semantics while making child
  broadening impossible;
- storage reads remain bounded to the 512-node/depth-16 hierarchy and fail closed
  for cycles, foreign categories, orphan relations, empty stored layers or
  oversized direct writes;
- `forum-category-audience-policy.json`, `category_audience_policy_sqlite` and
  `verify-forum-category-audience-policy.mjs` lock normalized persistence,
  inheritance, atomic replacement, tenant isolation and direct database guards;
- this slice adds no topic narrowing, category/topic/reply read composition,
  provider adapter, write-audience transport or cross-consumer behavior.

### Delivered in `FORUM-20H` through `FORUM-20Q`

- `FORUM-20H` adds normalized topic-local audience narrowing and owner replacement
  commands that compose after the inherited category layers and cannot broaden them;
- `FORUM-20I` migrates Forum notification description, audience, and target-open checks
  to the canonical topic visibility owner instead of notification-local predicates;
- `FORUM-20J` composes exact richer topic visibility from the route/channel rule,
  inherited category layers, optional topic narrowing, and bounded owner facts;
- `FORUM-20K` applies that richer exact decision to Forum notification source reads and
  reauthorization while retaining public fail-closed degraded behavior;
- `FORUM-20L` and `FORUM-20M` define and publish the exact recipient-context capability,
  backed by authoritative Users/RBAC owners and preserving deadline and trace context;
- `FORUM-20N` authorizes each notification target for the exact current recipient;
- `FORUM-20O` re-resolves mentioned recipients before descriptor/audience publication;
- `FORUM-20P` filters topic-subscription audience pages by current recipient visibility
  while preserving sparse advancing cursors;
- `FORUM-20Q` publishes the Groups-owned exact-membership facts adapter. Trust and
  channel-membership adapters were delivered later through FORUM-26 and FORUM-20AT.

### Delivered in `FORUM-20R` through `FORUM-20AF`

- `FORUM-20R/20S` add non-oracular exact inbox open authorization with current recipient
  privacy before source target authorization;
- `FORUM-20T` through `FORUM-20AA` add bounded authorized listing, exact state commands,
  reconciliation, mark-unread, exact unread count, and resumable mark-all read/unread/archive;
- `FORUM-20AB` adds bounded explicit selected-ID state commands;
- `FORUM-20AC` adds authorized exact-group listing over opaque group keys;
- `FORUM-20AD` makes group keys durable through a bounded owner migration and backfill;
- `FORUM-20AE` adds bounded group summaries with exact stored counts and an authorized
  latest-item projection;
- `FORUM-20AF` adds bounded exact-group mark-read, mark-unread, and archive commands.

### Delivered in `FORUM-20AG` through `FORUM-20AL`

- `FORUM-20AG` exposes a transport-neutral authenticated-user storefront inbox port with
  context-derived tenant/recipient identity and read/write admission semantics;
- `FORUM-20AH` adds native Leptos server-function adapters without owner identity fields;
- `FORUM-20AI` replaces the storefront placeholder with bounded grouped inbox rendering,
  paging, stale-response guards, allowed-only navigation, and authoritative refreshes;
- `FORUM-20AJ` mounts the localized Notifications action and exact unread badge through a
  generic manifest-driven storefront header slot;
- `FORUM-20AK` adds GraphQL parity for unread count and grouped summary/item reads while
  reusing the owner storefront port and host-composed policy/source runtime;
- `FORUM-20AL` adds GraphQL parity for fresh open authorization. Missing, foreign,
  suppressed, and no-longer-openable rows remain indistinguishable `UNAVAILABLE`, and
  only an owner-authorized route can produce `ALLOWED`.

### Delivered in `FORUM-20AM`

- synchronize this canonical ledger, the Notifications owner-local plan, owner README,
  and live contract through `FORUM-20AL` without rewriting unrelated task content;
- record the historical `FORUM-20H` through `FORUM-20AL` execution chain as source-ready
  and unvalidated rather than promoting the overall task to `done`;
- add `forum-notification-plan-sync.json` and
  `verify-forum-notification-plan-sync.mjs` so future owner/storefront work cannot silently
  leave the four authoritative documents at different milestones;
- update the latest `FORUM-20AL` handoff contract to point at this synchronization task.

### Delivered in `FORUM-20AN`

- add the module-owned GraphQL mutation `notificationInboxApplyGroupState` for bounded
  exact-group `MARK_READ`, `MARK_UNREAD`, and `ARCHIVE` commands;
- derive tenant and recipient identity from the authenticated human-user context, require
  module admission before command validation, and carry a five-second deadline plus one
  bounded caller idempotency key through `NotificationInboxStorefrontPort`;
- return only `scanned`, `changed`, `next_cursor`, and `has_more`, preserving owner state,
  timestamp, terminal-archive, pagination, and non-oracular invariants;
- select native writes for SSR/hydrate and GraphQL writes for CSR/headless without fallback,
  while keeping the existing UI call site and authoritative post-command refresh behavior.

### Delivered in `FORUM-20AO`

- expose one current storefront transport-context resolver that reads the reactive auth
  session token and tenant signals without placing owner identity in request DTOs;
- key the grouped bootstrap resource by both the existing manual refresh nonce and the
  current transport context, so sign-in, sign-out, token refresh, and tenant changes refetch;
- pass one resolved context snapshot through exact unread-count and first bounded summary-page
  reads instead of re-resolving credentials between owner calls;
- clear prior mutation feedback when the auth scope changes while preserving explicit
  post-command refresh, compile-profile transport selection, and no-fallback behavior.

### Delivered in `FORUM-20AP`

- materialize `forum.topic.created` descriptors for active topics that are already non-public
  when the host publishes the exact notification recipient-context capability;
- keep public-only descriptor creation when that capability is absent, preserving the existing
  fail-closed optional-module profile;
- expose only topic/category identifiers in the descriptor and defer all recipient authority to
  the bounded subscription audience recheck;
- preserve author exclusion, raw subscription cursor progress, current category/topic audience
  evaluation, non-oracular inactive targets, and later target-open authorization.

### Delivered in `FORUM-20AQ`

- add five normalized Forum-owned tables for a category-local topic-create audience layer plus
  typed role, channel, group, and explicit allow/deny relations;
- keep topic-create policy separate from category/topic visibility while inheriting every
  non-empty root-to-category layer as a conjunction;
- expose managed inspection and atomic replacement under `forum_categories:manage`, with an
  empty constraint set clearing only the local layer and restoring inheritance;
- enforce tenant/category composite ownership, raw relation bounds, immutable stored rows, and
  PostgreSQL/SQLite parity without changing `TopicService::create` or any transport.

### Delivered in `FORUM-20AR`

- enforce `forum_topics:create` before loading the bounded inherited category topic-create
  policy and require every root-to-category layer to allow the caller;
- keep unrestricted categories and locally decidable role/explicit-user layers independent of
  optional owner facts while preserving explicit-deny precedence;
- require exact tenant/user `PortContext` plus the optional facts capability only when trust,
  Channel, or Groups selectors remain unresolved, and fail closed when either is absent;
- gate every public `TopicService` create path before topic, relation, counter, user-stat, or
  event writes and publish context-aware owner seams without changing GraphQL or REST DTOs.

### Delivered in `FORUM-20AS`

- compose both legacy and inline-quote GraphQL topic-create mutations through one manifest-backed
  runtime wrapper and the existing context-aware owner methods;
- compose both REST topic-create handlers through `HostRuntimeContext`, using only authenticated
  tenant/user identity plus the middleware-resolved locale and route channel;
- attach read deadline, permission claims, and a bounded correlation id before any optional owner
  facts call, rejecting mismatched request tenant or actor before provider access;
- consume the existing feature-guarded Groups facts publication for both transports while keeping
  provider absence fail closed and adding no topic-create DTO, migration, or Forum-to-Groups dependency.

### Delivered in `FORUM-20AT`

- publish one host-owned composite `SharedForumAudienceFactsPort` for every Forum build while
  preserving the historical `FORUM-20Q` Groups adapter as an optional owner-backed fallback;
- accept a Channel match only from the exact normalized requested `PortContext.channel`, then
  confirm that slug through `ChannelReadPort` as active and tenant-matching;
- never list or probe unrequested channels, and let an exact current-Channel match decide the
  positive-selector union before optional Groups facts are consulted;
- keep Channel-only topic-create policy operational when Groups is not compiled, while requested
  Groups facts remain typed retryable unavailable when no delivered selector decides;
- add inline host tests, a machine-readable contract and a static verifier without migrations,
  public DTO changes, or a new Forum-to-Channel dependency.

### Delivered in `FORUM-20AU`

- add five normalized Forum-owned tables for a category-local reply-create audience layer plus
  typed role, channel, group, and explicit allow/deny relations;
- keep reply-create policy separate from category/topic visibility and topic-create eligibility
  while inheriting every non-empty root-to-category layer as a conjunction;
- expose managed inspection and atomic replacement under `forum_categories:manage`, with an
  empty constraint set clearing only the local layer and restoring inheritance;
- enforce tenant/category composite ownership, typed trust/role/effect values, raw relation
  bounds, immutable stored rows, and PostgreSQL/SQLite parity;
- leave `ReplyService::create`, REST, GraphQL and transport DTOs unchanged; command-time
  authorization remains a separate follow-up slice.

### Delivered in `FORUM-20AV`

- require `forum_replies:create`, resolve the exact tenant-scoped topic category, and evaluate
  every inherited category reply-create layer before reply-body preparation or owner writes;
- preserve explicit-deny precedence and locally decidable role/explicit-user decisions without
  optional facts calls;
- require exact tenant/user context and the host audience-facts capability only for unresolved
  trust, Channel, or Groups selectors;
- route both public reply create facades through the owner gate before reply, relation, counter,
  user-stat, journal, or event writes without changing DTOs.

### Delivered in `FORUM-20AW`

- compose both legacy and inline-quote GraphQL reply-create mutations through exact authenticated
  read-only `PortContext` values;
- compose both REST reply-create handlers from tenant, auth and middleware request context;
- forward claims, locale, resolved route channel, five-second deadline and bounded correlation
  identity without accepting owner identity from request DTOs;
- reuse the same optional host-published `SharedForumAudienceFactsPort` already used by topic
  creation, preserving fail-closed unresolved external selectors.

### Delivered in `FORUM-20AX`

- persist an optional normalized topic-local reply-create audience layer independently from topic
  visibility and inherited category reply-create storage;
- expose managed get/replace commands under `forum_topics:manage`, with empty constraints clearing
  only the topic layer;
- evaluate every category layer followed by the optional topic layer so topics may narrow but
  never broaden inherited eligibility;
- preserve existing owner and transport paths while PostgreSQL/SQLite enforce tenant ownership,
  typed values, immutable rows and bounded direct relations.

### Delivered in `FORUM-20AY`

- persist normalized inherited category moderation audience layers independently from visibility,
  topic-create and reply-create policy;
- expose managed get/replace commands and evaluate every inherited layer before topic/reply
  moderation transactions;
- preserve the exact topic-author solution path while requiring non-author moderators to hold
  scope and satisfy the moderation audience;
- publish context-aware moderation owner methods and PostgreSQL/SQLite ownership, immutability,
  bounds and advisory-lock guards.

### Delivered in `FORUM-20AZ`

- compose existing GraphQL and REST mark/clear-solution routes through one exact authenticated
  moderation context and the same host-published audience-facts port;
- derive tenant and actor only from authenticated transport context and forward claims, locale,
  route channel, five-second deadline and bounded correlation identity;
- preserve owner-side topic-author versus moderator authorization and keep the gate before
  solution, counter, user-stat, journal and outbox writes;
- add no new moderation endpoint; owner methods without an existing public route remain
  transport-neutral and future routes must reuse the context-aware boundary.

### Delivered in `FORUM-20BA`

- synchronize this canonical ledger through `FORUM-20AZ` and remove reply-create enforcement,
  topic-local narrowing, category moderation audience, and existing solution-route composition
  from remaining scope;
- correct historical owner-note text that still described delivered Forum trust facts as
  unavailable or blocked while preserving that AV-AZ did not create trust state themselves;
- add `forum-audience-plan-sync.json`, `forum-20ba-audience-plan-sync.md`, and
  `verify-forum-audience-plan-sync.mjs` to lock the ledger, five owner notes, five contracts,
  CRATE_API boundary and latest AZ handoff together;
- change no runtime behavior, migration, dependency or public contract.

### Compatibility and degraded mode

The nullable public/authenticated category floor and normalized category/topic layers require no
destructive backfill. Existing content and categories/topics without richer create or moderation
layers retain their previous behavior. Locally decidable role and explicit-user decisions do not
require optional facts providers. Exact trust, Channel, or Groups selectors use the shared
host-published facts capability and fail closed only when a still-required owner fact is absent.
Forum trust is authoritative and host-composed through `ForumUserTrustAudienceFactsPort`;
`forum_user_stats` activity counters are never treated as trust.

FORUM-20AV-AZ add no create/moderation DTO identity fields. GraphQL and REST derive tenant and
actor from authenticated context, and topic/reply/moderation owners validate exact context before
target lookup or mutation. Categories without reply-create layers and topics without local
narrowing preserve historical reply behavior. Existing solution routes are context-composed;
no new approve/reject/hide or pin/lock/status transport is claimed. Context-free owner methods
remain available for direct locally decidable consumers, while future public moderation routes
must use context-aware methods. FORUM-20BA is documentation-only.

Forum producer commands remain independent from Notifications availability. The Notifications
module stays default-off, derives storefront tenant and recipient identity from authenticated
context, and exposes explicit unavailable/degraded states rather than shadow inbox data. Native
and GraphQL storefront reads and writes select exactly one compile-profile transport path with no
cross-path fallback. Both group-state paths delegate to the same owner port; the GraphQL path
cannot select another tenant or recipient and must supply write deadline and idempotency semantics.

### Remaining scope

- migrate remaining Forum reads plus search/index, SEO, and deep-link authorization to the same
  exact richer audience decision;
- add visibility-scoped category/all-read commands over an exact bounded policy scope;
- require any future public moderation transports to reuse the delivered context-aware owner
  methods instead of implementing transport-local policy;
- add tenant-wide scheduled reconciliation, payload redaction, channel enqueue/transports, and
  delivery-time target authorization;
- add PostgreSQL concurrency, lease/contention, inheritance, and cross-consumer runtime evidence
  before promoting `FORUM-20` to `done`.

### Definition of done

Cross-tenant, blocked, private and channel-restricted content is consistently
absent from reads, search, SEO and notifications, including replay and cache
profiles.

### Verification

```bash
cargo test -p rustok-forum --test topic_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test category_visibility_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_authenticated_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_reply_owner_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test category_owner_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test audience_capability_contract -- --nocapture
cargo test -p rustok-forum --test category_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test category_reply_create_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_reply_create_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test moderation_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum reply_create_transport -- --nocapture
cargo test -p rustok-forum graphql::runtime_data -- --nocapture
cargo test -p rustok-forum moderation_transport -- --nocapture
node scripts/verify/verify-forum-topic-visibility-scope.mjs
node scripts/verify/verify-forum-category-visibility-policy.mjs
node scripts/verify/verify-forum-owner-read-visibility.mjs
node scripts/verify/verify-forum-category-owner-read-visibility.mjs
node scripts/verify/verify-forum-audience-capability-ports.mjs
node scripts/verify/verify-forum-category-audience-policy.mjs
node scripts/verify/verify-forum-topic-audience-policy.mjs
node scripts/verify/verify-forum-topic-audience-visibility.mjs
node scripts/verify/verify-forum-notification-visibility-composition.mjs
node scripts/verify/verify-forum-notification-recipient-context.mjs
node scripts/verify/verify-forum-notification-recipient-host-runtime.mjs
node scripts/verify/verify-forum-notification-recipient-target-open.mjs
node scripts/verify/verify-forum-notification-recipient-mention-audience.mjs
node scripts/verify/verify-forum-notification-topic-subscription-audience.mjs
node scripts/verify/verify-forum-audience-group-facts-host-runtime.mjs
cargo test -p rustok-server --features mod-forum forum_audience_facts -- --nocapture
cargo test -p rustok-server --features mod-forum,mod-groups forum_audience_facts -- --nocapture
node scripts/verify/verify-forum-audience-channel-facts-host-runtime.mjs
node scripts/verify/verify-forum-notification-inbox-storefront-port.mjs
node scripts/verify/verify-forum-notification-inbox-native-storefront-adapter.mjs
node scripts/verify/verify-forum-notification-inbox-grouped-storefront-ui.mjs
node scripts/verify/verify-forum-notification-navigation-badge.mjs
node scripts/verify/verify-forum-notification-inbox-grouped-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-open-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs
node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs
cargo test -p rustok-forum --test topic_create_audience_enforcement_sqlite -- --nocapture
node scripts/verify/verify-forum-topic-create-audience-enforcement.mjs
cargo test -p rustok-forum topic_create_transport -- --nocapture
node scripts/verify/verify-forum-topic-create-audience-transport-composition.mjs
node scripts/verify/verify-forum-category-reply-create-audience-policy.mjs
node scripts/verify/verify-forum-reply-create-audience-enforcement.mjs
node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs
node scripts/verify/verify-forum-topic-reply-create-audience-policy.mjs
node scripts/verify/verify-forum-moderation-audience-policy.mjs
node scripts/verify/verify-forum-moderation-audience-transport-composition.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
node scripts/verify/verify-forum-audience-plan-sync.mjs
cargo xtask module validate forum
npm run verify:forum:storefront-boundary
```

Tests, Cargo, verifiers and CI were not run while publishing the source-ready
FORUM-20C-AZ implementation slices or the FORUM-20BA documentation synchronization.
`FORUM-20` remains `in_progress` until the remaining owner and cross-consumer paths are
delivered with maintainer runtime evidence.

## `FORUM-26` — anti-spam, limits and trust levels

**Status:** `in_progress`  
**Priority:** P0  
**Dependencies:** FORUM-19, shared rate-limit capability

### Scope

Implement forum-local trust levels and explainable posting policy based on
account age, reading/activity, approved posts, flags, reputation and moderation
history. Bound topics/day, replies/minute, links, mentions, attachments, edits
and bump intervals. Add duplicate-content hashing and optional external/AI spam
scoring.

External/AI scoring is optional and cannot be a synchronous correctness
dependency. Forum owns policy; shared rate limiting owns distributed execution.

### Delivered in `FORUM-26A` through `FORUM-26J`

- Forum-owned tenant/user trust state, typed trust levels, managed writes and an
  authoritative exact-user trust facts adapter replace activity-counter or role
  inference;
- bounded posting-action, candidate-metric, rule, outcome, evidence and decision
  contracts define explainable local policy evaluation without executing owner
  writes or distributed reservations;
- the facts composer keeps missing owner capabilities explicit and preserves
  exact tenant/user actor identity, deadlines, typed retryability and one unique
  provider per fact kind;
- authoritative server/users account age, Forum topic-reading activity and
  current retained approved-post facts are published without copying owner data
  into policy configuration;
- `FORUM-26I` adds PostgreSQL/SQLite partial author indexes aligned to the exact
  approved-post query, source-ready `EXPLAIN` proofs, index-definition guards and
  the minimal platform-user fixture required by the isolated PostgreSQL Forum
  migration bootstrap;
- `FORUM-26J` publishes authoritative exact-user topic/reply create-window facts
  over all persisted owner rows, keeps soft-deleted and moderated attempts in the
  observed budget, composes both providers in the host, and adds PostgreSQL/
  SQLite author-time indexes plus source-ready `EXPLAIN` proof;
- none of these slices invokes the composer from topic/reply/edit/bump commands,
  reserves shared rate-limit capacity, hashes duplicate content, calls external
  scoring or automatically changes trust state.

### Compatibility and degraded mode

Trust rows, approved-post indexes and create-window indexes are additive.
Existing users without a trust row retain the documented compatibility level;
existing topic/reply rows are indexed automatically with no backfill or owner-
state rewrite. The approved-post query and fact value remain unchanged by its
index migration. Create-window facts count every persisted owner create inside
the exact inclusive observation window, including rows later soft-deleted and
replies in any moderation state, so ordinary deletion or moderation cannot reset
the budget. A hard retention purge removes historical rows and must remain later
than any configured production window unless a durable usage ledger replaces the
snapshot. Missing fact owners continue to return explicit unavailable results,
and optional external or AI scoring can never become a synchronous correctness
dependency. These facts are not a concurrency-safe reservation; rolling back the
index migrations changes only query performance, not fact semantics.

### Remaining scope

- publish authoritative active-flag and moderation-history facts from the
  moderation/report owner without inferring them from reply-status totals;
- publish authoritative reputation facts from the reputation owner and ledger;
- publish edit-window and topic-target bump-age facts only after exact editing
  actor and target identity are represented in the owner history and fact request;
- persist and administer bounded policy configuration with versioned audit;
- enforce the composed decision in topic, reply, edit and bump owner commands;
- reserve, commit and release distributed rate-limit capacity through the shared
  capability without making it Forum persistence;
- add retained duplicate-content fingerprints and optional external/AI scoring;
- add automatic trust promotion/demotion only through explicit explainable owner
  commands and immutable evidence;
- expose bounded admin/storefront/transport surfaces and capture maintainer-
  executed PostgreSQL, SQLite and cross-consumer runtime evidence.

### Definition of done

All posting owner paths evaluate one versioned bounded policy before mutation,
missing required facts fail closed with typed retryability, distributed limits
cannot be bypassed through another transport, duplicate/replay behavior is
idempotent, optional scoring failure does not break correctness, and every trust
change and posting denial is explainable and auditable.

### Verification

```bash
cargo test -p rustok-forum posting_policy
cargo test -p rustok-forum posting_policy_approved_facts -- --nocapture
cargo test -p rustok-forum posting_policy_create_window_facts -- --nocapture
cargo test -p rustok-forum --test approved_posts_index_sqlite -- --nocapture
cargo test -p rustok-forum --test approved_posts_index_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test create_window_facts_index_sqlite -- --nocapture
cargo test -p rustok-forum --test create_window_facts_index_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-forum-posting-policy-facts.mjs
node scripts/verify/verify-forum-topic-reading-posting-facts.mjs
node scripts/verify/verify-forum-approved-posts-posting-facts.mjs
node scripts/verify/verify-forum-approved-posts-index-debt.mjs
node scripts/verify/verify-forum-approved-posts-index-hardening.mjs
node scripts/verify/verify-forum-create-window-posting-facts.mjs
cargo xtask module validate forum
```

The FORUM-26J source and contract records do not claim successful runtime
verification until the maintainer runs the commands above.

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

## FORUM-28 Admin Locale-Switch Contract

This section defines the Forum admin dirty locale-switch policy left open by the canonical `FORUM-28` rich-text/admin work:
- The **active locale** owns current reads, writes, category tree labels, topic/reply reads, and rich-text direction/spellcheck.
- The **candidate locale** is only the text typed into the locale control.
- Any dirty localized or owner-editable state blocks the switch; the candidate is reset to the active locale and nothing is discarded.
- The same fallback guard also applies to the **initial editor load**. Opening an owner row directly in a locale that only has fallback content never exposes that fallback copy as writable target-locale form state.
- Missing target translation: blanked fields while structure is preserved; clearing fallback tag labels ensures Forum's update path does not bind wrong locale term identity. Omitting `tags` (`None`) when unchanged will preserve existing attachment rows.
- A topic locale switch is blocked while the reply composer contains an unsaved reply draft.
- Category tree alignment: on the category admin page, the category tree follows the category active locale; on the topic admin page, it follows the topic active locale.

## TAXONOMY-CAT-5 Forum Category Mounted Multilingual/RTL Browser Parity

Status: **executable browser source / maintainer execution pending**

The backend ownership/storage cutover is already complete; this contract defines mounted-browser evidence for the Forum Category → Taxonomy cutover.
The runner `apps/next-admin/tests/forum-category-taxonomy/browser-evidence.spec.ts` reuses `@playwright/test` for browser navigation only, validating RTL locale, root and child canonical Taxonomy slugs, icon/color presentation, and fallback locale behavior without bypassing Forum authorization.

## Shared Platform Integration Status

- Delivered in `NOTIFY-00A`: shared notifications foundation and event delivery integration.

- ### Delivered in FORUM-24B
- record_merge_redirect_aliases_in_tx
- Topic rename aliases and deletion tombstones remain
- FORUM-23B2G2B3D12
- crates/modules/rustok-forum/contracts/forum-search-versioned-invalidation-retained-evidence-promotion.json
- scripts/evidence/review-forum-search-versioned-invalidation-runtime-evidence.mjs
- target/forum-search-versioned-invalidation-runtime-evidence.json
- target/forum-search-versioned-invalidation-runtime-promotion-candidate.json
- all nine source artifacts
- all ten frozen scenarios
- immutable retention reference
- independently supplied SHA-256
- does not contact or authenticate the external retention service
- separate source PR
- No command above was run by the implementation agent
- A later retention/review step may inspect the generated aggregate artifact
- only then decide whether the canonical D0 status can be promoted
- # FORUM-21I/J canonical merged-topic resolution and HTTP redirect
- One source of truth
- at most 32 edges
- FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT
- 308 Permanent Redirect
- Cache-Control: private, no-store
- FORUM-21A through FORUM-21X
- FORUM-21J
- the only source-to-target canonical edge
- Canonical merged-topic resolution and HTTP redirect
- Location: /api/forum/topics/{canonical_topic_id}
- forum_permission_denied
- [FORUM-21I/J canonical resolution and HTTP redirect]
- # FORUM-21R topic split GraphQL transport
- splitForumTopicReplies
- ForumTopicSplitService::split_selected_replies
- immutable owner receipt
- FORUM-21` entry remains `planned`
- forum.topic.split
- # FORUM-21B idempotent topic merge owner
- crates/modules/rustok-forum/contracts/forum-topic-merge-owner.json
- crates/modules/rustok-forum/contracts/forum-topic-merge-admin-ui.json
- Admin composition
- mergeForumTopicResolvingSolution
- /modules/forum/merge
- /dashboard/forum/merge
- pub const FORUM_TOPIC_ROUTE_SHORT_ID_LEN: usize = 12
- # FORUM-21H topic merge accepted-solution policy
- FORUM-21L
- ## Explicit competing-solution resolution
- ## Append-only resolution audit
- schema version 1
- # FORUM-21L competing accepted-solution resolution
- merge_topic_resolving_solution
- forum_topic_merge_solution_resolutions
- Resolution audit ledger
- forum.topic.merged / schema version 1
- FORUM_TOPIC_MERGE_OPERATION_CONFLICT
- [FORUM-21L competing solution resolution]

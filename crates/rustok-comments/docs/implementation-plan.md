# Implementation plan for `rustok-comments`

## Current state

`rustok-comments` owns generic comment threads, comments, localized bodies,
thread status/moderation, and comment-domain observability. It is separate from
forum replies and shared content storage. Blog uses the module on its production
read/write path; page-like surfaces require explicit opt-in.

The admin moderation surface is the documented native-only comments admin exception: it has a
module-owned core, native transport facade, and Leptos adapter backed by
`HostRuntimeContext`. Thread and locale route/query policy is core-owned, and UI
applies the prepared shared `UiRouteQueryIntent`; it does not call raw
transport.

Thread write consistency is owner-enforced below the service facade. Transactional
comment inserts lock the tenant thread row before assigning the next position.
Explicit transactional counter refreshes take the same owner lock and replace a
caller-supplied `comment_count` with the exact number of tenant comments whose
`deleted_at` is null. Status-only and metadata-only thread updates do not activate
a counter write.

First-thread creation has a separate owner identity lock. Before a transactional
thread insert, `comment_thread::ActiveModelBehavior` upserts a persistent
`comment_thread_identity_locks` row with `ON CONFLICT DO NOTHING`, locks that row,
and checks for the canonical thread. A concurrent creator receives an owner-
classified application `DbErr` before its SQL INSERT, leaving the PostgreSQL
transaction usable. `find_or_create_thread_in_tx` performs canonical lookup only
for the exact tenant/target identity marker followed by one valid canonical thread
UUID; malformed or wrong-scope markers are rejected. Unrelated storage errors
propagate through `CommentsError::Database` instead of becoming
`CommentThreadNotFound`.

Append-only migrations repair historical counters, deterministically renumber
historical positions, enforce `UNIQUE(thread_id, position)`, and create the unique
identity-lock key `(tenant_id, target_type, target_id)` for PostgreSQL and SQLite.

The shared `rustok-api::richtext` document contract and
`rustok-content::richtext` `comment` profile are implemented and Comments is
the first owner cut over to them. `CreateCommentInput` and
`UpdateCommentInput` accept only `RichTextDocument`; `CommentRecord` returns
`RichTextView` plus the server-derived plain-text projection. Comment body rows
persist canonical ProseMirror/Tiptap JSON without a format selector, and public
previews use the plain-text projection. Non-canonical rows fail closed; the
repository contains no compatibility reader, dual-write path, or retained
format converter.

Comments moderation preserves each typed server-derived `RichTextView` in its
framework-agnostic row view-model and renders it through the shared
`RichTextHtml` Leptos boundary with the effective content locale. It no longer
collapses moderation content to a plain-text node, does not own a direct HTML
sink, and does not load the editor runtime for read-only moderation.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `CommentsThreadPort` / `comments.thread.v1` in
  `crates/rustok-comments/contracts/comments-fba-registry.json`.
- Comments FBA registry schema v4 locks the exact verify/test package order,
  the provider port-boundary leaf, thread-invariant leaf commands, focused
  self-tests, evidence paths, strict classifier unit harness, and shared owner
  runtime-order gate.
- Provider port source evidence schema v2:
  `crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json`
  with status `source_verified_no_compile`, compile policy `not_run_by_request`,
  runtime status `pending`, source-verified `in_process`, and pending
  `remote_adapter_placeholder`.
- Provider port source gate:
  `scripts/verify/verify-comments-port-boundary.mjs` with focused self-test
  `scripts/verify/verify-comments-port-boundary.test.mjs`. It locks all seven
  operations, shared read/write policy, typed error mapping, owner-selected
  comment richtext, tenant-scoped approved-only public reads, and rejects source
  or runtime promotion of the remote placeholder.
- Exact provider leaf commands: `verify:comments:port-boundary` and
  `test:verify:comments:port-boundary`; both are registered in
  `verify:comments:fba` / `test:verify:comments:fba` before thread invariants.
- Runtime-order evidence:
  `crates/rustok-comments/contracts/evidence/comments-provider-runtime-order-smoke.json`.
  Its executable source ordering remains uncompiled and unexecuted.
- Thread write invariant evidence schema v3:
  `crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json`
  with status `executable_no_run`.
- Thread invariant source gate:
  `scripts/verify/verify-comments-thread-write-invariants.mjs` with focused
  self-test `scripts/verify/verify-comments-thread-write-invariants.test.mjs`.
  It locks the identity-conflict-only fallback, requires a valid canonical thread
  UUID suffix, verifies that unrelated storage errors propagate, and forbids broad
  `Err(_)` fallback or prefix-only classification.
- Classifier unit harness:
  `crates/rustok-comments/src/entities/thread_insert_error_tests.rs`, registered by
  `#[cfg(test)] mod thread_insert_error_tests;` in the entities module. It records
  exact-scope acceptance, malformed-owner rejection, wrong-scope rejection, and
  unrelated `DbErr` preservation as database failure. It is written but not run.
- Exact thread leaf commands: `verify:comments:thread-write-invariants` and
  `test:verify:comments:thread-write-invariants`; both run after the provider leaf
  and before the shared owner runtime-order gate.
- Executable targets:
  `crates/rustok-comments/src/entities/thread_insert_error_tests.rs`,
  `crates/rustok-comments/tests/thread_write_invariants.rs`, and
  `crates/rustok-comments/tests/thread_creation_concurrency.rs`.
- Both concurrent PostgreSQL targets use two independent one-connection pools, an isolated
  schema, and `RUSTOK_COMMENTS_TEST_DATABASE_URL` or PostgreSQL `DATABASE_URL`.
- Public-port create/delete publish `comment.created` and `comment.deleted`
  through `TransactionalEventBus::publish_in_tx`. Blog's idempotent reply-count
  projection is implemented statically under
  `DECISIONS/2026-07-16-comments-blog-event-projection.md`; runtime delivery,
  retry, and recovery evidence remain open.
- The remote adapter remains pending. The Blog consumer now fails its selected
  native or GraphQL write path directly and never retries through another
  transport; remote-provider runtime evidence remains pending.
- `verify-comments-admin-boundary.mjs` and its focused self-test require the
  shared `RichTextHtml` moderation projection and reject a return to plain-text
  rendering while preserving the documented native-only comments admin
  exception.

## 2026-07-30 source continuation audit

The continuation audit at `8db76d1ae6e1bd5dce2314b9a5c11829373fa93d`
confirmed that the owner evidence, current-only verifier, focused negative fixture,
and both Rust concurrency targets already existed. The registry referenced the
evidence and Rust targets, but did not retain the JS self-test or an exact
verification-chain contract; the root package had no named leaf commands and no
Comments FBA test chain. Slice 11 registers those source assets without changing
the owner behavior or promoting `executable_no_run` to executed evidence.

The continuation audit at `6b5cd3f94265ff7ba382ca89916a73065806a0b5`
found that `find_or_create_thread_in_tx` still reloaded the canonical thread after
every insert `DbErr`. Slice 12 adds a tenant/target-scoped owner marker and
classifier, narrows fallback to that exact identity conflict, propagates unrelated
storage errors, upgrades retained evidence to schema v2, and adds focused negative
guards without recording Rust or PostgreSQL execution.

The continuation audit at `48d478dc2e4ef55dc6015ba8038dde59c908990a`
found that the new classifier accepted any custom error beginning with the expected
scope prefix, including a malformed or empty canonical-thread suffix, and had no
Rust unit harness retained by the FBA source gate. Slice 13 requires one valid UUID
suffix, adds and registers `thread_insert_error_tests`, upgrades registry and
evidence to schema v3, and adds focused regressions for prefix-only parsing,
missing test source, and missing test-module registration. No unit test, verifier,
compile, database, workflow, or CI execution is recorded.

The continuation audit at `7082e47699c7ec3c81d786d26fbea8c57800bc1b`
found that the implemented `InProcessCommentsThreadProvider`, all seven operations,
shared policy checks, typed error mapper, typed richtext DTOs, and approved-only
public projection were still represented by a schema-v1 matrix and registry status
`planned_cases_locked`. Slice 14 splits source-verified `in_process` from the
pending remote placeholder, promotes only source metadata, adds a dedicated
fail-closed verifier and focused fixture, upgrades registry schema v4, and registers
exact provider leaf commands. No verifier, self-test, Rust test, compile, database,
workflow, browser, or CI execution is recorded.

## Completed implementation slices

1. Added the transport-neutral `CommentsThreadPort` provider boundary and shared
   read/write `PortCallPolicy` enforcement.
2. Added approved-only public reads, native admin moderation, localized body
   fallback, and transactional create/delete event publication.
3. Replaced unprotected `MAX(position) + 1` allocation with a tenant-thread owner
   lock in comment `ActiveModelBehavior` for transactional inserts.
4. Replaced stale read-modify-write thread counters with an exact active-row count
   under the same tenant-thread owner lock for explicit counter refreshes.
5. Prevented status-only or metadata-only thread updates from becoming counter
   writers.
6. Added PostgreSQL/SQLite repair for stale counters and duplicate positions, then
   enforced `UNIQUE(thread_id, position)`.
7. Added persistent identity-lock storage and transactional first-thread
   serialization around the existing service fallback.
8. Added SQLite invariant coverage and a PostgreSQL create/create followed by
   create/soft-delete harness for an existing thread.
9. Added a separate PostgreSQL `thread_creation_concurrency` target proving that
   two `CommentsService` instances creating the first comments for one target
   return one thread with positions `1/2` and count `2`.
10. Added machine-readable evidence, current-only source/negative verifiers, and
    integrated all thread invariants into the main Comments FBA gate. These
    targets are written but not executed.
11. Registered the thread invariant verifier and focused self-test as a first-class
    Comments FBA source gate, added exact verify/test npm commands, and locked
    registry schema v2 plus aggregate order while preserving runtime execution as
    maintainer-owned.
12. Added an owner-classified first-thread identity marker, narrowed the service
    fallback to that exact tenant/target conflict, propagated every unrelated
    insert `DbErr`, upgraded thread evidence to schema v2, and added focused
    regression guards for the classifier, broad catch, and propagation branch.
13. Hardened the identity classifier from prefix-only matching to exact scope plus
    a valid canonical thread UUID, added and registered a four-case Rust unit
    harness, upgraded registry/evidence schema v3, and retained the harness in the
    existing FBA leaf.
14. Promoted the implemented in-process `CommentsThreadPort` source boundary from
    `planned_cases_locked`, split the pending remote adapter, added a focused
    provider verifier/self-test and exact npm leaf commands, upgraded registry
    schema v4 and provider matrix schema v2, and retained all runtime/fallback
    evidence as pending.
15. Added `rustok-comments-storefront-support`, which owns the reusable Leptos
    composer state, shared `discussion` frame, blank-document validation,
    authentication gate, busy state, and accessible result feedback. Blog now
    composes it with one post-bound action and exposes matching native and
    GraphQL commands. Both commands enforce tenant, permission, module/channel,
    published-status, and channel-visibility policy before Comments writes.

## Open results

1. **Execute thread concurrency evidence.** Run both env-gated PostgreSQL targets
   and retain active-row counts, unique/gap-free positions, status-only
   preservation, one-thread first-create evidence, and migration output.
   **Done when:** runtime evidence confirms the owner locks under real concurrent
   PostgreSQL transactions.

2. **Execute fallback-class runtime evidence.** The strict source classifier and
   pure Rust harness are written but not executed. Run the classifier harness,
   inject or reproduce an unrelated thread insert `DbErr`, and retain evidence that
   it propagates as a database failure rather than becoming
   `CommentThreadNotFound`; also retain the canonical concurrent identity-conflict
   lookup path.
   **Done when:** executed evidence proves exact marker parsing, unrelated storage
   error propagation, and one canonical thread for the expected first-thread race.

3. **Implement and execute the Blog reply-count event projection.** Consume
   `comment.created` and `comment.deleted` idempotently, publish the Blog-owned
   update event in the projection transaction, and prove retry/degraded behavior.

4. **Execute CommentsThreadPort runtime and remote-profile evidence.** The
   `in_process` source profile, seven operations, owner policy calls, typed error
   mapping, typed richtext, and approved-only public read are source-verified. Run
   the provider leaf, shared owner runtime-order verifier, consumer compatibility,
   and real calls; implement and cover the remote adapter separately.
   **Done when:** retained execution proves read/write policy, idempotency,
   deadlines, typed errors, pagination, public visibility, remote parity, and Blog
   compatibility without promoting planned fallback UI.

5. **Extend moderation and opt-in integrations through comment ownership.**
   Add a new commentable surface only with explicit target binding, moderation,
   rich-text, tenant, and observability contracts; do not reuse forum storage.
   **Depends on:** the consuming module's product requirement and public API.
   **Done when:** the new surface has owner-owned storage and transport tests,
   and its opt-in decision is documented.

6. **Keep operational guidance synchronized with thread semantics.** Update
   status alerts, moderation playbook, metrics, and local docs with a change to
   thread lifecycle or comment delivery.
   **Depends on:** the changed comments runtime contract.
   **Done when:** closed/spam/trash behavior and recovery are observable and
   documented for operators.

7. **Close the direct-write richtext bypass and join the atomic cutover.**
   **Implemented for Comments.** A direct `CommentsThreadPort` or service
   write accepts the typed `RichTextDocument`, selects the `comment` profile
   server-side, and passes the strict validator. `comment_bodies` no longer
   stores a format selector; reads use canonical HTML/plain-text projections.
   The remaining verification is runtime evidence for every consumer.
   **Depends on:** the
   [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md)
   and synchronized Blog consumer contract.
   **Done when:** invalid/empty/oversized documents fail at every entry point,
   no direct port bypass exists, and Next/Leptos reads share the server renderer.

8. **Complete the reusable public comment composer without a generic
   target-write bypass.** The Comments-owned Leptos and React bindings and the
   Blog-owned native/GraphQL target commands are implemented. Both storefront
   hosts compose the editor into the selected Blog detail surface. Add mounted
   save/error/auth/browser evidence. The browser must never
   submit arbitrary `target_type`/`target_id` pairs directly to
   `CommentsService`. Product reviews and later consumers repeat only the
   target adapter, not the editor/form implementation.
   **Depends on:** authenticated storefront write policy, Blog target binding,
   and the shared richtext frame.
   **Done when:** Leptos and Next Blog detail submit the same canonical comment
   document through native/GraphQL paths, invalid or hidden targets fail before
   Comments writes, the moderation admin stays read-only, and no consumer owns
   a copied comment composer.

## Verification

The shared moderation-rendering slice was checked locally on 2026-08-11 with
`cargo xtask module validate comments`, `cargo xtask module test comments`,
native and `wasm32-unknown-unknown` Comments admin checks, all 11 Comments admin
unit tests, the Comments admin boundary verifier and its eight focused fixture
tests. This does not promote the pending PostgreSQL concurrency, delivery,
remote-provider, or retained runtime evidence below.

The first public authoring slice was checked locally on 2026-08-11 with
`cargo check -p rustok-blog`, default/SSR/hydrate checks for
`rustok-blog-storefront`, and three passing
`rustok-comments-storefront-support` unit tests. The broad filtered Blog
GraphQL test command still encounters four existing channel tests whose SQLite
setup now rejects PostgreSQL-only channel migrations; this does not promote
mounted or PostgreSQL runtime evidence.

The matching Next source slice was checked locally on 2026-08-11 with the
whole `apps/next-frontend` typecheck, the shared `packages/richtext` typecheck,
the existing Next admin typecheck after extracting the common richtext frame
route adapter, and a successful Next storefront production build. The Blog
integration test proves draft and hidden-channel rejection and a pending write
for the visible channel. This is not mounted browser evidence; authentication,
submission UX, and save/reload behavior remain open.

Maintainers should run the relevant broader subset, including:

- `npm run verify:comments:port-boundary`
- `npm run test:verify:comments:port-boundary`
- `npm run verify:comments:thread-write-invariants`
- `npm run test:verify:comments:thread-write-invariants`
- `npm run verify:comments:fba`
- `npm run test:verify:comments:fba`
- `cargo test -p rustok-comments --lib thread_insert_error_tests`
- `cargo test -p rustok-comments --test thread_write_invariants`
- `RUSTOK_COMMENTS_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-comments --test thread_write_invariants postgres_concurrent_creates_and_delete_preserve_thread_invariants`
- `RUSTOK_COMMENTS_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-comments --test thread_creation_concurrency`
- Targeted injected storage-error coverage for `find_or_create_thread_in_tx`
- `npm run verify:comments:admin-boundary`
- `cargo xtask module validate comments`
- `cargo xtask module test comments`
- Targeted moderation/status, blog integration, comment-port, and admin runtime
  tests.
- `cargo check -p rustok-comments`
- `cargo check -p rustok-blog`

## Change rules

1. Keep generic comment storage and moderation in this module.
2. Preserve tenant-thread locking for transactional position and explicit counter
   derivation; no caller or transport may source `position` or `comment_count`.
3. Status-only and metadata-only thread updates must not set `comment_count`.
4. Preserve the identity-lock before first-thread insert and keep its unique
   `(tenant_id, target_type, target_id)` key.
5. Keep the owner-classified identity marker scoped to tenant and target, require
   exactly one valid canonical thread UUID suffix, and never restore prefix-only or
   broad insert-error fallback.
6. Keep the source-verified provider profile limited to `in_process` until a remote
   adapter exists and retains equivalent policy, error, richtext, and public-read
   evidence.
7. Keep migrations append-only and preserve both database uniqueness invariants.
8. Update local/central contracts when the Comments boundary changes.

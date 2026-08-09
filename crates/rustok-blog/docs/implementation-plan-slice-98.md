# rustok-blog implementation plan — slice 98 continuation

Status: `category_translation_postgres_evidence_source_ready_maintainer_execution_pending`.

This slice is an independent continuation of the Blog category Translation target pilot recorded in `crates/rustok-blog/docs/implementation-plan.md` and the active cross-module Translation plan. It does not bypass the separate slice-97 Comments audit relay execution boundary.

## Re-audit

The current `blog/category` owner target already has the production source required for the pilot:

- migration `m20260803_000016_add_blog_category_translation_target_support` adds resource and exact-locale revisions plus `blog_translation_changes`;
- `CategoryService::apply_exact_translation_in_tx` checks resource, source and target revisions and performs conditional target/resource writes;
- the target adapter uses the shared durable owner-operation receipt before applying and completes that receipt inside the owner transaction;
- category create/update/delete and exact Translation apply append Blog-owned change facts;
- `read_changes` exposes the owner cursor and `read_progress` binds aggregate progress to a stable before/after cursor observation;
- the SQLite pilot already covers up/down/up, exact apply, replay, stale validation, progress and owner cursor basics.

The root Blog plan correctly left three production-database results open: PostgreSQL migration behavior, concurrent CAS, and change-cursor recovery. No additional production write path is required to retain those source targets.

## Slice 98 — PostgreSQL pilot evidence source

New harness:

`crates/rustok-blog/tests/category_translation_target_postgres_test.rs`

The harness is environment-gated by `RUSTOK_BLOG_TRANSLATION_TEST_DATABASE_URL`, then falls back to `RUSTOK_BLOG_TEST_DATABASE_URL` and PostgreSQL `DATABASE_URL`. Every scenario creates a unique schema and sets `search_path` to that schema only; `public` is deliberately excluded.

### PostgreSQL migration up/down/up

The migration scenario applies the real Outbox and Taxonomy dependency migrations, applies all Blog migrations preceding `000016`, then executes the real Translation target migration as `up -> down -> up`.

After reapplication it uses the production `CategoryService::create` path and the production `BlogCategoryTranslationTargetProvider::read_changes` path. The retained assertions require resource revision one, source-locale revision one, one active change fact and a non-empty resume cursor.

The scenario does not create a parallel test schema or hand-written substitute for the owner migration.

### Concurrent same-revision apply

The concurrency scenario creates one source category and one exact source snapshot. Two independent PostgreSQL connections receive patches derived from the same resource/source/target revision facts but use distinct owner idempotency keys and distinct target values.

Both calls enter through the public `TranslationTargetProvider::apply_patch` contract. A barrier releases them together. The retained outcome is:

- exactly one successful apply;
- exactly one closed `Conflict` loser;
- final resource revision two;
- final target revision one;
- the exact target value belongs to the winning request;
- the journal contains only source creation plus one winning apply fact;
- `sys_events` contains one Blog reindex request from the winner.

This proves the public provider plus Category owner CAS converges without a duplicate target commit. The harness does not claim which caller wins and does not add locking or retry behavior to production.

### Change-cursor recovery after reconstruction

The recovery scenario retains a source-create cursor, drops the first provider, opens a new connection/provider, applies one exact target, resumes from the retained cursor, and captures the next cursor. It then reconstructs the owner again, deletes the category, reconstructs the provider again, and resumes from the apply cursor.

The retained assertions require:

- exactly one active apply change after the source cursor;
- exactly one deleted lifecycle change after the apply cursor;
- resource revisions `1 -> 2 -> 3` across source, apply and delete facts;
- an empty page after the final cursor;
- zero active resources after deletion;
- `read_progress.owner_change_cursor` equal to the final deleted change cursor.

Blog change IDs are generated through `rustok_core::generate_id`, which stores ULID bytes in a UUID. The recovery fixture separates its independent retained writes by two milliseconds so the test deterministically exercises ordinary resume semantics. It deliberately does **not** claim that the current cursor contract proves arbitrary concurrent transaction commit ordering; that is a separate contract question and is not promoted by this slice.

## Machine evidence and guard

Source evidence:

`crates/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json`

Fail-closed source guard:

`scripts/verify/verify-blog-category-translation-postgres-source.mjs`

The evidence remains unvalidated until maintainers run the PostgreSQL target and verifier at an exact revision.

## Preserved boundaries

This slice does not change:

- Blog category storage or Translation target schema;
- `CategoryService` mutation or CAS behavior;
- Translation target request/response contracts;
- owner-operation receipt semantics;
- Search reindex publication behavior;
- Translation inventory/checkpoint ownership;
- FFA/FBA status;
- Blog Comments audit relay execution status from slice 97.

No new endpoint, worker, retry lane, queue, provider capability or cross-module SQL path is added.

## Maintainer execution

```bash
node scripts/verify/verify-blog-category-translation-postgres-source.mjs

RUSTOK_BLOG_TRANSLATION_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-blog \
  --test category_translation_target_postgres_test \
  -- --nocapture --test-threads=1
```

No tests, Cargo commands, Node verifiers, PostgreSQL scenarios, formatting, Clippy, workflows, CI, browser targets or runtime validation were executed by the implementation agent.

## Next cursor

After successful maintainer execution, the Blog category pilot may record the PostgreSQL migration/concurrent-CAS/change-recovery result in the active Translation readiness view. The broader Translation plan still retains production enablement and live provider evidence as separate owner/maintainer work.

The Comments audit track remains independently blocked on maintainer execution of its retained slices before source-row/recovery-audit retention is advanced.

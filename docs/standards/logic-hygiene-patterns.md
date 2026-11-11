---
id: doc://docs/standards/logic-hygiene-patterns.md
doc_type: standard
status: active
owner: platform-architecture
canonical_for:
  - domain-logic-patterns
  - anti-slop-remediation
language: markdown
---

# RusToK — Domain Logic, Invariant Integrity, and Anti-Slop Patterns

This standard catalogs patterns and anti-patterns for RusToK. The evidence
inventory below identifies confirmed cases. Other entries are review guidance
until an actual owner path and its behavior have been verified. Illustrative
snippets describe techniques and must be checked against the owner's contract.
Doctor findings are candidates whose impact and confidence are reported separately.

## Confirmed cases in RusToK

These entries identify historical defects and their owner-side fixes. They do
not claim that every caller or deployment has been verified. A source change
without evidence of a failure is recorded separately below.

| Pattern | Observed defect and fix | Evidence |
|---|---|---|
| `P-TRIGGER-ROW-CHANGE` | Product's earlier `AFTER UPDATE OF index_revision` trigger missed revisions changed by a `BEFORE` trigger on ordinary Product updates. Change `48e692c886` fixes the trigger and the freshness fixture exercises a scalar update. | [Trigger migration](../../crates/modules/rustok-product/src/migrations/m20261109_000040_fix_index_graph_update_trigger.rs), [PostgreSQL fixture](../../crates/modules/rustok-distribution/tests/product_materialized_query_freshness_postgres.rs) |
| `P-OWNER-READ-BOUNDARY` | Distribution queried private Product and Channel tables. Change `8cc68c8fa2` moved the read contracts to their owners while retaining one PostgreSQL snapshot for admission. | [Decision](../../DECISIONS/2026-11-08-distribution-index-owner-read-contracts.md), [Product projection](../../crates/modules/rustok-product/src/index_projection.rs), [Channel projection](../../crates/modules/rustok-channel/src/index_projection.rs) |
| `P-FIXTURE-OWNERSHIP` | A migration verifier could attempt cleanup after creation failed because a fixture name already existed. Change `8b407a1bb3` tracks successful acquisition before cleanup. | [Verifier](../../crates/utils/rustok-migrations/src/bin/verify_migrations.rs), [bootstrap](../../crates/utils/rustok-migrations/src/verification/bootstrap.rs) |
| `P-DB-CHECK` | Forum reconciliation counted approved replies without excluding archived or deleted parent topics. Change `d55c32c990` aligns the predicates and adds corrective migration. | [Reconciler](../../crates/modules/rustok-forum/src/services/solution_reconciliation.rs), [migration](../../crates/modules/rustok-forum/src/migrations/m20261110_000045_reconcile_archived_counters.rs) |
| `P-ANTI-SWALLOW` | Channel's SQLite migration discarded an `ALTER TABLE` error. Change `dfaab54060` handles the known duplicate-column case and propagates other errors. Its message-based compatibility branch remains a separate limitation. | [Channel migration](../../crates/modules/rustok-channel/src/migrations/m20260730_000010_add_channel_index_revision.rs) |
| `P-ZERO-INCLUDE` | Forum's `1c5a07dc8b` mechanically inlined source when removing `include!`. Follow-up `37bb36fd80` repaired the resulting module structure. | [Forum services](../../crates/modules/rustok-forum/src/services/mod.rs) |

For `P-ID-ENTROPY`, Forum's route identifier changed from 12 to 16 hex
characters in `1c5a07dc8b`; this does not establish that a collision occurred.
See the [route implementation](../../crates/modules/rustok-forum/src/services/topic_route.rs).

The confirmed-cases inventory is intentionally narrower than the pattern
catalog. Add a concrete affected path, historical change or reproduction, and
verification status before describing another pattern as an observed defect.

The Forum error adapter currently searches formatted error details for SQLSTATE
and trigger tokens. This is a source-level candidate for
`P-SQLSTATE-DISCRIM`; a specific misclassification against a PostgreSQL error
has not been reproduced. [Forum error adapter](../../crates/modules/rustok-forum/src/error.rs)

The Alloy executor currently discards errors from `record_error` after a script
failure. If persistence fails, the failed execution may not advance the counter
used to disable a repeatedly failing script, and the accounting failure is not
reported by that call site. This is a confirmed source-level failure branch,
not evidence that such a persistence failure has occurred in production.
`P-ANTI-SWALLOW` reports the call for review. See the
[executor](../../crates/modules/alloy/src/runner/executor.rs),
[registry implementation](../../crates/modules/alloy/src/storage/sea_orm/mod.rs),
and [threshold](../../crates/modules/alloy/src/model/script.rs).

---

## 1. Core Philosophy: The 6 Universal Logic Dimensions

Under [AGENTS.md §4](../../AGENTS.md), verify the affected behavior and its owner contract across the **6 Universal Logic Dimensions**. Use source matches to find code to inspect. A match alone cannot establish a behavioral defect:

```mermaid
flowchart TD
    D1["1. Preconditions & Bounds<br/>(Fail-closed, settings validation, limits)"]
    D2["2. Branch Completeness<br/>(Zero error swallowing, no dummy Ok masks)"]
    D3["3. State Transition Integrity<br/>(DB CHECK constraints, monotonic status)"]
    D4["4. Side-Effect Completeness<br/>(Atomic outbox, lock release, cache flush)"]
    D5["5. Algorithmic Correctness<br/>(Entropy, math precision, collision handling)"]
    D6["6. Concurrency & TOCTOU<br/>(Lock-before-check, advisory locks, isolation)"]

    D1 --> D2 --> D3 --> D4 --> D5 --> D6
```

---

## 2. Catalog of Platform Patterns

### Pattern 1: Scope-Generic Deduplication (`P-SCOPE-GENERIC`)
* **Category:** Structural Architecture & Code Deduplication
* **Problem:** In complex modules, multiple entities require nearly identical storage, multi-layer inheritance, or batch insertion logic (e.g. category visibility, category moderation, topic visibility, reply policies). Writing separate services for each entity creates thousands of lines of copy-pasted SQL/SeaORM queries.
* **Pattern Solution:** The following generic interface is illustrative.
  Forum's actual scope types and ownership live in
  [audience_policy.rs](../../crates/modules/rustok-forum/src/services/audience_policy.rs).
  1. Define a marker trait `EntityScope` with static metadata:
     ```rust
     pub trait EntityScope: Send + Sync + 'static {
         const NAME: &'static str;
         const TABLE_PREFIX: &'static str;
         const SUBJECT_COLUMN: &'static str;
     }
     ```
  2. Implement a single generic struct with `PhantomData<S>`:
     ```rust
     pub struct PolicyEngine<S: EntityScope> {
         pub subject_id: Uuid,
         pub constraints: Option<AudienceConstraints>,
         _marker: PhantomData<S>,
     }
     ```
  3. Keep the shared storage functions (`insert_*_raw`, `load_*_raw`) parameterized and tested in a single submodule.
  4. **Preserve External DTOs:** Convert generic internals into strongly-typed public DTOs (`ForumCategoryModerationAudiencePolicy`, `SellerAudiencePolicy`) or type aliases. Public transports and OpenAPI/GraphQL schemas must never leak internal generic structures.
* **Anti-Pattern (Forbidden):**
  * Copy-pasting this generic engine into unrelated modules where the data model does not match.
  * Creating speculative scopes without an existing canonical consumer.

---

### Pattern 2: Database-Enforced State Invariants (`P-DB-CHECK`)
* **Category:** State Transition & Data Integrity
* **Problem:** Application-level validation is fragile. Migrations, raw SQL updates, or missed application branches can leave rows in corrupted states (e.g. `status = 'archived'` while `is_locked = false`, or `deleted_at IS NOT NULL` while `status = 'active'`). A database trigger can also overwrite a valid application update when it uses an older predicate. Forum's counter guards counted archived threads after the owner began excluding them. Inspect the trigger, command and reconciliation predicates together (`rg -n 'forum_enforce_.*reply_count|topic.status.*archived' crates/modules/rustok-forum/src`).
* **Pattern Solution:** Enforce the owner's actual invariant in each supported
  database and verify command, trigger, and repair predicates together. Forum's
  archived-topic lock invariant is implemented by its
  [owner migration](../../crates/modules/rustok-forum/src/migrations/m20261109_000043_enforce_archived_locked_invariant.rs).

---

### Pattern 3: Lock-Before-Check Concurrency (`P-LOCK-ORDER`)
* **Category:** Order of Operations & TOCTOU Elimination
* **Problem:** Checking invariants (such as rate limits, inventory availability, balance thresholds, or slug uniqueness) *before* acquiring the transactional or advisory lock introduces a Time-Of-Check to Time-Of-Use (TOCTOU) race condition.
* **Pattern Solution:**
  Always acquire the mutex or advisory lock first, and verify preconditions *inside* the protected critical section:
  ```rust
  // Correct Order of Operations:
  let txn = db.begin().await?;

  // 1. Acquire transaction-level lock first
  lock_entity_in_tx(&txn, tenant_id, entity_id).await?;

  // 2. Perform precondition & rate-limit checks
  enforce_rate_limit_in_tx(&txn, tenant_id, author_id).await?;

  // 3. Mutate entity state
  insert_or_update(&txn, ...).await?;

  txn.commit().await?;
  ```
* **Anti-Pattern (Forbidden):**
  * Performing rate-limit increment or stock validation on a non-transactional connection before `db.begin()`.
  * Using raw string formatting with `execute_unprepared` for advisory locks instead of parameterized SeaORM `Statement`.

---

### Pattern 4: Fail-Closed Security Floor (`P-FAIL-CLOSED`)
* **Category:** Preconditions, Bounds & Authorization
* **Problem:** Evaluating child permission layers (e.g. category-level audience, product visibility) before evaluating global tenant policies allows unauthorized access if a child entity is marked "public" while the entire tenant/feature is restricted.
* **Pattern Solution:**
  Tenant-wide and security policies act as a hard floor. Check fail-closed conditions at the very top of the evaluation chain:
  ```rust
  // Fail-closed floor check
  if !allow_anonymous_reading && security.is_public_read() {
      return Err(DomainError::AuthenticationRequired);
  }

  // Only if the global floor passes, evaluate category/resource policies
  evaluate_resource_audience(&security, resource_policy)?;
  ```
* **Anti-Pattern (Forbidden):**
  * Treating `PermissionScope::None` as permission to proceed when the guard owns the access decision. Return a denial in the guard's own contract, such as `Err(Forbidden)` or `Ok(false)`. A secondary policy may return `Ok(())` to mean “no additional restriction” when an earlier guard already owns authorization.
* **Verification:** Exercise a missing-scope caller against an otherwise public resource and verify the final outcome. The scanner flags explicit success branches for review.

---

### Pattern 5: Bounded Identifier Entropy (`P-ID-ENTROPY`)
* **Category:** Calculation & Algorithmic Correctness
* **Problem:** The probability of at least one collision among 10 million identifiers in one uniqueness namespace is about 16.3% at 48 bits and 0.000271% at 64 bits, assuming independent, uniform draws. Twelve hex characters encode at most 48 bits. A 12-character prefix of the simple form of UUIDv7 is its timestamp, as defined by [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562.html#name-uuid-version-7), so the uniform-random assumption does not apply.
* **Pattern Solution:** Choose the entropy budget for the namespace and expected volume. Enforce uniqueness at the owner persistence boundary and define an explicit collision outcome, such as retry on the relevant uniqueness conflict. A lookup with `LIMIT 2` detects ambiguity but cannot enforce uniqueness. Preserve existing public route compatibility when changing an identifier scheme.
* **Verification:** Inspect the generator, effective random bits, uniqueness scope, and collision path. The scanner recognizes narrow short-ID declarations, UUID compaction, and short-ID SQL column hints.

---

### Pattern 6: Zero Error Swallowing (`P-ANTI-SWALLOW`)
* **Category:** Branch Completeness & Anti-Slop
* **Problem:** Developers and AI agents frequently use `let _ = ...`, dummy `Ok(())` fallbacks, or `std::hint::black_box(...)` to silence compiler warnings and linter errors (such as `let_underscore_must_use = "deny"`), masking aborted rollbacks or unhandled failures.
* **Pattern Solution:**
  1. Propagate errors via typed `Result<T, DomainError>`.
  2. If an error cannot fail the outer operation (e.g. best-effort background notification or rollback cleanup), log it explicitly with structured context:
     ```rust
     if let Err(err) = txn.rollback().await {
         tracing::warn!(error = %err, %tenant_id, "Transaction rollback failed during error recovery");
     }
     ```
  3. Check the discarded expression’s type and its failure contract. The scanner flags discarded calls for review and skips plain values. Verify any required existence check at the owner boundary.
  4. For batch or adapter outcomes, inspect both typed errors and successful
     return values containing failure counts or error details. In the installer
     seed workflow, a loader failure with an unknown item count was previously
     stored with `failed_count = 0`; checking that count alone reported success.
     Preserve the unknown count and treat the diagnostic as a failed outcome:
     ```rust
     let succeeded = outcome.failed_count == 0 && outcome.errors.is_empty();
     ```
     When continuation is disabled, a partial outcome must fail the enclosing
     operation. Validate known missing dependencies before invoking owner ports.
     Find candidates with `rg -n 'failed_count == 0|continue_on_content_error' crates`;
     inspect each owner's semantics rather than treating the match as a defect.
* **Anti-Pattern (Forbidden):**
  * Using `std::hint::black_box` in production service code to suppress unused variable or must-use warnings.

---

### Pattern 7: Module Tree Decoupling (`P-ZERO-INCLUDE`)
* **Category:** Compilation Hygiene & Zero AI Slop
* **Problem:** The `include!("...")` macro is abused to merge multiple files into one module to bypass circular dependencies or file-count limits. This breaks `rust-analyzer`, `goto-definition`, incremental compilation, and cargo documentation.
* **Pattern Solution:**
  1. Use standard Rust module hierarchies and explicit exports, such as `mod category; pub use category::CategoryService;`.
  2. Break circular dependencies by defining narrow traits (e.g. `LockProvider`, `AncestorLoader`) implemented by the caller or foundation crate, rather than text-including source files.
  3. Keep handwritten Rust source in modules. Build output loaded through `concat!(env!("OUT_DIR"), ...)` is a separate use case. The scanner gates direct literal `include!("file.rs")` outside recognized test, example, and build contexts.
  4. When replacing a file-stitching macro, register the existing source file as a module and move child declarations to their owner as needed. Pasting the file body at the call site can leave a second copy and a stray statement delimiter. Retarget source guards to the compiled modules and verify the resulting tree with a scoped build.

---

### Pattern 8: Configuration & Settings Bounding (`P-CONFIG-BOUND`)
* **Category:** Preconditions & Anti-DoS
* **Problem:** Inconsistent settings bounds can make an owner's operations reject valid inputs. Confirm the relevant settings and write paths for each owner.
* **Pattern Solution:**
  The following illustrative validation demonstrates an ordering check:
  ```rust
  impl ModuleSettings {
      pub fn validate(&self) -> Result<(), DomainError> {
          if self.min_length > self.max_length {
              return Err(DomainError::Validation("min_length cannot exceed max_length".into()));
          }
          if self.rate_limit_seconds > 86_400 {
              return Err(DomainError::Validation("rate_limit_seconds cannot exceed 24 hours".into()));
          }
          Ok(())
      }
  }
  ```
  Apply the owner's actual bounds at its settings write boundary.

---

### Pattern 9: Elimination of Dead Twins (`P-DEAD-TWIN`)
* **Category:** Maintenance Integrity & Zero AI Slop
* **Problem:** An unused implementation can obscure the active owner. For example, Forum's experimental merge reconciliation executor added a second lock while each reconciliation path already acquired its owner-defined transaction lock. Removing an implementation based only on local callers can also break public consumers. Forum's posting policy fact providers and its create cooldown have distinct callers and responsibilities.
* **Pattern Solution:**
  * Trace reexports, host composition and tests before deleting a duplicate. Preserve the transaction and idempotency contracts of active callers.
  * If a prototype is planned for future delivery, preserve it in an ADR or design specification, not as dead code inside `src/`.

---

### Pattern 10: Bounded Tree Projections (`P-BOUNDED-TREE`)
* **Category:** Algorithmic Correctness & Memory Safety
* **Problem:** Recursive hierarchy traversals without an owner-defined bound can exhaust memory on sufficiently large inputs. Confirm the affected entrypoint and its actual bounds before classifying a finding as a defect.
* **Pattern Solution:**
  1. Establish a node or depth bound from the owner's requirements and resource budget.
  2. When hierarchy exceeds the bounded memory envelope, require depth-limited queries (`?depth=2`) or streaming traversals.

---

### Pattern 11: Transactional Batching & Anti-N+1 (`P-BATCH-QUERY`)
* **Category:** Algorithmic & Concurrency Performance
* **Problem:** Executing per-item queries inside a transaction can increase round trips and extend lock duration. Measure the affected path and check transaction semantics before changing it.
* **Pattern Solution:**
  1. Batch all IDs into an `is_in` set:
     ```rust
     let rows = Entity::find()
         .filter(Column::TenantId.eq(tenant_id))
         .filter(Column::Id.is_in(ids))
         .all(txn)
         .await?;
     ```
  2. Bound the batch size and index the results for lookup. Preserve the caller’s ordering, missing-ID behavior, and lock semantics when replacing per-item queries.

---

### Pattern 12: Zero Runtime Panics in Production (`P-ZERO-PANIC-PROD`)
* **Category:** Preconditions, Bounds & Anti-Slop
* **Problem:** Calling `.expect("static contract...")` or `.unwrap()` in production request handlers (e.g. parsing `OwnerSlug`, `ResourceKind`, `UUID` in translation targets or pricing calculation) leads to worker panic and service degradation when inputs deviate or schema constants evolve.
* **Pattern Solution:**
  1. Convert all runtime value parsing to return `Result<T, DomainError>` or `PortError::invalid_input`.
  2. For genuinely static constants, construct them at compile time via `const` or pre-validate them during module initialization.
  3. Enforce `#![deny(clippy::unwrap_used, clippy::expect_used)]` across production service crates.

---

### Pattern 13: Allocation Hygiene & Late Clones (`P-HOTPATH-ALLOC`)
* **Category:** Algorithmic Efficiency & Memory Optimization
* **Problem:** Cloning an input before a rejection can add allocations. Establish whether that cost matters for the actual path before refactoring.
* **Pattern Solution:**
  1. Validate borrowed slices (`&str`, `&[u8]`) first.
  2. Allocate and normalize (`.to_string()`, `.into()`, `.clone()`) strictly *after* all precondition gates have succeeded.

---

### Pattern 14: SQLSTATE Error Discrimination (`P-SQLSTATE-DISCRIM`)
* **Category:** Branch Completeness & Error Robustness
* **Problem:** Matching database errors via raw message substrings (e.g. `err.to_string().contains("topic conflict")`) is brittle. Changing a trigger message, database locale, or driver wording silently breaks error mapping, returning `500 Internal Server Error` instead of `409 Conflict`.
* **Pattern Solution:**
  1. Discriminate database errors via standard SQLSTATE error codes:
     * `23505`: Unique constraint violation (`Conflict`).
     * `23503`: Foreign key violation (`BadRequest` / `NotFound`).
     * `P0001`: PostgreSQL user-defined trigger exception (`RAISE EXCEPTION`).
  2. Prefer structured constraint identity or distinct owner-defined SQLSTATEs for failures that share a generic code. Where a legacy contract exposes only a stable message token, keep the fallback at that boundary and test it against the actual database error.

---

### Pattern 15: Contextual Observability Spans (`P-STRUCTURED-TRACE`)
* **Category:** Observability & Production Diagnostics
* **Problem:** When the effective span lacks tenant, actor, or operation context, operators cannot correlate authorization or storage failures with the affected request.
* **Pattern Solution:**
  1. Establish context at the owner entry point or inherit it from the caller. Exclude sensitive inputs from span fields:
     ```rust
     #[tracing::instrument(
         skip(self, security, input),
         fields(tenant_id = %tenant_id, user_id = %security.user_id())
     )]
     pub async fn update_entity(&self, tenant_id: Uuid, security: SecurityContext, input: Input) -> DomainResult<Output> {
         // ...
     }
     ```
  2. Verify the emitted span or trace for the failing operation. A function name and nearby span marker are lexical hints and cannot prove inherited context.

---

### Pattern 16: Ignored Transaction Finalization (`P-TXN-ROLLBACK-SWALLOW`)
* **Category:** Branch Completeness & Transaction Integrity
* **Problem:** Discarding a commit result can report success for a failed or uncertain write. Discarding a rollback result hides cleanup failure. PostgreSQL [transaction advisory locks](https://www.postgresql.org/docs/current/explicit-locking.html#ADVISORY-LOCKS) are released at transaction end. Session advisory locks survive rollback and require explicit release or session termination.
* **Pattern Solution:** Propagate commit failure and use the owner’s idempotency or reconciliation contract when the outcome is uncertain. On failure recovery, report explicit rollback errors with context while preserving the primary error:
  ```rust
  if let Err(rollback_err) = transaction.rollback().await {
      tracing::warn!(%rollback_err, "transaction rollback failed");
  }
  return Err(primary_error);
  ```
* **Observed Groups remediation:** The membership Moderation adapter discarded both rollback errors and failed terminal producer-receipt persistence. Application and receipt completion now share explicit rollback; if finalization itself fails, the adapter returns a retryable error preserving the primary code/message and the failed stage. A failed rollback must not be followed by recording a permanent rejection as though finalization succeeded.
* **Verification:** Identify the actual transaction and lock kind. Test that callers never receive false success and that cleanup failures remain observable. The scanner flags discarded `commit` and `rollback` calls.

---

### Pattern 17: Synchronous Lock Guard Across Await Points (`P-SYNC-LOCK-AWAIT`)
* **Category:** Async Concurrency & Thread Pool Starvation
* **Problem:** A synchronous guard retained across suspension keeps the lock held. A competing synchronous acquisition can then block an executor thread or deadlock with the suspended task. Some guards also make a future non-`Send`.
* **Pattern Solution:** Prefer a short synchronous scope that releases the guard before awaiting. Use an async lock when the owner genuinely requires a critical section that spans suspension.
  ```rust
  let snapshot = {
      let guard = self.cache.lock(); // parking_lot::Mutex
      guard.clone()
  };
  remote_client.send(snapshot).await?;
  ```
* **Verification:** Inspect the guard type and all reachable suspension paths, including conditional drops. Where supported, run [Clippy’s `await_holding_lock`](https://clippy.rust-lang.org/master/index.html#await_holding_lock) on the owning crate. The scanner uses lexical acquisition and scope cues.

---

### Pattern 18: Unprepared SQL String Interpolation (`P-RAW-SQL-INTERP`)
* **Category:** Security, Preconditions & Injection Prevention
* **Problem:** Formatting runtime values into SQL can change query semantics and permit injection. Fixed SQL fragments and identifiers selected from a closed set have a different trust boundary from request values.
* **Pattern Solution:**
  1. Bind runtime values as parameters. For identifiers that cannot be bound, use a typed query builder or a fixed allowlist. The scanner traces direct formatting and simple local bindings; verify the origin of each interpolated value.
  2. Use parameterized queries via SeaORM or SeaQuery:
     ```rust
     use sea_orm::{ConnectionTrait, Statement};

     let stmt = Statement::from_sql_and_values(
         db.get_database_backend(),
         "UPDATE orders SET status = $1 WHERE id = $2 AND tenant_id = $3",
         [status.into(), order_id.into(), tenant_id.into()],
     );
     db.execute(stmt).await?;
     ```

---

### Pattern 19: Tenant Isolation Fallback & Identity Leaks (`P-TENANT-FALLBACK-LEAK`)
* **Category:** Multi-Tenancy & Fail-Closed Security
* **Problem:** A default UUID used as a tenant selector can violate the authorization boundary if it reaches data access without an owner-defined denial. A source match alone does not establish a cross-tenant leak.
* **Pattern Solution:**
  1. Multi-tenant operations must fail closed:
     ```rust
     let tenant_id = tenant_id.ok_or(DomainError::TenantNotFound)?;
     ```
  2. Do not use dummy nil UUIDs as wildcard or fallback selectors. Verify that absent and wrong-tenant requests cannot reach another tenant’s rows. The scanner recognizes explicit tenant selectors and assignments, and cannot establish identity flow through wrappers.

---

### Pattern 20: Transactional Outbox Decoupling (`P-OUTBOX-DETACHED`)
* **Category:** Side-Effect Completeness & Atomicity
* **Problem:** Publishing independently of the owner’s mutation can emit an event for a rolled-back change or lose the event after a successful commit.
* **Pattern Solution:**
  1. In transactional scopes, always write outbox events through the transaction handle using `publish_in_tx`:
     ```rust
     event_bus.publish_in_tx(&txn, domain_event).await?;
     txn.commit().await?;
     ```
  2. Verify that commit makes both rows durable and rollback leaves neither the change nor its outbox row. The scanner flags direct publication when it sees transaction evidence in the same function. Verify the publisher implementation and transaction handle at the owner boundary.

---

### Pattern 21: UI Reactive Signal Invalidation (`P-LEPTOS-DEAD-SIGNAL`)
* **Category:** Client Architecture & UI Reactivity
* **Problem:** A discarded signal read outside a reactive tracking context does not establish a subscriber. Verify the actual consumer before treating the source match as a UI defect.
* **Pattern Solution:**
  1. Bind signal dependencies inside an effect when the operation needs reactive tracking:
     ```rust
     Effect::new(move |_| {
         let value = signal.get();
         update_view_model(value);
     });
     ```
  2. In Leptos views, reference signals directly within view macros: `move || signal.get()`.

---

### Pattern 22: Lingering Incomplete Code & Work Markers (`P-UNFINISHED-CODE`)
* **Category:** Production Readiness & Maintenance Integrity
* **Problem:** Reachable `todo!()` and `unimplemented!()` can panic in production. A `TODO` or `FIXME` comment identifies work to review; it does not establish that an invariant is broken.
* **Pattern Solution:** Complete runtime paths with typed error and edge-case behavior. Track agreed follow-up work in the owning plan or ADR. Check that affected inputs cannot reach incomplete execution paths.

---

### Pattern 23: Blocking Sleep in Async Functions (`P-SYNC-SLEEP-AWAIT`)
* **Category:** Async Concurrency & Thread Pool Starvation
* **Problem:** Calling `std::thread::sleep` on an executor worker occupies the thread for the whole delay. Concurrent waits can starve requests and timers.
* **Pattern Solution:** Use the runtime timer for async waiting. Keep necessary blocking work on a blocking executor.
  ```rust
  tokio::time::sleep(delay).await;
  ```
* **Detection:** Start with `rg -n 'std::thread::sleep' crates apps -g '*.rs'`. Verify the execution context, including `spawn_blocking` and dedicated threads. The scanner recognizes direct synchronous sleep calls in lexically async functions.

---

### Pattern 24: Owner SQL Read Boundaries (`P-OWNER-READ-BOUNDARY`)
* **Category:** Module Isolation & Concurrency
* **Problem:** Distribution's Product and Channel Index adapters read owner-private tables. A storage change can break the adapter despite an unchanged owner API. Splitting freshness checks into separate reads can admit stale rows or produce the wrong page or count.
* **Pattern Solution:** Put the projection and selection rules in the owner. Compose the published SQL in the existing transaction or query when the consumer needs one snapshot. For example, the relation resolver uses the Channel owner's statement:
  ```rust
  let statement = rustok_channel::index_projection::relation_channel_ids(
      tenant_id, selected_slugs, fetch_limit,
  );
  let rows = transaction.query_all_raw(statement).await?;
  ```
* **Detection:** Start with `rg -ni '(from|join) (products|product_[a-z_]+|channels|channel_index_[a-z_]+)' crates/modules/rustok-distribution/src`. Review ownership and verify that delayed or deleted owner state is rejected before pagination and exact count.

---

### Pattern 25: Observe Trigger-Derived Revisions (`P-TRIGGER-ROW-CHANGE`)

* **Category:** Transactional integrity. **Dimension:** Side-effect completeness.
* **Problem:** A PostgreSQL `AFTER UPDATE OF revision` trigger checks the statement's target columns. An earlier `BEFORE UPDATE` trigger can change the revision on a scalar edit, leaving the append-only projection at its previous watermark. Both the initial Product graph ledger and its canonical successor contained this defect.
* **Pattern Solution:** Match the row transition after the write. Reuse the owner reconciler so the source and derived watermark commit together.
  ```sql
  CREATE OR REPLACE TRIGGER trg_products_index_graph_projection_update
  AFTER UPDATE ON products
  FOR EACH ROW
  WHEN (OLD.index_revision IS DISTINCT FROM NEW.index_revision)
  EXECUTE FUNCTION rustok_product_capture_index_graph_projection_from_product();
  ```
* **Detection:** Run `rg -ni 'AFTER UPDATE OF .*revision|NEW\\.[a-z_]*revision\\s*:=' crates/modules` and compare each live update trigger with the preceding triggers on its table. On PostgreSQL, update an indexed scalar alone and verify that the projection records the final revision.

---

### Pattern 26: Clean Up Only Created Fixtures (`P-FIXTURE-OWNERSHIP`)

* **Category:** Resource ownership. **Dimension:** Side-effect completeness.
* **Problem:** The PostgreSQL smoke and fresh verifier used a fixture name as evidence of ownership. If creation failed because the database or role already existed, dropping that name could delete a resource from another run.
* **Pattern Solution:** Treat each successful exclusive creation as the acquisition of that resource. Carry partial acquisition into cleanup and surface cleanup errors alongside the primary failure.
  ```rust
  create_postgres_database(&admin, &name).await?;
  let result = run_fixture(&name).await;
  let cleanup = drop_postgres_database_if_exists(&admin, &name).await;
  finish_fixture(result, cleanup)
  ```
* **Detection:** Run `rg -ni 'DROP (DATABASE|ROLE) IF EXISTS|drop_postgres_database_if_exists' crates` and trace ownership on each cleanup path. Pre-create the fixture name and verify that the attempt fails while the original resource survives.

---

## 3. Summary Matrix

The evidence column describes how to verify the invariant at its owner. “Manual” means RusToK Doctor has no rule for that pattern.

| ID | Dimension | Evidence to verify | RusToK Doctor |
|---|---|---|---|
| `P-SCOPE-GENERIC` | Structure | Equivalent behavior and one owner across scopes | Manual |
| `P-DB-CHECK` | State integrity | Database rejects invalid state transitions or combinations | Manual |
| `P-LOCK-ORDER` | Concurrency | Competing writers cannot invalidate a checked precondition | Manual |
| `P-FAIL-CLOSED` | Authorization | Missing scope is denied even for an otherwise public resource | Heuristic: explicit success for absent scope |
| `P-ID-ENTROPY` | Calculation | Effective entropy, uniqueness scope, and collision handling | Heuristic: named widths, UUID compaction, short-ID schema |
| `P-ANTI-SWALLOW` | Failure paths | Required failures propagate or are handled and observable | Heuristic: discarded call or `black_box` |
| `P-ZERO-INCLUDE` | Module structure | Handwritten Rust is wired with explicit modules | High: direct literal Rust inclusion outside recognized exemptions |
| `P-CONFIG-BOUND` | Preconditions | All writers reject invalid combinations and limits | Manual |
| `P-DEAD-TWIN` | Ownership | Active callers and exports show whether an implementation is redundant | Heuristic: broad inner lint suppression |
| `P-BOUNDED-TREE` | Algorithmic bounds | Deep, wide, and cyclic input respects the declared budget | Manual |
| `P-BATCH-QUERY` | Performance and concurrency | Query count stays bounded without changing lock or result semantics | Heuristic: transaction query in a loop |
| `P-ZERO-PANIC-PROD` | Reliability | Runtime inputs cannot trigger panic paths | Heuristic: panic-prone calls |
| `P-HOTPATH-ALLOC` | Allocation | Invalid requests avoid unnecessary allocations on the affected path | Heuristic: early clone |
| `P-SQLSTATE-DISCRIM` | Error mapping | Real SQLSTATE and constraint failures map correctly | Heuristic: error-message substring match |
| `P-STRUCTURED-TRACE` | Observability | Emitted or inherited spans contain required context | Heuristic: mutation name and nearby span marker |
| `P-TXN-ROLLBACK-SWALLOW` | Failure paths | Commit errors propagate and rollback errors remain observable | Heuristic: discarded finalization call |
| `P-FIXTURE-OWNERSHIP` | Side effects | Name collisions leave pre-existing fixtures intact | Manual |
| `P-SYNC-LOCK-AWAIT` | Concurrency | Actual synchronous guard lifetimes exclude suspension | Heuristic: lock and await in lexical scope |
| `P-RAW-SQL-INTERP` | Security | Runtime values are bound and interpolated identifiers are trusted | Heuristic: formatted direct SQL or local binding |
| `P-TENANT-FALLBACK-LEAK` | Tenant isolation | Missing and wrong-tenant identities fail closed | Heuristic: fallback on a tenant selector |
| `P-OUTBOX-DETACHED` | Side effects | Domain write and outbox write commit or roll back together | Heuristic: direct publish with transaction evidence in the same function |
| `P-LEPTOS-DEAD-SIGNAL` | UI reactivity | An update reruns the intended reactive consumer | Heuristic: discarded signal read outside known reactive scopes |
| `P-UNFINISHED-CODE` | Runtime completeness | Affected runtime paths cannot reach incomplete code | Heuristic: work marker or incomplete-code macro |
| `P-SYNC-SLEEP-AWAIT` | Concurrency | Blocking delay runs outside executor workers | Heuristic: direct blocking sleep in async function |
| `P-OWNER-READ-BOUNDARY` | Ownership and concurrency | Owner controls storage reads and the adapter preserves snapshot and admission semantics | Manual |
| `P-TRIGGER-ROW-CHANGE` | Side effects | An update that derives a revision in a trigger commits the matching projection | Heuristic: column-filtered AFTER UPDATE trigger in migration SQL; inspect active migration chain and dialect |
| `P-COMMAND-ADMISSION` | Concurrency | Same-key concurrent commands serialize admission before reading receipt state | Manual |
| `P-REPLAY-PRECONDITION` | Idempotency | A replay preserves the original command identity and revision | Manual |

---

## 4. Periodic Domain Hygiene & Code Cleanup Utility

> [!NOTE]
> **Автономная периодическая чистка (Offline Periodic Maintenance Utility):**
> `rustok-doctor` предоставляет вспомогательную утилиту для периодической фоновой санитарной чистки кодовой базы.
> Он **НЕ** входит в обязательный протокол непрерывного аудита (`CONTINUOUS_CODE_REVIEW.md`, `continuous-review-ledger.md`) и **НЕ** заменяет глубокую архитектурную проверку и 6 Universal Logic Dimensions.
> Агенты, выполняющие директиву `реализуй план аудита`, выполняют проверку крейта по контракту аудита. Doctor предоставляет дополнительные данные для анализа.

Doctor проверяет исходный код Rust и находит кандидатов из матрицы антипаттернов для плановой чистки:

```bash
# Periodic domain-module maintenance sweep (e.g. monthly cleanup)
npm run verify:hygiene:strict

# Scan a specific domain module (smart resolution: forum -> crates/modules/rustok-forum)
cargo xtask doctor hygiene --crate forum

# Scan multiple crates at once
cargo xtask doctor hygiene --crate commerce,forum,index

# Scan staged Rust blobs directly from the Git index
cargo xtask doctor hygiene --staged --strict

# Scan all modified files in working tree
cargo xtask doctor hygiene --changed

# Scan changes relative to a base branch
cargo xtask doctor hygiene --diff main

# Filter by minimum severity (P0 = critical only, P1 = high+, P2 = all)
cargo xtask doctor hygiene --crate commerce --severity P0

# Display remediation guidance
cargo xtask doctor hygiene --crate commerce --suggest-fixes

# Export machine-readable JSON or GitHub-flavored markdown report artifact
cargo xtask doctor hygiene --crate commerce --markdown-report commerce_hygiene.md
cargo xtask doctor hygiene --crate commerce --json --fail-on P1

# Generate a patch that appends advisory comments to the scanned source
cargo xtask doctor hygiene --crate commerce --patch commerce_hygiene.patch

# Run regression tests for the scanner
npm run test:verify:hygiene
```

The scanner runs through the `rustok-doctor` library and `xtask`. Git modes require Git.
Severity (`P0` through `P2`) describes potential impact. The separate `confidence`
field is `high` for a direct source-level violation or `heuristic` for a candidate
that needs review. Text, JSON, CSV, and Markdown reports show both. `--severity`
filters findings by impact. `--strict` is equivalent to `--fail-on P2`: the scanner
returns `1` only for high-confidence findings at or above the threshold, in text
and JSON modes. The current high-confidence rule has severity `P1`, so
`--fail-on P0` does not block on any current rule. A target-selection or
source-read error returns `2`.

Git modes select files and scan their full contents. A pre-existing match in a
selected file can fail a gate even when the matching line was not edited.

The patch keeps flagged Rust statements intact and appends advisory comments with
their original line numbers. Check a working-tree patch with `git apply --check`
and a `--staged` patch with `git apply --cached --check`.

Rules use lexical scope and preserve literal contents for checks that need them.
They do not resolve Rust types, expand macros, or follow calls between functions.
The Leptos rule recognizes effect, memo, and `view!` delimiters. An empty report
means the scanner found no matches in the selected sources; verify behavioral
invariants with the owner’s tests and runtime evidence.

---

## Related Documents

- [RusToK Doctor](../../crates/utils/rustok-doctor/README.md) — Repository hygiene diagnostics
- [Coding Standards](./coding.md) — Rust syntax and styling rules
- [Engineering standards navigation](./patterns-vs-antipatterns.md) — Links to the owners of platform contracts
- [Continuous Code Review Protocol](../CONTINUOUS_CODE_REVIEW.md) — The audit workflow
- [Continuous Review Ledger](./continuous-review-ledger.md) — Living audit progress
- [AGENTS.md](../../AGENTS.md) — Canonical platform contract



## P-COMMAND-ADMISSION — Serialize receipt admission before snapshots

- **Category:** Idempotency and transaction concurrency.
- **Logic dimensions:** Order/concurrency, state transitions, side-effect completeness.
- **Observed defect:** Groups invitation/application writes queried receipts before
  waiting on child locks. Concurrent identical requests could both miss a receipt;
  the loser then validated changed lifecycle state instead of replaying success.
  On SQLite a read before writer reservation could also fail snapshot promotion.
- **Canonical solution:** Bound and normalize the tenant/key; acquire transaction
  admission before reads; replay the receipt; then lock the aggregate and children,
  authorize, mutate, append audit/outbox where required, and persist the receipt in
  the same transaction. PostgreSQL advisory-key hash collisions may serialize extra
  requests but must never replace exact receipt identity/hash comparison.

```rust
let transaction = database.begin().await?;
reserve_command(&transaction, tenant_id, &key).await?;
if let Some(result) = replay_receipt(&transaction, &key).await? {
    transaction.commit().await?;
    return Ok(result);
}
// Lock aggregate before children; authorize and write owner state and receipt.
transaction.commit().await?;
```

Detection: `rg -n 'begin\(\)|replay_receipt|reserve_command' crates/modules/rustok-groups/src`.
Review ordering in the actual compiled module graph; source markers do not prove
runtime admission or replay. Keep parallel same-key DB tests beside sequential replay.

## P-REPLAY-PRECONDITION — Preserve the original command revision

- **Category:** Idempotency and optimistic concurrency.
- **Logic dimensions:** State transitions and order/concurrency.
- **Observed defect:** Installer seed toggles reused a deterministic command key
  while reading the current aggregate revision again. After the first successful
  toggle the new revision changed the command fingerprint, so a retry conflicted
  with its own receipt. Blindly accepting the old receipt can also conceal a later
  opposite toggle.
- **Canonical solution:** Recover the immutable revision from the owner's exact
  receipt and verify all command identity fields; fail when historical evidence
  is absent. Handle a same-key admission racing the initial lookup. If the caller
  promises current state, check that state after replay and describe its locking
  limits instead of treating historical success as proof of present state.

```rust
let original = owner.original_toggle_revision(tenant, slug, enabled, &context).await?;
let revision = match original {
    Some(revision) => revision,
    None => owner.static_lifecycle_snapshot(tenant, slug).await?.revision,
};
// Submit with the original identity and revision; reconcile a concurrent admission.
```

Detection: `rg -n 'idempotency_key|expected_revision' crates/modules crates/utils`;
trace retries through the owning journal and test both repeated commands and a
later opposing transition.

## P-CONSUMER-GRAMMAR — Contain untrusted text with the consumer's grammar

- **Category:** Output containment and parser differentials.
- **Logic dimensions:** Preconditions and bounds, algorithmic correctness.
- **Observed defect:** Both storefronts wrapped landing CSS in `@scope (…) { … }` after
  scanning it with a hand-written approximation of CSS: a quote was "escaped" when
  the previous character was a backslash, `url(` was recognised by its neighbours,
  and Unicode whitespace was treated as CSS whitespace. A stylesheet could show the
  scanner one block structure and the browser another, so a `}` the scanner kept
  closed the wrapper and released the remaining rules onto the host page. The
  Leptos twin also returned any input starting with `@scope` unchanged, and the
  result could contain `</style` although it is emitted as `<style>` text.
- **Canonical solution:** Recognise tokens exactly where the consuming parser
  starts them (for CSS: comments, strings, escapes, names, hash/at-keyword/numeric
  tokens, `url()` tokens, `{}`/`()`/`[]` blocks). Make the output well-formed by
  construction: drop a closer that would end the wrapper, close what the input left
  open, and pass through only the function's own output. Escape for every grammar
  the result is embedded in (here CSS inside an HTML raw-text element). Upstream
  sanitization narrows the input; it does not replace containment at the boundary.

```rust
let scan = scan_blocks(&css, true); // tokenizer-accurate block structure
let mut completed = scan.text;      // stray top-level `}` already dropped
if !scan.open.is_empty() {
    completed.push('\n');           // backslash-newline is not an escape
    completed.extend(scan.open.iter().rev().map(|block| block.closer()));
}
```

Detection: `rg -n -F -e "[i - 1]" -e "starts_with(\"@scope\")" -e "inner_html" -e "dangerouslySetInnerHTML" crates apps`
lists previous-character checks, prefix-based pass-through and raw embedding
sites. A match is a place to read, not a defect: confirm which parser consumes
the text and compare the scanner with that parser's token rules.

## P-TWIN-VECTORS — Pin twin implementations with shared vectors

- **Category:** Leptos ↔ Next.js parity.
- **Logic dimensions:** Branch completeness, side-effect completeness.
- **Observed defect:** `scopeLandingCss` (Next.js) was hardened while its Rust twin
  `scope_landing_css` (Leptos) kept the original defects; each side had its own
  copy of the tests, so nothing failed. The Rust side had drifted further: the
  stylesheet was scoped to a class the host could replace, which left published
  landings unstyled in `rustok-pages-storefront`, and two of four surfaces emitted
  project CSS unscoped.
- **Canonical solution:** When the same pure function exists in Rust and
  TypeScript, keep one vector file under the owning module's `contracts/` and
  assert it from both test suites with exact expected output. Change both
  implementations in one change and add a vector for each behaviour change. Anchor
  a UI contract to something the component renders itself, never to a value the
  caller may override, and assert that every surface goes through the shared
  function.

```rust
let vectors: ParityVectors =
    serde_json::from_str(include_str!("../../contracts/landing-css-scope-parity.json"))?;
for case in vectors.cases {
    assert_eq!(scope_landing_css(&case.css, SELECTOR), wrap(SELECTOR, &case.inner));
}
```

Detection: `rg -n "byte-exact copy|Rust twin|must stay in sync" apps crates` lists
declared twins; for each, check that a shared fixture exists and that both suites
read it. Shared vectors prove agreement on the listed inputs only.

## P-RETRY-PROGRESS — A retry must make progress or reach its deadline

- **Category:** Retry loops and repeated side effects.
- **Logic dimensions:** Branch completeness, state transitions, order/concurrency.
- **Observed defect:** The draft-session token lock broke a stale lock with
  `let _ = fs::remove_file(..); continue;`. When the removal failed, the lock was
  found stale again on every pass and the `continue` skipped the deadline check: an
  unbounded busy loop on a request thread. In the Pages builder, a draft whose save
  had failed was saved again on every later intent, including ones that changed
  nothing, so a persistent failure answered each click with an error and a reload.
- **Canonical solution:** A branch that retries must either observe the state
  change that justifies the retry or fall through to the bound. Propagate the
  failure of the step the retry depends on. Key automatic retries to a change in
  the input that failed (here the document hash), and leave an explicit action as
  the way to retry unchanged input. Count a cleanup as done only when it happened.

```rust
if stale {
    remove_file_if_present(&lock_path)
        .map_err(|err| SsrDraftSessionError::Io(err.to_string()))?;
    continue; // sound only because the stale lock is gone
}
```

Detection: `rg -n -B3 "continue;" crates | rg "let _ ="` and
`rg -n "\.is_ok\(\)|let _ = .*remove|let _ = .*delete" crates` list retries and
counters that follow a discarded result. Read each loop for the path that reaches
its deadline.

## P-UNRECOGNIZED-TO-SINK — Input a reader does not recognise never reaches the sink

- **Category:** Output containment and fail-open fallbacks.
- **Logic dimensions:** Preconditions and bounds, branch completeness.
- **Observed defect:** The Next storefront fetched a landing artifact only when the
  page body announced one of two formats that no server path produces. Every real
  page took the last branch, which wrapped the received text in a `<div>` and
  rendered it with `dangerouslySetInnerHTML`. That text was the authoring document
  from GraphQL, so published landings showed their source instead of the page, and
  markup in an authoring text field reached the browser without the publish
  sanitizer. The artifact splitter had the same shape: when it found no `<body>` it
  returned the whole input as the body.
- **Canonical solution:** A reader accepts the one format its producer writes and
  rejects everything else with a typed error; it has no branch that returns the
  input unchanged. Markup reaches an HTML sink only from the reader's success
  path. Address the trusted resource by identity (page id and locale) instead of
  deciding from the content what it might be. Pin the reader to the producer with
  a test that compares the two definitions. Where the owner can expose the trusted
  value as a typed field, prefer that to any reader: the consumer then has one
  shape to render and an explicit absent state, and nothing to recognise.

```tsx
// `landing` is the typed published content; the authoring `body` is not selected.
const landing = page?.status === "published" ? page.landing : null;
if (!landing) {
  notFound();
}
```

Detection: `rg -n "dangerouslySetInnerHTML|inner_html" apps crates` lists the sinks;
for each, follow the value back and read every branch of its reader for one that
returns or wraps the input (`rg -n "return \{ html: " apps` finds the direct form).

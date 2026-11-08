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

This standard codifies systemic patterns and anti-patterns discovered across RusToK domain modules (such as `rustok-forum`, `rustok-commerce`, `rustok-marketplace`, `rustok-moderation`, and `rustok-auth`). It serves as the canonical reference for engineers and AI agents performing deep audits, refactoring, and feature implementation under [AGENTS.md](../../AGENTS.md) and [CONTINUOUS_CODE_REVIEW.md](../CONTINUOUS_CODE_REVIEW.md).

---

## 1. Core Philosophy: The 6 Universal Logic Dimensions

Superficial code cleanup (fixing only scanner warnings or regex hits) is prohibited by [AGENTS.md §2.1](../../AGENTS.md). High-signal code quality requires systematic verification across the **6 Universal Logic Dimensions**:

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
* **Pattern Solution:**
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
* **Problem:** Application-level validation is fragile. Migrations, raw SQL updates, or missed application branches can leave rows in corrupted states (e.g. `status = 'archived'` while `is_locked = false`, or `deleted_at IS NOT NULL` while `status = 'active'`).
* **Pattern Solution:**
  Make invalid states unrepresentable in PostgreSQL and SQLite using DDL `CHECK` constraints.
  ```sql
  -- Soft-delete coherence
  ALTER TABLE forum_topics ADD CONSTRAINT forum_topics_deleted_at_status_coherent
      CHECK ((deleted_at IS NULL AND status != 'deleted') OR (deleted_at IS NOT NULL AND status = 'deleted'));

  -- Terminal lock coherence
  ALTER TABLE forum_topics ADD CONSTRAINT forum_topics_archived_implies_locked
      CHECK (status != 'archived' OR is_locked = TRUE);
  ```
* **Application Across Domains:**
  * **`commerce` / `order`:**
    ```sql
    CHECK ((cancelled_at IS NULL AND status != 'cancelled') OR (cancelled_at IS NOT NULL AND status = 'cancelled'));
    CHECK (status NOT IN ('completed', 'cancelled') OR is_locked = TRUE);
    ```
  * **`marketplace-seller`:**
    ```sql
    CHECK (suspended_at IS NULL OR status = 'suspended');
    ```

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
  * Returning `Ok(())` on `PermissionScope::None` in RBAC evaluation. Missing scope must strictly resolve to `Err(Forbidden)`.

---

### Pattern 5: Bounded Identifier Entropy (`P-ID-ENTROPY`)
* **Category:** Calculation & Algorithmic Correctness
* **Problem:** 12-hex character short identifiers provide only 48 bits of entropy. At scale ($10^7$ records), the birthday paradox results in a ~1.7% collision rate, triggering unexpected `500 Conflict` errors for end-users.
* **Pattern Solution:**
  1. Standardize on 16-hex characters (`VARCHAR(16)`, 64 bits of entropy) for public routes and short identifiers.
  2. Implement an explicit deterministic collision-fallback resolution:
     ```rust
     pub const ROUTE_SHORT_ID_LEN: usize = 16;

     // Primary: lower hex string truncated to 16 chars
     // Fallback upon collision: deterministic HMAC/SHA256 slice
     ```
  3. Ensure database queries check with `LIMIT 2` to verify uniqueness unambiguously.

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
  3. If a variable is extracted (e.g. `target_entity_id`), verify its active existence rather than discarding it with `let _ = target_entity_id;`.
* **Anti-Pattern (Forbidden):**
  * Using `std::hint::black_box` in production service code to suppress unused variable or must-use warnings.

---

### Pattern 7: Module Tree Decoupling (`P-ZERO-INCLUDE`)
* **Category:** Compilation Hygiene & Zero AI Slop
* **Problem:** The `include!("...")` macro is abused to merge multiple files into one module to bypass circular dependencies or file-count limits. This breaks `rust-analyzer`, `goto-definition`, incremental compilation, and cargo documentation.
* **Pattern Solution:**
  1. Use standard Rust module hierarchies: `mod category; pub use category::*;`.
  2. Break circular dependencies by defining narrow traits (e.g. `LockProvider`, `AncestorLoader`) implemented by the caller or foundation crate, rather than text-including source files.
  3. Zero `include!()` directives in `src/` (excluding build-time `OUT_DIR` code generation).

---

### Pattern 8: Configuration & Settings Bounding (`P-CONFIG-BOUND`)
* **Category:** Preconditions & Anti-DoS
* **Problem:** Accepting tenant settings without mathematical and logical bounds validation allows operators or attackers to save impossible limits (e.g. `min_length = 20`, `max_length = 10`), causing immediate DoS on all subsequent write operations.
* **Pattern Solution:**
  Implement a mandatory `validate(&self) -> Result<(), DomainError>` method on all module settings DTOs:
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
  Invoke this validator in every `put_settings` / `update_settings` lifecycle handler.

---

### Pattern 9: Elimination of Dead Twins (`P-DEAD-TWIN`)
* **Category:** Maintenance Integrity & Zero AI Slop
* **Problem:** Keeping two parallel implementations of a capability (such as an active `posting_rate.rs` and an abandoned 2500-line `posting_policy.rs` prototype) causes new contributors and AI agents to waste effort fixing the wrong files.
* **Pattern Solution:**
  * Delete uncalled and dormant duplicates completely.
  * If a prototype is planned for future delivery, preserve it in an ADR or design specification, not as dead code inside `src/`.

---

### Pattern 10: Bounded Tree Projections (`P-BOUNDED-TREE`)
* **Category:** Algorithmic Correctness & Memory Safety
* **Problem:** Recursive hierarchy traversals (categories, reply threads, organization units) without depth limits or page limits cause Out-Of-Memory (OOM) crashes when catalogs grow.
* **Pattern Solution:**
  1. Enforce strict `MAX_TREE_NODES` bounds (e.g. 512 or 2048) in single-query projections.
  2. When hierarchy exceeds the bounded memory envelope, require depth-limited queries (`?depth=2`) or streaming traversals.

---

### Pattern 11: Transactional Batching & Anti-N+1 (`P-BATCH-QUERY`)
* **Category:** Algorithmic & Concurrency Performance
* **Problem:** Executing round-trip database queries inside iterative loops (`for id in ids { Entity::find_by_id(id).one(txn).await? }`) inside an open database transaction creates severe connection bottlenecks and 100–200 RTTs while holding row locks.
* **Pattern Solution:**
  1. Batch all IDs into an `is_in` set:
     ```rust
     let rows = Entity::find()
         .filter(Column::TenantId.eq(tenant_id))
         .filter(Column::Id.is_in(ids))
         .all(txn)
         .await?;
     ```
  2. Materialize rows into a `HashMap<Uuid, Entity>` in memory before iterating.

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
* **Problem:** Prematurely cloning inputs (`let canonical = input.name.clone();`) before running preconditions or validations wastes heap allocations on payloads that are immediately rejected.
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
  2. Fall back to secondary message token checks only when discriminating between multiple trigger branches sharing code `P0001`.

---

### Pattern 15: Contextual Observability Spans (`P-STRUCTURED-TRACE`)
* **Category:** Observability & Production Diagnostics
* **Problem:** Entrypoint and service methods that lack `#[tracing::instrument]` or omit tenant/user context produce uncorrelatable logs. When an authorization or storage error occurs, operators cannot determine which tenant, actor, or entity caused the failure.
* **Pattern Solution:**
  1. Decorate all public domain service methods with `#[instrument]`:
     ```rust
     #[tracing::instrument(
         skip(self, security, input),
         fields(tenant_id = %tenant_id, user_id = %security.user_id())
     )]
     pub async fn update_entity(&self, tenant_id: Uuid, security: SecurityContext, input: Input) -> DomainResult<Output> {
         // ...
     }
     ```
  2. Explicitly record span errors and durations on failure branches.

---

---

### Pattern 16: Transaction Rollback Swallowing & Advisory Lock Retention (`P-TXN-ROLLBACK-SWALLOW`)
* **Category:** Concurrency, Resource Leaks & Transaction Integrity
* **Problem:** Suppressing transaction rollback errors with `let _ = transaction.rollback().await;` causes serious concurrency anomalies. If rollback fails or is silently ignored, database locks (such as PostgreSQL session or transaction advisory locks `pg_advisory_xact_lock`) remain held until the connection is returned to the pool or dropped, leading to mysterious tenant-wide lock contention.
* **Pattern Solution:**
  1. Never suppress `rollback().await` with `let _ =`.
  2. Explicitly inspect the rollback result and log errors with contextual fields:
     ```rust
     if let Err(rollback_err) = transaction.rollback().await {
         tracing::warn!(%rollback_err, "failed to roll back transaction cleanly");
     }
     ```
  3. Where appropriate, rely on SeaORM's automatic `Drop` implementation to trigger rollback without writing an unhandled expression.

---

### Pattern 17: Synchronous Lock Guard Across Await Points (`P-SYNC-LOCK-AWAIT`)
* **Category:** Async Concurrency & Thread Pool Starvation
* **Problem:** Acquiring a synchronous mutex or read/write lock (`std::sync::Mutex`, `parking_lot::Mutex`, `std::sync::RwLock`) and holding its guard across an `.await` boundary blocks the underlying OS thread in Tokio's multi-threaded runtime. This can starve the worker pool, cause circular deadlocks between tasks, and violate `Send` bounds on futures.
* **Pattern Solution:**
  1. Use asynchronous locks (`tokio::sync::Mutex`, `tokio::sync::RwLock`) when data must be accessed across `.await` points.
  2. If using synchronous locks for in-memory structures, restrict the guard's lifetime to a narrow synchronous sub-block before awaiting:
     ```rust
     let snapshot = {
         let guard = self.cache.lock().unwrap();
         guard.clone()
     }; // guard is dropped here
     remote_client.send(snapshot).await?;
     ```

---

### Pattern 18: Unprepared SQL String Interpolation (`P-RAW-SQL-INTERP`)
* **Category:** Security, Preconditions & Injection Prevention
* **Problem:** Calling `execute_unprepared(&format!(...))` or formatting query strings directly bypasses parameterization. Even when arguments appear internal, string interpolation creates SQL injection vulnerabilities, breaks query caching, and corrupts query semantics when strings contain unescaped characters.
* **Pattern Solution:**
  1. Never format raw values into SQL strings.
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
* **Problem:** Calling `.unwrap_or_default()` or using default UUIDs (`Uuid::nil()`) on optional tenant context variables silently coerces unauthenticated or cross-tenant requests into the default tenant or zero UUID. This breaks tenant isolation and leaks shared catalog, order, or user state.
* **Pattern Solution:**
  1. Multi-tenant operations must fail closed:
     ```rust
     let tenant_id = tenant_id.ok_or(DomainError::TenantNotFound)?;
     ```
  2. Do not use dummy nil UUIDs as wildcard or fallback selectors.

---

### Pattern 20: Transactional Outbox Decoupling (`P-OUTBOX-DETACHED`)
* **Category:** Side-Effect Completeness & Atomicity
* **Problem:** Invoking `event_bus.publish()` directly within transactional business workflows creates a two-phase commit defect. If the subsequent database transaction fails or is aborted, the event was already published to the message broker, causing orphan consumer processing and data inconsistencies across microservices.
* **Pattern Solution:**
  1. In transactional scopes, always write outbox events through the transaction handle using `publish_in_tx`:
     ```rust
     event_bus.publish_in_tx(&txn, domain_event).await?;
     txn.commit().await?;
     ```
  2. The outbox relay worker guarantees that the event is dispatched only after the transaction successfully commits.

---

### Pattern 21: UI Reactive Signal Invalidation (`P-LEPTOS-DEAD-SIGNAL`)
* **Category:** Client Architecture & UI Reactivity
* **Problem:** Calling `let _ = signal.get();` outside of a reactive tracking context (such as `view!` or `create_effect`) reads the signal value without establishing a reactive subscriber dependency. Subsequent signal updates will not trigger UI rerendering, causing silent UI desynchronization.
* **Pattern Solution:**
  1. Bind signal dependencies inside `create_effect`:
     ```rust
     create_effect(move |_| {
         signal.get();
         // run dependent side-effect or data reload
     });
     ```
  2. In Leptos views, reference signals directly within view macros: `move || signal.get()`.

---

### Pattern 22: Lingering Incomplete Code & Work Markers (`P-UNFINISHED-CODE`)
* **Category:** Production Readiness & Maintenance Integrity
* **Problem:** Leaving `todo!()`, `FIXME`, or `TODO` markers in production service code risks runtime panics (for `todo!()`) or signals unhandled edge cases that bypass platform invariants.
* **Pattern Solution:**
  1. Completely implement domain logic, error variants, and edge-case handling before merging.
  2. If an architectural capability requires multi-phase rollout, track it formally in the owning ADR and `continuous-review-ledger.md` rather than scattering lingering inline stubs.

---

## 3. Summary Matrix

| ID | Pattern Name | Dimension | Detection / Verification |
|---|---|---|---|
| `P-SCOPE-GENERIC` | Scope-Generic Deduplication | Algorithmic / Structural | Multiple `insert_*_raw` or `load_*_layers` across services |
| `P-DB-CHECK` | DB-Enforced State Invariants | State Transitions | Look for status/deleted_at without SQL `CHECK` constraints |
| `P-LOCK-ORDER` | Lock-Before-Check Concurrency | Concurrency / TOCTOU | Check before `lock_` or `pg_advisory_xact_lock` |
| `P-FAIL-CLOSED` | Fail-Closed Security Floor | Bounds / Preconditions | `PermissionScope::None => Ok` or missing global authorization check |
| `P-ID-ENTROPY` | Bounded Identifier Entropy | Calculation Correctness | `VARCHAR(12)` or `[..12]` short ID patterns |
| `P-ANTI-SWALLOW` | Zero Error Swallowing | Branch Completeness | `let _ =` error suppression and `black_box` compiler abuse |
| `P-TXN-ROLLBACK-SWALLOW` | Transaction Rollback Swallowing | Concurrency & Leaks | `let _ = txn.rollback().await;` holding advisory locks until drop |
| `P-ZERO-INCLUDE` | Module Tree Decoupling | Code Quality | `include!("*.rs")` file-stitching macros |
| `P-CONFIG-BOUND` | Configuration Bounding | Preconditions / Anti-DoS | `put_settings` handler missing `.validate()` |
| `P-DEAD-TWIN` | Dead Twin Elimination | Maintenance Integrity | Module-level `#[allow(dead_code)]` or parallel redundant services |
| `P-BOUNDED-TREE` | Bounded Tree Projections | Memory Safety | Recursive query without depth cap or node limit |
| `P-BATCH-QUERY` | Transactional Batching | Performance / Concurrency | `.one(txn)` / `.all(txn)` queries executed inside loop bodies |
| `P-ZERO-PANIC-PROD` | Zero Runtime Panics | Preconditions / Reliability | `.expect()` / `.unwrap()` in production execution paths |
| `P-HOTPATH-ALLOC` | Allocation Hygiene | Performance / Memory | Input `.clone()` before validation gates in services |
| `P-SQLSTATE-DISCRIM` | SQLSTATE Error Mapping | Error Robustness | Raw message substring checks (`msg.contains`) instead of SQLSTATE |
| `P-STRUCTURED-TRACE` | Contextual Trace Spans | Observability | Missing `#[instrument]` with `%tenant_id` on mutation services |
| `P-SYNC-LOCK-AWAIT` | Sync Lock Across Await | Concurrency & Deadlocks | Synchronous lock guard held across `.await` in async code |
| `P-RAW-SQL-INTERP` | Unprepared SQL Interpolation | Security & Integrity | `execute_unprepared(&format!(...))` injection risk |
| `P-TENANT-FALLBACK-LEAK` | Tenant Fallback Leakage | Multi-Tenancy & Security | `tenant.unwrap_or_default()` breaking tenant isolation |
| `P-OUTBOX-DETACHED` | Transactional Outbox Decoupling | Side-Effect Completeness | `event_bus.publish()` without `_in_tx` in transactional contexts |
| `P-LEPTOS-DEAD-SIGNAL` | Reactive Signal Invalidation | UI Reactivity | `let _ = signal.get()` outside reactive contexts |
| `P-UNFINISHED-CODE` | Lingering Incomplete Markers | Production Readiness | `todo!()`, `FIXME`, or `TODO` markers in production code |

---

## 4. Automated Verification Scanner

The platform provides a dedicated automated verification scanner to audit any crate or module against these patterns:

```bash
# Scan a specific domain module (smart resolution: forum -> crates/modules/rustok-forum)
python scripts/verify/verify-domain-hygiene.py --crate forum

# Scan multiple crates at once
python scripts/verify/verify-domain-hygiene.py --crate commerce,forum,index

# Scan git staged files (instant pre-commit check)
python scripts/verify/verify-domain-hygiene.py --staged

# Scan all modified files in working tree
python scripts/verify/verify-domain-hygiene.py --changed

# Scan changes relative to a base branch
python scripts/verify/verify-domain-hygiene.py --diff main

# Filter by minimum severity (P0 = critical only, P1 = high+, P2 = all)
python scripts/verify/verify-domain-hygiene.py --crate commerce --severity P0

# Display ready-to-copy code remediation suggestions
python scripts/verify/verify-domain-hygiene.py --crate commerce --suggest-fixes

# Export machine-readable JSON or GitHub-flavored markdown report artifact
python scripts/verify/verify-domain-hygiene.py --crate commerce --markdown-report commerce_hygiene.md
python scripts/verify/verify-domain-hygiene.py --crate commerce --json

# Enforce in CI / pre-commit (exits with code 1 if issues are detected)
npm run verify:hygiene:strict
```

The scanner runs out-of-the-box using the Python 3 standard library without third-party dependencies, making it universally available in developer terminals and CI runners.

---

## Related Documents

- [Automated Domain Hygiene Scanner](../../scripts/verify/verify-domain-hygiene.py) — Verification script
- [Coding Standards](./coding.md) — Rust syntax and styling rules
- [Patterns vs Antipatterns](./patterns-vs-antipatterns.md) — Platform-wide architectural DOs and DON'Ts
- [Continuous Code Review Protocol](../CONTINUOUS_CODE_REVIEW.md) — The audit workflow
- [Continuous Review Ledger](./continuous-review-ledger.md) — Living audit progress
- [AGENTS.md](../../AGENTS.md) — Canonical platform contract


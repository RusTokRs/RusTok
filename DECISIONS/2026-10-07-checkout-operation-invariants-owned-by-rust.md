# Checkout operation invariants are owned by typed Rust, not database triggers

- Date: 2026-10-07
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-commerce (checkout operation journal); rustok-payment (payment provider execution)
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

`checkout_operations` is the single durable journal of the staged storefront checkout. One row tracks
a cart through `pending → executing → completed`, or through the compensation branch
`compensation_required → compensating → compensated | failed`, plus the operational park
`reconciliation_required`. The partial unique index `ux_checkout_operations_active_cart` allows at
most one active operation per cart, so the journal is also the cart's checkout lock.

Historically the row's business rules were enforced by database triggers instead of by the owning
Rust service:

- `m20260713_000009` installed the PostgreSQL function `enforce_checkout_operation_integrity()` with
  the `checkout_operations_integrity_guard` trigger (identity-column immutability, cross-row tenant
  lookups against `carts`, `orders`, `payment_collections`, and a status transition whitelist), plus
  the SQLite twins `checkout_operations_guard_insert` / `checkout_operations_guard_update` and a
  MySQL parking trigger.
- `m20260713_000017` re-installed the guard with an extended matrix and an `AFTER UPDATE` rewrite
  that silently moved a row from `compensation_required` to `reconciliation_required` whenever
  `last_error_code` was `checkout.compensation_manual_reconciliation`.
- `m20260713_000016` installed `payment_provider_operations_checkout_guard`, which raises an error
  when a payment provider operation is claimed while its checkout operation is being compensated.

The cost of that split ownership was concrete (deep e-commerce review, findings RECON-01 and
ECOM-DB-01):

- the Rust status enum did not contain `reconciliation_required` although the trigger could write it,
  so `CheckoutOperationJournal::get` parsed the row back as a validation error and the parked cart
  surfaced as a raw unique-index failure;
- the guard's transition whitelist had no exit from `reconciliation_required`, so a parked operation
  could never be closed and its cart stayed locked forever;
- one state machine existed in four vocabularies (PostgreSQL, SQLite, MySQL, Rust) which had already
  drifted;
- the rules were not testable without a database, and the parking side effect was invisible to the
  service that caused it.

`AGENTS.md` §196-197 already states the contract: "Multi-row domain rules, state transition
validations, workflow guards, and cross-aggregate invariants MUST be owned and validated by typed
Rust domain entities and services inside a transaction boundary. Do NOT implement complex business
rules, cascading side-effects, or cross-row business validations inside PL/pgSQL constraint
triggers."

## Decision

1. `CheckoutOperationJournal` in `rustok-commerce` is the single owner of the checkout operation
   state machine. The transition matrix is `CheckoutOperationStatus::allowed_transitions()`, and
   every status write passes through `CheckedTransition::new`, which rejects a transition the matrix
   does not allow with a typed `CheckoutOperationError::Conflict` before the row is touched.
2. The parking decision is Rust policy: `compensation_next_status(error_code)` maps the
   manual-reconciliation code to `reconciliation_required` (with `completed_at` set) and every other
   code to `compensation_required`.
3. The completion shape is Rust policy: `requires_completed_at()` defines which statuses carry
   `completed_at`; `release_lease_with_error` and `mark_terminal` write it accordingly.
4. Cross-aggregate tenant checks run in Rust before the journal changes state: `ensure_cart_tenant`,
   `ensure_order_tenant`, and `ensure_payment_collection_tenant` query the owning entities with the
   tenant in the predicate.
5. The database keeps only declarative schema shape: column types, `NOT NULL`, enumeration `CHECK`
   constraints, the completion-shape `CHECK`, and the `ux_checkout_operations_active_cart` unique
   index. Behavioral triggers on `checkout_operations` are removed.
6. The same rule applies to provider blocking: `payment_provider_operations_checkout_guard` is
   superseded by a Rust check in the payment provider execution path owned by `rustok-payment`.
   Delivered on 2026-10-07 (see Progress and "Remaining cutover" below): the admission contract runs in
   typed Rust in both owners and `m20261007_000013` removes the trigger with the same
   `up`-drops / `down`-restores pattern as `m20261007_000010`.
7. Migration `m20261007_000010_move_checkout_operation_guards_to_rust` implements items 1-5, parks
   stale manual-reconciliation rows, and restores the exact `m20260713_000017` guard text in
   `down()` so the change is reversible.

## Sources of truth and ownership

- `checkout_operations` (the journal) is owned by `rustok-commerce`; it is authoritative for
  lifecycle status, stage, lease, last error, and settlement timestamps.
- `checkout_reconciliation_actions` is owned by `rustok-commerce`; it is the append-only operator
  evidence for parked operations.
- Payment collections, refunds, and provider operations are owned by `rustok-payment`.
  `rustok-commerce` reaches them only through owner ports (`PaymentAdminReadPort`,
  `PaymentAdminCollectionCommandPort`, `PaymentAdminRefundCommandPort`, `PaymentProviderRegistry`).
- Migrations own schema shape only; they do not own behavior. There is no stored procedure or
  trigger carrying business rules for this aggregate.
- Dependency direction is `rustok-commerce → rustok-payment`, never the reverse.

## Invariants

- At most one active operation per cart (`ux_checkout_operations_active_cart`).
- `begin` is idempotent on `(tenant_id, cart_id, idempotency_key)` with `request_hash` and
  `snapshot_hash` equality; the same key with a different payload is a conflict.
- Status and stage are closed vocabularies (9 statuses, 10 stages) enforced by `CHECK` constraints
  and mirrored by Rust enums.
- Identity columns (`id`, `tenant_id`, `cart_id`, `idempotency_key`, `request_hash`) are never
  updated after insert; Rust has no writer for them.
- Lease fields are only meaningful while `executing` or `compensating`; every writer clears them on
  exit.
- `completed_at` is set exactly for `reconciliation_required`, `completed`, `compensated`, and
  `failed`.

### Allowed states

- `pending → executing`
- `executing → retryable_error | compensation_required | reconciliation_required | completed | failed`
- `retryable_error → executing`
- `compensation_required → compensating | reconciliation_required`
- `compensating → compensation_required | reconciliation_required | compensated | failed`
- `reconciliation_required → compensated | failed | compensation_required`
- `completed`, `compensated`, `failed` are terminal.

### Forbidden states

- Any transition outside the matrix, for example `executing → compensated`, `completed → failed`, or
  `compensation_required → completed`.
- A checkpoint (stage advance) on a non-`executing` row or from a lease that is not the holder.
- Rewriting identity columns.
- Claiming or scanning a parked `reconciliation_required` operation from the compensation sweep.
- A settled or parked row without `completed_at`, or an active row with it set.

## Non-goals

- Money movement semantics, refund approval limits, and PSP capabilities (payment owner).
- Transport contracts beyond the admin checkout-operation and reconciliation endpoints.
- Event publication for the checkout journal (finding ECOM-EVT-01 requires its own ADR).
- Reconciliation policy-as-data (tenant-configured policies) — a later ADR.
- MySQL parity: MySQL is not a deployed backend for this deployment. Its migration branch stays
  syntactically valid but carries no behavior.

## Data, transaction, and concurrency boundary

- Database-enforced: column types, `NOT NULL`, status/stage `CHECK` constraints, the
  completion-shape `CHECK`, and the partial unique index. No triggers and no stored procedures.
- Transaction boundary: each transition is one single-row
  `UPDATE ... WHERE tenant_id = ? AND id = ? AND status = ? AND lease_owner = ? AND lease_expires_at > now`
  statement (compare-and-set). `rows_affected == 0` is a typed conflict, never a silent no-op.
- Revision/optimistic concurrency: the status plus lease predicate is the revision check; there is
  no separate `version` column.
- Idempotency/retry: `begin` is keyed by the request hash; reconciliation actions are keyed by
  `(tenant, operation, idempotency_key)` with a request hash, and a replay returns the recorded
  action; the refund creation key given to the payment owner derives from that key, so the PSP call
  is idempotent across retries; the compensation sweep re-claims by lease expiry only.
- Concurrent duplicates: when two requests with the same key pass the lookup before either closes the
  operation, the payment owner still moves money once (shared creation key), the compare-and-set admits
  one close, and that close writes the decision row in the same transaction — so the loser either reads
  the winner's row and answers as a replay, or (when the winner used a different key) sees
  `CloseAfterMoneyMoved` with the refund id and attests the outcome by hand. "Money moved but the
  operation could not be closed" therefore always means "nothing was recorded for this attempt", never
  "the record is missing while the operation is closed".
- Destructive operations: none. Both journals are append-only and no `DELETE` path exists.

## Context dimensions

- Tenant: mandatory and non-nil on every row, query, and cross-aggregate check.
- Actor: operators are recorded on the action row and in the resolution error message.
- Currency: carried on money-bearing actions and validated against the payment collection.
- Time: UTC (`DateTime<FixedOffset>`). No locale or timezone dimension participates.
- Channel, locale, and policy: not part of this decision.

## Events and projections

- `checkout_operations` publishes its park/close family through the transactional outbox:
  `checkout.operation.parked` (`operation_id`, `cart_id`, bounded `reason`) and
  `checkout.operation.reconciled` (`operation_id`, `cart_id`, bounded `outcome`, `operator_id`),
  schema version 1, defined in `crates/libs/rustok-events/src/checkout_operation.rs`.
- The family is published with `TransactionalEventBus::publish_contract_in_tx` **inside** the
  transaction that writes `checkout_operations.status`, so a consumer can never observe a status
  transition without its event and can never be told about a transition that was rolled back. The
  four writers are `release_lease_with_error` (manual-reconciliation park),
  `park_exhausted_compensation` (attempt cap), `resolve_reconciliation_required` (compensated /
  failed) and `request_compensation_retry` (`compensation_required`); the
  `rustok_checkout_reconciliation_parked_total` sample is recorded only after the commit.
- The bounded park vocabulary (`manual_reconciliation`, `attempts_exhausted`) is defined once, in
  `rustok-events`, and is both the metric label and the event payload label, so the two cannot drift
  apart (before this change the journal's local constant published `manual` to the metric while the
  contract and the documentation said `manual_reconciliation`).
- A resolution or retry records `last_error_code` (`checkout.reconciliation_resolved`,
  `checkout.reconciliation_retry_requested`) so an operator can audit from the row alone.
- The journal is still authoritative: the events are a projection for consumers, never a substitute
  for the row or the append-only action journal.
- Payment and refund events remain payment-owned; if they are introduced (ECOM-EVT-01) they must be
  published in the owner's transaction, not by the checkout journal.

## Failure semantics

- An unknown status or stage string read back from a row is a `Validation` error (fail closed),
  never a default.
- A CAS loss, an expired lease, or a wrong lease holder is a `Conflict` with the current row read
  back for diagnostics.
- Database errors surface as a typed `Database` error; the service does not retry internally.
- A parked operation keeps its cart blocked (fail closed) until an operator executes a
  reconciliation action; the sweep must not retry it.
- When the payment owner rejects a money step, no action row is appended and the operation stays
  parked; the operator sees the owner's error.

## Migration and cutover

- `m20261007_000010_move_checkout_operation_guards_to_rust`:
  - drops `checkout_operations_integrity_guard` and `enforce_checkout_operation_integrity()` in
    PostgreSQL, the SQLite guards, and the MySQL parking trigger;
  - parks leftover `compensation_required` rows carrying the manual reconciliation code so no cart
    remains swept forever;
  - `down()` restores the exact `m20260713_000017` text, including the parking trigger, so the
    change is reversible for verification.
- `m20261007_000012_add_checkout_operation_admission` adds the admission record of the checkout
  journal: `execution_admission` (`open` / `settling` / `closed`) and `admission_epoch`, backfilled by
  status with the PostgreSQL/MySQL `CHECK` shape.
- `m20261007_000013_drop_provider_execution_checkout_guard` removes
  `payment_provider_operations_checkout_guard` (`m20260713_000016`) and its PL/pgSQL function; its
  `down()` restores the exact text of that migration, so the change is reversible for verification.
  The Rust claim gate is now the only enforcement point of provider execution admission.
- Zero legacy: no wrapper service, no dual write, and no feature flag. Schema and writer set ship in
  the same release; roll forward is preferred, `down()` exists for verification and incident use.
- The module documentation and the audit remediation log point at this ADR as the active decision
  for the boundary.

### Remaining cutover: provider execution admission (target shape)

`payment_provider_operations_checkout_guard` (`m20260713_000016`) is the last business rule of this
contour still owned by the database, and it is cross-module: `rustok-payment` reads a projection of
`rustok-commerce` state. Replacing it is therefore not a mechanical port but a small contract. The
target shape is:

1. **Admission state on the checkout journal.** The checkout operation owns an explicit, versioned
   admission record: `execution_admission` (`open` / `settling` / `closed`) plus a monotonically
   increasing `admission_epoch`. Entering `compensation_required`, `compensating` or
   `reconciliation_required` moves admission to `settling` and increments the epoch; closing moves it
   to `closed`; a fresh operation for the same cart starts a new epoch.
2. **Epoch-carrying provider operations.** Every provider operation that belongs to a checkout records
   the epoch it was admitted under at creation. Claiming it for execution becomes one conditional
   write (`status = 'executing'` only while admission is `open` and the stored epoch matches), so the
   check and the transition happen in the same statement: no check-then-act window, and a concurrent
   park invalidates in-flight claims instead of racing them.
3. **Derived projection with a reconciliation path.** Payment keeps the admission/epoch it needs
   locally — written through a typed owner command or pulled through a narrow port before the claim,
   whichever keeps the checkout journal the single source of truth. Derived state requires a
   rebuild/reconcile path (diagnostic query plus an admin resync action), so drift is detectable
   instead of invisible.
4. **Effect-kind policy.** The trigger blocks every provider execution. The target contract separates
   *extending* effects (`authorize`, `capture`, new charges) — refused while admission is `settling` —
   from *unwinding* effects (`cancel`, `void`, refund completion), which stay allowed because
   compensation itself may need the provider.
5. **Fail-closed, observable refusals.** A missing or unknown admission, or an epoch mismatch, refuses
   the claim and records a bounded reason on the provider operation plus a metric; today a trigger
   exception leaves no trace beyond the rejected statement.
6. **One enforcement point.** Once the Rust gate is live on every execution path, the trigger and its
   migration are deleted with the same `up`-drops / `down`-restores pattern as
   `m20261007_000010`. Two enforcement points are not kept "for defence in depth": the drift that
   produced RECON-01 came exactly from duplicated state machines.

Sequencing: this section (accepted target) → the admission event/port contract together with
ECOM-EVT-01 → the Rust gate and the epoch column migration → delete the trigger. The sequence
completed on 2026-10-07: the checkout-owned half shipped in the fourth pass and the payment half plus
the trigger removal in the fifth pass, both described below.

**The last database business rule of this contour, removed in the sixth pass**:
`m20260713_000015_bind_checkout_payment_collections` installed
`bind_checkout_payment_collection()` plus the `payment_collections_bind_checkout_operation*` triggers,
which validated the collection's checkout identity and *wrote* `checkout_operations.payment_collection_id`
from inside a trigger on `payment_collections` — the same class of object this decision removed twice, and
the second writer of a column this journal owns. Both of its rules already existed in Rust, strictly
wider: `validate_collection` in the payment stage checks tenant, cart, order, customer, currency, amount
and the `checkout.operation_id` metadata before any provider execution, and `checkpoint` writes the
binding — and, since this pass, keeps it write-once, the one part of the guard that had no Rust
equivalent: the conditional write accepts an unbound operation or re-asserts the collection it already
carries and refuses to re-point it at another one, because provider operations, marketplace financial
rows and refunds are keyed by that column. What kept the trigger alive was an unsafe ordering: while the park-time fence read
`operation.payment_collection_id`, the window between the collection being created and the payment stage
binding it left the fence with nothing to stamp, and the trigger was what guaranteed that a checkout with
provider operations always had the binding before it could park. The sixth pass closed that window on the
fence side instead — the fence is now scoped to the cart (below) — and dropped the trigger with the
`up`-drops / `down`-restores pattern of `m20261007_000010` in
`m20261007_000015_drop_payment_collection_binding_trigger`.

Progress on that sequencing (2026-10-07, fourth pass): the checkout-owned half of the **admission
contract** is delivered. `checkout_operations.execution_admission` (`open` / `settling` / `closed`) and
`admission_epoch` are added by `m20261007_000012_add_checkout_operation_admission` (backfill by status,
PostgreSQL/MySQL `CHECK` shape); the typed `CheckoutExecutionAdmission` level is derived from the status
machine by `for_status` and written by all seven status writers inside the same conditional update as
the status, with the generation moving exactly when the level moves; `begin` starts the next generation
for the cart; `checkout.operation.admission_changed` is published in the writer's transaction by
`begin`, `release_lease_with_error`, `mark_terminal` and `resolve_reconciliation_required`; the admin
operation response exposes the level and the generation; and
`scripts/verify/verify-checkout-execution-admission-contract.mjs` verifies the ownership, the writer
coverage and the publication order. Porting the trigger also exposed two defects that the Rust contract
fixes on the owner side: the blocked status set omitted `completed` (a provider operation could still
start after a successful checkout) and it blocked the `cancel`/`refund` unwinding operations the
compensation itself needs, forcing manual reconciliation (ECOM-ADM-01/ECOM-ADM-02 in the audit).
Still open after the fourth pass were the payment-side half and the trigger removal; both shipped in
the fifth pass.

Progress on that sequencing (2026-10-07, fifth pass): the **payment half** is delivered, and with it
the trigger. Payment reads the admission through the narrow owner port
`rustok_payment::CheckoutExecutionAdmissionPort`, implemented by `rustok-commerce`
(`services/checkout_execution_admission.rs`) over a tenant-scoped read of `checkout_operations`. That
is the branch of item 3 that keeps the checkout journal the single source of truth — chosen over a
payment-side copy precisely because a copy needs a rebuild path and can drift, which is the failure
mode this cutover exists to remove: there is no derived state, so there is nothing to resync.

`payment_provider_operations` gains `admission_epoch` and the bounded refusal columns
(`m20261007_000122_add_provider_operation_admission`, with the PostgreSQL/MySQL `CHECK` on the refusal
vocabulary); `begin` records the generation the operation is created under, and
`PaymentProviderOperationJournal::claim_execution` decides an extending claim with one conditional
write: `status = 'executing'` only while the owner reports `open` and the stored generation still
matches the observed one. A pre-contract row (`admission_epoch = 0`) is adopted into the observed
generation only while the level is `open` — the guard's own behaviour for such rows. Unwinding effects
(`cancel`, `refund`) are admitted while `settling`/`closed` and never read the admission at all, so
the compensation can no longer be trapped by the blanket refusal (ECOM-ADM-02). Every refusal records
a bounded `admission_refusal_code`
(`checkout_admission_settling|closed|unavailable|epoch_mismatch|effect_unknown`), increments
`rustok_payment_provider_execution_admission_refused_total{operation,reason}` — both labels bounded:
the refusal vocabulary above and the effect class the gate decided on (`extending` / `unwinding` /
`unknown`, so a foreign operation string cannot turn the label into one time series per string) — and
surfaces to the caller through `execution_admission_refusal_error`, so a refusal is never a silent
`None`.

A park publishes the new level and, in the same transaction, stamps the new generation on the cart's
non-terminal provider operations (`invalidate_provider_execution_admitted_by` →
`stamp_admission_epoch_for_cart`): a claim that was decided under the previous generation fails its own
conditional write instead of racing the park, and a claim that already committed is ordered before it.
The scope was the bound collection until the sixth pass; the cart is the smallest scope that is
guaranteed to hold every claim the park must invalidate, because at most one collection per cart is
active (`ux_payment_collections_active_cart`) and the checkout journal admits one live operation per cart
(`ux_checkout_operations_active_cart`), while the binding column the fence used to read is written only
later, by `checkpoint`.
Operations already `executing` keep the generation they were admitted under, because their invocation
is already with the provider.

`m20261007_000013_drop_provider_execution_checkout_guard` deletes the trigger and its PL/pgSQL
function with the `up`-drops / `down`-restores pattern of `m20261007_000010`, and
`scripts/verify/verify-checkout-execution-admission-contract.mjs` now checks both halves: the owner
port and its tenant scope, the claim gate and its refusal vocabulary, the metric, the park-time stamp,
the wiring of every production construction site of the payment execution ports, and the removal of
the trigger. `AdminCheckoutOperationResponse` keeps exposing the level as the bounded string the column
stores, next to `status` and `stage`, which are exposed the same way: the vocabulary is owned by the
typed `CheckoutExecutionAdmission` enum and enforced by the database `CHECK`, and turning that one
response field into an OpenAPI enum would be an API-contract change for a payload the admin UI already
receives as a bounded label (recorded as closed by decision in the audit's tail pass). Still open in this
pass: the maintainer's `cargo fmt` / `cargo check` / `cargo test` run.

Progress on that sequencing (2026-10-07, sixth pass): the **binding guard** is removed and the last
database business rule of the checkout contour with it.
`m20260713_000015_bind_checkout_payment_collections` is not edited (an applied migration is immutable);
`m20261007_000015_drop_payment_collection_binding_trigger` drops the `payment_collections` triggers and
`bind_checkout_payment_collection()` on PostgreSQL, SQLite and MySQL — one statement per
`execute_unprepared` call, because the SQLite and MySQL drivers prepare a single statement at a time —
and its `down()` restores the exact text of the original migration, verified byte-identical against
`m20260713_000015`. The migration is registered in `rustok-commerce`'s migration list with explicit
dependencies on the table owner and on the migration it supersedes.

Removing it needed no new ordering for the binding, because the fence stopped reading it. The park-time
invalidation is now `invalidate_provider_execution_admitted_by` →
`PaymentProviderOperationJournal::stamp_admission_epoch_for_cart`: the payment journal resolves the
collections of the operation's cart (tenant-scoped, `payment_collections.tenant_id` + `cart_id`) and
stamps their non-terminal (`pending`, `provider_error`) operations in the transition's own transaction.
The parameter is the cart, not the collection, so the window the trigger used to cover — a park landing
after the collection exists but before `checkpoint` writes the binding — now stamps the claims that
would otherwise have raced the park, and the early return that silently skipped stamping while no
collection was bound is gone. The scope is exact rather than merely wider: a provider operation is
created by the payment stage from a collection of the checkout's cart, `ux_payment_collections_active_cart`
(`m20260713_000106`) admits one active collection per cart, and `ux_checkout_operations_active_cart`
admits one live checkout per cart, so the cart holds every claim that can still reach this checkout and
no foreign tenant's operations are touched (`tenant_id` is a filter, not a decoration). `payment_collection_id`
keeps a single writer — `checkpoint` — and its two readers (the admin response and the payment
metadata-based link resolution) never needed the trigger.

`CheckoutOperationJournal::checkpoint` also absorbed the last rule the guard owned: the operation's
binding is write-once. The checkpoint carries the "unbound or the same collection" predicate in the same
conditional update that writes the collection, and a zero-row update whose cause is a different
collection returns a bounded `Conflict` naming the write-once binding instead of a generic CAS failure.
That keeps the money evidence attached to the checkout it belongs to: a re-pointed binding would detach
provider operations, marketplace financial rows and refunds from their operation.

The same window is closed on the compensation side. A park landing before `checkpoint` writes the
binding reaches the payment compensation with a null `collection_id`, and the owner used to answer
`Ok(None)` — "nothing recorded" — leaving the `pending` collection behind. The request now carries the
checkout cart, exactly as the order compensation request always has, and
`PaymentService::find_collection_by_cart_checkout_operation` resolves the attempt's collection from
`metadata.checkout.operation_id` — the same link the claim gate reads — scoped to the tenant and the
cart and without a status filter, because a collection this checkout already cancelled is one of the
states the compensation has to confirm. A `pending` collection is therefore cancelled, a `cancelled`
one is confirmed, a `captured` one still goes to manual reconciliation, and `Ok(None)` now means the
cart holds no collection of that operation at all; the commerce wrapper compares the returned snapshot
with the binding only when the journal recorded one. The integration tests in
`crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs` cover the cancel and the
refusal to adopt another checkout's collection. One already-failing verifier
(`verify-payment-checkout-compensation-local-context`) pins the replaced owner shape as "preserved
owner behavior" and therefore joins the ECOM-VERIFY-01 re-pin batch.

`scripts/verify/verify-checkout-execution-admission-contract.mjs` now pins the new contract instead of
the old one: the cart-scoped stamp signature and its resolution bounds, the fence passing `operation.cart_id`
(and a `forbidText` on the binding-parameter form), the drop statements of the new migration plus the
restoration of the exact original text in `down()`, the write-once binding pins on `checkpoint`, and the
journal's own doc statement of why it does not read the binding. The verifier also forbids `checkout_operations` anywhere in the payment journal, so the
payment half may not name the checkout table even in prose. The regression this contract exists for is
covered by integration tests in `crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs`:
`park_time_fence_reaches_provider_operations_before_the_collection_is_bound` creates a collection for the
cart without binding it, parks the checkout and asserts that the collection's pending provider operation
carries the new generation and is refused, while `checkout_operations.payment_collection_id` stays null;
the reconciliation smoke test's rebind block checkpoints another collection of the same tenant onto the
bound operation and asserts the bounded refusal plus an unchanged binding and stage.
Still open after the sixth pass: the maintainer's `cargo fmt` / `cargo check` / `cargo test` run, and the
`migration-infra-approved` label the repository asks for on migration changes.

Progress on the event sequencing (2026-10-07): the checkout-owned half of the event contour is delivered —
the `checkout.operation.parked` / `checkout.operation.reconciled` family, its in-transaction
publication from the four journal writers, the shared bounded park vocabulary and
`scripts/verify/verify-checkout-operation-event-contract.mjs` (wired into the ecommerce-hardening
workflow), with the audit remediation row 25 recording the change. All three of the items this
paragraph listed as open have since shipped: the admission contract and the epoch column in the
fourth pass, the `rustok-payment` gate with the `m20260713_000016` guard removal in the fifth pass
(`m20261007_000013`), and the payment-collection binding guard in the sixth pass
(`m20261007_000015_drop_payment_collection_binding_trigger`). Adding the
family changed the canonical event contract, so
`crates/libs/rustok-events/contracts/event-contract-digests.json` must be regenerated by the
maintainer with `cargo run --locked -p rustok-events --example event_contract_digests -- --write`
before the digest gate is green again.

## Alternatives considered

1. Keep the database guards and extend the matrix with the missing exit edge (the first remediation
   attempt for RECON-01). Rejected: it re-states one state machine in three SQL dialects, cannot be
   unit-tested, hides side effects from the service, and caused RECON-01 in the first place;
   `AGENTS.md` §196-197 forbids it.
2. Keep the guards as defence in depth while adding Rust checks. Rejected: two sources of truth for
   one machine, with silent drift — exactly how the Rust enum lost `reconciliation_required`; a
   fix would have to be repeated in four places.
3. Drop the column constraints as well and rely only on Rust. Rejected: enumeration `CHECK`s and the
   active-cart uniqueness are declarative schema shape, portable, cheap, and do not encode workflow.
4. Move the active-cart lock to a Rust-only advisory lock. Rejected: the partial unique index is
   storage integrity, composes with the CAS update, and survives process crashes.
5. Adopt the Medusa v2 shape wholesale (a generic workflow/saga engine owns the sequence and its
   compensations, payment ambiguity is resolved by provider webhooks, and unresolved cases surface as
   a single order-level `requires_action` flag). The full comparison is in
   `docs/audits/ecommerce-deep-review-2026-10-07.md` (Appendix B). Rejected as the primary design
   because three requirements here have no equivalent there: a permanent, tenant-scoped audit of who
   moved money and on what evidence (Medusa's workflow executions are garbage-collected after
   `retentionTime`, and `completeCartWorkflow` is declared `idempotent: false`), money-path events
   published inside the writer's transaction, and a cross-module admission rule that the engine cannot
   host because the two modules must not read each other's tables. The borrowable parts (per-step
   retry scheduling, one "temporary failure" plus a manual retry step, a single human-needed flag)
   belong to increments 3-4, not to this decision.

## Verification

- `npm run verify:adrs` validates this registry entry and metadata.
- `cargo test -p rustok-commerce` covers `CheckedTransition`, `compensation_next_status`,
  fingerprints, and reconciliation-service validation next to the code.
- The migration smoke test `crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs`
  must observe: parking produced by `mark_compensation_required` / `mark_compensation_retryable`, a
  provider claim still blocked while the operation is compensating, and a zero scan for parked rows.
- Schema evidence: after `up()` no trigger remains on `checkout_operations` (PostgreSQL and SQLite);
  after `down()` the `m20260713_000017` text is restored verbatim.
- Grep gates: no migration outside the guard migrations creates a trigger on `checkout_operations`,
  and no code path updates the identity columns.
- Executed in the review environment: the 292 source verifiers matching `checkout`, `compensation`,
  `reconciliation`, `refund`, `payment`, `fulfillment`, `pricing`, `cart`, `order`, `commerce`,
  `recovery`, `staged`, `telemetry` and `metric` were run against this tree and against a clean export
  of the base commit (`git archive a53a4dd`): every script reports the same failure count on both
  trees, except `verify-commerce-admin-checkout-operation-diagnostic-safety.mjs`, which reports one
  fewer failure here (41 -> 40). Two regressions the sweep caught in the payment provider execution
  path (an unbounded `operation_id` log field and a fifth `PaymentError` fact extraction, both added
  by an earlier pass of this change set and both rejected by the encoding/diagnostic-safety contracts)
  were fixed, so the sweep is green relative to the baseline rather than merely different.
- Not executed in the review environment (no Rust toolchain or database): compilation, unit tests,
  and migrations must be run by the maintainer before merge; see
  `docs/audits/ecommerce-deep-review-2026-10-07.md` for the verification statement.

## Consequences

- One vocabulary and one testable owner for the state machine. Changing a transition is now a Rust
  change plus, at most, a schema-shape migration.
- The MySQL branch of the guard disappears; a future MySQL deployment would rely on the Rust writer
  set alone.
- Slightly less defence in depth: a future direct SQL writer could bypass the matrix. Mitigations:
  the table has a single owner service, every write is CAS-filtered, and the verification gates
  above assert the absence of other writers.
- Follow-up work: the provider-blocking trigger move, the policy-as-data ADR, the payment/refund
  event ADR (ECOM-EVT-01), and the correlation-safe diagnostic mapper cleanup.

## Operational signals

Parking and operator work are observable through two bounded-cardinality counters (no tenant label,
per [`DECISIONS/2026-09-27-bounded-tenant-metric-cardinality.md`](./2026-09-27-bounded-tenant-metric-cardinality.md)):

| Metric | Labels | Meaning |
| --- | --- | --- |
| `rustok_checkout_reconciliation_parked_total` | `reason` in {`manual_reconciliation`, `attempts_exhausted`} | a checkout operation entered `reconciliation_required` |
| `rustok_checkout_reconciliation_actions_total` | `action` (the six registry actions), `result` (bounded outcome set) | one operator action attempt, including replays and rejections |

Example alert rules (documentation, not an executed gate):

```promql
# Any new parked checkout needs an operator
increase(rustok_checkout_reconciliation_parked_total[1h]) > 0

# Compensation gave up after the attempt cap
increase(rustok_checkout_reconciliation_parked_total{reason="attempts_exhausted"}[1h]) > 0

# Operator actions or the payment owner are failing
increase(rustok_checkout_reconciliation_actions_total{result=~"payment_owner|storage_error|close_after_money_moved"}[15m]) > 0
```

The attempt cap itself is state, not a metric: it is enforced by `MAX_CHECKOUT_COMPENSATION_ATTEMPTS`,
reported by the sweep (`exhausted`), and visible on the operation row
(`last_error_code = checkout.compensation_attempts_exhausted`).

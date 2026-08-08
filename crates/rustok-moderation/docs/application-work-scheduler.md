# Moderation application work scheduler composition

Status: **bounded source-ready slice / maintainer execution pending**

## Scope

This slice composes durable Moderation decision application into the existing shared `rustok_runtime::ModuleWorkScheduler`. It does not add a Moderation-owned polling loop, host-specific task spawn, second lease protocol, alternate adapter invocation path or operator recovery UI.

`ModerationModule::register_runtime_extensions` publishes one `ModuleWorkRegistration`. The server's existing module-work bootstrap already materializes all selected registrations, starts the shared scheduler only in runtime modes that run background workers, and supplies the deployment-owned stop signal.

Case/application audit lifecycle is now implemented below this scheduler boundary inside Moderation owner `claim/mark_*` primitives. The scheduler remains intentionally unaware of case state and audit events.

## Worker identity

The Moderation registration owns one worker slug:

```text
moderation_decision_application
```

The shared scheduler remains module-neutral. It does not know Moderation tables, decision kinds, subject adapters, case lifecycle, audit events or retry semantics.

## Candidate discovery versus durable claim

`ModerationApplicationWorkAdapter::claim` performs a bounded, read-only lookup for the earliest due `moderation_application_operations` candidate across the owner table. It mirrors the existing due states:

- `pending` or `retryable` with `next_attempt_at <= now`;
- `applying` with an expired `lease_expires_at`.

Discovery is only a scheduling hint. It does **not** create or renew the Moderation operation lease. The returned generic `ModuleWorkItem.lease_token` is an envelope token for the shared scheduler only and is never used as Moderation/domain idempotency.

The handler immediately calls the existing `ModerationService::dispatch_application_operation_once`. That method repeats the authoritative due predicate and acquires the existing UUID operation lease through the existing CAS before reconstructing the command or invoking any domain adapter.

This preserves race behavior across multiple hosts: two schedulers may discover the same candidate, but only one can win the Moderation CAS. A host that loses the claim receives `None`; no second domain call or terminal result is fabricated.

## Existing dispatcher and owner primitives remain authoritative

The scheduler adapter does not call `apply_moderation_decision` itself and does not classify domain results. The existing one-attempt dispatcher remains the only owner path for:

- immutable command reconstruction;
- exact `(subject_module, subject_kind)` adapter lookup;
- trusted `rustok-moderation` service context;
- immutable decision UUID domain idempotency;
- adapter deadline;
- retry/backoff classification;
- stale `Conflict` / invariant operator review;
- rejected non-retryable outcomes;
- applied-evidence validation;
- operation lease finalization through owner primitives.

Those owner primitives now also atomically maintain the Moderation case/audit lifecycle. The scheduler does not duplicate any of it:

- first successful claim may move `decided -> applying_decision` and append start/attempt audit facts;
- retryable completion keeps the case applying and appends the retry audit fact;
- applied completion closes the case and appends applied/closed audit facts;
- rejected/operator-review completion escalates the case and appends matching audit facts.

`ModuleWorkSource::complete` is intentionally a no-op. `moderation_application_operations` remains the durable completion source of truth; the generic scheduler must not write a second applied/retry/rejected state, case transition or audit row.

If dispatcher execution returns an owner/runtime error, the generic scheduler records only an in-memory retryable envelope outcome. The Moderation owner transaction remains authoritative: errors before the CAS leave the candidate due, while committed claim errors after the domain boundary recover through the existing operation lease. Case/application/audit state is never partially advanced by the scheduler.

## Host lifecycle

No new server startup code is required. Existing server module-work bootstrap already:

- builds `HostRuntimeContext` after module runtime extensions and host providers are composed;
- registers all `ModuleWorkRegistrations` into one `ModuleWorkScheduler`;
- runs the scheduler only when the selected runtime mode enables background workers;
- polls at the shared bounded interval;
- uses deployment `StopHandle` so shutdown prevents new claims while already claimed work can finish.

The Moderation registration requires the host-materialized `Arc<ModerationSubjectAdapterRegistry>`. Missing materialization fails module-work registration rather than silently running without adapters.

## Throughput boundary

The generic scheduler asks each registered worker for at most one item per scheduler pass. Moderation therefore contributes at most one application candidate per pass per host. Horizontal hosts remain safe because the canonical Moderation CAS decides ownership.

No inner loop, unbounded batch, tenant fan-out or host-side Moderation table knowledge is introduced by this slice.

## Ownership and Reactions boundary

Moderation continues to own decision application orchestration, case lifecycle and its internal audit ledger. Domain modules own their local mutation and receipt transaction. Forum does not gain worker, retry, lease, case or audit state and still depends only on `rustok-moderation-api` for the application adapter boundary.

This slice is unrelated to Reactions. `rustok-reactions` remains the sole reaction catalog/state/command/aggregate/event/repair owner and `rustok-reactions-storefront` remains the reusable presentation owner. No reaction state, transport or UI is added to Moderation or Forum.

## Explicitly not claimed

The combined scheduler + application lifecycle source does not add or complete:

- operator retry/requeue/re-review commands or UI;
- automatic replacement/rewrite of immutable decisions after escalation;
- typed public/cross-domain Moderation application events;
- per-module scheduler configuration or throughput tuning;
- retained multi-host race, graceful-shutdown, lifecycle-atomicity, SQLite/PostgreSQL or lost-response execution evidence;
- FORUM-19 runtime promotion.

## Maintainer verification handoff

Suggested checks, intentionally not run while preparing this source slice:

```bash
node scripts/verify/verify-moderation-application-work-scheduler.mjs
node scripts/verify/verify-moderation-application-dispatch-once.mjs
node scripts/verify/verify-moderation-application-operation.mjs
node scripts/verify/verify-moderation-application-audit-lifecycle.mjs
cargo check -p rustok-moderation --all-targets
cargo test -p rustok-moderation
cargo check -p rustok-server --no-default-features --features mod-moderation
cargo check -p rustok-server --no-default-features --features "mod-forum mod-moderation"
cargo xtask module validate moderation
git diff --check
```

Retained evidence should cover: Moderation registration presence; background-worker-disabled no dispatch; earliest-due selection; two-host same-candidate CAS convergence; expired-lease reclaim; stop-before-new-claim behavior; in-flight completion after stop; missing materialized registry startup failure; Moderation-only empty adapter registry retry behavior; Forum+Moderation application; first-claim/retry/terminal case/audit atomicity; lost-response receipt replay followed by exactly one case close; and owner failure before/after claim.

No tests, Cargo commands, Node verifiers, formatting, migrations, database scenarios, workflows or CI were executed while preparing this slice.

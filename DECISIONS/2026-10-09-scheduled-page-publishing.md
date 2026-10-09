# Scheduled page publishing

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `rustok-pages` (Pages module)
- Extends: [Multilingual content contract](./2026-03-28-multilingual-content-contract.md), [Page body working copies: draft revisions and body revision journal](./2026-10-09-page-body-draft-and-revision-history.md)
- Supersedes: None
- Superseded by: None

## Context

The pages/page-builder functional audit (2026-10-09) recorded gap G-3: publishing is
immediate-only. Editors cannot schedule a page to go live at a fixed time; every
competitor (WordPress scheduling, Webflow, Tilda, Strapi) treats scheduled publishing as
standard CMS behavior.

The existing publication command is `publish_reviewed`: an atomic, idempotent publish of
one reviewed Page Builder runtime. It requires a review hash over a specific runtime
scenario plus exact per-locale body revision tokens (CAS), and it records a durable
publish receipt keyed by an idempotency key. There is no general-purpose business job
runner in the platform; `rustok-outbox`'s relay (`run()` loop plus bounded
`process_pending_once()`) is the established background-work idiom.

## Decision

Add **scheduled page publishing** as a Pages-owned job queue around the existing
`publish_reviewed` command:

1. A new `page_publish_jobs` table stores, per scheduled attempt, the **exact reviewed
   publish command** (`expected_version`, `expected_body_revisions`, reviewed runtime,
   idempotency key) together with `publish_at`, an execution state machine
   (`scheduled -> executing -> published | failed`, plus `canceled`), attempt accounting,
   and `created_by` attribution.
2. Scheduling captures the command at request time through the same reviewed pipeline the
   admin uses for immediate publish. The scheduler never synthesizes publish input.
3. A bounded sweep (`process_due_publish_jobs_as_of`, plus a relay-style `run()` loop)
   claims due `scheduled` rows and replays the stored command through `publish_reviewed`
   as `SecurityContext::system()`. Because the command carries its idempotency key and
   exact body revision tokens, replays are safe and drift is rejected rather than
   silently published.
4. One pending schedule per page: a partial unique index on `(tenant_id, page_id)` where
   `state = 'scheduled'`. Rescheduling rewrites the pending row's `publish_at` and
   command; job rows are never deleted, so scheduling history is append-only.

## Sources of truth and ownership

- `page_publish_jobs` (Pages schema) is the only source of truth for scheduled
  publications. Nothing else stores `publish_at` state.
- The reviewed publish command stored in `command` is owned by Pages and replayed only
  through `publish_reviewed`; its idempotency key is the one the admin derived from the
  reviewed snapshot at scheduling time.
- The page-builder publish runtime review contract remains the sole authority for what a
  reviewed runtime is; this decision does not alter it.

## Invariants

- A job's stored command is exactly the reviewed input accepted at scheduling time; the
  sweep never recomputes or relaxes revisions, version, or review hash.
- At most one `scheduled` job exists per `(tenant_id, page_id)` at any time.
- Job rows are append-only: states transition in place, rows are never deleted.
- `publish_at` and stored timestamps are truncated to microseconds (same round-trip rule
  as body revision tokens).
- Scheduling requires `pages:publish`; execution is a system-actor publish.

## Non-goals

- Per-locale or partial-set scheduling (waits for the audit's per-locale publish gap).
- A generic cross-module job scheduler; this is a Pages-owned publish queue only.
- Automatic re-review or conflict resolution when content drifts after scheduling.
- Publishing non-builder pages through the scheduler (there is no live non-builder
  publish path; `publish_reviewed` requires at least one Page Builder body).

## Data, transaction, and concurrency boundary

Table `page_publish_jobs` (migration `m20261009_000002_create_page_publish_jobs`,
append-only, backfill `mode: none`):

- `id uuid` primary key (client-generated, as for `page_body_drafts`).
- `tenant_id uuid`, `page_id uuid`, composite foreign key
  `(tenant_id, page_id) -> pages (tenant_id, id)` on delete cascade.
- `publish_at timestamptz` truncated to microseconds.
- `state text` check (`scheduled|executing|published|canceled|failed`).
- `command jsonb` — the serialized reviewed publish input.
- `attempts int`, `last_error_code text`, `last_error_message text`,
  `publish_operation_id uuid`, `created_by uuid`, `created_at`, `updated_at`.
- Partial unique index `uq_page_publish_jobs_active (tenant_id, page_id) where state = 'scheduled'`.

Concurrency: the sweep claims a due job with an optimistic state transition
(`scheduled -> executing` guarded by `state = 'scheduled'`), so concurrent sweeps cannot
execute the same job twice on any backend. Each job execution opens one transaction
inside `publish_reviewed`; job bookkeeping updates are separate statements.

## Context dimensions

- Tenant: all job rows are tenant-scoped; the sweep processes due jobs across tenants and
  passes each row's tenant to `publish_reviewed`.
- Locale: the stored command carries the full per-locale body revision set; scheduling is
  page-granular.
- Actor: `created_by` records the scheduling user; execution is `SecurityContext::system()`.

## Events and projections

No new domain event kinds: scheduling and cancellation are editor state. The executed
publish emits the existing `NodeUpdated`/`NodePublished` events through the transactional
outbox, exactly like an interactive publish. No read projections are added.

## Failure semantics

- Deterministic failures (revision/version conflict, validation, feature gating) mark the
  job `failed` immediately with `last_error_code`/`last_error_message`.
- Unknown errors return the job to `scheduled` with the attempt counter incremented, up
  to 5 attempts, then `failed`.
- No automatic retry after `failed`; rescheduling is explicit and creates a fresh
  pending state (rewriting the pending row if one exists).
- Cancel is allowed only from `scheduled`; `executing`, `published`, `failed`, and
  `canceled` jobs are terminal for cancellation.

## Migration and cutover

One append-only migration with backfill `mode: none`; no existing rows are touched and no
deploy ordering constraint exists. Scheduling is opt-in editor behavior from cutover;
immediate publishing is unchanged.

## Alternatives considered

- **Publish whatever is current at the due time**: rejected — it bypasses the reviewed
  publish contract (review hash and body revision CAS) and would silently publish
  unreviewed content.
- **Delay inside the publish request (long-running transaction)**: rejected — unbounded
  transactions and lost requests on restart.
- **Reuse the alloy cron scheduler**: rejected — alloy schedules scripting jobs for its
  own registry; Pages needs a durable, per-page, CAS-bearing command queue.
- **Store `publish_at` on `pages`**: rejected — it cannot hold attempt history, error
  state, or the reviewed command, and it clutters the page lifecycle state machine.

## Verification

- New sqlite integration test `tests/page_publish_scheduler_sqlite.rs`: schedule/reschedule
  semantics, due sweep executes the stored command once (idempotent replay safe), due
  sweep honors `publish_at`, cancellation prevents execution, deterministic publish
  failure marks the job failed with the recorded error, and non-due jobs are untouched.
- Toolchain-less static gates (rustfmt, tree-sitter syntax, module resolution,
  `verify:adrs`, migration backfill self-test) must pass in the authoring environment;
  `cargo test`/`clippy` remain CI authority (no Rust toolchain in the environment).

## Consequences

- Editors can schedule and cancel a publication; the sweep is safe to run concurrently
  and safe to replay.
- Content edited after scheduling intentionally fails the scheduled publish instead of
  publishing unreviewed content; the job failure surface is the editor's signal to
  reschedule after re-review.
- Hosts must run the sweep (or the `run()` loop) for schedules to fire; documented in the
  module README Known Limitations.

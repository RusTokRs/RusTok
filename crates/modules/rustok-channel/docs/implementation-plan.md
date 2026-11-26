# Implementation plan for `rustok-channel`

## Channel lifecycle and invariant hardening (2026-11-26)

Landed in code, **not compiled or executed** in the authoring sandbox (no Rust
toolchain), so every item below still needs `cargo`/PostgreSQL/SQLite/Redis
evidence before it can be called verified:

- **Lifecycle commands exist.** `ChannelService::{update_channel,
  set_channel_active, delete_channel, set_default_channel_for_tenant,
  ensure_channel_for_tenant}`; REST `PATCH /api/channels/{id}`,
  `POST /api/channels/{id}/active`, `DELETE /api/channels/{id}`; matching SSR
  server functions and admin card actions. Deactivating or deleting the tenant
  default promotes the oldest remaining active channel in the same transaction,
  and the PostgreSQL tombstone/index-revision triggers now fire from a real
  deletion instead of being unreachable code.
- **`channels.status` is gone.** `ChannelStatus` is the typed projection of
  `is_active`, and the index projection derives the `status` field in SQL, so the
  Sales Channel Index can no longer index a value that contradicts `is_active`.
- **Invariants are structural.** Migration `m20261126_000013` adds partial unique
  indexes for one default channel, one active policy set, one primary target and
  one web-host claim per tenant, plus a composite `(tenant_id, channel_id)` FK for
  targets and a denormalized `channel_targets.tenant_id`; conflicting legacy rows
  are repaired before the indexes are created. Writes use single-transaction
  promotion, `ON CONFLICT` upserts and unique-violation mapping to `409`.
- **Selector errors are client errors.** An unknown `X-Channel-ID` records a
  `Miss` and falls through like an unknown slug instead of returning `500`.
- **Admin transport has no fallback.** One selected transport per build, no
  `native -> REST` replay, no server-side API base URL and no loopback default.
- **A broken invalidation runtime degrades, it does not fail the replica.** The
  `channel resolution durable invalidation runtime` guardrail is `Degraded`, and
  the channel middleware stops serving resolution from the process cache while
  the durable listener is not ready (`channel_resolution_cache_bypassed`), so
  stale routing decisions are impossible without taking the replica out of
  rotation. See `rustok-cache/docs/operations.md`.
- **Detail reads are set-based.** `build_channel_details` assembles targets,
  module bindings and OAuth apps with three `IN (...)` queries per page instead
  of three per channel. `list_channel_options(tenant_id, include_inactive)`
  replaces the `list_channels(tenant, 1, 250)` idiom at every consumer
  (commerce storefront, pricing storefront and admin), which used to truncate
  silently and offered unusable channels.
- **Module gates are tenant-scoped.** Every request-path module check uses
  `is_module_enabled_for_tenant`; the unscoped `is_module_enabled` is
  owner-internal only. Manual SQL fixtures no longer seed `channels.status`.
- **The admin renders derived copy, never wire values.** `is_active` is the only
  fact: `core::channel_status_badge`, `core::channel_lifecycle_toggle`,
  `core::channel_rename_command` and `core::CHANNEL_DELETE_FEEDBACK` derive the
  badge label, the lifecycle command and its feedback, so the Leptos adapter binds
  values and keeps only view state. The admin DTOs carry no `status` field.
- **Concurrency outcomes are typed, not `500`.** Every write that can lose a race
  against the 000013 partial unique indexes (default promotion, policy-set
  activation, create-as-default) maps the unique violation to
  `ChannelError::Conflict`; the REST controller answers `409` and the Commerce
  GraphQL shim answers `CHANNEL_CONCURRENT_CONFLICT` with `retryable: true`.
- **Owner reads are set-based, including policy sets.** `build_channel_details`
  and `list_resolution_policy_sets` each issue one `IN (...)` query per child
  collection instead of one per parent row.
- **Lifecycle invariants are pinned, not just written.** Migration 000013 and
  the write ordering it depends on are asserted by
  `scripts/verify/verify-channel-lifecycle-invariants.mjs` (part of
  `verify:channel:fba`): the four partial unique indexes with their predicates on
  both backends, the composite `(tenant_id, channel_id)` FK, the
  repair-before-index order, the clear-before-set order of the default and
  policy-set pointers, the conflict mapping of every racy write, and the absence
  of any reader of the retired `channels.status`.
- **Lost races answer `404`, never `500`.** `remove_resolution_rule` deletes with
  one `policy_set_id`-scoped `delete_many()` and reports a zero row count as
  `NotFound`; `ActiveModel::delete()` (whose `DbErr::RecordNotDeleted` surfaced
  as a `500`) is no longer used anywhere in the owner.
- **A target conflict is classified from state, not from the driver message.**
  `classify_target_conflict` decides between a real duplicate
  (`uq_channel_targets_tenant_host` → `TargetAlreadyExists`) and a lost primary
  slot (`uq_channel_targets_channel_primary` → retryable `Conflict`) by querying
  the tenant-scoped state after the rollback, because SQLite names columns while
  PostgreSQL names the index in its unique-violation message.
- **Bind upserts and their read-back are one transaction.** `bind_module` and
  `bind_oauth_app` read the row they just upserted through the same transaction,
  so a concurrent unbind cannot turn a successful bind into a `500`.
- **A foreign identifier answers `404` everywhere.** The controller's OAuth-app
  guard now matches the channel and policy-set guards: a `400` for "belongs to
  another tenant" told the caller that the app exists elsewhere.
- **Channel `settings` has a closed shape.** `create_channel` and `update_channel`
  reject anything that is not a JSON object, or that exceeds 64 KiB, with
  `ChannelError::Validation`. The content stays module-owned; only the shape the
  readers rely on is enforced at the write boundary.

- **The PostgreSQL-only half of 000013 has its own opt-in suite.**
  `tests/lifecycle_invariants_postgres.rs` (ignored unless
  `RUSTOK_MIGRATION_SMOKE_ADMIN_URL` points at a disposable server) proves the
  `NOT NULL` tenant column, the composite `(tenant_id, channel_id)` FK, the
  host/primary/default indexes and the symmetric `down()`.
- **A storefront channel conflict is a retry.** The commerce storefront mapper
  answers `ChannelError::Conflict` with `503 commerce_store_channel_conflict`
  and `retryable: true` instead of the `500` catch-all; a `GET` never answers
  `409`.
- **The invariants have a runtime test, not only a text gate.**
  `tests/lifecycle_invariants_sqlite.rs` exercises the default/policy-set
  pointers, web-host claims, the single primary target, the closed `settings`
  shape and the `404` for an already-removed rule against in-memory SQLite with
  the real migrations, so `cargo test` is the first thing that must pass after
  this change. Its PostgreSQL-only parts (the composite `(tenant_id,
  channel_id)` FK and `SET NOT NULL`) are covered by the opt-in
  `tests/lifecycle_invariants_postgres.rs` suite described above, which has not
  been executed in this sandbox either.

Residual work is tracked in the audit report
(`channel-audit-report-2026-11-26.md`, section 10.6): UI-kit migration of the
admin cards, per-tenant invalidation generation, the `oauth_apps` Auth port,
`und` locale handling, Next.js parity, `channel_translations`, and the stale
forum route-transport contract test.

## Channel identity snapshots for Product (2026-11-17)

The [Channel identity port](../ports/README.md) has an embedded reader and a
[standalone gRPC service](../README.md#standalone-index-identity-service).
The PostgreSQL fixture passed with the standalone process. It verified tenant
and credential checks, reader parity, generation and UUID consistency across
an uncommitted and then committed rename, a retained generation after deletion,
and the membership limit. Distribution compiles with the embedded resolver
using the Channel-owned snapshot query.

Product's standalone worker consumes Channel's gRPC port and commits the
witness with a revision check in its own database. Localized Index replay and
serving admission still need wiring across database boundaries.

## Selected commerce channel reads (2026-10-30; source work)

`ChannelReadPort::read_selected_channel` is implemented as a shared composition of the
provider's `read_channel`: explicit ID/slug selectors are completed and correlated against
one active tenant-owned non-nil Channel. Blank/nil, inconsistent, foreign, missing/inactive
and unavailable results refuse. No selectors returns `None` for global scope, without page,
host or default substitution. This is a saved-commerce selector read, not a replacement for
the request-routing `ChannelResolver` precedence below.

Pricing retains explicit trusted-context admission and delegates pair correlation to this
owner method. Commerce caps waiting for its saved-Cart read at two seconds and uses the
canonical slug for preflight/shipping and new reservation allocation. No new provider,
persistence or routing resolver is introduced; saved Cart/journal fields are not rewritten.
Regression source in `ports_tests.rs` covers completion, global scope, pair/provider mismatch,
tenancy/activity and failure admission. Source is parsed/formatted only; tests, Rust compilation,
database, cancellation and mounted/remote execution are unrun. This does not promote readiness.

## Current state

`rustok-channel` owns request-channel resolution, typed policies, and the
channel admin package. Resolution order is fixed and verified:
`explicit selectors -> built-in host slice -> typed policies -> explicit default
-> unresolved`. The built-in host slice remains a deliberate fast layer, not a
policy-only fallback.

The built-in host fast-path is retained: explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved.

Server middleware supplies locale and OAuth-app request facts, and the cache key
includes both. The admin package keeps a Leptos-free core, owner transport
facade, native server adapter, and REST secondary adapter; it is host-neutral.

Channel Admin binds authenticated authority to the middleware-resolved tenant
before admitting `settings:manage` or `modules:manage`. All sixteen authenticated
native endpoints call one shared guard with the full `AuthContext` and resolved
tenant id. A mismatch returns the static public message `Channel admin access is
denied`; both tenant ids remain only in structured diagnostics with code
`channel.admin_tenant_scope_mismatch`. The retained source-contract test requires
every Auth/Tenant extractor pair to use the scoped guard and requires tenant
equality to precede permission admission.

The host channel cache is byte-weighted, uses bounded request facts, and has a
bounded monotonic tenant-generation registry with full-clear rollover and
fail-safe bypass on allocator exhaustion. Channel mutations advance
`channel_resolution_invalidation_state` through database triggers in the same
transaction as the changed channel row. Successful REST/native mutations clear
the local tenant token and publish the durable generation as a low-latency fast
path. Every serving replica owns supervised local/Redis/reconcile workers; the
five-second database reconciliation performs a safe namespace-wide local clear
when delivery was missed, the generation regressed, or a replica starts from an
unverified baseline. The worker runtime is a critical host guardrail.

PR #2469 stages database-owned fail-closed invariants found in cycle-001.
PostgreSQL and SQLite serialize default-channel, active-policy-set and
primary-target promotion; host target claims are rebuilt from authoritative
channel rows and uniquely scoped by tenant; OAuth bindings and policy actions
reject cross-tenant relations and incompatible parent tenant moves. Historical
duplicates or mismatches block the migration instead of being silently
rewritten. The draft remains open and is not part of `main`; its current SHA
`53ce761f7801d8893278744b84051325f2439ede` has no accepted same-SHA compile,
migration, PostgreSQL or Redis evidence.

The source now includes eleven durable-recovery evidence layers:

- SQLite reader tests prove that independent replica handles observe committed
  generations without PubSub, rolled-back changes do not advance the epoch, and
  missing generation state fails closed before recovery.
- A server runtime test starts two independent listener/cache runtimes on one
  database, proves both become not-ready when generation state disappears, and
  proves both recover after the durable state returns without Redis.
- An ignored PostgreSQL integration test, wired into the permanent cache
  workflow with ephemeral Postgres 17, covers statement triggers, an independent
  replica connection, commit/rollback, concurrent owner mutations and migration
  replay after state loss.
- An ignored live Redis server test publishes only from replica A and proves
  remote replicas consume validated invalidations before their five-second poll.
  Replica B covers fail-closed degradation, while a fresh replica C covers
  recovery so the result cannot be explained by the same worker's periodic tick.
- An ignored Axum integration test makes replica B cache the old default-channel
  name, commits a triggered update, publishes only from replica A, and requires
  B to return the new resolved channel name within three seconds. This protects
  actual resolved-value convergence rather than inferring correctness from
  worker readiness alone.
- A non-Redis Axum integration test confirms the old value remains cached after
  a direct committed mutation with no publication, then requires the persisted
  generation poll to replace it within the documented recovery window. This is
  the source contract for a completely missed fast-path event.
- A deterministic local-broadcast test exceeds the 256-message buffer, observes
  `RecvError::Lagged`, proves the runtime fails closed while durable state is
  absent, and recovers readiness only after database reconciliation succeeds.
- A combined serving-path lag test caches an old channel name, subscribes both
  the real worker and a probe at the same cursor, publishes 300 no-Redis events
  without a suspension point, confirms `RecvError::Lagged` with two subscribers,
  observes not-ready plus the stale Axum value, restores only durable state and
  requires both readiness and the resolved channel name to recover without a
  replacement fast-path publication.
- A resolved-value database-state-loss test commits a new channel value, removes
  the generation table before reconciliation, observes critical readiness
  degrade, restores the persisted generation and requires the Axum replica to
  return the new value.
- A resolved-value generation-regression test first applies a forward epoch,
  rewinds the persisted epoch, commits a new channel value and requires the next
  reconciliation to rebuild the namespace instead of retaining the old value.
- An ignored self-hosted Redis restart test starts two existing replicas, stops
  Redis, proves polling convergence during the outage, restarts Redis, waits for
  both original subscriptions to return through `PUBSUB NUMSUB`, and then
  requires a new resolved value to arrive within three seconds before the next
  database poll.

The durable cross-replica contract, normal delivery, missed publication, local
listener lag, database loss/recovery, generation regression and Redis
restart/reconnect scenarios are source-complete. The cache-owned live suite also
uses Redis 7 `CLIENT PAUSE` to protect the shared two-second operation timeout,
fast circuit-open rejection and half-open recovery contract. None of these are
compiled or live verified on the current revision until the permanent cache
workflow reports successful compiled, PostgreSQL and Redis jobs.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `ChannelReadPort` / `channel.read_projection.v1` in
  `crates/modules/rustok-channel/contracts/channel-fba-registry.json`.
- Static and fallback evidence:
  `crates/modules/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json`
  and `crates/modules/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json`.
- `scripts/verify/verify-channel-admin-boundary.mjs`,
  `npm run verify:channel:resolution-contract`, and
  `npm run verify:channel:proof-points` lock the UI boundary, canonical
  resolution order, and current consumer proof points.

Current proof points are `rustok-pages`, `rustok-blog`, `rustok-commerce`, and
`rustok-forum`; `verify:channel:proof-points` keeps their channel-aware
contracts documented and source-locked.

## Open results

1. **Move channel display copy to locale-attributed storage.** `channels.slug`
   remains the language-neutral identity, while `channels.name` is still
   human-facing copy in the base row. Add tenant-composite
   `channel_translations`, backfill legacy copy with truthful provenance, require
   a host-resolved effective locale for writes, and return exact-locale/fallback
   projections consistently from REST, native server functions, owner ports,
   cache values and admin UI. Policy-set display names must be classified and
   cut over in the same owner migration if they are tenant-visible copy.
   **Depends on:** the accepted multilingual database contract and translation
   control-plane boundary.
   **Done when:** base rows are language-neutral, locale columns are at least
   `VARCHAR(32)`, writes are atomic with translations, and runtime tests cover
   requested -> tenant default -> first available selection without returning
   storage-only `und` as a request fallback.

2. **Merge and verify the tenant/concurrency invariant fix.** The invariant work
   described above now lives in `main`'s branch history (lifecycle commands,
   partial unique indexes, upserts, single-transaction promotion); it still needs
   the compile/migration/concurrency evidence listed below. PR #2469 added
   database-owned default/active/primary promotion, host-claim serialization,
   tenant relation guards, historical-data preflights and an SQLite transaction
   seal. Reconcile the old branch with current `main`, fix every channel-specific
   format, compile, migration, PostgreSQL or Redis failure, and do not treat
   queued, cancelled or stale-head jobs as evidence.
   **Depends on:** a Rust 1.96 runner plus PostgreSQL 17 and Redis 7 jobs.
   **Done when:** the relevant jobs pass on one current PR SHA and the source
   changes are merged into `main` without weakening fail-closed migrations.

3. **Execute the permanent durable cache gate.** Run the source-complete SQLite,
   server two-replica, lagged-listener resolved-value, PostgreSQL, Redis readiness,
   Redis restart and cache-owned latency/circuit scenarios on one reconciled
   `main` revision, then fix every format, compile, test or Clippy failure before
   recording the revision as verified.
   **Depends on:** GitHub Actions visibility or another Rust 1.96 build
   environment with ephemeral PostgreSQL, Redis 7 and `redis-server`.
   **Done when:** `compiled-contract`, `postgres-channel`, and `live-redis` pass
   on the same revision and the result is recorded without copying raw logs.

4. **Collect full runtime evidence for channel resolution.** Exercise
   `ChannelReadPort` and server middleware with real locale/OAuth facts, policy
   selection, inactive/degraded behavior, cache isolation, generation rollover,
   and the durable cross-replica behavior before promotion beyond
   `boundary_ready`.
   **Depends on:** a composed server runtime and representative request fixtures.
   **Done when:** targeted Rust middleware/port tests provide reproducible
   runtime evidence for every published read and fallback profile.

5. **Extend channel-aware proof points only with owner evidence.** New domain
   reads must use the already resolved `ChannelContext`, local tests, and local
   documentation; they must not introduce a second channel-selection mechanism.
   **Depends on:** the consuming module's public contract.
   **Done when:** the proof-point verifier and affected module docs identify the
   same resolved-channel source and visibility behavior.

6. **Defer richer target or connector taxonomy until pressure is concrete.**
   Do not add speculative target types or connector abstraction merely to expand
   the model.
   **Depends on:** a demonstrated runtime/product need.
   **Done when:** a new type has resolution semantics, migration ownership,
   operator UI implications, and focused contract tests.

## Verification

- `npm run verify:channel:admin-boundary`
- `npm run verify:channel:fba`
- `npm run verify:channel:resolution-contract`
- `npm run verify:channel:proof-points`
- `cargo test -p rustok-channel-admin --test tenant_scope_contract`
- `cargo check -p rustok-channel-admin --features ssr`
- `cargo check -p rustok-channel --lib`
- `cargo test -p rustok-channel --lib`
- `cargo test -p rustok-channel invalidation_generation --lib`
- `cargo test -p rustok-channel sqlite_triggers_advance_generation_and_replay_preserves_it --lib`
- `cargo test -p rustok-server channel_cache_invalidation --lib`
- `cargo test -p rustok-server --test channel_cache_architecture_guard`
- `cargo test -p rustok-server --test channel_cache_resolved_value`
- `RUSTOK_CHANNEL_TEST_POSTGRES_URL=postgres://... cargo test -p rustok-channel --test postgres_invalidation_generation -- --ignored --nocapture --test-threads=1`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://... RUSTOK_CACHE_REDIS_SERVER_BIN=/usr/bin/redis-server cargo test -p rustok-server --test channel_cache_resolved_value -- --ignored --nocapture --test-threads=1`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://... cargo test -p rustok-server redis_publication_drives_remote_replica_readiness_recovery --lib -- --ignored --nocapture --test-threads=1`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://... cargo test -p rustok-cache --test real_redis_hardening -- --ignored --nocapture --test-threads=1`
- `cargo clippy -p rustok-channel --lib -- -D warnings`
- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- Targeted policy-lifecycle and migration-invariant tests.

## References

- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)
- [Cache operations and recovery runbook](../../rustok-cache/docs/operations.md)
- [Multilingual database contract audit](../../../../docs/architecture/database-multilingual-audit.md)

## Change rules

1. Keep resolution precedence and policy ownership in this module.
2. Keep durable generation allocation in the same database transaction as the
   channel mutation; PubSub must never become the source of truth.
3. Update local docs, `rustok-module.toml`, server middleware docs, and route
   selection documentation with a public contract change.
4. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
5. Keep channel base rows language-neutral; tenant-visible display copy belongs
   to locale-attributed owner translations.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-31`
- Scope inspected: `channel ownership and transport boundaries; authenticated/resolved tenant equality across all Channel Admin native endpoints; tenant-scoped reads/writes; OAuth and policy relation integrity; default/active/primary selection concurrency; host claim uniqueness; durable invalidation and migration replay; cache dimensions; multilingual database contract`
- Findings: `P0=1, P1=5, P2=0, P3=1`
- Fixed in this pass: `merged PR #2671 / main commit dabb11a321ad02935e98dacbdcd7e59eef2cb65f so all sixteen authenticated Channel Admin native endpoints require AuthContext.tenant_id == TenantContext.id before settings:manage/modules:manage admission, return a static denial on mismatch and retain both ids only in structured diagnostics; added rustok-channel-admin tenant_scope_contract source regression; PR #2469 still stages DB-owned single-default, single-active-policy and single-primary promotion, tenant-scoped host claims, cross-tenant OAuth/policy guards, fail-fast historical preflights, replay-safe claim rebuild, SQLite transaction seal and cfg(test)-only legacy OAuth fixture normalization`
- Remaining risks or blockers: `P1 channel-display-name multilingual cutover remains open; draft PR #2469 is still based on old main, currently reports non-mergeable and has no accepted same-SHA format/compile/migration/PostgreSQL/Redis evidence; permanent channel cache and composed runtime evidence remain pending; local clone/build remains unavailable because github.com DNS resolution fails`
- Evidence: `PR #2671 exact two-file patch and source-contract test prove tenant equality precedes permission admission at every authenticated Channel Admin endpoint; owner service, REST/native adapters, ChannelReadPort, navigation consumer, resolution pipeline, all channel migrations, durable invalidation tests, SeaORM migration transaction behavior, cache workflow, migration workflow and multilingual database audit were inspected; draft PR #2469 remains source evidence only`
- Next action: `reconcile PR #2469 onto current main without weakening its fail-closed migrations, run channel-admin tenant-scope compile/test plus channel migration/cache gates on one SHA, fix every channel-specific failure, then implement channel translation ownership before closing-gate completion`
- Resume command: `cargo test -p rustok-channel-admin --test tenant_scope_contract && cargo check -p rustok-channel-admin --features ssr && cargo xtask module validate channel && cargo xtask module test channel && cargo test -p rustok-channel --lib`

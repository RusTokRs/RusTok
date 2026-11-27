# Forum enforces configured posting-policy window rules in create transactions

- Date: 2026-11-27
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum`
- Extends: [Posting rate limits](./2026-10-09-forum-posting-rate-limits.md)
- Supersedes: None
- Superseded by: None

Implementation is source-ready but Rust verification is pending (no toolchain).
Related: `forum-logic-review-2026-11-27.md` (D1), implementation plan item 7.

## Context

The canonical posting policy stack delivered by FORUM-26A–26J is complete up to
the composition boundary: normalized rules/facts/decision contracts (26C), the
pure evaluator with explicit precedence (26D), the facts composer with owner
fact ports (26E), and authoritative account-age, trust, reading, approved-post
and topic/reply create-window fact providers. The host already publishes the
composed `SharedForumPostingPolicyFactsComposer` in its runtime extension
registry. What never happened is the last mile: no create transaction invoked
the composer or the evaluator, so the only runtime limit remained the flat
per-author cooldown from `posting_rate`. Plan item 7 tracked this as
outstanding.

## Decision

1. **Two new opt-in module settings** carry the window rules:
   - `posting_policy_topic_create_limit: { maximum_count, window_seconds }`
   - `posting_policy_reply_create_limit: { maximum_count, window_seconds }`

   Both are `Option`, default `None` (old settings JSON keeps working via
   serde defaults). The manifest declares optional objects without defaults and
   requires both integer properties. `ForumModuleSettings::validate` bounds them: count
   1..=1_000_000, window 1..=2_592_000 seconds (30 days). No admin form
   fields — the settings JSON only, matching the cooldown settings. All other
   rule kinds (trust, account age, flags, reputation, moderation history, edit
   windows, bump interval, link/mention/attachment limits) stay at their
   defaults and are not tenant-configurable; their fact owners and settings
   slices remain open scope.

2. **Create-path pipeline** (`posting_policy_enforcement.rs`), split around the
   transaction:
   - *Prepare* (before `begin()`): resolve rules from the content-limits
     settings snapshot; skip everything when the actor is not a `User`
     (system/service exemption identical to `posting_rate`) or when the rules
     require no facts (default settings keep the pipeline inert — the composer
     is never touched). Otherwise compose the evaluation input through the
     host-published composer using the create request's `PortContext`.
   - *Enforce* (inside the transaction, after authorization and the flat
     cooldown): read module settings through the transactional provider and
     require the rules to equal the prepared snapshot, including when the
     prepared rules required no facts. Reject any change as an internal
     fail-closed error; a fresh request must compose against the new rules.
     Then run the pure evaluator and map the decision before the first write. Composing before the transaction keeps fact reads off a second
     connection while a write transaction is open (SQLite writer lock); the
     decision itself is pure. Fact snapshots are advisory observations, not a
     concurrency-safe reservation — distributed reservation remains open scope.

3. **Decision-to-error mapping** reuses existing typed errors (no new
   `ForumError` variants):
   - `Denied` with a temporal reason (window rate limits) →
     `ForumError::RateLimited { retry_after_seconds }` → HTTP 429 with
     `Retry-After`, stable code `FORUM_RATE_LIMITED`, exactly like the
     cooldown.
   - `Denied` with any non-temporal reason (unreachable while only window
     rules exist) → `ForumError::forbidden(...)` fail-closed.
   - `Indeterminate` with `retryable: true` (transient fact-owner timeout or
     unavailability) → `RateLimited` with a bounded 60-second hint.
   - `Indeterminate` with `retryable: false` (missing provider, missing
     composer, missing request context, composition failure) →
     `ForumError::Internal` fail-closed (HTTP 500, generic body).

4. **Wiring**: REST (`ForumHttpRuntime`) and GraphQL
   (`ForumGraphqlRuntimeData`) read `SharedForumPostingPolicyFactsComposer`
   from the host extension registry and attach it through
   `with_posting_policy_facts` builders down `TopicService` (facade → owner →
   persistence) and `ReplyService` (facade → owner). Transports that do not
   publish the composer behave as "composer missing": inert under default
   rules, fail-closed once a tenant configures window rules.

## Invariants

- Authorization and the flat cooldown still run before any policy denial; the
  ADR ordering ("authorization runs before the rate check") is preserved.
- The cooldown is not replaced: both checks run on every user create, and
  `posting_rate` keeps its own settings and semantics.
- Imports, UGC translation apply and moderation writes never call the create
  methods and stay exempt; the starter driver runs as `System` and is exempt.
- Window denials count every persisted owner create inside the window
  (including later-soft-deleted rows) — the 26J fact semantics are unchanged
  by this wiring.
- Empty rules allow without touching facts (FORUM-26D `empty_rules_allow`).

## Non-goals

- No default-on thresholds: without configuration nothing changes.
- No admin UI fields; no versioned policy audit (open scope: "persist and
  administer bounded policy configuration").
- No edits, bumps, or non-window rules yet (open scope: "enforce the composed
  decision in topic, reply, edit and bump owner commands" — creates only, per
  plan item 7).
- No distributed rate-limit reservation, duplicate-content hashing or
  external scoring.
- No new HTTP status codes or error variants.

## Verification

Source-ready and unvalidated: `cargo` is unavailable in this pass, so no Rust
test, lint or formatter ran. Repository Node/Python source verifiers were
executed with a `git stash` baseline comparison (no new failures). Maintainer
follow-up:
- `cargo test -p rustok-forum posting_policy`
- `cargo test -p rustok-forum --test contract_surface posting_policy`
- `cargo test -p rustok-forum --test posting_limits_sqlite posting_policy_`
- The SQLite/PostgreSQL create-window suites and transport-level denial checks.

Three authored, unexecuted SQLite service tests in `posting_limits_sqlite.rs`
exercise real DB-backed topic/reply window fact providers with cooldowns disabled.
They cover first-create admission, threshold denial, exact-author isolation,
window expiry through backdated rows, and missing context/composer/provider.
Before and after a denied command they compare all persisted fixture tables,
including routes, translations, counters, reply positions and outbox. This is
service-level coverage, not executed evidence or REST/GraphQL transport coverage.
A Tree-sitter syntax-only parse of the 18 changed Rust files found no errors;
it does not establish name resolution, type correctness or runtime behavior.

## Sources of truth and ownership

`rustok-forum` owns settings, create admission and the decision mapping. The
host supplies the facts composer; each fact port retains ownership of its data.
Transport builders only carry these dependencies and do not decide policy.

## Data, transaction, and concurrency boundary

Rules and fact snapshots are prepared before the transaction. Transactional
settings are re-read and compared before the pure decision, including the
unconfigured-to-configured transition. This prevents using facts for a different
policy snapshot, but does not serialize a later settings writer. The decision
runs before mutation, beside the existing cooldown.
No new table or database constraint is introduced. Observed counts are not a
reservation: concurrent creates can both pass a window threshold. This decision
does not claim a strict concurrency-safe quota or add retry idempotency.

## Context dimensions

Tenant, authenticated principal, action and observation time scope the facts.
The request PortContext carries deadline/correlation context to fact owners.
Channel and locale do not partition the author window. Currency and timezone
are not applicable to elapsed-second windows.

## Events and projections

Existing create transactions retain ownership of their events and counters.
Denials produce no create mutation or event. Fact snapshots are observations,
not a new authoritative projection or an invalidation mechanism.

## Failure semantics

The decision-to-error mapping above is authoritative: configured rules fail
closed on missing context or facts; only retryable indeterminate decisions and
temporal denials carry the existing rate-limit retry contract. Defaults require
no facts and preserve existing behavior.

## Migration and cutover

No persisted-data migration is needed. Absent settings deserialize as None.
The manifest publishes both keys with required bounded integer properties, so
the platform settings owner accepts and validates them rather than rejecting
them as unknown. Disabling a rule means omitting its optional object.
REST and GraphQL share the owner enforcement path; context-less user entry
points fail closed when configured. The flat cooldown remains in force.

## Alternatives considered

Retiring the evaluator would discard the existing fact-owner contracts.
Composing inside the transaction risks second-connection SQLite lock contention.
Default-on windows would change existing tenant policy without configuration.
A strict reservation design is deferred rather than implied by observed counts.

## Consequences

Configured tenants gain bounded window admission with shared transport behavior.
Operators must provide the composer and request context. Rust tests and a real
configured-window denial check remain required before claiming runtime proof.

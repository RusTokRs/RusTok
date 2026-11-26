# Forum configured policy requires transactional settings for writes

- Date: 2026-11-26
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-forum
- Extends: [Forum soft defaults](./2026-10-09-forum-soft-default-settings.md)
- Supersedes: Only the missing-transactional-reader fallback in [Vote policy settings](./2026-10-09-forum-vote-policy-settings.md) and [Delete policy admission](./2026-11-26-forum-delete-policy-admission.md); policy values, authority and defaults are unchanged
- Superseded by: None

## Context

`ForumSettingsProviders` supports both default-only services and host-provided
settings. Read-only consumers legitimately receive only the ordinary settings
reader. The transactional accessor previously treated an absent transactional
reader as absent stored policy, even when the host had supplied a static reader.
An edit admitted by preflight could therefore use the unlimited default window
instead of saved policy. Other transactional settings, including pre-moderation
and voting restrictions, had the same unsafe fallback.

## Decision

The shared provider preserves all four capability configurations through
`ForumSettingsProviders::from_optional_readers`:

- No readers: explicit default-only service configuration, retaining existing defaults.
- Paired static/transactional readers: resolve writes through the transactional
  reader in the caller's transaction. Missing rows and disabled-module snapshots
  retain their existing default semantics; reader errors propagate.
- Static reader only: nontransactional reads remain supported, but an attempted
  transactional policy read fails with `FORUM_TRANSACTIONAL_SETTINGS_UNAVAILABLE`.
  It must not substitute defaults or invoke the static reader from the transaction.
- Transactional reader only: ordinary and transactional policy reads both fail with
  `FORUM_STATIC_SETTINGS_UNAVAILABLE`; this is not a default-only service.

No new settings or transport-specific rules are introduced. Configure paired
capabilities when composing any owner that writes using stored tenant policy.
REST, GraphQL and native composer/read/continuation runtime assembly must pass both
optional readers unchanged to the shared constructor, so the owner can distinguish
either incomplete configuration from explicit default-only composition. Native admin
already requires both capabilities before constructing policy-controlled services.

## Sources of truth and ownership

The host settings owner remains authoritative for saved tenant policy.
`services/engagement_mode.rs` owns provider admission and error construction.
Content, quote, slug, delete, voting and posting owners use its existing
transactional accessor; clients never decide whether fallback is safe.

## Invariants

### Allowed states

Default-only services and ordinary read-only consumers remain usable. A paired
reader that returns no saved row still yields the canonical module defaults.

### Forbidden states

A configured host policy must not become permissive defaults merely because its
static or transactional capability was omitted. No successful domain write, relation revision,
delete snapshot or outbox event may survive this capability refusal.

## Non-goals

Changing policy values, module enablement semantics, permission scopes, writer
isolation, capability registration architecture, or eliminating the explicit
no-reader default-only service configuration.

## Data, transaction, and concurrency boundary

No schema change. A transactional capability error is raised at the shared settings
read inside the caller's existing transaction. Any preliminary source claim rolls back.
A transaction-only provider also refuses ordinary reads before a write can begin.
This does not claim that a transactional read serializes later settings writers.
Existing source revision/CAS fences and atomic domain/outbox commits remain required.

## Context dimensions

Tenant policy is read in the existing tenant-scoped command context. Principal,
channel, locale and trace handling are unchanged. Currency and display timezone
are not applicable to capability admission.

## Events and projections

No new events or projections. Refusal emits no success event or invalidation.

## Failure semantics

Missing transactional capability uses the existing `CapabilityUnavailable` mapping
with capability `static_module_settings_transactional` and stable code
`FORUM_TRANSACTIONAL_SETTINGS_UNAVAILABLE`. Staff exemptions from a policy value
are not exemptions from resolving that policy capability. Missing static capability
in a transaction-only configuration uses `static_module_settings` with stable code
`FORUM_STATIC_SETTINGS_UNAVAILABLE`. Both errors retain HTTP 503/nonretryable and
typed GraphQL error semantics. The anonymous public gate retains its existing
`FORUM_PUBLIC_SETTINGS_UNAVAILABLE` when no static reader is available.

GraphQL settings defaults (topic order and topic/reply page sizes) must propagate
`ForumError` unchanged through `?`, including when the field is aliased. Rebuilding
an `async_graphql::Error` from the display string loses the capability-specific code
and the retry contract. Transient reader failures retain `FORUM_CAPABILITY_FAILURE`,
`retryable: true` and the existing `retryAfter: 5` hint; owner diagnostics remain
redacted. Missing configured capabilities remain nonretryable with no retry hint.

## Migration and cutover

No data migration. Hosts that supply only the ordinary reader must add the paired
transactional reader before invoking policy-controlled writes. No runtime fallback,
parallel provider, feature flag or permissive compatibility path is added.

## Alternatives considered

- Default missing capabilities: silently removes saved restrictions.
- Read static settings inside the write transaction: preserves neither the owner's
  transactional port contract nor the intended transaction boundary.
- Require transactional readers for all reads: breaks legitimate read-only consumers.
- Remove default-only services here: changes a separate supported composition contract.

## Verification

Provider tests distinguish all four configurations and retain static-only ordinary
reads. REST host-runtime regressions cover both incomplete pairs; GraphQL regressions
retain both stable capability codes. `graphql_settings_error_contract` exercises all
eight live resolver default branches with aliased fields and both a missing static
capability and a transient reader failure, asserting code/retry hints, redaction and
no owner reads. These Rust cases remain unexecuted without Cargo. Supplemental
mutation fixtures cover typed error propagation at every default call site.
Topic edit service cases on SQLite/PostgreSQL must
assert capability failure and unchanged source, translations, revisions, aggregates
and outbox after refusal for paired, static-only and transaction-only providers.
Topic/reply quote tests assert no new relation revision;
delete tests assert the stable capability code. Supplemental source fixtures reject
restoring the fallback or ignoring admission. Rust compilation/service execution
remains required; passing source fixtures alone does not complete this decision.

## Consequences

Misconfigured write runtimes now report an explicit dependency failure instead of
weakening tenant policy. Correctly paired runtimes and default-only services retain
their existing behavior. No database or frontend contract duplication is necessary.

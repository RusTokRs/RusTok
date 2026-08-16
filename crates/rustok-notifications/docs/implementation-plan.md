# `rustok-notifications` module-local implementation gates

The canonical cross-module roadmap remains
`crates/rustok-forum/docs/implementation-plan.md`. This ledger records the
owner-local boundaries that every Notifications slice must preserve. The program
remains `in_progress` until maintainer-run verification and canonical promotion
are recorded.

## Scope

Preserve the neutral producer boundary, owner-only persistence, optional-module
degraded behavior, mandatory recipient privacy, tenant capability enforcement,
and module-owned UI packages while inbox and delivery products are implemented
incrementally.

## Current state

Forum publishes live neutral providers for `forum.topic.created` and
`forum.mention.user_added`. Notifications owns five ordered PostgreSQL/SQLite
migrations covering persistence, durable source intake, candidate processing,
outbox acceptance receipts, and permanent intake quarantine.

The runtime pipeline has three independent, default-off stages:

1. outbox envelope intake into `notification_source_inbox`;
2. source descriptor materialization and bounded audience fanout;
3. recipient preference/privacy/source-policy candidate processing.

The server starts these stages in intake → fanout → candidate order. Fanout and
candidate workers expose tenant-scoped work and recheck effective policy before
foreign provider calls. Disabled work receives 300-second durable backoff;
temporary policy lookup failure receives 30 seconds.

Bounded audience owners may return a sparse page with zero recipients only when
its next cursor differs from the claimed cursor. The fanout service persists that
progress under the existing lease/CAS transition and creates no candidate items.
A repeated cursor remains a non-retryable stalled-provider failure.

Candidate pre-claim resolution captures one effective-policy snapshot containing
the deterministic revision and manifest default-enabled module set. The final
notification transaction invokes an injected commit guard that locks the
Modules-owned `module.lifecycle` cursor and resolves tenant overrides with that
observed manifest input on the same transaction. No manifest/pool read occurs
while the final transaction is active. PostgreSQL lifecycle tenant toggles advance
the cursor inside their tenant-state transaction, serializing candidate commit and
tenant enable/disable by commit order.

The owner inbox plane now includes exact open authorization, bounded authorized
listing, exact and selected-ID state commands, exact unread count, resumable mark-all
commands, exact-recipient reconciliation, durable group keys, exact-group listing,
bounded group summaries, and bounded group-state commands. Open and authorized reads
recheck current recipient privacy before source target authorization; state commands
remain owner-local and archived remains terminal.

The authenticated storefront plane is also delivered. `NotificationInboxStorefrontPort`
derives tenant and recipient scope from `PortContext`, native Leptos server functions serve
SSR/hydrate, while GraphQL serves CSR/headless grouped reads, fresh open authorization,
and bounded group-state writes; no transport fallback is permitted. The module-owned grouped
inbox UI pages owner results, uses stale-response guards, refreshes authoritatively after
writes, navigates only after an `Allowed` open decision, and automatically reloads its
bootstrap when the reactive auth token or tenant changes. A generic manifest-driven header
action exposes the localized Notifications route and exact unread badge. Forum topic-created
sources also materialize minimal initially non-public descriptors when exact recipient context is
composed, while audience pages still reauthorize every subscriber. Tenant-wide scheduling/redaction,
delivery transports, and PostgreSQL cross-consumer evidence remain open.

## Invariants

- producer modules depend only on `rustok-notifications-api`;
- producer transactions never call the Notifications owner synchronously;
- Notifications never reads producer-private or Modules-private tables;
- executable hosts decode envelopes and compose cross-owner policy ports;
- audience resolution is cursor-based and capped at 256 recipients per page;
- a zero-recipient page may continue only with a different non-empty cursor;
- final notification creation requires preference, privacy, current source
  authorization, current tenant enablement, and matching policy revision;
- no allow-all recipient policy exists;
- disabled or unresolved tenant capability fails closed before provider calls;
- tenant-policy deferral leaves later work reachable in bounded selection;
- server workers never read Notifications private tables directly;
- final candidate transactions do not open a second connection for manifest reads;
- PostgreSQL lifecycle tenant toggle and final candidate commit share one cursor
  serialization point;
- exact inbox open/list reads require tenant and recipient ownership before policy
  or source calls;
- inbox listing defaults to 20 raw rows, is capped at 64, and orders by
  `created_at DESC, id DESC`;
- inbox listing cursors preserve timestamp nanoseconds and the UUID tie-breaker;
- inbox listing progress is derived from the last scanned raw row, so an empty
  authorized page may still carry a next cursor;
- retryable privacy or source failures abort an inbox page without a partial result;
- inbox reads expose no dedicated route or structural target fields and never
  mutate seen/read/archive state or delivery attempts;
- exact inbox state commands require notification, tenant, and recipient ownership;
- `mark_seen` advances only unread rows and `mark_read` advances unread/seen rows;
- direct unread-to-read assigns one timestamp to both `seen_at` and `read_at`;
- seen-to-read and archive preserve existing state timestamps;
- `mark_unread` changes only seen/read rows, clears `seen_at` and `read_at`, and
  leaves already-unread rows unchanged;
- archive advances every non-archived row and archived remains terminal against
  mark-seen, mark-read, and mark-unread commands;
- state commands rewrite `updated_at` only on an actual transition;
- inbox state commands call no recipient policy, source provider, target, or
  delivery owner and create no delivery attempts;
- exact unread counts require non-nil tenant and recipient identities;
- unread counting applies tenant, recipient, and `unread` state filters before the
  owner-table aggregate and reuses `idx_notifications_inbox`;
- unread totals are never derived from bounded or privacy-filtered list pages;
- unread counting returns only the aggregate, calls no foreign owner, and mutates no
  inbox timestamp, reconciliation state, or delivery attempt;
- mark-all-read requires non-nil tenant and recipient identities, defaults to 20
  rows and is capped at 64;
- mark-all-read reuses the `i1` cursor and orders eligible rows by
  `created_at DESC, id DESC`;
- only unread and seen rows are selected; read and archived rows remain outside the
  mark-all-read command;
- one bounded raw mark-all-read page is loaded before any exact state mutation;
- every selected mark-all-read row delegates to
  `NotificationInboxStateService::mark_read`, preserving unread/seen timestamp
  invariants;
- mark-all-read progress is derived from the last scanned raw eligible row and the
  response exposes only scanned/marked counts plus continuation metadata;
- mark-all-read calls no privacy, source, target, or delivery owner and creates no
  delivery attempt;
- mark-all-unread requires non-nil tenant and recipient identities, defaults to 20
  rows and is capped at 64;
- mark-all-unread reuses the `i1` cursor and orders eligible rows by
  `created_at DESC, id DESC`;
- only seen and read rows are selected; unread and archived rows remain outside the
  mark-all-unread command;
- one bounded raw mark-all-unread page is loaded before any exact state mutation;
- every selected mark-all-unread row delegates to
  `NotificationInboxStateService::mark_unread`, clearing `seen_at` and `read_at`
  while archived remains terminal;
- mark-all-unread progress is derived from the last scanned raw eligible row and the
  response exposes only scanned/marked counts plus continuation metadata;
- mark-all-unread calls no privacy, source, target, or delivery owner and creates no
  delivery attempt;
- mark-all-archive requires non-nil tenant and recipient identities, defaults to 20
  rows and is capped at 64;
- mark-all-archive reuses the `i1` cursor and orders eligible rows by
  `created_at DESC, id DESC`;
- only unread, seen, and read rows are selected; archived rows remain outside the
  mark-all-archive command;
- one bounded raw mark-all-archive page is loaded before any exact state mutation;
- every selected mark-all-archive row delegates to
  `NotificationInboxStateService::archive`, preserving existing seen/read history
  and keeping archive terminal;
- mark-all-archive progress is derived from the last scanned raw eligible row and
  the response exposes only scanned/marked counts plus continuation metadata;
- mark-all-archive calls no privacy, source, target, or delivery owner and creates
  no delivery attempt;
- recipient reconciliation scans only exact-recipient non-archived rows in bounded
  `created_at DESC, id DESC` pages;
- reconciliation raw selection completes before privacy or source owner calls;
- only an `Unavailable` open decision triggers the exact state-owner archive command;
- retryable reconciliation failures stop the page while prior archives remain
  durable and idempotent on restart;
- reconciliation responses expose only counts and continuation metadata;
- delivery work remains outside candidate finalization;
- worker enablement is never inferred from provider readiness.

## Delivered milestones

### `NOTIFY-00B`

- optional owner/runtime/distribution composition;
- deferred provider factory materialization through `HostRuntimeContext`;
- duplicate or mismatched provider identity fails startup;
- Forum commands remain independent from Notifications availability;
- admin/storefront packages expose explicit foundation/unavailable states.

### `NOTIFY-01A`

- migration `m20260721_000010_create_notification_persistence`;
- typed notification, delivery, fanout, preference, digest, and encrypted push
  entities;
- tenant-composite recipient integrity, dedupe, bounded payloads, leases, and
  encrypted endpoint storage;
- SQLite and opt-in PostgreSQL invariant evidence.

### `NOTIFY-01B / NOTIFY-03A`

- migration `m20260722_000011_create_notification_source_inbox`;
- durable source identity and changed-replay conflict detection;
- recoverable source/job leases and bounded cursor fanout;
- one descriptor job per source event and idempotent pending candidates;
- no final notification or delivery before policy;
- contract `contracts/notifications-source-fanout.json` and verifier
  `scripts/verify/verify-notifications-source-fanout.mjs`.

### `NOTIFY-03B / NOTIFY-07A`

- migration `m20260722_000012_add_candidate_processing`;
- recoverable candidate leases, retry timing, and terminal states;
- exact source/type preference precedence before wildcards;
- mandatory injected recipient policy with typed suppression/error outcomes;
- recipient-specific source authorization and final-transaction preference recheck;
- idempotent notification insert plus candidate completion in one lease-CAS
  transaction;
- zero delivery attempts;
- contract `contracts/notifications-candidate-policy.json` and verifier
  `scripts/verify/verify-notifications-candidate-policy.mjs`.

### `NOTIFY-07B`

- Profiles owner privacy read port and runtime;
- mandatory Notifications block/mute runtime contracts;
- server policy order profile → block → mute;
- missing relation providers fail closed;
- contract `contracts/notifications-recipient-policy-runtime.json` and verifier
  `scripts/verify/verify-notifications-recipient-policy-runtime.mjs`.

### `SOCIAL-01A / NOTIFY-07C`

- Social Graph PostgreSQL/SQLite block and mute persistence;
- tenant-composite relation integrity and monotonic revisions;
- owner command/read ports;
- server adapters into Notifications policy contracts;
- relation-policy readiness true while candidate enablement remains separate;
- contract `crates/rustok-social-graph/contracts/social-graph-notification-policy.json`.

### `NOTIFY-03C`

- bounded `NotificationCandidateWorker`, default batch 32 and hard maximum 64;
- stable pending/due-retry/expired-processing selection;
- canonical service owns every claim and completion CAS;
- default-off host flag `RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED`;
- shared shutdown checks between candidates;
- contract `contracts/notifications-candidate-worker.json` and verifier.

### `NOTIFY-03D`

- migrations `m20260723_000013_add_outbox_intake_receipts` and
  `m20260723_000014_add_outbox_intake_rejections`;
- owner intake selects supported committed `sys_events` envelopes without relay
  status coupling;
- event decoding is injected by the executable host;
- semantic source identities for topic-created and mention events;
- source inbox and accepted receipt commit in one transaction;
- permanent invalid envelopes enter owner-local quarantine;
- accepted replay re-decodes and validates full semantic identity;
- accepted/rejected outcomes are mutually exclusive;
- default-off host flag `RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED`.

### `NOTIFY-03E`

- bounded `NotificationFanoutWorker`, default/hard batch 32/64 and page 256;
- tenant-scoped source/job work projections;
- stable selection without acquiring leases;
- canonical `NotificationFanoutService` owns every claim/page transition;
- default-off host flag `RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED`;
- effective policy checked before descriptor/audience provider calls;
- SQLite evidence covers bounded fanout and zero final delivery rows.

### `NOTIFY-03F`

- `NotificationFanoutPolicyDeferral` defines tenant-disabled and
  policy-unavailable outcomes;
- disabled work enters `retryable_error` for 300 seconds; lookup failures receive
  30 seconds;
- CAS increments attempts, persists stable metadata, clears leases, and prevents
  bounded queue starvation;
- SQLite evidence is `tests/fanout_policy_deferral_sqlite.rs`.

### `NOTIFY-03G`

- `NotificationCandidateWorkItem` exposes candidate and tenant IDs while keeping
  persistence private;
- the server resolves effective tenant policy before every candidate claim;
- disabled work invokes neither recipient policy nor source provider;
- candidate CAS backoff mirrors fanout 300/30-second semantics;
- SQLite evidence proves tenant-scoped selection, queue-head advancement, retry
  metadata, and zero notification rows.

### `NOTIFY-03H`

- public `NotificationTenantCapabilityCommitGuard` request/decision/error contract;
- guarded `NotificationCandidateService` and `NotificationCandidateWorker`
  constructors preserve trusted compatibility paths while production uses guarded
  paths only;
- pre-claim `EffectiveModulePolicyService::resolve_snapshot` forwards exact policy
  revision and manifest default-enabled module set;
- final transaction validates lease before commit guard and runs guard before
  preference recheck or notification insert;
- the commit request validates and carries observed manifest defaults, so the final
  transaction never opens a second pool connection to reload the manifest;
- Modules owner exposes `lock_and_resolve_static_policy_in_transaction` and keeps
  all `tenant_modules` reads outside server/Notifications;
- PostgreSQL guard locks `module.lifecycle` cursor with `FOR UPDATE`;
- production lifecycle state transition advances the same cursor in its transaction;
- disabled/revision-changed/guard-error outcomes roll back notification insert and
  enter durable candidate retry;
- SQLite evidence covers transaction-bound policy resolution and revision rejection
  rollback; PostgreSQL contention evidence remains maintainer-owned;
- candidate worker contract schema 6 and candidate policy contract schema 8 record
  the narrow lifecycle serialization and connection-safety guarantees.

### `NOTIFY-03I`

- `NotificationFanoutService` accepts an empty provider page only when the next
  cursor differs from the claimed cursor;
- sparse progress persists through the existing lease/CAS page transition with
  zero candidate inserts and keeps the job pending;
- oversized pages remain rejected and repeated cursors remain non-retryable
  `NOTIFICATION_FANOUT_CURSOR_STALLED` failures;
- contract `contracts/notifications-source-fanout.json` advances to schema 5;
- SQLite evidence is `tests/fanout_sparse_page_sqlite.rs`.

### `FORUM-20R / FORUM-20S`

- `NotificationInboxOpenService` requires exact notification, tenant, and recipient
  identity before any foreign owner call;
- missing and foreign rows return the same `Unavailable` result without an
  existence oracle;
- current recipient privacy/block policy runs before source target authorization;
- suppression and stale targets return `Unavailable`, while retryable policy/source
  failures preserve retryability;
- only the current source-owned safe route is returned;
- open authorization does not expose the stored row, mutate inbox state, or create
  delivery attempts;
- SQLite evidence is `tests/inbox_open_authorization_sqlite.rs`.

### `FORUM-20T`

- `NotificationInboxListService` provides exact-recipient bounded listing with
  default/hard limits 20/64 and optional exact `NotificationState` filtering;
- keyset order is `created_at DESC, id DESC` and the versioned cursor preserves
  seconds, nanoseconds, and UUID identity;
- raw `limit + 1` selection derives continuation from the last scanned row;
- each raw row reuses `NotificationInboxOpenService`, so privacy/source suppression
  can produce an empty page with a next cursor;
- retryable privacy or source failures abort the page without a partial result;
- the typed list item adds no route or structural target fields and listing mutates
  neither read state nor delivery attempts;
- contract `crates/rustok-forum/contracts/forum-notification-inbox-listing.json`,
  verifier `scripts/verify/verify-forum-notification-inbox-listing.mjs`, and SQLite
  evidence `tests/inbox_listing_sqlite.rs`.

### `FORUM-20U`

- `NotificationInboxStateService` provides exact notification/tenant/recipient
  `mark_seen`, `mark_read`, and `archive` commands;
- missing, cross-tenant, and cross-recipient identities return one unavailable
  decision without a notification-existence oracle;
- transitions are monotonic and idempotent: unread → seen → read → archived;
- direct unread-to-read sets `seen_at` and `read_at` together, seen-to-read preserves
  `seen_at`, and archive preserves existing seen/read timestamps;
- same/later-state commands preserve state timestamps and `updated_at`;
- state commands expose no semantic target, call no privacy/source/delivery owner,
  and create no delivery attempts;
- mark-unread, bulk/mark-all, counts, grouped views, external transport, and UI
  remained closed at this milestone;
- contract
  `crates/rustok-forum/contracts/forum-notification-inbox-state-mutations.json`,
  verifier `scripts/verify/verify-forum-notification-inbox-state-mutations.mjs`, and
  SQLite evidence `tests/inbox_state_sqlite.rs`.

### `FORUM-20V`

- `NotificationInboxReconcileService` scans one exact-recipient non-archived page
  with default/hard limits 20/64 and `created_at DESC, id DESC` keyset ordering;
- its versioned cursor preserves seconds, nanoseconds, UUID identity, and the shared
  128-byte/control-character validation boundary;
- raw selection completes before each row reuses `NotificationInboxOpenService`;
- current recipient privacy remains before source target authorization;
- only `Unavailable` rows archive through `NotificationInboxStateService`, preserving
  existing seen/read timestamps and leaving delivery attempts unchanged;
- retryable owner failures stop the page while prior archives remain durable and
  idempotent on restart;
- the response exposes only scanned/archived counts and continuation metadata;
- tenant-wide scheduled reconciliation, payload redaction, transport, and UI remain
  closed;
- contract
  `crates/rustok-forum/contracts/forum-notification-inbox-reconciliation.json`,
  verifier `scripts/verify/verify-forum-notification-inbox-reconciliation.mjs`, and
  SQLite evidence `tests/inbox_reconcile_sqlite.rs`.

### `FORUM-20W`

- `NotificationInboxStateService::mark_unread` changes only exact-recipient seen or
  read rows back to unread;
- the command clears `seen_at` and `read_at` to satisfy the owner persistence state
  constraint and rewrites `updated_at` only when a transition occurs;
- already-unread rows remain unchanged and archived rows never reopen;
- missing, cross-tenant, and cross-recipient rows return the same unavailable
  decision without a notification-existence oracle;
- the command exposes no semantic target, calls no privacy/source/delivery owner,
  and creates no delivery attempts;
- bulk/mark-all mutations, canonical unread counts, grouped views, external
  transport, and UI remain closed;
- contract
  `crates/rustok-forum/contracts/forum-notification-inbox-mark-unread.json`,
  verifier `scripts/verify/verify-forum-notification-inbox-mark-unread.mjs`, and
  SQLite evidence `tests/inbox_state_sqlite.rs`.

### `FORUM-20X`

- `NotificationInboxUnreadCountService` counts only exact-recipient rows in stored
  owner state `unread` after validating non-nil tenant and recipient identities;
- tenant, recipient, and state filters precede aggregation and reuse the existing
  `idx_notifications_inbox` index;
- the Notifications owner table is authoritative, so callers must not derive totals
  from bounded or privacy-filtered list pages;
- empty, missing, cross-tenant, and cross-recipient scopes all return zero without
  exposing notification identity;
- current privacy or source-policy changes affect the count after exact or scheduled
  reconciliation archives unavailable rows;
- the count calls no privacy/source/target/delivery owner, mutates no inbox or
  delivery state, and returns no source, target, route, notification, or cursor data;
- transport, UI, bulk/mark-all mutations, and grouped views remain closed;
- contract
  `crates/rustok-forum/contracts/forum-notification-inbox-unread-count.json`,
  verifier `scripts/verify/verify-forum-notification-inbox-unread-count.mjs`, and
  SQLite evidence `tests/inbox_count_sqlite.rs`.

### `FORUM-20Y`

- `NotificationInboxMarkAllReadService` scans one exact-recipient page of unread or
  seen rows in `created_at DESC, id DESC` order after non-nil identity validation;
- requests default to 20 rows, cap at 64, and reuse the versioned `i1` cursor with
  nanosecond timestamp and UUID tie-breaker validation;
- one bounded raw page is selected before any mutation and continuation derives from
  the last scanned raw eligible row;
- every selected row delegates to `NotificationInboxStateService::mark_read`,
  preserving direct unread-to-read timestamp equality and seen-to-read history;
- read and archived rows remain outside selection, while empty and foreign scopes
  return an empty page without notification identity;
- earlier exact transitions remain durable and idempotent if a later database
  operation fails;
- the response exposes only scanned/marked counts and continuation metadata, calls
  no privacy/source/target/delivery owner, and leaves delivery attempts unchanged;
- mark-all-unread, mark-all-archive, arbitrary selected-ID bulk commands, grouped
  views, transport, and UI remain closed at this milestone;
- contract
  `crates/rustok-forum/contracts/forum-notification-inbox-mark-all-read.json`,
  verifier `scripts/verify/verify-forum-notification-inbox-mark-all-read.mjs`, and
  SQLite evidence `tests/inbox_mark_all_read_sqlite.rs`.

### `FORUM-20Z`

- `NotificationInboxMarkAllUnreadService` scans one exact-recipient page of seen or
  read rows in `created_at DESC, id DESC` order after non-nil identity validation;
- requests default to 20 rows, cap at 64, and reuse the versioned `i1` cursor with
  nanosecond timestamp and UUID tie-breaker validation;
- one bounded raw page is selected before any mutation and continuation derives from
  the last scanned raw eligible row;
- every selected row delegates to `NotificationInboxStateService::mark_unread`,
  clearing `seen_at` and `read_at` while archived remains terminal;
- unread and archived rows remain outside selection, while empty and foreign scopes
  return an empty page without notification identity;
- earlier exact transitions remain durable and idempotent if a later database
  operation fails;
- the response exposes only scanned/marked counts and continuation metadata, calls
  no privacy/source/target/delivery owner, and leaves delivery attempts unchanged;
- mark-all-archive, arbitrary selected-ID bulk commands, grouped views, transport,
  and UI remain closed at this milestone;
- contract
  `crates/rustok-forum/contracts/forum-notification-inbox-mark-all-unread.json`,
  verifier `scripts/verify/verify-forum-notification-inbox-mark-all-unread.mjs`, and
  SQLite evidence `tests/inbox_mark_all_unread_sqlite.rs`.

### `FORUM-20AA`

- `NotificationInboxMarkAllArchiveService` scans one exact-recipient page of unread,
  seen, or read rows in `created_at DESC, id DESC` order after non-nil identity
  validation;
- requests default to 20 rows, cap at 64, and reuse the versioned `i1` cursor with
  nanosecond timestamp and UUID tie-breaker validation;
- one bounded raw page is selected before any mutation and continuation derives from
  the last scanned raw eligible row;
- every selected row delegates to `NotificationInboxStateService::archive`,
  preserving existing seen/read timestamps and keeping archive terminal;
- archived rows remain outside selection, while empty and foreign scopes return an
  empty page without notification identity;
- earlier exact transitions remain durable and idempotent if a later database
  operation fails;
- the response exposes only scanned/marked counts and continuation metadata, calls
  no privacy/source/target/delivery owner, and leaves delivery attempts unchanged;
- arbitrary selected-ID bulk commands, grouped views, transport, and UI remain
  closed;
- contract
  `crates/rustok-forum/contracts/forum-notification-inbox-mark-all-archive.json`,
  verifier `scripts/verify/verify-forum-notification-inbox-mark-all-archive.mjs`, and
  SQLite evidence `tests/inbox_mark_all_archive_sqlite.rs`.

### `FORUM-20AB`

- bounded explicit selected-ID `mark_seen`, `mark_read`, `mark_unread`, and `archive`;
- one through 64 unique non-nil IDs validated before mutation;
- input-order owner delegation with non-oracular changed/not-changed counts.

### `FORUM-20AC / FORUM-20AD`

- bounded authorized listing for one exact opaque group key;
- durable `g1:{target_owner}:{target_id}` group-key population and historical backfill;
- explicit producer-supplied keys remain authoritative.

### `FORUM-20AE / FORUM-20AF`

- bounded exact-recipient group summaries with stored item/unread counts and an
  authorized latest-item projection;
- bounded exact-group mark-read, mark-unread, and archive commands through the exact
  state owner.

### `FORUM-20AG / FORUM-20AH`

- transport-neutral authenticated-user storefront port with context-derived owner scope;
- native Leptos server-function adapter for reads, fresh open authorization, and grouped
  state commands;
- human-user admission, tenant match, deadline, and idempotency semantics remain explicit.

### `FORUM-20AI / FORUM-20AJ`

- module-owned grouped Leptos inbox with SSR bootstrap, bounded paging, stale-response
  protection, authoritative mutation refresh, and allowed-only navigation;
- generic manifest-driven storefront header action with localized route and exact unread
  badge; optional failures hide the action without breaking the header.

### `FORUM-20AK / FORUM-20AL`

- GraphQL parity for unread count, bounded group summaries/items, and fresh open
  authorization;
- request DTOs cannot select tenant, recipient, or user identity;
- SSR/hydrate select native reads while CSR/headless select GraphQL with no fallback;
- the later `FORUM-20AN` slice closes group-state command parity without changing these
  grouped read and open-authorization contracts.

### `FORUM-20AM`

- canonical Forum plan, this owner-local ledger, owner README, and live contract are
  synchronized through `FORUM-20AL`;
- the latest handoff contract records the synchronization and a dedicated static verifier
  guards all four documents from future milestone drift;
- no runtime capability or validation status is promoted.

### `FORUM-20AN`

- GraphQL mutation parity for bounded exact-group mark-read, mark-unread, and archive;
- typed action plus bounded group/cursor/limit inputs and a required bounded idempotency key;
- authenticated context-derived tenant/recipient scope with five-second write deadline;
- selected native SSR/hydrate and GraphQL CSR/headless command paths without fallback;
- unchanged owner state service, timestamp invariants, terminal archive, and UI refresh flow.

### `FORUM-20AO`

- grouped bootstrap source combines the manual refresh nonce and the reactive transport context;
- auth token, tenant, sign-in, sign-out, and refresh-session changes trigger automatic reload;
- one context snapshot is reused for exact unread count and the first bounded summary page;
- auth-scope changes clear prior mutation feedback without polling or shadow client state.

### `FORUM-20AP`

- active initially non-public topic-created descriptors materialize only when the host publishes
  exact Forum notification recipient context;
- descriptors contain topic/category identifiers only and do not carry title, body, route, or
  recipient identity;
- bounded category subscription fanout still reauthorizes current Forum visibility for every
  exact candidate and retains public-only fallback when recipient context is absent.

## Remaining `NOTIFY-01`

- promote module-local migrations into verified global server migration
  composition;
- retention, tenant-wide scheduled reconciliation, payload redaction, repair,
  quarantine replay/purge, and administrative command state;
- keep arbitrary selected-ID bulk commands, preferences, digests, delivery
  transports, and external inbox adapters closed until matching owner commands
  exist.

## Remaining `NOTIFY-03`

- serialize active-manifest, artifact-security, maintenance, and node-readiness
  policy mutations with final candidate commits;
- grouping policy and bounded moderator-directory expansion;
- channel work enqueue only after candidate policy acceptance;
- PostgreSQL cursor/lease/contention/retry evidence;
- worker health, queue lag, retry, and quarantine metrics before default deployment
  enablement.

## Remaining `NOTIFY-07`

- tenant restrictions beyond effective module capability;
- block/mute management transports and relation change events;
- privacy and source rechecks on delayed delivery;
- tenant-wide scheduled reconciliation and payload redaction after source/profile
  changes;
- executable blocked/private/deleted and cross-tenant evidence beyond SQLite owner
  service coverage.

## UI gate

Admin and storefront remain module-owned. The storefront now uses the authenticated
`NotificationInboxStorefrontPort`, native SSR/hydrate adapters, GraphQL CSR/headless reads,
and the grouped Leptos UI. It must continue to use the exact owner unread count, preserve
bounded cursor semantics, navigate only after fresh owner authorization, and create no
shadow inbox storage. Group-state writes select the native SSR/hydrate path or the
GraphQL CSR/headless path without fallback, and both preserve owner write admission plus
caller idempotency semantics. The grouped bootstrap also tracks the reactive auth
transport context and reloads automatically when its token or tenant changes, while manual
post-command refresh remains authoritative. The admin package remains outside this storefront
completion claim and retains its explicit degraded state.

## Verification

### Maintainer verification set

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-modules --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications-api --all-targets --all-features
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --all-targets
cargo test -p rustok-modules --test policy_commit_guard_sqlite -- --nocapture
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sparse_page_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_worker_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_open_authorization_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_listing_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_state_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_count_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_mark_all_read_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_mark_all_unread_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_mark_all_archive_sqlite -- --nocapture
cargo test -p rustok-notifications --test inbox_reconcile_sqlite -- --nocapture
cargo test -p rustok-notifications --test outbox_intake_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_worker_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_policy_deferral_sqlite -- --nocapture
cargo test -p rustok-social-graph --test privacy_sqlite -- --nocapture
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
NOTIFICATIONS_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-notifications --test persistence_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-notifications-foundation.mjs
node scripts/verify/verify-notifications-runtime.mjs
node scripts/verify/verify-notifications-persistence.mjs
node scripts/verify/verify-notifications-source-fanout.mjs
node scripts/verify/verify-notifications-candidate-policy.mjs
node scripts/verify/verify-notifications-recipient-policy-runtime.mjs
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-notifications-candidate-worker.mjs
node scripts/verify/verify-notifications-outbox-intake.mjs
node scripts/verify/verify-notifications-fanout-worker.mjs
node scripts/verify/verify-forum-notification-inbox-open-authorization.mjs
node scripts/verify/verify-forum-notification-inbox-open-privacy.mjs
node scripts/verify/verify-forum-notification-inbox-listing.mjs
node scripts/verify/verify-forum-notification-inbox-state-mutations.mjs
node scripts/verify/verify-forum-notification-inbox-reconciliation.mjs
node scripts/verify/verify-forum-notification-inbox-mark-unread.mjs
node scripts/verify/verify-forum-notification-inbox-unread-count.mjs
node scripts/verify/verify-forum-notification-inbox-mark-all-read.mjs
node scripts/verify/verify-forum-notification-inbox-mark-all-unread.mjs
node scripts/verify/verify-forum-notification-inbox-mark-all-archive.mjs
node scripts/verify/verify-forum-notification-inbox-selected-state.mjs
node scripts/verify/verify-forum-notification-group-key-population.mjs
node scripts/verify/verify-forum-notification-inbox-group-listing.mjs
node scripts/verify/verify-forum-notification-inbox-group-summaries.mjs
node scripts/verify/verify-forum-notification-inbox-group-state.mjs
node scripts/verify/verify-forum-notification-inbox-storefront-port.mjs
node scripts/verify/verify-forum-notification-inbox-native-storefront-adapter.mjs
node scripts/verify/verify-forum-notification-inbox-grouped-storefront-ui.mjs
node scripts/verify/verify-forum-notification-navigation-badge.mjs
node scripts/verify/verify-forum-notification-inbox-grouped-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-open-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
cargo xtask module validate notifications
```

These commands were not executed while publishing the
`NOTIFY-03D/03E/03F/03G/03H/03I` and
`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN/20AO/20AP` source and documentation slices. `Cargo.lock` was
not regenerated because this work does not change the package dependency graph.

## Update rules

- keep canonical program status in the cross-module plan;
- never move producer subscriptions, contact data, or channel SDKs into this owner;
- never add synchronous notification calls to producer transactions;
- never create final notification rows before tenant/preference/privacy/source
  policy;
- never create channel delivery work in candidate finalization;
- add persistence or UI behavior only with matching contracts, migrations,
  degraded-mode notes, and verification commands.

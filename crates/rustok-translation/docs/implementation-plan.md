---
id: doc://crates/rustok-translation/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
last_reviewed: 2026-08-09
---

# Translation implementation plan

## Scope

Build the optional, admin-only translation control plane described by the
central plan without taking ownership of domain-localized data or runtime locale
selection.

## Current State

- The platform module, manifest, RBAC resources, migration source, and
  distribution feature are present.
- The module persists tenant-scoped inventory identities and provider cursor
  checkpoints without storing source or translated text.
- `TranslationInventoryService` consumes only the neutral provider registry,
  checks Translation workflow permission, delegates owner authorization to the
  provider, validates bounded requests, rejects cross-provider identities and
  non-advancing or missing cursors, collapses duplicate identities, and
  advances checkpoints with optimistic revision protection.
- Integration evidence covers cursor replay, tenant-isolated inventory and
  checkpoints, invalid bounds, provider outage, missing cursors, and
  cross-provider identity rejection without partial persistence. Bounded
  full-rescan drains the owner cursor, replaces one provider projection
  atomically, and rolls back if that checkpoint advances during listing.
  File-backed Translation-side inventory evidence now covers independent
  replica pools reading the same checkpoint revision: one advances and one
  receives `CheckpointConflict`, without duplicate inventory. Separate
  processes also recover from a provider outage, resume cursor sync, and
  atomically replace the projection through full-rescan. Actual isolated Media
  deployment evidence remains a provider-owned rollout gate.
- The first manual-workflow persistence slice creates tenant-scoped jobs,
  immutable owner-provider source snapshots, proposal/approval tables, and
  application-receipt tables. `TranslationWorkflowService` exposes idempotent
  job creation and item admission plus owner-validated proposal save, submit,
  and approval transitions. Request hashes, item/job revision CAS, persisted QA
  issues, current-proposal checks, and translator/reviewer separation guard the
  implemented flow.
- Approved proposals now apply through a durable intent state machine. The
  exact owner patch is persisted before invocation; same-key retries are bound
  to the original actor and request; retryable unknown outcomes remain
  `applying`; owner conflicts become terminal `conflict`; and `applied` is
  committed only together with a validated stable owner receipt.
- Apply attempts use expiring owner-execution leases. An operator holding both
  Translation Manage and Publish can recover an unknown outcome through a
  separately idempotent, actor-bound command with a mandatory reason and
  expected-attempt guard. Recovery is audited before owner invocation, cannot
  steal an unexpired lease, and reuses the original owner mutation key.
- Item assignment and unassignment are actor-bound idempotent commands with
  explicit expected revisions and append-only audit rows. Assigned drafts can
  be saved or submitted only by the assignee or a Translation manager.
- Job cancellation atomically cancels remaining mutable items, clears their
  assignments, preserves applied/excluded items, stores the operator reason,
  and rejects jobs with an unresolved owner apply.
- Job creation/cancellation, assignment changes, proposal submission/approval,
  apply request/completion/failure, and privileged recovery publish sealed,
  content-free `TranslationWorkflowEvent` contracts through the Core outbox in
  the same transaction as the corresponding workflow state.
- A non-empty job completes automatically only after all items become applied,
  excluded, or cancelled. Blocked items can return to their current approved
  proposal only through an actor-bound, idempotent, audited retry; stale and
  conflict items remain rebase-required.
- `translation_job_progress` is a content-free, transactionally maintained
  projection of item states, assignments, required/optional units,
  approved/applied units, completed resources, and character workload.
  `TranslationProgressService` provides tenant-isolated reads and a
  Manage-authorized deterministic rebuild that verifies source/proposal
  digests and owner receipt evidence. Reviewer queue and workload reads derive
  directly from tenant-scoped job-item and proposal evidence: the queue contains
  only submitted, unapproved current proposals for `in_review` items, while
  workload groups nonterminal work by current assignee, including unassigned
  work. Both reads fail closed on inconsistent workflow evidence and enforce
  explicit queue and workload bounds.
- Media, Taxonomy, Blog category, Navigation menu, and Pages metadata are registered owner
  providers with durable change-cursor repair and exact-locale aggregate
  coverage. Taxonomy applies term `name`, review-only `slug`, and optional
  `description` through owner CAS and the shared Outbox receipt ledger. Blog
  applies category copy through its service and publishes its existing Search
  reindex request. Navigation applies its menu name and every item title as one
  CAS-guarded locale aggregate through `MenuService`, using a content-free
  cursor journal without claiming a generic menu event. Pages applies exact
  title, review-only slug, and optional SEO metadata through `PageService`,
  keeping Fly/GrapesJS bodies outside this pilot. Translation validates provider
  facts and reports tenant-scoped projection freshness as `current`, `behind`,
  or `unknown` by opaque cursor equality.
- `TranslationPolicyService` owns a revisioned, tenant-scoped required-target
  locale subset. It validates through `TenantLocalePolicyPort`, rejects
  disabled/duplicate locales, stores the Tenant policy revision, and uses
  expected revision CAS plus actor-bound durable idempotency receipts. Stale
  policy remains readable for operator rebase, but required-target progress
  fails closed until replacement.
- Job creation rejects disabled source or target locales. Required-target
  provider progress uses the policy as its cross-locale denominator, excludes
  the source locale, uses checked totals, and reports worst-target freshness.
- `TranslationGlossaryService` now owns bounded tenant-scoped glossary
  list/read/create/update/term-replacement/lifecycle operations under the
  separate Translation Glossaries permission resource. It enforces normalized
  case-insensitive names, hierarchical owner/resource/field scope, locale
  policy, compare-and-set revisions, actor-bound durable idempotency, one
  preferred variant per concept, deterministic source/target conflict rules,
  and exact do-not-translate invariants.
- Glossary term rows are append-only across revision windows. Historical
  snapshots remain readable, while job creation accepts only an active current
  glossary revision owned by the same tenant and exact locale pair and stores
  that immutable binding on the job. Integration tests cover tenant isolation,
  replay/conflict behavior, revision snapshots, lifecycle, invalid terms, and
  job binding rejection.
- Save, submission, and approval QA now read the immutable glossary revision
  captured by the job. Applicable owner/resource/field scopes enforce
  preferred, allowed, forbidden, and do-not-translate policies with exact,
  whole-word, or substring matching and deterministic severity.
- The Translation Memory backend and operator lifecycle slice is implemented.
  Successful owner
  apply atomically ingests only user-reviewed public or tenant-private fields,
  with proposal, reviewer, owner-resource, source-hash, and apply-receipt
  provenance and replay-safe uniqueness. Tenant-scoped lookup supports exact
  and context-aware fuzzy ranking with bounded candidates, Unicode
  normalization, stable ordering, and explicit score evidence. Per-entry
  owner-lifecycle, retain-until, and legal-hold policies plus revision-guarded
  tombstone and purge use durable actor-bound idempotency receipts. Tombstoned
  entries leave lookup immediately; purge removes content while preserving
  content-free receipt evidence. Incremental synchronization and full rebuild
  now atomically project owner `Deleted` observations into matching memory
  entries as content-free lifecycle revision/time evidence; `Unavailable`
  never counts as deletion. The module-owned
  `translation_memory_retention` work adapter uses entry revision CAS and the
  existing receipts as its durable queue/completion boundary. It automatically
  tombstones expired `retain_until` and deleted `owner_lifecycle` entries,
  waits 24 hours before purge, and excludes legal hold and machine-operation
  pins. File-backed retention evidence now covers independent replica pools
  concurrently claiming the same revision and converging on one transition and
  receipt. Separate child processes also reclaim an item after the original
  runtime crashes post-claim, then complete tombstone and purge across
  successive restarts without duplicate lifecycle evidence.
- The bounded interchange core is implemented. Job export reads only immutable
  workflow snapshots and enforces item, field, value-byte, and total-document
  bounds. It exports only public or tenant-private non-excluded owner fields
  with exact identity/revision/hash/protected-token evidence. Import is atomic
  per item, binds schema/job/item/identity/source digest, rejects ineligible
  fields, and creates an `import` proposal through canonical owner validation
  and deterministic QA. Matching GraphQL export/import fields, native and
  GraphQL admin operations, and Leptos/Next Jobs controls now expose the same
  bounded interchange contract.
- Translation-owned interchange artifacts now persist only bounded document
  bytes at private tenant-scoped object-storage keys in a canonical camel-case
  wire document. The
  `translation_exchange_jobs` metadata records lifecycle, actor/idempotency
  binding, an exclusive short-lived import-processing lease, SHA-256 checksum,
  size, expiry/deletion, and content-free aggregate import outcomes; it does
  not duplicate document content. Reads verify size and checksum; a
  Translation-owned runtime worker deletes expired private objects even for an
  inactive tenant, and absent runtime storage fails closed. Export/import
  artifacts have a 5-minute to 7-day lifetime and an 8 MiB cap. An import
  starts only with enough remaining lifetime for the bounded lease, and
  concurrent retries fail retryably rather than executing twice. GraphQL,
  registered native HTTP, Leptos, and Next expose the same create/list/read/
  store/process lifecycle; direct bounded interchange remains the distinct
  inline/paste workflow.
- Translation now owns a lazily registered, fixed-cardinality
  `rustok_translation_*` telemetry collector and matching content-free tracing
  spans. It covers provider operation success/failure/latency, checkpoint
  freshness and elapsed age, authorized workflow-progress snapshots, owner
  apply attempts/replays/errors, Translation Memory match kind, QA violation
  family/severity, and interchange size/duration/rejection/import/expiry
  outcomes. It has no tenant, resource, object-key, cursor, locale, business
  text, or arbitrary provider-error label. Per-tenant job state remains the
  authorized progress read model, and broker-backed event lag remains a
  runtime consumer/outbox concern that must use durable positions rather than
  event age or opaque cursor values.
- Deterministic QA is implemented for resource lifecycle, required fields,
  empty required values, character limits, excluded fields, explicit protected
  tokens, whitespace shape, and unchanged-value warnings. It runs on save,
  review submission, and approval, combines typed owner validation evidence,
  and persists blocking failures.
- The module manifest now composes Translation-owned GraphQL query and mutation
  roots plus a capability-owned runtime-data factory. GraphQL publishes target
  discovery, policy, job/provider progress, inventory synchronization/rebuild,
  and every implemented workflow command through authenticated tenant-scoped
  `PortContext` values. The host remains provider-neutral.
- `rustok-translation-admin` now provides the shared transport and Leptos UI
  package for the same control plane: one typed operation/response contract, an
  SSR/hydrate native `#[server]` adapter over `HostRuntimeContext`, and a
  CSR/headless GraphQL adapter over `rustok-graphql`. Both paths cover the same
  49 operations, including the six glossary and six memory operations,
  bounded job export and atomic item import, object-storage interchange
  artifact create/list/read/store/process, reviewer queue/workload reads, and
  private append-only job/item workflow-note list/create/resolve operations, plus
  non-billable machine-translation estimation, machine-proposal generation,
  status, cancellation, and recovery. Both module-owned workbenches expose the
  same machine workflow controls plus revision-guarded assignment/unassignment,
  blocked-item retry, job cancellation, and owner-apply recovery; GraphQL
  documents are validated against the module-owned schema, and every
  idempotency-bound command carries its caller key into `PortContext`.
- Translation exposes one redacted public-error classifier shared by GraphQL
  and native adapters, so database/internal details never become client
  messages and stable Translation error codes stay aligned across transports.
- Registered HTTP server-function tests now execute the Translation control
  plane through the URL-encoded Leptos protocol and the same dispatch and
  owner-service paths used by the host. Runtime evidence covers policy,
  glossary lifecycle, bounded direct interchange and object-storage artifacts,
  assignment, manual proposal
  save/submit/review/apply, deterministic QA rejection, cancellation, retry,
  unknown-outcome apply recovery, job progress and rebuild, reviewer queue and
  workload reads, private workflow-note create/list/resolve, Translation Memory lookup/lifecycle, inventory sync/full
  rebuild, provider/required-target progress, and machine generation/status/
  cancellation/audited recovery through
  a deterministic neutral `MachineTranslationPort` factory. It also covers
  degraded machine-provider health, cross-tenant isolation, mismatched
  extracted host contexts, empty idempotency keys, and fail-closed missing
  runtime dependencies. A server-level runtime test now also executes the
  registered read-policy function through the production application router
  with tenant resolution/cache, locale negotiation, JWT/session resolution,
  RBAC, channel resolution, rate limiting, and security headers; it verifies
  the effective `de-DE` response locale and rejects a valid token replayed
  against another tenant. Production AI execution remains open.
- The manifest now publishes and mounts the Leptos workbench, and the matching
  `@rustok/translation-admin` package owns the Next workbench over the same
  GraphQL contract. Both workbenches expose six tabs and keep glossary and
  memory selection in `glossary_id` and `memory_entry_id`. The Next route now
  enforces tenant module enablement through the host `ModuleGuard`.
  Authenticated browser execution verified the real Next route, URL-owned tab
  selection, accessible tab semantics, bounded interchange controls, and the
  disabled-module fallback. A compiled Leptos CSR browser fixture now renders
  the production `TranslationAdmin` component with authenticated host context
  and the real browser router. Playwright verified all six URL-owned tabs,
  roving focus with arrow/Home/End navigation, exact tab/panel relationships,
  and localized form-label associations; axe reported zero violations on every
  tab. Transport execution remains covered by the registered native HTTP and
  authenticated GraphQL suites rather than by the isolated UI fixture.
  Production AI bridge composition and missing-keyring behavior are verified.
  Deterministic composed AI runtime evidence covers ordered fallback,
  fail-closed output-schema validation, sanitized failure, exact settlement,
  request conflict, in-flight cancellation with reservation release, and
  quota rejection before provider execution plus encrypted restart replay
  without a second call or bill. Configuration-level provider unavailability
  produces typed degraded health; live external-provider runtime evidence
  remains open. The ignored operator-only durable structured-runtime probe is
  ready for an approved billable deployment run.
  Separate-process file-backed evidence already covers expired AI attempt
  recovery, reservation preservation, and reclaim with a new lease.
- Translation now owns a bounded `MachineTranslationPort` SPI with explicit
  source/target locales, stable unit/source identities, field
  profile/strategy/classification, protected tokens, exact-digest-bound
  glossary and Translation Memory context, provider health,
  execution/attempt/usage/cost evidence, and a
  mandatory review-required result. `rustok-translation` imports no AI crate.
- The SPI and `TranslationMachineService` expose a conservative estimate path
  over the same canonical batch, glossary, memory, routing, attempt, and price
  snapshot inputs as execution. Estimate requests authorize and validate like
  generation but create no operation, proposal, memory pin, budget reservation,
  or provider call.
- The stateless `rustok-ai-translation` support crate now maps that SPI to the
  AI-owned `AiStructuredTaskPort`, owns the `machine_translation` policy and
  typed schemas, and rejects stale policy, missing/extra units, placeholder
  drift, length violations, and missing usage/attempt evidence. Each complete
  packet is at least tenant-private because it includes tenant-scoped resource
  context; personal and sensitive units raise its classification, and the
  AI-owned provider policy rejects unpermitted egress before registration or a
  provider call. The explicit
  optional distribution bridge now publishes the Translation-owned lazy
  factory, and the production server selects it. Missing deployment keyring
  composition is verified as optional and fail-closed; live provider evidence
  remains open.
- `TranslationMachineService` now owns the proposal-generation command. It
  selects explicit AI-eligible fields from the immutable job snapshot,
  projects only applicable terms from the job-bound glossary revision, adds at
  most five tenant-scoped Translation Memory suggestions per unit, validates
  provider capacity and health, and invokes only `MachineTranslationPort`.
  Successful output is revalidated and saved through
  `TranslationWorkflowService::save_proposal` with `ProposalOrigin::Ai`, so
  owner validation and deterministic QA cannot be bypassed.
- `translation_machine_operations` is the durable, tenant-scoped,
  actor/idempotency-bound handoff journal. It stores request/context digests,
  provider policy, execution/attempt/usage/cost evidence, diagnostic codes, and
  the resulting proposal identity, but never duplicates source, memory, or
  translated content. A crash after AI completion can replay the AI execution
  and proposal save with deterministic child keys while the bound request
  projection is intact; any projection drift fails with an idempotency
  conflict rather than submitting a different billable request.
- Registered operations pin normalized Translation Memory entry identities,
  ordering, and match scores without duplicating segment content. Replay reads
  the exact pinned entries even after tombstone; purge is blocked while a pin
  exists, and pins are released atomically on completion or explicit
  cancellation.
- Translation-owned cancellation is actor-bound and idempotent. The original
  requester can cancel a registered operation; another actor needs Translation
  Manage. Cancellation records the private reason, marks the operation
  `cancelled`, records AI propagation status/execution/error evidence, and
  releases memory pins in one transaction. The AI adapter cancels by stable
  execution idempotency identity, including before execution registration;
  exact receipt replay retries incomplete propagation. Once proposal save has
  entered `saving`, cancellation fails closed because the canonical save
  outcome may already be in flight.
- GraphQL and native Leptos transports expose the same machine estimate,
  proposal, cancellation, and recovery commands plus content-free
  local/provider status.
  Recovery is Manage/Update-authorized, actor/idempotency-bound, revision
  guarded, and persists a content-free audit receipt before retrieving an
  already completed result through the stable provider key. It reconstructs
  and revalidates the original request digest, requires an exact content-free
  AI execution binding for that batch, and resumes canonical proposal save
  without another billable execution. Manual Translation surfaces remain
  available when the optional machine provider is absent or fails to
  materialize.
- File-backed separate-process recovery now covers both canonical `saving`
  crash boundaries: provider completion before proposal persistence, and
  proposal persistence before operation completion. The original runtime closes
  its database before a child process resumes the audited command. Evidence
  verifies one proposal, one recovery receipt, atomic memory-pin release,
  preserved proposal identity after the second boundary, and terminal replay
  without another provider recovery or billable execution.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - module-owned core and neutral provider dependency are separated;
  - module-owned GraphQL roots and manifest runtime composition are compiled;
  - the isolated server composition profile builds with
    `--no-default-features --features mod-translation`;
  - the native/GraphQL admin transport compiles and has schema and idempotency
    parity tests;
  - the manifest publishes the module-owned Leptos `core/transport/ui`
    workbench and both host composition roots select it without owning
    Translation business UI;
  - the matching Next package uses the host GraphQL executor, host locale, and
    the same URL-owned `tab`, `glossary_id`, and `memory_entry_id` selection
    contracts;
  - authenticated GraphQL schema tests execute bounded direct export/import and
    the private object-storage artifact create/list/read/store/process lifecycle
    through the real `AuthContext` and `RequestContext`; they verify invalid
    bounds, stale source rejection, aggregate import outcomes, cross-tenant
    isolation, and mismatched tenant denial;
  - the artifact GraphQL fixture explicitly supplies `StorageRuntime`. The
    production server still needs to compose that initialized runtime for a
    Translation-only GraphQL profile instead of gating it on `mod-media`;
  - registered native HTTP server-function tests execute policy, glossaries,
    bounded direct interchange and private object-storage artifacts, assignment,
    manual workflow/apply, QA rejection,
    cancellation, retry, apply recovery, job progress, reviewer queue/workload,
    Translation Memory, inventory rebuild, provider/required-target progress,
    and machine generation/status/cancellation/recovery with URL-encoded requests and
    extracted authenticated host contexts; negative evidence includes invalid
    bounds, stale source rejection, cross-tenant isolation, context mismatch,
    idempotency validation, owner failure, unknown outcome, degraded machine
    health, and missing-runtime failure;
  - authenticated Next browser execution verifies the Jobs/interchange and
    Glossaries surfaces, URL-owned tab state, accessible tab semantics, and the
    host module-disabled fallback;
  - a file-backed separate-process test closes the original runtime and recovers
    both machine `saving` crash boundaries, proving proposal uniqueness,
    proposal-identity preservation, one audit receipt, memory-pin release, and
    provider-free terminal replay;
  - the production application-router runtime test verifies tenant cache and
    resolution, locale negotiation, JWT/session/RBAC, channel, rate-limit, and
    security middleware around the registered native read-policy function,
    including cross-tenant token rejection;
  - compiled Leptos CSR browser execution verifies all six URL-owned tabs,
    literal `aria-selected` state, one roving tab stop, arrow/Home/End focus and
    URL synchronization, tab/panel relationships, and localized form-label
    associations; axe reports zero violations on every tab.
- Last verified at (UTC): 2026-07-30
- Owner: Translation module maintainers

## Milestones

1. File-backed Translation-side multi-replica/restart evidence is complete for
   inventory replay, stale-checkpoint conflict, provider outage recovery, and
   atomic full-rescan. Tenant isolation is covered by integration execution.
   Complete the provider-owned gate with isolated live Media deployment and
   production-database multi-replica evidence.
2. Registered native HTTP server-function parity is runtime-verified for
   recovery, assignment, cancellation, retry, policy, glossaries, Translation
   Memory, QA, progress, reviewer queue/workload, inventory, direct interchange,
   private artifact lifecycle, and
   manual workflow operations.
3. File-backed independent-pool and separate-process evidence is complete for
   automated Translation Memory retention: duplicate replica claims converge
   on one revision/receipt, a crash after claim is reclaimed, and successive
   restarts complete tombstone then purge. Owner-deletion propagation,
   bounded memory, and immutable glossary projections are implemented. Retain
   production-database multi-replica evidence separately.
4. Full application-router middleware execution is runtime-verified for the
   registered native read-policy function, including tenant cache/resolution,
   locale, JWT/session/RBAC, channel, rate-limit, security headers, and
   cross-tenant rejection. Authenticated GraphQL and registered native HTTP
   tests separately cover bounded direct interchange and private artifact
   lifecycle, malformed bounds, stale source rejection, tenant isolation, and
   successful import through canonical QA.
   The server GraphQL host must also attach initialized `StorageRuntime` when
   Translation is enabled without Media before the artifact lifecycle can claim
   that deployment-profile evidence.
5. Registered HTTP parity for machine generation/status/cancellation/recovery
   is runtime-verified through a deterministic neutral provider, including
   degraded health and audited stuck-save recovery. Production-provider
   execution evidence remains in milestone 7.
6. Leptos browser/accessibility evidence is complete for the compiled
   production component: all six URL-owned tabs, semantic tab/panel state,
   roving keyboard focus, localized form-label associations, and zero axe
   violations are verified. Authenticated Next URL-state, semantic tab,
   interchange-control, and module-disablement evidence is also complete.
7. The implemented optional `ai-translation` distribution bridge is enabled in
   the production profile, and composed missing-keyring behavior is verified as
   optional and fail-closed. Deterministic composed evidence covers ledger
   fallback, invalid output, replay/conflict, exact budget settlement, in-flight
   cancellation, quota rejection, and restart without duplicate provider calls
   or billing.
   Collect live external-provider runtime outage/degradation, restart, and
   recovery evidence with the ignored durable structured-runtime probe and
   retain the run output. File-backed separate-process AI recovery is complete;
   retain production-database multi-replica concurrency evidence separately.
8. File-backed separate-process restart evidence is complete for the audited
   `saving` recovery command. It covers crash after provider completion and
   crash after proposal save, while proving one canonical proposal, stable
   proposal identity, one audit receipt, atomic memory-pin release, and replay
   without another provider call.

## Verification

- `cargo check -p rustok-translation`
- `cargo test -p rustok-translation`
- `cargo test -p rustok-translation --features graphql`
- `cargo clippy -p rustok-translation --all-targets --all-features -- -D warnings`
- `cargo test -p rustok-translation-admin`
- `cargo check -p rustok-translation-admin --features ssr`
- `cargo test -p rustok-server --lib --no-default-features --features mod-translation application_router_executes_authenticated_server_function`
- `cargo xtask module validate translation`
- `cargo xtask validate-manifest`
- `cargo check -p rustok-server --lib --no-default-features --features mod-translation`
- `node scripts/verify/verify-translation-surface-registry.mjs`
- `npm run verify:translation:admin-boundary`
- `cargo test -p rustok-ai-translation`
- `cargo test -p rustok-distribution --no-default-features --features ai-translation selected_ai_translation_bridge_publishes_factory_and_stays_optional_without_keyring`
- `npm run verify:ai-translation:boundary`

## Update Rules

- Never query or mutate owner tables from this module.
- Never count runtime fallback as an exact translation.
- Never let AI output apply owner data without deterministic validation and
  review.
- Keep the local FFA/FBA block and central readiness board synchronized when
  transport or UI appears.
- Keep `rustok-translation-targets` dependency-neutral even if its directory is
  physically colocated with this module later.

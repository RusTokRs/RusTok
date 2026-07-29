---
id: doc://crates/rustok-translation/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
last_reviewed: 2026-07-28
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
  digests and owner receipt evidence.
- Media is the first owner provider with durable change-cursor repair and
  exact-locale aggregate coverage. Translation validates provider facts and
  reports tenant-scoped projection freshness as `current`, `behind`, or
  `unknown` by opaque cursor equality.
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
  content-free receipt evidence.
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
  35 operations, including the six glossary and six memory operations plus
  machine-proposal generation, status, and cancellation; GraphQL
  documents are validated against the module-owned schema, and every
  idempotency-bound command carries its caller key into `PortContext`.
- Translation exposes one redacted public-error classifier shared by GraphQL
  and native adapters, so database/internal details never become client
  messages and stable Translation error codes stay aligned across transports.
- The manifest now publishes and mounts the Leptos workbench, and the matching
  `@rustok/translation-admin` package owns the Next workbench over the same
  GraphQL contract. Both workbenches expose six tabs and keep glossary and
  memory selection in `glossary_id` and `memory_entry_id`. Live browser,
  accessibility, module-disablement, and authenticated native/GraphQL runtime
  evidence remain open, as do owner-deletion propagation and automated
  retention enforcement, bounded interchange, production AI enablement, and
  live AI/runtime evidence.
- Translation now owns a bounded `MachineTranslationPort` SPI with explicit
  source/target locales, stable unit/source identities, field
  profile/strategy/classification, protected tokens, glossary and Translation
  Memory context, provider health, execution/attempt/usage/cost evidence, and a
  mandatory review-required result. `rustok-translation` imports no AI crate.
- The stateless `rustok-ai-translation` support crate now maps that SPI to the
  AI-owned `AiStructuredTaskPort`, owns the `machine_translation` policy and
  typed schemas, and rejects stale policy, missing/extra units, placeholder
  drift, length violations, and missing usage/attempt evidence. The explicit
  optional distribution bridge now publishes the Translation-owned lazy
  factory; production-profile enablement and live evidence remain open.
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
- GraphQL and native Leptos transports expose the same machine-proposal and
  cancellation commands plus content-free local/provider status. Manual
  Translation surfaces remain available when the optional machine provider is
  absent or fails to materialize. Operator recovery for indefinitely `saving`
  operations remains open.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - module-owned core and neutral provider dependency are separated;
  - module-owned GraphQL roots and manifest runtime composition are compiled;
  - the native/GraphQL admin transport compiles and has schema and idempotency
    parity tests;
  - the manifest publishes the module-owned Leptos `core/transport/ui`
    workbench and both host composition roots select it without owning
    Translation business UI;
  - the matching Next package uses the host GraphQL executor, host locale, and
    the same URL-owned `tab`, `glossary_id`, and `memory_entry_id` selection
    contracts;
  - live browser, accessibility, module-disablement, and authenticated
    native/GraphQL runtime parity evidence remains required.
- Last verified at (UTC): 2026-07-29
- Owner: Translation module maintainers

## Milestones

1. Complete Media multi-replica evidence for the implemented inventory replay,
   tenant isolation, stale-checkpoint conflict, provider outage, and
   full-rescan recovery contracts.
2. Mount and runtime-verify the implemented native server-function parity for
   recovery, assignment, cancellation, retry, policy, QA, progress, inventory,
   and workflow operations.
3. Complete owner-deletion propagation and automated retention enforcement for
   Translation Memory. The bounded memory and immutable glossary projections
   into machine requests are implemented.
4. Add bounded owner-aware import/export.
5. Complete live native/GraphQL parity evidence, including tenant isolation and
   host disablement.
6. Complete Leptos/Next browser, accessibility, URL-state, and module
   disablement evidence.
7. Enable the implemented optional `ai-translation` distribution bridge in
   the production profile and collect live ledger, replay, budget, fallback,
   cancellation, restart, and recovery evidence.
8. Add an audited recovery command for indefinitely `saving` operations using
   the implemented stable-key AI result recovery contract.

## Verification

- `cargo check -p rustok-translation`
- `cargo test -p rustok-translation`
- `cargo test -p rustok-translation --features graphql`
- `cargo test -p rustok-translation-admin`
- `cargo check -p rustok-translation-admin --features ssr`
- `cargo xtask module validate translation`
- `cargo xtask validate-manifest`
- `cargo check -p rustok-server --lib --no-default-features --features mod-translation`
- `node scripts/verify/verify-translation-surface-registry.mjs`
- `npm run verify:translation:admin-boundary`
- `cargo test -p rustok-ai-translation`
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

# Translation module contract

## Purpose

The Translation module is the tenant translation control plane. It coordinates
work while each domain owner remains authoritative for localized business data.

## Responsibility Zone

The module owns inventory projections, provider checkpoints, translation jobs,
proposals, review and approval state, assignments, quality evidence, translation
memory, glossaries, bounded direct interchange and private artifact lifecycle,
and owner-application receipts.
It also owns the provider-neutral `MachineTranslationPort` SPI and bounded
request/result evidence. AI routing and inference remain outside this module.
Machine requests carry glossary or Translation Memory context only as an exact
digest-bound non-empty projection; empty subsets carry no binding, so replay
and recovery cannot substitute context.
Every machine-translation packet is at least `tenant_private`, including a
batch of public units, because the packet contains tenant-scoped resource
identity and may carry glossary, memory, style, or evidence context. Personal
and sensitive units raise its classification. The AI-owned provider policy
enforces that classification before routing, reservation, and external egress.

The implemented persistence foundation owns:

- `translation_inventory_resources`;
- `translation_provider_checkpoints`;
- `translation_policies`;
- `translation_policy_receipts`;
- `translation_glossaries`;
- `translation_glossary_terms`;
- `translation_glossary_receipts`;
- `translation_memory_entries`;
- `translation_jobs`;
- `translation_job_items`;
- `translation_proposals`;
- `translation_item_assignments`;
- `translation_job_cancellations`;
- `translation_job_progress`;
- `translation_item_retries`;
- `translation_apply_operations`;
- `translation_apply_recoveries`;
- `translation_apply_receipts`;
- `translation_machine_operations`;
- `translation_machine_memory_bindings`;
- `translation_machine_cancellations`;
- `translation_machine_recoveries`;
- `translation_exchange_jobs`;
- bounded provider change-cursor synchronization with optimistic checkpoint
  revision protection, provider-identity isolation, and cursor-progress
  validation;
- bounded full-rescan recovery that atomically replaces one provider's
  inventory only while its checkpoint remains unchanged;
- file-backed inventory concurrency where independent database pools read one
  checkpoint revision and converge on one update plus one typed conflict, and
  separate-process outage recovery that resumes cursor sync before an atomic
  full-rescan;
- idempotent job creation and owner-provider-backed immutable item snapshots
  with request hashes and job revision CAS;
- tenant-scoped glossary metadata and lifecycle CAS, durable actor-bound
  idempotency receipts, bounded concept/variant validation, and append-only term
  rows whose revision windows preserve every job-readable terminology snapshot;
- immutable optional job bindings to an active, current glossary revision with
  the same source/target locale pair;
- approved-and-applied translation-memory ingestion in the owner-apply
  transaction. The default policy admits only user-reviewed public or
  tenant-private owner fields, rejects `und`, and preserves proposal/reviewer/
  resource/apply-receipt provenance;
- bounded tenant-scoped exact and contextual-fuzzy memory lookup with Unicode
  normalization and explainable deterministic basis-point scoring;
- bounded owner-aware job interchange. Export carries immutable
  owner/resource/revision/hash evidence and only public or tenant-private
  non-excluded fields. Import is atomic per item, rejects stale source digests
  and ineligible fields, and creates an `import` proposal only through
  canonical owner validation and deterministic QA;
- tenant-scoped object-storage interchange artifacts. Only bounded document
  bytes live at private object keys in a canonical camel-case wire document;
  lifecycle metadata records checksum, byte length, actor/idempotency binding,
  a short exclusive import-processing lease,
  expiry/deletion, and aggregate import outcomes. Reads verify integrity,
  a module-owned runtime worker deletes expired artifacts independently of
  later tenant requests, missing storage fails closed, and concurrent import
  retries fail retryably instead of running twice;
- revision-guarded memory retention, legal hold, tombstone, and purge with
  actor-bound durable idempotency receipts. Tombstoned entries are excluded
  from lookup, while purge preserves content-free operation evidence. Owner
  `Deleted` observations atomically add content-free lifecycle evidence during
  inventory synchronization. The module-owned runtime worker automatically
  tombstones expired or owner-deleted entries and purges them after a 24-hour
  grace period, while excluding legal hold and machine-operation pins.
  File-backed evidence verifies concurrent independent replica pools converge
  on one revision and receipt, and separate processes reclaim post-claim work
  across tombstone and purge restarts;
- owner-validated proposal drafts, review submission, and approval transitions
  with operation-specific idempotency bindings, item revision CAS, persisted QA
  evidence, and translator/reviewer separation;
- revisioned required-target-locale policy validated against the current
  Tenant-owned enabled-locale projection. Writes use expected-revision CAS,
  actor-bound durable idempotency receipts, and retain the exact Tenant locale
  policy revision used for validation;
- deterministic platform QA on save, submission, and approval, combined with
  typed owner warnings/errors. Blocking rules cover lifecycle, required fields,
  empty required values, maximum character counts, excluded fields,
  protected-token multiplicity, owner-declared whitespace shape, forbidden
  glossary terms, missing preferred terms, and changed do-not-translate terms.
  Allowed non-preferred terms produce warnings. Every pass reads the immutable
  historical glossary revision captured by the job;
- durable owner-apply intents that preserve the exact approved patch before the
  external call, reconcile retryable unknown outcomes under the same actor and
  idempotency key, and transition to `applied` only with a validated stable
  owner receipt;
- lease-guarded operator recovery that requires Translation Manage and Publish,
  persists the recovery actor, reason, request binding, and observed attempt
  count before owner invocation, and cannot steal an unexpired apply lease;
- actor-bound, idempotent assignment commands with expected item revision CAS,
  append-only audit history, and assignee enforcement on draft and submit;
- job cancellation with a mandatory audited reason, expected job revision,
  per-item CAS, applied/excluded preservation, and a fail-closed guard while any
  owner apply outcome remains unresolved;
- automatic successful job completion only after every item reaches
  `applied`, `excluded`, or `cancelled`, with `conflict`, `stale`, and
  `blocked` remaining visibly incomplete;
- actor-bound explicit retry from `blocked` to the existing approved proposal,
  with an append-only private reason and no blind retry for stale owner
  revisions or conflicts;
- a content-free per-job progress projection covering workflow states,
  assignments, required/optional units, approved/applied units, completed
  resources, and character workload. Workflow mutations refresh it
  transactionally, and `TranslationProgressService::rebuild_job_progress`
  repairs it from source snapshots, current proposals, and owner receipts;
- provider-level exact-locale progress through the neutral owner SPI.
  `TranslationProgressService::read_provider_progress` validates owner facts
  and compares the owner cursor with the tenant/provider inventory checkpoint,
  exposing only truthful `current`, `behind`, or `unknown` freshness. Opaque
  cursors are never treated as numeric distances;
- required-target progress that reads the Translation policy, omits the current
  source locale, sums every required source/target provider fact with checked
  arithmetic, and reports the worst target freshness;
- typed content-free workflow events for job, assignment, proposal, apply, and
  recovery transitions, including job completion and explicit item retry,
  persisted through the Core outbox in the same transaction as their state
  change;
- content-free machine-operation status, stable-key provider cancellation, and
  one actor/idempotency-bound recovery receipt per operation. Recovery is
  accepted only for the exact observed `saving` revision, revalidates the
  original generation command and reconstructed request digest, retrieves only
  an already completed provider result whose content-free execution binding
  matches that exact batch, and resumes canonical proposal save without another
  billable translation call.

File-backed separate-process evidence closes the original runtime before
recovery and covers both durable `saving` states: no proposal persisted yet,
and the canonical proposal already persisted while operation completion was
interrupted. Both paths produce one proposal and one audit receipt, release
memory pins on completion, preserve an already persisted proposal identity, and
make terminal replay provider-free.

Inventory rows never copy source or translated field values. Source text is
stored only in workflow item snapshots with an explicit job/tenant boundary;
owner tables remain canonical.

## Observability

Translation owns a lazily registered `rustok_translation_*` collector in the
single process telemetry registry. It reports content-free provider operation
availability and latency, observed checkpoint freshness and age, workflow
apply attempts/replays/owner-error categories, Translation Memory strongest
match kind, QA warning/error family, and interchange artifact operation,
size, aggregate import outcome, rejection category, and expiry-cleanup result.
The observer creates matching `tracing` spans for provider, workflow, and
interchange boundaries.

Every label is selected from a fixed module enum. No metric or trace field
contains a tenant, actor, job, item, resource, object key, opaque cursor,
locale, source/translated value, glossary term, or arbitrary provider error
code. Checkpoint age is an elapsed-time observation only; opaque cursors are
never interpreted as numeric lag or distance.

System-health metrics intentionally do not provide a cross-tenant job-state
gauge. `TranslationProgressService` remains the canonical tenant-authorized
content-progress surface, while its aggregate counts are emitted only as
content-free trace fields. Broker-backed event-consumer lag remains owned by
the runtime consumer/outbox observer and must be derived from a durable
partition checkpoint, not event age or a Translation cursor.

## Integration

Owner modules register `TranslationTargetProvider` implementations through
`ModuleRuntimeExtensions`. The module consumes the resulting
`TranslationTargetRegistry`; missing providers and missing capabilities fail
explicitly.

Translation policy and job creation consume
`rustok-tenant::TenantLocalePolicyPort`. Translation never reads
`tenant_locales`; disabled source/target job locales fail before persistence,
and a stored required-target policy becomes explicitly stale when its bound
Tenant locale-policy revision changes. Stale policy remains readable with its
CAS revision and disabled-locale evidence, while required-target progress fails
closed until an authorized replacement revalidates it.

Registered pilot aggregates are `media/asset`, `taxonomy/term`,
`navigation/menu`, and `pages/page_metadata`. Canonical Blog Category copy is
not a separate Translation aggregate: Blog binds its categories to same-ID
Taxonomy Category terms, so exact Category `name`, review-only `slug`, optional
`description`, revision, apply, and change-cursor behavior are supplied only by
`taxonomy/term`. The former `blog/category` provider, Blog Category Translation
change journal, and Blog-local Category translation storage are retired and
must not be recreated as a control-plane apply path.

Translation never reads owner tables directly: each registered owner supplies
exact target facts and an opaque owner cursor. Media counts source-eligible
active assets in a stable change window; Taxonomy counts active terms including
Category terms consumed by Blog; Navigation counts only full exact menu
aggregates; Pages counts active Pages with an exact source metadata row.
Navigation exposes a required menu name plus one required exact title per menu
item; Pages exposes title, review-only slug, and optional SEO metadata. Runtime
locale fallback does not contribute to any aggregate. Production enablement
continues to require the documented provider-specific database evidence; Blog
Category does not add a second provider evidence gate beyond the canonical
Taxonomy owner.

`rustok-translation-targets` remains a separate Cargo package even if its
physical directory is later moved under `crates/rustok-translation/`. This
preserves the dependency direction: owners may depend on the neutral SPI but
must never depend on the Translation control-plane crate.

Owner providers authorize every apply caller. Their idempotency request hash is
actor-neutral and binds the tenant, operation kind, exact patch, and original
mutation key. This lets a separately authorized recovery operator reconcile an
unknown owner outcome without creating a second write identity; recovery audit
and authorization remain actor-specific in the Translation control plane.

`TranslationWorkflowService` requires `TransactionalEventBus`; it has no
non-transactional or eventless constructor. Outbox failure rolls back the local
workflow mutation. If the owner call already completed, durable apply
reconciliation retries the same owner mutation identity before recording the
Translation receipt and completion event.

Every provider patch-validation response must carry typed warning/error
severity and have acceptance consistent with that severity. Template-aware
owners publish an explicit protected-token ledger in each field snapshot;
Translation compares exact token multiplicity without guessing placeholder
syntax.

The module manifest publishes `graphql::TranslationQuery` and
`graphql::TranslationMutation`. The GraphQL surface exposes target discovery,
policy, versioned glossary list/read/create/update/term replacement/lifecycle,
Translation Memory list/read/exact-or-contextual lookup and
retention/tombstone/purge lifecycle, job and provider progress, inventory
synchronization/rebuild, and the implemented workflow commands. Job creation
accepts an optional immutable glossary revision binding. Authentication,
tenant, locale, permission claims, deadlines, and caller-supplied idempotency
keys are converted into the same transport-neutral `PortContext` used by
native adapters. Runtime data is materialized by
`graphql_runtime::attach_schema_data`; owner modules remain visible only
through the neutral registry. The optional `StorageRuntime` is used only for
the private interchange artifact lifecycle; its absence fails that lifecycle
closed without creating an in-memory fallback.

The module-owned `rustok-translation-admin` package defines a single typed
operation/response boundary over that service contract. SSR/hydrate selects a
native `#[server]` adapter backed by `HostRuntimeContext`; CSR/headless selects
the `rustok-graphql` adapter. The native endpoint reuses host auth, tenant,
locale, permission, deadline, and idempotency evidence and never reads an owner
table. Both adapters share Translation's redacted public-error classification.
Its six-tab Leptos `core/transport/ui` workbench is manifest-mounted in
`apps/admin`; the matching `@rustok/translation-admin` package is mounted by
the Next host through a thin client wrapper and uses the same 49-operation
GraphQL contract. The Workflow surface includes a non-billable conservative
machine-translation estimate derived from AI-owned tenant routing and immutable
price snapshots before proposal generation, along with generation, status,
cancellation, and recovery controls. It also provides revision-guarded
assignment/unassignment, bounded reviewer queue and workload reads, private
append-only job/item workflow notes, blocked-item retry, job cancellation, and
owner-apply recovery. Notes are bounded and resolution-revision-guarded; their
bodies remain out of Translation Memory, machine requests, owner application,
and event bodies. The Jobs surface provides
bounded immutable snapshot export and atomic per-item import through canonical
QA, plus private object-storage artifact create/list/read/store/process with a
5-minute to 7-day lifetime, size/checksum validation, and aggregate conflict
reports. Both clients use `memory_entry_id` for explicit memory selection and
never auto-select the first entry.

## Verification

- `cargo check -p rustok-translation`
- `cargo test -p rustok-translation`
- `cargo test -p rustok-translation --features graphql`
- `cargo test -p rustok-translation-admin`
- `cargo check -p rustok-translation-admin --features ssr`
- `cargo xtask module validate translation`
- `cargo xtask validate-manifest`
- `npm run verify:translation:admin-boundary`

## Related Documents

- [Implementation plan](implementation-plan.md)
- [Central translation plan](../../../docs/modules/translation-implementation-plan.md)
- [Translation surface registry](../../../docs/modules/translation-surfaces.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)

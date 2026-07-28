# Translation module contract

## Purpose

The Translation module is the tenant translation control plane. It coordinates
work while each domain owner remains authoritative for localized business data.

## Responsibility Zone

The module owns inventory projections, provider checkpoints, translation jobs,
proposals, review and approval state, assignments, quality evidence, translation
memory, glossaries, interchange operations, and owner-application receipts.

The implemented persistence foundation owns:

- `translation_inventory_resources`;
- `translation_provider_checkpoints`;
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
- bounded provider change-cursor synchronization with optimistic checkpoint
  revision protection, provider-identity isolation, and cursor-progress
  validation;
- bounded full-rescan recovery that atomically replaces one provider's
  inventory only while its checkpoint remains unchanged;
- idempotent job creation and owner-provider-backed immutable item snapshots
  with request hashes and job revision CAS;
- owner-validated proposal drafts, review submission, and approval transitions
  with operation-specific idempotency bindings, item revision CAS, persisted QA
  evidence, and translator/reviewer separation;
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
- typed content-free workflow events for job, assignment, proposal, apply, and
  recovery transitions, including job completion and explicit item retry,
  persisted through the Core outbox in the same transaction as their state
  change.

Inventory rows never copy source or translated field values. Source text is
stored only in workflow item snapshots with an explicit job/tenant boundary;
owner tables remain canonical.

## Integration

Owner modules register `TranslationTargetProvider` implementations through
`ModuleRuntimeExtensions`. The module consumes the resulting
`TranslationTargetRegistry`; missing providers and missing capabilities fail
explicitly.

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

## Verification

- `cargo check -p rustok-translation`
- `cargo test -p rustok-translation`
- `cargo xtask module validate translation`
- `cargo xtask validate-manifest`

## Related Documents

- [Implementation plan](implementation-plan.md)
- [Central translation plan](../../../docs/modules/translation-implementation-plan.md)
- [Translation surface registry](../../../docs/modules/translation-surfaces.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)

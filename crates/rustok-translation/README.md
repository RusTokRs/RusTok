# rustok-translation

## Purpose

`rustok-translation` is the optional RusToK control-plane module for
owner-safe translation inventory, workflow, review, memory, glossaries, and
machine-translation orchestration.

## Responsibilities

- Maintain rebuildable tenant translation inventory and provider checkpoints.
- Maintain a content-free, rebuildable per-job workflow progress projection.
- Persist tenant-scoped jobs, immutable owner source snapshots, proposal and
  approval records, and owner-application receipts.
- Call owner modules only through `rustok-translation-targets`; never read or
  write owner translation tables directly.
- Keep machine-translation output review-required and route AI execution
  through the future `rustok-ai-translation` adapter.
- Remain fully usable for manual translation when the AI capability is absent.

## Entry points

- `TranslationModule`
- `TranslationInventoryService`
- `TranslationInventoryRebuildResult`
- `TranslationInventorySyncResult`
- `TranslationWorkflowService`
- `TranslationProgressService`
- `JobProgressRecord`
- `CreateJobInput`
- `AddItemInput`
- `SaveProposalInput`
- `SubmitProposalInput`
- `ApproveProposalInput`
- `AssignItemInput`
- `UnassignItemInput`
- `AssignmentRecord`
- `CancelJobInput`
- `CancellationRecord`
- `RetryItemInput`
- `RetryRecord`
- `ApplyProposalInput`
- `RecoverApplyInput`
- `ApplyRecord`
- `migrations::migrations`

## Interactions

- Depends on `rustok-translation-targets` for the neutral owner-provider SPI.
- Requires the Core transactional outbox for content-free, typed workflow
  events. A workflow state transition and its event always share one database
  transaction.
- Uses owner-declared permissions and revisions in addition to Translation
  workflow permissions.
- Stores active assignment on the job item and append-only assignment commands
  separately. Assigned draft/review work is writable only by that actor or a
  Translation manager.
- Cancels remaining job work under job/item revision CAS while preserving
  already applied or excluded terminal items and rejecting unresolved owner
  apply outcomes.
- Completes a non-empty job automatically only when every item is applied,
  excluded, or cancelled. Conflict, stale, and blocked work never counts as
  successful completion.
- Allows an audited, actor-bound retry to return a blocked item with a current
  approved proposal to `approved`; stale and conflict items require explicit
  rebase instead of a blind owner retry.
- Updates per-job state, required/optional unit, applied/approved unit,
  assignment, resource-completion, and character-workload counters in the same
  transaction as workflow mutations. A Manage-authorized rebuild repairs the
  projection deterministically from workflow evidence.
- Persists apply intent before invoking an owner and records `applied` only
  after validating and durably storing the owner's stable receipt.
- Allows an operator with both Translation Manage and Publish permissions to
  recover an unknown outcome through an audited, lease-guarded command. The
  owner reauthorizes that operator and reconciles the original mutation key.
- Does not own localized business data and has no dependency on Media, Product,
  Pages, Blog, Commerce, or other provider implementations.

See the [local module contract](docs/README.md) and
[implementation plan](docs/implementation-plan.md).

# rustok-translation

## Purpose

`rustok-translation` is the optional RusToK control-plane module for
owner-safe translation inventory, workflow, review, memory, glossaries, and
machine-translation orchestration.

## Responsibilities

- Maintain rebuildable tenant translation inventory and provider checkpoints.
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
- `CreateJobInput`
- `AddItemInput`
- `SaveProposalInput`
- `SubmitProposalInput`
- `ApproveProposalInput`
- `migrations::migrations`

## Interactions

- Depends on `rustok-translation-targets` for the neutral owner-provider SPI.
- Will add the Core `outbox` runtime dependency together with the first
  transactional workflow event; current workflow persistence has no event
  publisher yet.
- Uses owner-declared permissions and revisions in addition to Translation
  workflow permissions.
- Does not own localized business data and has no dependency on Media, Product,
  Pages, Blog, Commerce, or other provider implementations.

See the [local module contract](docs/README.md) and
[implementation plan](docs/implementation-plan.md).

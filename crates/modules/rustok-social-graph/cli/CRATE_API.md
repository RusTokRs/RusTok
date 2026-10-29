# rustok-social-graph-cli / CRATE_API

## Platform Capability Bindings
- Implements `CommandProvider` from `rustok-cli-core` for Social Graph operational CLI commands.
- Integrates with `rustok-runtime` and `rustok-api`.

## Domain Collaborations
- Collaborates with `rustok-social-graph::SocialGraphReceiptMaintenancePort` for receipt cleanup and maintenance.
- Dispatched by platform operator tooling and CLI runners.

## Public surface
- `pub struct SocialGraphCommandProvider`: Command provider implementing `CommandProvider` trait for `social_graph` namespace.

## Minimum Contract Set

### Input DTOs/Commands
- `social_graph receipt-cleanup`: Command deleting completed Social Graph command receipts outside an explicit retention window (supports `--dry-run`).

### Domain Invariants
- Derives explicit receipt cutoff from reviewed retention inputs.
- Bounded batch operations enforced (`MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH`).
- Operates within explicit tenant context.

### Events / Outbox Side Effects
- CLI execution removes expired receipts; does not mutate domain social graph relations.

### Errors / Failure Codes
- Validation and execution failures map to typed `CliCoreError` variants.

# Pages / Page Builder Activation-Recovery Parity Actualization

Date: 2026-08-07  
Status: current-source-overlay / missing-binding-activation-recovery-source-ready / activation-recovery-postgres-harness-source-ready / execution-and-rollout-open  
Scope: recheck Pages / Page Builder parity against current `main`, implement the next confirmed repair gap, and preserve execution boundaries

This overlay supersedes the source-gap wording in `docs/modules/pages-page-builder-recovery-binding-gap-actualization-2026-08-07.md`. Older broad parity plans remain historical context; when their checkbox wording conflicts with the source state below, this overlay is authoritative.

## Recheck against current main

The current source was rechecked against the recent merged Pages packets through the recovery-gap actualization. The following previously completed source capabilities remain present and must not be reopened as implementation work merely because runtime evidence is still pending:

- typed Pages consumer metadata contribution and published standalone metadata surface;
- immutable rollback owner plus GraphQL/HTTP/OpenAPI/admin controls;
- event-driven Pages cache namespace/generation boundary and verified public readers;
- authenticated real-DOM Page Builder authoring source with anonymous authoring-code exclusion;
- reviewed static-publish resource budgets;
- bounded immutable-artifact integrity audit plus GraphQL/HTTP/OpenAPI transport;
- reviewed-publish rebuild provenance;
- explicit append-only immutable artifact rebuild;
- ordinary existing-binding rebuilt-artifact activation;
- bounded tenant-admin rebuild/activation transports and static public error envelopes;
- source guards and unexecuted SQLite/PostgreSQL/request-contract packets already merged for the above areas.

Those items remain **source-ready but execution-unvalidated** where their retained evidence still says no execution occurred.

## Newly closed source gap

The physical-source-artifact-loss chain previously stopped after explicit rebuild:

```text
reviewed publish
-> remove binding / manifest reference
-> physically lose canonical source artifact
-> retained provenance survives
-> explicit rebuild reproduces immutable artifact
-> locale binding remains absent
```

`PageService::replace_rebuilt_artifact_binding` previously treated the absent binding as an unconditional current-conflict. That was correct for ordinary replacement but left no bounded explicit recovery for the physical-loss state.

The activation owner now keeps two mutually exclusive admission paths.

### Existing-binding path

When a locale binding exists, the old fences remain unchanged:

- bound body id equals retained provenance body id;
- bound artifact id equals `expected_current_artifact_id` / rebuild source artifact;
- rebuilt artifact is not already current;
- any mismatch fails closed and never falls back to recovery.

### Missing-binding physical-loss path

When the locale binding is absent, recovery is admitted only after all common activation fences plus these exact checks:

1. the rebuild source artifact row is absent for the exact tenant/page/locale;
2. the retained `source.page_body_id` row still exists for the exact tenant/page/locale;
3. the retained `source.operation_id` publish operation still exists for the exact tenant/page;
4. that operation equals the rebuild receipt's `source_publish_operation_id`;
5. `publish_operation.result_version == expected_version`, while the common version fence already proves `expected_version == current page.version`.

Only the retained body identity is consumed by the recovery decision; mutable current draft content is not used as repair authority.

After admission, the owner reuses `PageBuilderArtifactService::bind_existing_body_in_tx`, advances the page version once, retains `published`, emits `NodeUpdated` + `NodePublished`, and stores the existing activation receipt. It does not recreate the missing source artifact or combine rebuild and activation.

## PostgreSQL recovery packet now authored

A dedicated environment-gated PostgreSQL harness source now covers the complete recovery chain without changing the older artifact-loss rebuild packet:

```text
crates/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-activation-recovery-postgres.mjs
```

The source packet contains three retained scenarios:

1. successful physical source loss -> explicit rebuild -> explicit activation, requiring one restored locale binding, page version +1 only at activation, exact two lifecycle events, source artifact still absent, unchanged provenance/rebuild receipt and exact replay;
2. rejection when the locale binding is absent but the source artifact still exists;
3. rejection when the source publish receipt is older than the locked current page version.

The harness and guard are **source-ready only**. They were not executed by this authoring workflow.

## Preserved safety boundaries

This implementation does not add:

- automatic audit-to-rebuild;
- automatic rebuild-to-activation;
- timestamp/newest-receipt inference;
- fallback from an existing mismatched binding;
- source-artifact recreation;
- mutable draft-content repair authority;
- migration/schema/DTO changes;
- new GraphQL/HTTP/OpenAPI routes;
- inline cache mutation;
- worker/retry policy;
- FFA/FBA promotion.

## Actualized repair matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Typed metadata contribution | Source-ready | Conflict/browser evidence pending |
| Immutable rollback | Source-ready | Database/request evidence pending |
| Event-driven cache + public readers | Source-connected | Cache/browser/relay evidence pending |
| Authenticated real-DOM authoring | Source-ready | Browser/tenant evidence pending |
| Anonymous authoring-code exclusion | Source-ready | Artifact/browser evidence pending |
| Reviewed publish resource limits | Source-ready | Accepted policy/runtime evidence pending |
| Immutable artifact integrity audit | Source-ready | SQLite/PostgreSQL execution pending |
| Reviewed-publish rebuild provenance | Source-ready | PostgreSQL execution pending |
| Explicit append-only rebuild | Source-ready | SQLite/PostgreSQL execution pending |
| Explicit rebuild after physical source-artifact loss | Harness + guard ready | PostgreSQL execution pending |
| Existing-binding activation | Source-ready | Execution pending |
| Missing-binding activation after physical source-artifact loss | **Harness + guard ready** | Dedicated PostgreSQL execution pending |
| Repair GraphQL/HTTP/OpenAPI transports | Source-ready | Generated/request execution pending |
| Automatic audit-to-rebuild | Deliberately absent | Not allowed |
| Automatic rebuild-to-activation | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Evidence truthfulness

The binding-replacement source evidence is actualized to distinguish consuming retained-body identity from using mutable draft content as repair authority, and to register the new PostgreSQL recovery harness as source-ready.

No execution is claimed by this authoring slice. In particular, the following remain false until a maintainer executes and retains accepted evidence:

- Rust tests;
- static verifier execution;
- Cargo check/clippy/fmt;
- SQLite/PostgreSQL scenarios;
- GraphQL/HTTP/OpenAPI dispatch;
- cache/lifecycle observation;
- workflows/CI.

## Actualized next cursor

1. Run `verify-pages-artifact-loss-activation-recovery-postgres.mjs` and the PostgreSQL `artifact_loss_activation_recovery_postgres` harness; retain accepted evidence instead of promoting from source review alone.
2. Confirm the successful case restores exactly one locale binding, advances the page version once only during activation, writes exactly `NodeUpdated` + `NodePublished`, keeps the source artifact absent, preserves provenance/rebuild receipt and replays idempotently.
3. Confirm the source-artifact-still-present negative case fails closed with no activation receipt, binding or lifecycle events.
4. Confirm the stale-source-publish-version negative case fails closed even when the request's `expected_version` matches the locked current page version.
5. Run the updated binding-replacement source guard plus the provenance, artifact-loss rebuild, audit, repair atomicity/failure/cache and transport guards.
6. Execute the retained SQLite/PostgreSQL/request/browser/tenant packets and store accepted evidence; do not convert source readiness into execution readiness before those runs.
7. Keep audit-to-rebuild and rebuild-to-activation automation absent unless a later accepted policy explicitly introduces it.
8. Keep FFA/FBA promotion blocked until the broader metadata, cache, artifact/HTTP/browser and tenant Wave evidence chain is accepted.

## Validation boundary

Source review and authoring only. Tests and verifiers were not executed in this workflow, per maintainer instruction.

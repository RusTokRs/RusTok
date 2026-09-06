# Profiles checkpoint: physical DLQ duplicate alert observability

Status: **identifier-free telemetry and optional health companion source-complete; runtime execution pending**.

## Profiles authorization boundary

Physical DLQ duplicate observability exists only for operations. It does not authorize or alter profile visibility, audience evaluation, relationships, blocks, mutes, storefront or author-card presentation, ownership, ranking, indexing, retry, replay, or mutation.

Profiles continues to consume authoritative owner-port results. It does not read Prometheus series or duplicate-observer health state.

## Count-free projection

A server-owned companion reads only the existing public observer mode, latest runtime snapshot, and task-finished flag. Its operator projection is limited to deployment mode, scan mode, bounded lifecycle state, generation, alert level, two duplicate/conflict booleans, and task-finished state.

Prometheus labels are finite deployment, scan, state, availability, level, and evaluation-flag values. The projection excludes tenant IDs, broker coordinates, offsets, message IDs, payloads/digests, receipt identities, credentials, threshold values, source counts, timestamps, raw errors, and inferred failure stages.

## No readiness coupling

Duplicate observability does not authorize Profiles and does not participate in readiness or liveness. An unavailable companion cannot stop profile reads/writes, event delivery, poison handling, or the application host.

## Source ownership

```text
telemetry metrics:
  crates/rustok-telemetry/src/dlq_duplicate_alert_metrics.rs
server observer:
  apps/server/src/services/event_dlq_duplicate_alert_observer.rs
server projection:
  apps/server/src/services/event_dlq_duplicate_alert_observability.rs
machine contract:
  crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-observability-source.json
verifier:
  scripts/verify/verify-event-dlq-duplicate-alert-observability.mjs
owner guide:
  crates/rustok-iggy/docs/dlq-duplicate-alert-observability.md
```

## Remaining work

1. run focused source tests and verifier;
2. retain a reviewed metrics scrape and health projection;
3. execute the locked external-Iggy moving observer packet;
4. keep notification delivery and destructive reconciliation outside Profiles.

Tests, Cargo commands, repository verifiers, server startup, metrics scrape, external-Iggy execution, and retained capture were not run by the implementation agent.

# Physical DLQ duplicate alert observability

Status: **bounded Prometheus telemetry and identifier-free health companion source-complete; runtime scrape and retained execution pending**.

## Purpose

The physical DLQ duplicate observer already publishes a latest-value, count-free alert evaluation. A separate server-owned companion projects that public handle into the shared telemetry registry and an in-process health snapshot without changing scanner, retry, cursor, or event-delivery behavior.

## Health projection

`EventDlqDuplicateAlertObservabilityHandle::current()` returns only:

```text
deployment mode
scan mode, when applicable
health state
runtime generation, when available
alert level, when available
physical-duplicate flag
identity-conflict flag
task-finished flag
```

The bounded states are `disabled`, `not_applicable`, `starting`, `available`, `unavailable`, and `stopped`.

The projection excludes broker endpoints, stream/topic/partition/offset coordinates, message identities, payloads or digests, receipt identities, credentials, threshold values, source counts, timestamps, and raw errors.

## Prometheus families

The single `rustok-telemetry` registry owns three bounded families:

```text
rustok_dlq_duplicate_alert_observer_state
rustok_dlq_duplicate_alert_snapshots_total
rustok_dlq_duplicate_alert_evaluation_flags
```

The bounded labels are limited to closed deployment, scan-mode, state, availability, level, and evaluation-flag domains. No tenant, identifier, coordinate, raw error, threshold, source-count, or arbitrary label is accepted.

The companion records state only after a state transition and records snapshot counters/flags only after the runtime generation changes. It does not infer whether an unavailable transition came from connection, polling, validation, or publication, so no guessed failure-stage label is emitted.

## Lifecycle projection

- Disabled and not-applicable observer handles produce explicit static state without another task.
- Active Iggy observers are projected through a one-second read-only companion loop.
- Initial generation zero is `starting`.
- A successful latest evaluation is `available`.
- A later unavailable generation clears level and duplicate flags.
- A finished observer task or shared shutdown becomes `stopped`.
- The existing observer remains the sole owner of connection, scan, retry, and moving-cursor preservation semantics.

## No readiness coupling

The health snapshot reports `affects_readiness = false`. The companion is not inserted into `/health/ready`, liveness, module authorization, or event-delivery gating.

Unavailable or stopped duplicate observation never stops outbox publication, source acknowledgement, poison recovery, or the application host.

## Mutation and authorization boundary

The observability companion cannot store consumer offsets, acknowledge, publish, delete, purge, replay, retry, mutate receipts, dispatch notifications, or change Profiles or Social Graph policy.

Profiles authorization does not consume observer mode, scan mode, health state, generation, alert level, flags, or metric values.

## Source paths

```text
crates/rustok-telemetry/src/dlq_duplicate_alert_metrics.rs
crates/rustok-telemetry/src/lib.rs
apps/server/src/services/event_dlq_duplicate_alert_observer.rs
apps/server/src/services/event_dlq_duplicate_alert_observability.rs
apps/server/src/services/server_bootstrap.rs
crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-observability-source.json
scripts/verify/verify-event-dlq-duplicate-alert-observability.mjs
```

## Remaining work

1. run the focused Rust tests and source verifier;
2. retain one metrics scrape and one health projection from a reviewed observer execution;
3. execute and retain the locked external-Iggy moving-window packet;
4. define notification routing, cooldown, and suppression separately;
5. add persistent cursor ownership only if restart continuity is required.

Tests, Cargo commands, repository verifiers, server startup, metrics scrape, external-Iggy execution, and retained capture were not run by the implementation agent.

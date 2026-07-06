# Implementation plan for `rustok-iggy-connector`

Status: connector abstraction is already separated from the transport crate; further
work is related to hardening the real SDK/lifecycle path and maintaining a clean
boundary of responsibility.

## Execution checkpoint

- Current phase: lifecycle_hardening
- Last checkpoint: no-compile increment: added `ConnectorAckToken` as a unified simulated/real Iggy SDK ack seam; remote/embedded subscribers now source-level validate stream/topic/partition scope before ack, and `verify-iggy-connector-source.mjs` locks the guardrail without compilation.
- Next step: connect `ConnectorAckToken::iggy_sdk` to the actual SDK subscriber receive/commit path and replace source-level evidence with targeted cargo tests when compilation is allowed.
- Open blockers: compile/test evidence deferred due to explicit iteration constraint: no compilations.
- Hand-off notes for next agent: Preserve opaque-token contract for transport consumers; when wiring real SDK, extract offset/consumer cursor into `ConnectorAckToken::iggy_sdk`, without pulling retry/DLQ/replay policy into the connector crate.
- Last updated at (UTC): 2026-06-20T14:30:00Z

## Scope of work

- keep `rustok-iggy-connector` as a low-level connector layer;
- synchronize mode switching, lifecycle contracts and local docs;
- prevent pulling transport-level semantics into the connector crate.

## Current state

- `IggyConnector`, remote/embedded implementations and config model already exist;
- optional `iggy` feature already serves as a seam for real SDK integration;
- request building, mode serialization and error handling are already separated into their own crate;
- `rustok-iggy` uses this crate as a low-level dependency.

## Stages

### 1. Contract stability

- [x] lock connector boundary separate from the transport crate;
- [x] keep embedded/remote mode abstraction inside the connector crate;
- [x] maintain sync between connector contracts, `rustok-iggy` expectations and local docs.

### 2. Lifecycle hardening

- [ ] bring full SDK integration path, reconnection and pooling semantics;
  - [x] fix lifecycle read surface `is_connected()` for remote/embedded connectors;
  - [x] add subscriber metadata for offset/ack/retry without transport policy;
  - [x] add explicit ack override seam for remote/embedded subscriber adapters;
  - [x] centralize simulated ack token builder for remote/embedded metadata;
  - [x] add `ConnectorAckToken` seam for simulated and real Iggy SDK ack cursor with source-level scope validation;
- [ ] cover batching, TLS and real connection failure cases with targeted tests;
- [ ] keep simulation mode as an explicit documented compatibility path.

### 3. Operability

- [ ] evolve health/metrics/runbook guidance for the connector layer;
- [ ] keep local docs synchronized with transport docs;
- [ ] document lifecycle guarantees simultaneously with changing connector surface.

## Verification

- targeted compile/tests for configuration, mode switching, request building and connector errors;
- integration tests for real embedded/remote paths;
- docs sync between connector and transport crates.
- contract tests cover all public use-case connector surface.

## Update rules

1. When changing connector contract, update this file first.
2. When changing public surface, synchronize `README.md` and `docs/README.md`.
3. When changing transport boundary, update related docs in `rustok-iggy`.


## Quality backlog

- [x] Update test coverage for key module scenarios: added unit assertions for subscriber metadata/message builders (execution deferred without compilations).
- [x] Verify completeness and currency of `README.md` and local docs: README/docs/CRATE_API describe the metadata surface.
- [x] Lock source-level assertions for canonical simulated ack tokens (execution deferred without compilations).
- [x] Lock/update verification gates for current module state: `node scripts/verify/verify-iggy-connector-source.mjs` (no-compile) and `cargo test -p rustok-iggy-connector --lib` when compilation is allowed.
- [ ] Connect real SDK subscriber receive/ack path to `ConnectorAckToken::iggy_sdk` and replace source-level guardrail with actual targeted tests.

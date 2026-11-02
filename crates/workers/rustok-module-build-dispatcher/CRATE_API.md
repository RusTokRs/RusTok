# rustok-module-build-dispatcher / CRATE_API

## Role

`rustok-module-build-dispatcher` is the dedicated transport-neutral delivery consumer for queued module builds in the RusToK platform.
It bridges the external event broker (`rustok-iggy`) to the owner-side module build queue service (`SeaOrmModuleBuildService`), decoupling the build worker execution from broker-specific delivery and cursor-tracking details.

The dispatcher process operates under strict isolation boundaries:
- It maintains **no Cargo binary access**, **no CAS storage mutation capabilities**, and **no local build container invocation**.
- It communicates with the build worker daemon (`rustok-module-build-worker`) exclusively via mTLS gRPC over `rustok-module-build-transport`.
- It connects to the database solely through `SeaOrmModuleBuildService` to obtain durable build claims and persist terminal results.
- It commits broker consumer offsets **only after** a terminal result is successfully persisted or an idempotent redelivery is confirmed (`AlreadySettled` or `ClaimedElsewhere`).

## Public surface

### Public modules

- `host`: Deployment configuration (`ModuleBuildDispatcherConfig`) and daemon process lifecycle (`run_dispatcher`).

### Primary public types, traits, and constants

- `MODULE_BUILD_CONSUMER_GROUP`: Constant identifying the dedicated external consumer group (`"rustok-module-build-dispatcher"`).
- `ModuleBuildDelivery`: Normalized delivery message containing `delivery_id`, `request_id`, and `tenant_id`.
- `ModuleBuildDeliverySource`: Asynchronous trait for delivery acknowledgement (`acknowledge(&self, delivery_id: Uuid)`).
- `IggyModuleBuildDeliverySource`: Production broker delivery source backed by a persistent Iggy consumer group on the `module-build` topic.
- `ModuleBuildDeliveryResult`: Result enum for delivery execution attempts (`Persisted(ModuleBuildResultRecord)`, `AlreadySettled`, `ClaimedElsewhere`).
- `ModuleBuildDeliveryConsumer`: Coordinator executing owner-validated remote build dispatches against `SeaOrmModuleBuildService` and remote `ModuleBuildWorker`.
- `ModuleBuildDispatcherConfig`: Deployment-owned configuration parsing database URL, worker endpoint, Iggy configuration, and idle polling duration from environment variables.
- `run_dispatcher`: Main async loop executing event reception, build dispatch, and graceful shutdown on SIGINT/Ctrl+C.

## Configuration & Environment

The standalone `rustok-module-build-dispatcher` host requires the following environment variables:
- `RUSTOK_MODULE_BUILD_DISPATCHER_DATABASE_URL`: Database connection string for the owner-side control plane.
- `RUSTOK_MODULE_BUILD_DISPATCHER_WORKER_ENDPOINT`: HTTPS endpoint (`https://...`) of the gRPC build worker.
- `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_ADDRESSES`: Comma-separated list of Iggy broker TCP addresses.
- `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_USERNAME` & `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_PASSWORD`: Broker client credentials.
- `RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_TLS_ENABLED`: Must be explicitly set to `"true"` (plaintext transport is strictly prohibited).
- `RUSTOK_MODULE_BUILD_DISPATCHER_IDLE_POLL_DELAY_MS`: Optional idle poll duration between 1 and 60,000 ms (default: 1,000 ms).
- Client mTLS variables (`RUSTOK_MODULE_BUILD_CLIENT_CERT_PEM`, `RUSTOK_MODULE_BUILD_CLIENT_KEY_PEM`, `RUSTOK_MODULE_BUILD_SERVER_CA_PEM`, `RUSTOK_MODULE_BUILD_SERVER_DOMAIN`).

## Minimum Contract Set

### Input DTOs/Commands
- `ModuleBuildDelivery`: Validated queue delivery tuple (`delivery_id`, `request_id`, `tenant_id`) extracted from `module.build.queued` event envelopes.
- `ModuleBuildDispatcherConfig`: Strongly-typed configuration parsed with validation bounds and sanitized debug display.

### Domain Invariants
1. **Result-First Acknowledgement**: External broker message delivery offsets are acknowledged only after a terminal result is durably stored in the database or an idempotent terminal state is verified.
2. **No Cargo or Compiler Egress**: The dispatcher contains zero direct compilation tools, toolchains, or container runtimes; all build execution is delegated over mTLS gRPC.
3. **Idempotent Redelivery Safety**: Out-of-order or duplicate deliveries encountering an already settled build (`NotQueued`) or an active execution claim (`ExecutionInProgress`) acknowledge without triggering duplicate builds.
4. **Envelope and Tenant Binding**: Event payloads must strictly match envelope correlation and tenant identifiers; nil UUIDs are rejected upfront.
5. **Masked Sensitive Credentials**: Debug representations of connection configurations redact sensitive database connection strings and passwords.

### Events / Outbox Side Effects
- Consumes: `DomainEvent::ModuleBuildQueued` from topic `module-build` under consumer group `rustok-module-build-dispatcher`.
- Produces: Does not directly publish outbox events; status transitions and outcome events are transactionally committed by `SeaOrmModuleBuildService`.

### Errors / Failure Codes
- `ModuleBuildProtocolError`: Protocol-level errors surfaced during owner service dispatch (`NotQueued`, `ExecutionInProgress`, `WorkerUnavailable`, `Transport`, etc.).
- `String`: Explicit typed error strings for configuration and broker transport failures.

## Verification & Test Coverage

Verified via:
- Manifest standardization tests in `contract_tests.rs`.
- Contract assertions covering `CRATE_API.md` required sections, debug implementations, and constant identifiers.
- Envelope validation and idempotent error mapping unit tests in `src/lib.rs`.
- Scoped verification: `cargo test -p rustok-module-build-dispatcher`.
- Scoped linter: `cargo clippy -p rustok-module-build-dispatcher --all-targets`.
- Gatekeeper verification: `python scripts/verify/verify-remediation-gate.py`.

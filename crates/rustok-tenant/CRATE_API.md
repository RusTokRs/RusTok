# rustok-tenant / CRATE_API

## Public Modules
`dto`, `entities`, `error`, `services`.

## Primary Public Types and Signatures
- `pub struct TenantModule`
- Public tenant DTOs/services from `services`.
- `TenantModule` implements `RusToKModule` with `ModuleKind::Core`.

## Events
- Publishes: `tenant.created`, `tenant.updated`, `tenant.module.toggled`, `tenant.locale.enabled`, and `tenant.locale.disabled`.
- Every owner mutation inserts its validated event into the canonical transactional outbox in the same database transaction through `TransactionalEventBus::publish_root_in_tx`.
- Event publication is mandatory for `TenantService::new`; there is no host-configured or fail-open event-bus constructor.
- Consumes: N/A.

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-events`
- `rustok-outbox`

## Common AI Mistakes
- Mixes up `tenant slug` and internal `tenant_id`.
- Does not add tenant isolation in queries and access checks.
- Constructs a parallel tenant writer or makes lifecycle event publication conditional on host wiring.

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate (see sections with `Create*Input`/`Update*Input`/query/filter above and corresponding `pub` exports in `src/lib.rs`).
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters in `apps/server`.

### Domain Invariants
- Module invariants are enforced in services/state machines and DTO validation; invalid transitions/parameters must result in a domain error.
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.

### Events / Outbox Side Effects
- Every tenant, tenant-module, or locale-policy mutation that changes owner state must publish its lifecycle event through the canonical transactional outbox before commit.
- Installer/bootstrap calls to `TenantService::ensure_tenant` use the same owner transaction and event contract as ordinary tenant creation.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.

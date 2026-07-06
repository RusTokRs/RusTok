# rustok-rbac / CRATE_API

## Public Modules
`dto`, `entities`, `error`, `services`.

## Primary Public Types and Signatures
- `pub struct RbacModule`
- Public RBAC DTOs/services from `services`.
- Authorization contracts are reused from `rustok_api` and `rustok_core::rbac`.
- Re-export policy helpers at crate root:
  - `has_effective_permission_in_set`
  - `missing_permissions`
  - `check_permission`
  - `check_any_permission`
  - `check_all_permissions`
  - `PermissionCheckOutcome`
  - `denied_reason_for_denial`
  - `DeniedReasonKind`

## Events
- Publishes: typically does not publish business events by default.
- Consumes: N/A (direct service calls).

## Dependencies on Other RusToK Crates
- `rustok-core`

## Common AI Mistakes
- Confuses `Resource/Action/Permission` from core with local DTOs.
- Adds permission checks in the wrong layer (instead of application/service boundary).

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate (see sections with `Create*Input`/`Update*Input`/query/filter above and corresponding `pub` exports in `src/lib.rs`).
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters in `apps/server`.

### Domain Invariants
- Module invariants are enforced in services/state machines and DTO validation; invalid transitions/parameters must result in a domain error.
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.

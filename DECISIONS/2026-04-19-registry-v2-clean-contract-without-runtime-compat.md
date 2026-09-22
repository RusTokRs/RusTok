# Registry V2 clean contract without runtime-compat layer

- Date: 2026-04-19
- Status: Accepted

## Context

The registry/governance surface before clean-cutover contained several classes of problems:

- header-based actor model for live authority;
- string-based error classification;
- mixing of public contract and internal audit payload;
- filesystem-oriented artifact contract;
- legacy naming and runtime fallback that blurred the canonical principal-based read/write contract.

At the same time, the platform is still at an early stage, so maintaining runtime backward compatibility for the old registry payload shape provides no value, but increases the complexity of live code, UI, and agent context.

## Decision

1. For `Registry V2`, a **big-bang cleanup** is adopted:
   - live authority is built only from session-backed user bearer auth;
   - legacy actor/publisher headers are not supported;
   - the controller maps public errors only through typed `RegistryGovernanceError`;
   - runtime/admin do not maintain fallback for legacy `*_actor` and `stage/gate` keys.
2. Historical registry audit rows are normalized by **migration**, not by a runtime compatibility layer.
3. Principal-based naming (`owner`, `owner_principal`, `publisher`) is considered canonical for live code, read-side, and docs.
4. Registry artifacts live on a storage-backed contract (`artifact_storage_key`, `artifact_download_url`) without exposing the local filesystem path to clients.
5. Remaining `artifact_url` / `artifact_path` outside registry governance are allowed only as part of the build/release subsystem and are not considered registry compatibility obligations.

## Consequences

### Positives

- Reduces the volume of live code and agent context: there is no second old registry contract.
- Public/admin/runtime read the same typed payload shape.
- Errors and permissions are predictably mapped by type, not by string heuristics.
- Registry reset can be considered closed in terms of code, migrations, and docs, rather than as a perpetual transition.

### Trade-offs

- Pre-migration registry audit payload shape is no longer supported at runtime.
- Any old data must be brought in line by migration before the new runtime starts.
- If historical replay of the old payload shape is needed in the future, it must be a separate offline/import path, not a return of legacy fallback in live code.

## Closeout

Registry reset is considered closed for the registry surface in the following scope:

- RTK-001..RTK-010 are closed by code, migrations, and updated docs;
- runtime compatibility for legacy registry payload is intentionally not preserved;
- canonical rules for further development live in `docs/modules/module-authoring.md`, `docs/modules/manifest.md`, and related ADRs.

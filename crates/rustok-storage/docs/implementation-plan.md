# Implementation plan for `rustok-storage`

## Current state

`rustok-storage` owns the shared `StorageBackend`, `UploadedObject`,
`StoredObject`, and `StorageService` contracts, backend selection/configuration,
path generation, public URL construction, and path-safety guarantees. It also
owns conditional object creation and trusted-prefix listing used by durable CAS
adapters. The local backend is development-only; domain modules, including
`rustok-media` and `rustok-modules`, must not bypass this boundary with
backend-specific logic.

The server is a composition layer for `StorageService`; storage does not own
media metadata or other domain business rules.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This shared infrastructure module has no module-owned UI or FBA provider port.

## Open results

1. **Restore the required crate README.** Add the root `README.md` describing
   purpose, responsibilities, interactions, entry points, and links to local
   documentation, then keep it synchronized with the existing docs.
   **Depends on:** the established crate documentation contract.
   **Done when:** the crate root and local docs give consumers one consistent
   storage ownership and integration map.

2. **Harden external backend guarantees.** Keep S3-compatible or other
   production backends behind `StorageBackend`, including conditional create
   and trusted-prefix listing semantics required by content-addressed storage.
   **Depends on:** backend configuration, credentials, and deployment policy.
   **Done when:** backend-specific failure/configuration integration tests prove
   compatible upload, conditional create, listing, deletion, and path-safety
   semantics.

3. **Publish operational storage guarantees.** Evolve health, metrics, and
   runbook guidance alongside backend support and synchronize them with media
   and host runtime documentation.
   **Depends on:** the selected backend and observability requirements.
   **Done when:** operators can identify backend health, configuration, and
   failure recovery without domain-specific storage workarounds.

## Verification

- Structural checks for storage contract and documentation sync.
- Targeted compile/tests when changing `StorageBackend`, `StorageService`, path
  safety, or backend configuration.
- Backend integration and health checks when an implementation changes.

## Change rules

1. Keep backend abstraction, path safety, and URL policy in this module.
2. Update local docs and media/host runtime docs with a storage contract change.
3. Update `docs/modules/registry.md` with an ownership or module-status change.

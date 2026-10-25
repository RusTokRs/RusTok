# rustok-runtime / CRATE_API

## Role
Host runtime foundation helpers providing neutral composition of database handles, settings, instance filesystem layouts, role materialization, and module payload caching.

## Public modules
- `deployment`: Node-local static role materialization and restart receipts.
- `layout`: Portable instance layout resolution, path containment, and placement bindings.
- `module_cache`: Content-addressed module payload cache and artifact preparation.

## Primary public types and functions
- `pub struct RuntimeComposition`: Host-neutral DB connection, settings snapshot, and instance root container.
- `pub struct InstanceLayout`, `pub struct InstancePlacement`: Portable filesystem layout descriptor and binding.
- `pub fn inspect_instance_layout(path: &Path) -> Result<InstanceLayout, InstanceLayoutError>`
- `pub fn bind_instance_placement(path: &Path, marker: InstancePlacement) -> Result<InstanceLayout, InstanceLayoutError>`
- `pub fn prepare_instance_layout(path: &Path) -> Result<InstanceLayout, InstanceLayoutError>`
- `pub fn materialize_role(req: &RoleMaterializationRequest) -> Result<RoleMaterializationReceipt, RoleMaterializationError>`
- `pub fn materialize_module_payload(req: &ModulePayloadMaterializationRequest) -> Result<ModulePayloadMaterializationReceipt, ModulePayloadCacheError>`
- `pub fn record_prepared_module(root: &Path, manifest: &ModuleManifest) -> Result<PreparedModuleCacheReceipt, ModulePayloadCacheError>`
- `pub enum RuntimeHandleError`: Typed errors for missing host context or invalid instance-relative paths.
- `pub enum RuntimeCompositionError`: Configuration, database, and root resolution errors.

## Contract invariants
- **Filesystem containment**: Instance-relative path resolution (`instance_path`) strictly rejects traversal (`..`), empty components, or escapes outside `RUSTOK_INSTANCE_ROOT`.
- **Digest-addressed immutability**: Role and module materialization verify SHA-256 digests and create immutable directories; they never pull network registries or execute arbitrary commands.
- **Fail-closed inspection**: Unprepared instance directories are never automatically claimed by inspection; binding requires explicit placement transition.

## Errors
- [`RuntimeHandleError`]: Returned when host context or requested shared handle is absent.
- [`RuntimeCompositionError`]: Returned when environment settings or database connectivity fail.
- [`InstanceLayoutError`]: Returned on path escape or layout corruption.
- [`RoleMaterializationError`]: Returned on digest mismatch or filesystem I/O failure.
- [`ModulePayloadCacheError`]: Returned on cache payload verification failure.

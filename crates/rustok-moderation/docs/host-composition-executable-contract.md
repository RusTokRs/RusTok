# Moderation executable host-composition contract

Status: **source-ready / maintainer execution pending**

## Scope

The server-owned executable composition evidence is split across:

- `apps/server/tests/moderation_composition_profiles.rs` for the supported feature/module matrix;
- `apps/server/tests/moderation_factory_failure_composition.rs` for fail-closed producer-factory initialization.

Both targets exercise `build_shared_runtime_extensions_with_host_providers`, the same server composition seam that transfers module runtime extensions into `HostRuntimeContext` and materializes the Moderation subject-adapter registry.

## Retained composition matrix

With the corresponding Cargo features selected, the existing profile target retains these deployment contracts:

- Forum without Moderation remains a valid host composition and does not require the Moderation owner registry;
- Moderation without Forum materializes a valid, empty `ModerationSubjectAdapterRegistry`;
- Forum plus Moderation materializes exactly the Forum topic and reply adapters under the exact `forum/forum_topic` and `forum/forum_post` keys;
- selecting `mod-moderation` while omitting `ModerationModule` from `ModuleRegistry` fails composition with the explicit owner-missing error instead of silently starting without the owner.

These are executable server composition tests, not source-only assertions over feature declarations.

## Producer factory failure

`moderation_factory_failure_composition.rs` adds a test-only producer module through the ordinary `RusToKModule::register_runtime_extensions` hook. Its factory declares a valid `broken_moderation_producer/forum_post` key but returns `ModerationSubjectAdapterBuildError::InvalidConfiguration` when the host asks it to build an adapter.

The server must reject the whole composition. The retained assertions require the error to preserve all three layers of context:

- server materialization boundary: `moderation subject adapter materialization failed`;
- exact producer key: `broken_moderation_producer/forum_post`;
- neutral typed build failure: `moderation subject adapter configuration is invalid`.

No fallback adapter is installed and no partially usable Moderation registry is returned to the caller. Factory build failure therefore remains a startup failure, as required by the neutral registry contract.

## Dependency boundary

The failure fixture uses only dependencies already owned directly by `rustok-server`: `rustok-core`, `rustok-api`, optional `rustok-moderation`, SeaORM and `sea-orm-migration`. It does not add a test helper crate, workspace member or Cargo lockfile change.

The producer is test-only and owns no migrations. It exists solely to exercise host materialization failure through the same typed extension path used by real modules.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
cargo test -p rustok-server \
  --no-default-features --features mod-moderation \
  --test moderation_composition_profiles \
  --test moderation_factory_failure_composition -- --nocapture

cargo test -p rustok-server \
  --no-default-features --features mod-moderation,mod-forum \
  --test moderation_composition_profiles -- --nocapture

node scripts/verify/verify-moderation-host-executable-contract.mjs
```

No tests, Cargo commands, Node verifiers, formatters, workflows or CI were executed while preparing this file.

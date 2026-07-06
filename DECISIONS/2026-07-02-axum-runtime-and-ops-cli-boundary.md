# Axum runtime and ops CLI boundary

- Date: 2026-07-02
- Status: Accepted

## Context

RusToK is moving away from Loco RS as the application/runtime owner. At the same time, some Loco
conventions are useful as operator/dev workflow: migrate, seed, install,
maintenance tasks, and future distribution-aware builds.

If CLI and maintenance code remain inside the production server runtime, the server
binary will carry executable baggage that not all distributions need.
If all module-specific commands are put into a single central CLI crate, that crate
will become a dumping ground for commands from all core, optional, and external modules. If the CLI
is placed in the module's domain core, the module will start depending on `clap`, stdout/stderr, exit
codes, and operator UX, which breaks the hexagonal boundary.

## Decision

1. `apps/server` is a pure Axum runtime entrypoint: HTTP startup/shutdown,
   router composition, runtime context, workers, and lifecycle.
2. The production server binary does not depend on the ops CLI crate and does not contain
   maintenance command code.
3. The operator/dev CLI belongs to a separate ops layer: `rustok-ops` runner,
   parser, registry, settings loading, and exit-code/output policy.
4. The module's domain core does not depend on ops CLI contracts.
5. Module-specific commands live next to the module as a separate `cli/` adapter
   package, for example `crates/rustok-index/cli`, and call the public typed API
   of their module.
6. `rustok-ops` aggregates command providers through an explicit module/distribution
   manifest or generated registry, not through a hardcoded list of all modules.
7. External modules may ship their own `cli/` adapter package; if they do not,
   the host/distribution may keep the adapter in the integration layer.
8. Distribution-aware builds are an acceptable follow-up: `rustok-ops` can
   generate runtime/ops registries, build a server binary without the ops layer, and
   an ops binary only with providers of the selected distribution.

## Consequences

- Removing Loco CLI/tasks does not require moving maintenance code into
  `apps/server`.
- Module ownership is preserved: commands, scripts, and maintenance adapters live
  next to the module, but not inside the domain core.
- The central ops runner remains an infrastructure orchestration layer, not a
  catalog of all commands of all modules.
- Distributions can build different sets of runtime modules and ops providers without
  manually editing the server crate.
- Any future cutover from Loco tasks must translate the use case into a typed Rust
  API and call it from a module-local `cli/` adapter via `rustok-ops`.

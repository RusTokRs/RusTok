# rustok-installer

`rustok-installer` is the shared installer foundation for RusToK. It owns the
install plan, state-machine, secret-reference, receipt, checksum, and preflight
contracts that CLI, server HTTP, web wizard, and dev bootstrap wrappers should
reuse.

## Purpose

RusToK needs a hybrid installer: CLI-first for repeatable operations and CI/CD,
with a web wizard as a friendly first-run facade. This crate is the source of
truth for the shared installer semantics so those interfaces do not duplicate
bootstrap logic.

## Responsibilities

- Model install plans, profiles, database policy, seed profiles (including
  canonical parsing and the profile module set), and tenant enablement inputs.
- Track install state transitions and resumable step receipts.
- Redact secrets and distinguish secret references from plaintext setup input.
- Resolve local `env`, file, mounted-file and dotenv secret references through
  one reusable installer contract; external secret managers remain explicit
  adapter work.
- Provide deterministic checksums for idempotent step skipping.
- Provide preflight policy checks that are independent from any specific UI.
- With the default `seed-runtime` feature, define a consumer-owned seed
  workflow over narrow tenant, identity, role and module ports, without server
  model dependencies.
- Define the canonical unversioned topology, trusted exact-bundle binding, and
  one neutral distribution deployment hand-off without build-provider
  dependencies or per-role release heads.
- Under the accepted target, resolve one trusted operator-selected instance
  root on any supported operating system and derive the complete portable
  relative layout without making its physical path part of release identity.

## Interactions

- `apps/server` is a thin HTTP/setup-wizard adapter over these contracts; it
  must not own a second installation state machine.
- `rustok-installer-cli`, selected by `rustok-cli`, provides `install plan`,
  `install preflight`, `install apply`, `install status`, and `seed apply`
  through the shared executor; it does not import `apps/server`.
- The local CLI requires `--root <path>`, including a relative path
  resolved from its invocation directory. The web wizard consumes only a
  root selected or allowed by its trusted local host adapter and cannot submit
  an unrestricted filesystem path.
- `xtask install-dev` remains a dev convenience wrapper and will delegate to
  the platform CLI/executor rather than the production server binary.
- The current executor adapters resolve local secret refs (`env`, `file`,
  `mounted-file`, `dotenv`) during `apply`; external secret managers remain
  contract-level references until an external resolver is added.
- Migrations are owned by `rustok-migrations`; installer schema-selection
  must not pretend to omit module-owned schema while the server migrator is still
  globally composed.
- Durable SeaORM session and receipt storage is owned by
  `rustok-installer-persistence`; this foundation crate deliberately keeps no
  database adapter.
- `rustok-distribution` supplies a composition revision/hash compatibility
  check. The distributed target additionally requires the trusted host to bind
  the exact public `preparation_id`, distribution release, OCI bundle root, and
  role-set digest from owner admission or the signed fresh-bootstrap receipt.
  The HTTP host now resolves an exact locally selected release through the
  current admitted `rustok-modules` ledger. HTTP and CLI hosts now verify the
  bounded Ed25519 signature, signer-key digest, validity interval, immutable
  bundle identity, and executable-composition match of a fresh-bootstrap
  receipt before mutation. Importing that verified receipt into the sole owner
  ledger before the remaining schema is applied remains open.
- Under the accepted target, `rustok-build` constructs/validates the role plan,
  `rustok-static-distribution-worker` alone executes/publishes the complete
  bundle, and `rustok-modules` owns admission and desired/observed rollout. The
  unsafe per-role `rustok-build` active-release adapter has been removed.
  Distributed apply still fails closed until the desired/observed rollout
  adapter is composed; the standalone CLI additionally needs its trusted owner
  resolver.
- The deployment controller and node agent are a separately signed,
  digest-pinned host-provisioning prerequisite. Installer preflight binds their
  exact tool release and external protocol revision, but installer apply and
  candidate roles never install or self-update them. After bootstrap,
  `rustok-modules` coordinates their `operations_tool_maintenance` in the same
  canonical operation ledger and fleet fence; the host supervisor remains a
  narrow assignment executor with the predecessor tools retained.
- The install plan carries one typed instance placement. Native apply creates
  only the canonical relative layout, records `state/instance.json`, resumes
  only the matching instance (including its exact create-new pending marker
  after process loss), and rejects nonempty unmarked or overlapping roots
  without deleting user-owned entries.

## Feature Boundary

- The feature-neutral crate surface contains install plans, state, receipts,
  preflight, deployment hand-offs, secret references, and executor ports. It is
  safe for browser clients that only consume installer contracts.
- `host-runtime` and `seed-runtime` are enabled by default for native server and
  CLI consumers. They add safe filesystem preparation and the seed workflow.
- Browser consumers must disable default features; they do not execute tenant,
  identity, role, or module seed operations.

## Entry Points

The current foundation API is exposed from the crate root:

- `InstallPlan`
- `InstallState`
- `InstallStep`
- `InstallReceipt`
- `PreflightReport`
- `evaluate_preflight`
- `InstallDistributionBinding`
- `InstallDistributionDeploymentRequest`
- `execute_distributed_deployment`

## Verification

```powershell
cargo test -p rustok-installer
cargo check -p rustok-installer --no-default-features
```

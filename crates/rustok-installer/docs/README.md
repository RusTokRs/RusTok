# Documentation `rustok-installer`

`rustok-installer` is a support crate for the RusToK hybrid installer. It is not
a platform module and does not participate in tenant-level enable/disable.

## Purpose

The crate defines the common installer contract reused by:

- CLI `rustok-cli install ...`;
- HTTP surface `/api/install/*`;
- first-run web wizard;
- dev wrappers such as `cargo xtask install-dev`.

## Boundaries

- PostgreSQL is the default production DB and the only production-ready engine.
- SQLite is allowed only for `local`, `demo` and `test` scenarios.
- Module selection controls tenant enablement and build/profile intent, but not
  physical exclusion of module-owned schema from the global `Migrator`.
- `SeedProfile` owns its canonical default module set; host installers may only
  apply explicit enable/disable overrides from the install plan.
- `SeedExecutionRequest` composes tenant, identity, role and module owner ports;
  it has no server model or database adapter dependency and is exposed by the
  default `seed-runtime` feature.
- Install-plan, state, receipt, preflight, deployment, secret, and executor
  contracts remain available without default features so the web wizard can
  share their exact types without linking the native seed runtime.
- Rollback after schema application must not promise a universal reverse
  migration; production restore relies on backup/snapshot.
- The default local/monolith installation lives under one operator-selected
  instance root on any supported operating system. Configuration, operations
  tools, releases, sources, object storage, bundled-service data, state, work,
  caches, logs, and runtime files use canonical relative subdirectories. A fixed Linux path,
  root privilege, container, or global package-manager layout is not required.
- Physical paths are trusted host placement facts, never distribution/module/
  migration/object identity. Advanced adapters may map logical subtrees to
  external storage, OCI layers, volumes, logging, or runtime facilities.

## Portable instance root

The local CLI selects the directory explicitly; it is not a compiled-in or
Linux-specific default. Relative input is allowed and resolves from the CLI
invocation directory:

```text
rustok-cli install plan --root .
rustok-cli install plan --root D:\Sites\shop-a
rustok-cli install plan --root /home/operator/shop-a
```

The resulting plan derives the same relative layout in every case. An
independent installation selects another root and separate database/object
authority. The web wizard uses only a root chosen or allowlisted by its trusted
local host adapter. The shared installer executor and current CLI/HTTP adapters
already use this placement contract; storage, release materialization, build
work, caches, logs, and runtime files derive from the selected root.

Multiple instances do not need separate binaries or a machine-global install
directory. Each process receives its own `RUSTOK_INSTANCE_ROOT`, database
credentials, object-store namespace, listen address, and process supervisor
unit. The same immutable release may be materialized independently in those
roots without changing its release ID or digests.

Apply accepts an absent/empty root or the exact marker for resuming the same
installation. It never recursively deletes the selected directory or unrelated
files; failed-attempt cleanup removes only create-only entries proven to belong
to that attempt.

Runtime processes inspect this layout without claiming it. When
`state/instance.json` exists they reuse its durable `instance_id`; when it does
not exist, path inspection remains non-authoritative and does not write a
marker. Only installer bind/prepare may turn an empty root into an owned
instance.

The physical tree is deliberately small and predictable:

```text
<instance-root>/
|-- config/                         instance configuration and secret references
|-- operations/releases/sha256/    signed controller/agent packages
|-- releases/platform/sha256/      immutable platform role bundles and browser assets
|-- sources/objects/               media-typed source CAS (Rust/WASM/Rhai inputs)
|-- sources/receipts/              source ownership and publication receipts
|-- storage/                       local object-store mapping when external S3 is not selected
|-- data/services/                 bundled PostgreSQL/Iggy/other service data, when used
|-- state/deployment/{slots,journal}/ desired/observed rollout slots and restart journal
|-- state/operations/{slots,journal}/ operations-tool slots and restart journal
|-- state/instance.json            instance marker and identity
|-- work/static-distribution/      disposable build/materialization workspace
|-- cache/{deployment,module-runtime}/ rebuildable caches
|-- logs/                          instance logs or forwarding spool
`-- run/                           sockets, pid/lease files, and other ephemeral runtime state
```

WASM and Rhai are not copied beside the server executable. Their immutable
source objects and receipts live under `sources/`; admitted runtime payloads
and prepared-engine cache entries live under `storage/` and
`cache/module-runtime/` according to their content digests. PostgreSQL remains
the authority for which exact release is installed, enabled, serving, or
retained for recovery. Next applications remain external/manual consumers and
are not installed into or rolled back by this tree.

## Installation states

Main happy path:

```text
Draft
-> PreflightPassed
-> ConfigPrepared
-> DatabaseReady
-> SchemaApplied
-> SeedApplied
-> AdminProvisioned
-> Deploying
-> Verified
-> Completed
```

Error/operational states:

```text
Failed
FreshInstallCleaned
RecoveryRequired
```

`FreshInstallCleaned` is legal only before durable production state exists.
`RecoveryRequired` is the common operator terminal state; its receipt carries
the exact typed recovery action such as restore, forward repair, or containment,
and it is never reported as a code rollback result. The pipeline records bundle
pre-stage and pre-install recovery-boundary readiness
before `SchemaApplied`, then uses `Deploying` only for starting/switching the
already pre-staged roles.

## Current adapters and target topology

The server CLI parser was removed with the Axum cutover. `apps/server` hosts a
thin HTTP adapter; `rustok-cli install plan|preflight|apply|status` and
`rustok-cli seed apply` use the shared typed executor and SeaORM adapters. The
standalone apply adapter opens the target database itself, so a requested
database may be created before a CLI runtime database exists.

The current adapters derive storage/work roots from the trusted instance-root
selection in the shared installer contract. Local CLI input may select any
directory, including `.` resolved against the invocation directory. The HTTP
wizard may display and consume a host-approved root but cannot inject an
unrestricted path. Independent installations use independent roots, database
authority, and object-store namespaces; a platform-specific path never enters
bundle or module identity.

An apply operation resolves local secret refs `env:<VAR>`, `file:<path>`,
`mounted-file:<path>`, `dotenv:<path>#<VAR>` and `dotenv:<VAR>`. External
backends such as `vault:*`, `kubernetes:*` and cloud secret managers remain
contract-level refs for `plan`/`preflight` and fail-fast on `apply` until an
external resolver is connected.

The HTTP adapter publishes a thin wizard surface:
`GET /api/install/status`, `POST /api/install/plan`,
`POST /api/install/preflight`, `POST /api/install/apply`,
`GET /api/install/jobs/{job_id}`, and
`GET /api/install/sessions/{session_id}/receipts`. HTTP `apply` starts a
background job; the UI must not duplicate migration, seed, or admin logic.

The topology contract distinguishes a one-role `monolith` from a distributed
deployment descriptor. Trusted CLI and HTTP hosts bind the selected
distribution revision/hash as a compatibility check and, before any apply,
must also bind the exact
`distribution_release_id`, OCI bundle-root digest, role-set digest, and exact
per-role artifact digests before
preflight and apply; a wizard never supplies those identities. They resolve
from the owner admission ledger or, only for a fresh target, a platform-signed
base-distribution receipt that also binds its public `preparation_id`. A
distributed role is single-purpose (`api`,
`admin_ssr`, `storefront_ssr`, `worker`, or `registry`) and may not claim
another role's surface.

Before any target mutation, the host also verifies the separately signed,
digest-pinned operations-tool release containing the external deployment
controller and node agent, including exact component/target digests and their
external protocol revision. These tools are installed by the host
provisioner/service supervisor and are not role-bundle contents or an
installer/candidate self-update. After bootstrap, their upgrade is the
`operations_tool_maintenance` class in the same canonical `rustok-modules`
operation ledger and fleet fence. The supervisor applies exact desired
assignments as a narrow executor, retains the predecessor tools, and reports
protocol-compatible convergence.

The production target sends the complete role and surface set through one
neutral distribution deployment request. `rustok-modules` owns one admitted
role-bundle release and one desired/observed rollout; the bound deployment
receipt contains per-node, per-role observations but creates no per-role
release head. Fresh bootstrap pre-stages the exact candidate bundle, verifies
its recovery boundary, creates the minimal owner schema, imports/revalidates
the signed receipt into the sole `rustok-modules` ledger, and only then applies
the remaining schema, tenant seed, and admin provisioning once. Build and
publication are never `install apply` dependencies. The independent per-role
Axum-to-`rustok-build` path has been removed. The typed installer now creates
one request and one receipt for the complete admitted bundle and requires
exact per-role observations. The server HTTP host ignores a client-supplied
bundle and resolves `RUSTOK_INSTALL_DISTRIBUTION_RELEASE_ID` through the
current admitted `rustok-modules` ledger. For a fresh target, the trusted host
instead requires both `RUSTOK_INSTALL_BASE_DISTRIBUTION_RECEIPT` and
`RUSTOK_INSTALL_BASE_DISTRIBUTION_PUBLIC_KEY`; it verifies the bounded regular
file, strict Ed25519 signature, signer-key digest, validity interval, immutable
bundle identity, and executable-composition match before binding the receipt
to the checksummed plan. Configuring this pair together with
`RUSTOK_INSTALL_DISTRIBUTION_RELEASE_ID` is rejected. The CLI accepts the same
environment inputs or matching `--base-distribution-*` options. The shared
executor now applies the minimal owner schema, imports the signed receipt
transactionally into the empty `rustok-modules` ledger, and only then runs the
remaining canonical migrations. HTTP and CLI keep the public key outside the
transport plan; the owner verifies it again, and exact replay uses one
deterministic instance/release idempotency key. The owner-controlled
deployment stage is mandatory for monolith and distributed plans, so a
successful apply always contains one deployment receipt rather than an
optional shortcut. Its desired/observed adapter remains a fail-closed
implementation gap.
See the
[implementation plan](implementation-plan.md) and the
[release and rollback plan](../../../docs/modules/module-release-rollback-plan.md)
for ownership and rollout.

## Related documents

- [Hybrid installer ADR](../../../DECISIONS/2026-04-26-hybrid-installer-architecture.md)
- [Installer topology composition identity ADR](../../../DECISIONS/2026-07-12-installer-topology-composition-identity.md)
- [Module release rollback safety ADR](../../../DECISIONS/2026-08-06-module-release-rollback-safety.md)
- [Module architecture](../../../docs/architecture/modules.md)
- [Platform database schema](../../../docs/architecture/database.md)
- [Installer implementation plan](implementation-plan.md)

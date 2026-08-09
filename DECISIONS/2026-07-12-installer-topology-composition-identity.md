# Installer Topology Composition Identity

- Date: 2026-07-12
- Status: Accepted, amended on 2026-08-09

Production publication and deployment are amended by
[Module release rollback safety](./2026-08-06-module-release-rollback-safety.md).
The installer submits one complete distribution deployment request; it does
not build, publish, activate, or track an independent release head per role.

## Context

An install profile describes UI and build intent but is not a deployable
topology. The installer must record which selected module distribution created
the schema and tenant state. A manually entered revision or hash is not
trustworthy: the CLI and HTTP host already know the executable distribution,
while a wizard client does not.

## Decision

`rustok-distribution` publishes `composition_identity()` from its selected
compile-time module registry. The identity contains a readable revision and a
canonical SHA-256 hash over module slug, version, kind, and dependencies. This
is a host compatibility check, not deployable identity: it does not bind the
toolchain, target, role artifacts, browser assets, or publication evidence.

The trusted local installer also resolves one operator-selected
`<instance-root>` on any supported operating system. Relative input is resolved
against the installer invocation directory; the normalized result is retained
only as host-local placement and restart evidence. The instance layout is one
portable set of relative directories for configuration, operations tools,
immutable platform releases, sources, object storage, bundled-service data,
state, build work, caches, logs, and runtime files. No fixed Linux FHS path,
drive, separator, container mount, or root privilege is required. A remote
wizard cannot inject an unrestricted host path: it consumes the root selected
or allowed by its trusted local installer adapter.

Before mutation, the root must be absent/empty or contain the exact installer
marker for an idempotent resume of the same installation. Cleanup removes only
create-only paths proven owned by that failed attempt; it never recursively
deletes the selected root or unrelated operator files.

An instance-root path is never distribution, module, migration, object, or
operation identity. Release and object authority remains exact IDs, digests,
and logical object keys. Independent installations use independent roots and
owner data planes. Distributed adapters may map individual relative subtrees
to OCI layers, object storage, volumes, logging, or operating-system runtime
facilities without changing the canonical layout or identities. The default
local/monolith installation remains self-contained under the selected root.

`rustok-installer::InstallTopology` is the canonical descriptor of selected
surfaces and their role ownership. A topology may arrive unbound from a
transport client. The CLI and HTTP host replace its composition identity with
their own selected distribution before preflight, receipt creation, or apply.
The trusted owner/host also resolves and binds the exact admitted
`distribution_release_id`, OCI bundle-root digest, and role-set digest before
preflight. A wizard cannot supply those identities. On a fresh standalone
target with no release ledger, the host verifies a platform-signed
base-distribution receipt binding a platform-public `preparation_id`, those
identities, and migration/data/evidence digests, then imports the same receipt
into `rustok-modules` as soon
as its minimal owner schema exists. That bounded bootstrap handoff is not a
second release owner. Installer checksum, deployment request, observations, and
terminal receipt all bind the exact bundle identity.
The core validates that every selected surface has exactly one role owner.
Distributed roles are single-purpose: an `api`, `admin_ssr`, `storefront_ssr`,
`worker`, or `registry` role may only own its matching surface. The Axum host
recognizes `RUSTOK_RUNTIME_HOST_MODE=api`, `admin_ssr`, `storefront_ssr`, and
`worker`, plus the registry process mode
`RUSTOK_RUNTIME_HOST_MODE=registry_only`. API and SSR modes bootstrap the
runtime without background workers; the worker mode starts them while exposing
only health and metrics HTTP surfaces, and `registry_only` owns only the
registry surface.

The installer binds the full required role and surface set into one immutable
distribution request. Production apply consumes an owner-admitted base bundle
or, only for fresh bootstrap, the exact platform-signed receipt described
above; source/build/publication preparation is a separate operation and is not
an `install apply` dependency. `rustok-modules` owns the single role-bundle
release, desired rollout, and observed rollout state. First install has no
direct predecessor. `rustok-build` constructs and validates the canonical role
plan; `rustok-static-distribution-worker` alone executes that plan, owns the
job/publisher configuration, publishes the bundle, and returns one bound
receipt containing the per-role artifact and evidence identities. Neither can
activate a release or advance a serving head. The deployment controller
reconciles the admitted bundle and records per-node, per-role observations
under the same installer operation. The installer completes only after the
complete requested bundle is observed healthy.

The deployment controller and node agent are an independently published,
signed operations-tool release outside the application role bundle. Before
mutation, installer preflight binds and verifies its exact package/component
digests, target, evidence identity, and external protocol revision. A host
provisioner/service supervisor installs the bootstrap tools; installer apply
and candidate application code never self-update them. Subsequent tool upgrade
is the `operations_tool_maintenance` class in the same canonical
`rustok-modules` operation/receipt ledger with a fleet conflict fence. The
supervisor is its narrow executor, retains the exact predecessor tools, and
reports protocol-compatible convergence before application lifecycle work
resumes.

`rustok-build` owns portable role-plan construction and validation contracts.
The static-distribution worker owns fixed build/publication executor inputs,
workspace rules, and publisher configuration. Operations hosts own deployment
settings parsing and secret resolution, but neither a host nor `rustok-build`
owns production release selection. The canonical publisher accepts explicit
worker-owned configuration and artifact paths rather than inferring repository
layout. The deployment controller and a future standalone CLI control adapter
use the same owner lifecycle without importing `apps/server` and cannot invoke
the publisher from install apply.

Distributed topology fails preflight when its host has no typed deployment
adapter. No host may silently treat a distributed request as a monolith
installation.

## Consequences

- Installer receipts and plan checksums contain a deterministic distribution
  identity, exact distribution release, bundle root, and role-set digest.
- Installer placement receipts bind the trusted selected instance root for
  local restart without folding its platform-specific path into distribution,
  module, migration, object, or cross-node operation identity.
- A default installation may live in any operator-selected directory and keeps
  one portable relative layout; FHS paths, containers, and external storage are
  optional adapter mappings rather than separate lifecycle contracts.
- The wizard remains a thin client and never imports distribution internals.
- Every deployment adapter consumes the topology descriptor as one role-bundle
  request and records per-role observations under one deployment receipt; it
  may not redefine the composition identity or create per-role release heads.
- Installer preflight verifies a signed, digest-pinned operations-tool release
  and protocol revision that is installed independently of the candidate role
  bundle and remains recoverable through owner-ledger
  `operations_tool_maintenance` executed narrowly by the host supervisor.
- A worker deployment is no longer a headless API alias; its host mode has an
  explicit runtime boundary carried through build/release automation.
- Server and future CLI control adapters share the owner-owned deployment
  contract; build executors retain build/publication side effects, and only the
  external deployment controller and node agents perform runtime replacement.

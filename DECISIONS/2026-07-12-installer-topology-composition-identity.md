# Installer topology composition identity

- Date: 2026-07-12
- Status: Accepted

## Context

An install profile describes UI and build intent but is not a deployable
topology. The installer must record which selected module distribution created
the schema and tenant state. A manually entered revision or hash is not
trustworthy: the CLI and HTTP host already know the executable distribution,
while a wizard client does not.

## Decision

`rustok-distribution` publishes `composition_identity()` from its selected
compile-time module registry. The identity contains a readable revision and a
canonical SHA-256 hash over module slug, version, kind, and dependencies.

`rustok-installer::InstallTopology` is a versioned descriptor of selected
surfaces and their role ownership. A topology may arrive unbound from a
transport client. The CLI and HTTP host replace its composition identity with
their own selected distribution before preflight, receipt creation, or apply.
The core validates that every selected surface has exactly one role owner.
Distributed roles are single-purpose: an `api`, `admin_ssr`, `storefront_ssr`,
`worker`, or `registry` role may only own its matching surface. The Axum host
recognizes `RUSTOK_RUNTIME_HOST_MODE=api`, `admin_ssr`, `storefront_ssr`, and
`worker`. API and SSR modes bootstrap the runtime without background workers;
the worker mode starts them while exposing only health and metrics HTTP
surfaces.

The Axum host composes the first role-specific build/release adapter when
`rustok.build.enabled=true`: it requests the role plan from `rustok-build`,
publishes an active release, and records one receipt per role.

`rustok-build` owns the portable `DeploymentSettings`, `DeploymentBackend`,
and `DeploymentWorkspace` contracts. Hosts own settings parsing, secret
resolution, concrete release publication, and post-activation side effects.
The server publisher accepts these build-owned deployment configuration and
artifact/runtime-path contracts rather than inferring its repository layout, so
a future standalone CLI adapter can use the same surface without importing
`apps/server`.

Distributed topology fails preflight when its host has no typed deployment
adapter. No host may silently treat a distributed request as a monolith
installation.

## Consequences

- Installer receipts and plan checksums contain a deterministic distribution
  identity.
- The wizard remains a thin client and never imports distribution internals.
- Every deployment adapter consumes the topology descriptor and records
  per-role deployment receipts; it may not redefine the composition identity.
- A worker deployment is no longer a headless API alias; its host mode has an
  explicit runtime boundary carried through build/release automation.
- Server and future CLI adapters share the build-owned deployment settings
  contract while retaining their own executable-host side effects.

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

Distributed topology is representable now but fails preflight until a typed
deployment adapter exists. No host may silently treat a distributed request as
a monolith installation.

## Consequences

- Installer receipts and plan checksums contain a deterministic distribution
  identity.
- The wizard remains a thin client and never imports distribution internals.
- A future deployment adapter must consume the topology descriptor and record
  per-role deployment receipts; it may not redefine the composition identity.

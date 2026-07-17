# Exact installation identity for sandboxed module artifacts

- Date: 2026-07-17
- Status: Accepted

## Context

Durable artifact work targets an exact admitted installation. A sandbox subject
previously identified an artifact only by slug, version, and digest. The same
release can be installed for distinct tenants or scopes, so that release
identity cannot select one capability scope safely. A production-wide sandbox
runtime also cannot be composed from brokers that each capture one installation
scope at construction.

## Decision

`SandboxSubject::ModuleArtifact` carries the exact non-nil `installation_id`
selected by the module owner. `InstalledModuleArtifact` is the only module
adapter that constructs this subject. The identifier is host-controlled
execution metadata: an artifact cannot provide, replace, or read it.

The future production artifact capability router must resolve its host-owned
scope using the exact installation ID together with the tenant context and
admitted artifact identity. It must deny an unknown, disabled, mismatched, or
otherwise ineligible installation. It must not select a scope from the latest
release by slug, and it must not use an Alloy deny broker or a static scoped
broker as a production fallback.

The host-owned admission command carries the initial `SandboxPolicy`, which is
persisted with the exact installation and its capability-grant revision. A
tenant installation receives a tenant-bound policy; a platform installation
receives a platform default that a later tenant-specific policy may replace.
The normal empty policy grants nothing. The resolver rechecks active lifecycle,
tenant eligibility, revision, positive limits, and descriptor-declared
capabilities before returning a policy. A missing or mismatched record denies
execution.

## Consequences

- Sandbox admission, execution, capability, and audit observers receive exact
  installation identity for installed artifacts. New durable execution evidence
  persists that identity; older redacted audit rows remain nullable because it
  cannot be reconstructed safely.
- Dynamic owner routes share an exact-installation gate that reloads active
  admission, lifecycle, uninstall, policy revision, and the capability's
  explicit durable grant before a broker is created. The server can compose one
  neutral sandbox runtime only after it supplies those owner routes, their
  deployment-specific adapters, and the policy resolver.
- Capability-policy changes require a separate owner revision command; a
  descriptor declaration never becomes an implicit policy update or grant.
- Existing fixed-scope brokers remain useful as narrow deployment adapters and
  tests, but do not satisfy production-wide artifact runtime composition.

# Effective Module Policy Decision

Status: Accepted

Date: 2026-07-22

## Context

Module availability was exposed primarily as an enabled-module set assembled
from a definition catalog, platform defaults, and tenant overrides. That
projection is useful to callers, but it cannot explain why a module is enabled
or denied and provides no immutable identity for cache invalidation, audit, or
stale-decision rejection. Allowing each host to reconstruct those facts would
create divergent lifecycle, routing, runtime, transport, and UI policy.

## Decision

`rustok-modules` owns one serializable `ModuleEffectivePolicy` result.
The internal owner query normalizes platform defaults and tenant overrides,
combines them with the exact artifact-aware definition catalog and redacted
runtime evidence, and computes a deterministic `sha256:` policy revision over
that complete canonical input.

Every known module has a typed decision containing:

- its exact definition version, kind, and platform-native, promoted-native, or
  artifact source identity;
- whether it was selected by platform defaults;
- any persisted tenant override;
- the observed enabled state of each declared dependency;
- the exact active installation identity, scope, release digest, immutable
  dependency-graph revision/digest, and admitted capability-grant revision;
- whether the exact durable capability policy and isolated executor are
  available;
- a stable owner denial taxonomy when it is not enabled.

An unknown module is explicitly denied with `unknown_module`. Core definitions
remain enabled even if stale tenant data contains a disabling override; the
override remains visible as a contributing fact. Enabled-module collections are
projections of the same decision object, not a parallel policy calculation.

`ModuleLifecycleDbWriter`, `EffectivePolicyService`, and the server adapter all
resolve this decision before deriving their enabled projection. Revision
encoding fails closed rather than returning an unversioned decision.

Artifact runtime inputs are obtained only through the existing active
installation and sandbox-policy owner resolvers. Those resolvers apply tenant
RLS, exact release identity, lifecycle state, uninstall exclusion, policy scope,
and capability-revision equality. A selected artifact without that evidence,
without an injected isolated executor, or with an unavailable transitive
dependency is denied. Grant contents and resolver error text are not copied into
the policy object.

This decision now covers catalog/default/tenant intent plus artifact
installation, capability-policy, executor, dependency, registry, quarantine,
emergency-revocation, channel, and revisioned maintenance availability.
Maintenance blocks serving without rewriting tenant intent. Node-readiness
is now represented by a host-owned snapshot that must observe the base policy
revision; the final revision includes that validated readiness evidence. An
unready or stale observation fails closed before serving.

## Consequences

- Host adapters can expose or retain explainable policy evidence without
  reading owner tables directly.
- Caches can key and invalidate the current slice by a deterministic revision.
- Input ordering cannot change the revision after normalization.
- Adding a policy-relevant input requires adding it to both the decision facts
  and revision material; silently consulting an unversioned host value is
  prohibited.
- Outbox consumers must apply policy transitions through a predecessor-bound
  revision gate. Digest values are opaque identities; duplicate transitions are
  idempotent and stale/out-of-order transitions cannot advance a durable
  cursor.
- Durable consumers persist that cursor by tenant and consumer key under RLS;
  cursor creation, predecessor validation, and advancement occur in one
  transaction, while the event journal remains the sole event source. Owner
  services may call the consumer's transaction-bound adapter so owner state,
  outbox append, and cursor advancement are committed atomically.
- Effective-policy producers use the explicit
  `module.effective_policy_revision_changed` event and
  `ModuleEffectivePolicyTransitionPublisher`, which rejects non-digest or
  no-op transitions before appending to the owner transaction. Security and
  native distribution command revisions are separate identities and cannot be
  used as effective-policy predecessor/successor values. Tenant lifecycle
  toggles use `ModuleEffectivePolicyTransitionCoordinator`, which advances the
  lifecycle cursor and appends that event in the same state transaction, so a
  stale concurrent transition rolls back the tenant mutation.
- The enabled projection alone is insufficient evidence for execution once the
  remaining Phase 8 facts are integrated.

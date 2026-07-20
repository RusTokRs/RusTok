# rustok-modules documentation

## Purpose

This Core module owns the module platform control plane and artifact lifecycle.

## Responsibility Zone

It owns marketplace release identity, digest-pinned package admission,
tenant-policy rules and the contracts for installation, activation, rollback,
capability grants and static-promotion admission. Persistence adapters and
owner transports are still being moved from the server. It does not own sandbox
implementation or Alloy source authoring.

## Integration

Rhai and WebAssembly artifact descriptors select executors from
`rustok-sandbox`. A package preserves both its OCI manifest digest and its
verified payload-layer digest. Alloy drafts carry source lineage and create
immutable module releases/packages. The server supplies infrastructure adapters
and mounts owner transports.

`OciDistributionArtifactRegistry` resolves only digest-pinned references. It
requires the returned manifest digest to match the requested reference, reads
the descriptor from the manifest config, and downloads exactly one payload
layer whose digest and media type match that descriptor. Admission limits reject
an oversized descriptor and the OCI-declared layer size before `pull_blob`, then
stream the received bytes into a private temporary file while enforcing the
same size limit and SHA-256 digest.

`OciDistributionArtifactPublisher` publishes the descriptor-selected payload
and OCI 1.1 SBOM/provenance referrers. The isolated build worker subsequently
uses Cosign with a deployment-owned KMS provider reference to sign the returned
artifact digest, then resolves Cosign's compatible OCI signature manifest to a
digest-pinned publication receipt. The standard Cosign tag is used only while
resolving the signature manifest and never becomes installation identity.
That receipt records build-service signature evidence only; author signatures
and marketplace approvals remain separate owner-governance facts.
Before that publication window the worker obtains a repository-scoped,
short-lived lease through its deployment-owned credential broker. Credentials
never enter module contracts, descriptors, build requests, runner output, or
artifact persistence.

`strict_oci_distribution_client` configures the enforceable subset of registry
transport policy through `OciRegistryTransportPolicy`: HTTPS only, certificate
validation, no redirects or cross-host authentication, deployment-boundary-only
proxy handling, bounded request/retry/transfer/decompression ceilings, no
image-index fallback, and one concurrent upload/download. The client applies
the controls exposed by `oci-distribution`; the deployment egress boundary
must enforce the remaining redirect, proxy, retry, timeout, and decompression
controls because the upstream client has no corresponding hooks. A weaker
policy is rejected before client construction, and deployment verification of
those boundary controls remains tracked in the central plan.

During admission, `ModuleInstaller` verifies the OCI package and places its
payload in an `ArtifactBlobStore` under the descriptor payload digest.
`ArtifactRuntime` reads only that admitted digest-pinned blob for execution;
the external OCI registry is a distribution source and is not consulted at
runtime. Missing or corrupted blobs fail closed before a sandbox request is
created.

Every dynamic execution is selected by an admitted `ModuleRuntimeBinding`.
`ArtifactRuntime` wraps the owner payload in the strict versioned
`ArtifactBindingDispatchEnvelope` before the sandbox runs, so artifact code
cannot select its binding or phase. It validates the enclosed owner payload
against the binding's exact descriptor-bundled Draft 2020-12 schema; after
execution it validates the decoded owner output against the corresponding output
schema.
Artifact persistence is limited to a revision plus a bundled schema digest for
brokered namespaced values; descriptor decoding rejects unknown fields, so an
artifact cannot attach SQL, DDL, native migrations, a bucket path, or a host
storage handle.
Dynamic artifact UI is similarly declarative-only: v1 accepts only
`admin_settings` and `admin_actions` contribution surfaces with immutable
localization and declared permissions. Descriptor fields cannot carry a
component, URL, iframe, or native frontend package; native UI remains a static
promotion concern.
Schemas are keyed by their canonical digest, compile into a bounded node-local
LRU cache with linear-time regex limits, and use a `jsonschema` build without
filesystem or HTTP resolver features. Non-local `$ref`, `$dynamicRef`, and
`$recursiveRef` values are rejected during descriptor admission.

Artifact permission descriptors carry immutable localized labels and
descriptions. Admission sends them through the shared
`ArtifactPermissionRegistrationPort` only after the installation is committed;
the installation ID makes the owner operation idempotent and a retry repeats
registration for an already admitted release. The port adds RBAC vocabulary
only and cannot assign a permission to a role or actor.

`module_artifact_installations` is the host-managed persistence boundary. Its
PostgreSQL migration enables RLS; tenant-scoped connections must set
`rustok.tenant_id` before querying or mutating tenant installation rows.
`SeaOrmArtifactInstallationStore` performs that setup while atomically writing
the installation, admission metadata, and `module.artifact.admitted` outbox
envelope. It stores the reference and canonical descriptor, never artifact
bytes. `StorageArtifactBlobStore` supplies the production CAS adapter over the
platform `StorageService`: it uses private staging keys, conditional creation
of digest-derived final keys with the admitted media type, and verified reads. CAS publication remains
outside the database transaction; the reconciler removes an orphan only after
it has no committed admission reference and an explicit durable retention
snapshot rule allows deletion. A missing snapshot rule fails closed; the rule
must be expired and free of legal hold, rollback protection, and audit
retention. Runtime never falls back to the OCI registry: it executes only a
verified admitted CAS blob and returns `BlobNotFound` before sandbox execution
when that blob is unavailable.
`InMemoryArtifactBlobStore` is test/local-only. Host production configuration
must wire `StorageArtifactBlobStore` to a durable object-storage driver, never
a node-local cache.

After verification the current storage upload API still accepts a bounded
buffer. The next admission slice replaces that final boundary with a streaming
sink and multipart/object-store upload; no unbounded fallback is permitted.

## Verification

- `cargo xtask module validate modules`
- `cargo test -p rustok-modules`
- `cargo check -p rustok-server --lib`

## Related Documents

- [Implementation plan](./implementation-plan.md)
- [Neutral sandbox ADR](../../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md)
- [Module control-plane plan](../../../docs/modules/module-control-plane-consolidation-plan.md)

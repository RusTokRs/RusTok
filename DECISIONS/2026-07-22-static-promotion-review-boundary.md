# Static Promotion Review Boundary

Status: Accepted, amended on 2026-08-09

Date: 2026-07-22

The production publication, deployment, and automatic rollback mechanisms are
amended by
[Module release rollback safety](./2026-08-06-module-release-rollback-safety.md).
The canonical production output is one complete immutable role bundle, not an
independently activated artifact or release head per role. All other promotion,
build, verification, and trust-boundary decisions remain in force.

## Context

RusToK supports sandboxed marketplace artifacts and reviewed native modules,
but native compilation cannot be a fallback from artifact installation. A
promotion decision must retain the exact published release, source, dependency
lock, review evidence, and platform approver before distribution tooling can
consider native composition.

## Decision

`rustok-modules` owns the single current static-promotion workflow through
`ModuleControlPlane::promotion`.

A promotion request is eligible only for an active `platform_built` marketplace
release. The owner reloads its publication staging row and completed
tenant-scoped build request/result, validates the build result and OCI
publication receipt again, and persists the exact release, publish request,
CAS source reference, source digest, dependency-lock digest, Cargo package, and
normalized crate-local native entry type. Package and entrypoint identity come
from the registry release rather than the promotion caller. A release without
a valid native Rust entry type is not promotion-eligible.

Approval requires optimistic revision CAS, a non-nil platform actor, an
immutable policy identity, and digest-pinned ownership, dependency-audit, test,
and static-review evidence. A mandatory host authorization port has distinct
request and approval decisions, both fail closed. Request and approval commands
use a durable global idempotency journal that replays only the original
status/revision receipt. The persisted requester cannot approve the same
promotion.

Approved records are inert. This service has no compiler, active-composition
writer, native loader, compatibility alias, or alternate versioned path.
`ModuleControlPlane::static_distribution` is the only owner that can select
approved records for native composition. Each accepted command replaces the
complete selection, pins the platform source, toolchain and target, creates an
immutable build-lineage-linked build intent, and advances a separate CAS head.
That lineage supports reproducibility and supersession audit; it is not the
serving direct predecessor for recovery.
Platform and promoted source references must exactly equal
`cas://sha256:<hex>`. Each immutable distribution item carries the reviewed
Cargo package and native entry type, so both values participate in the
composition digest and are revalidated during release admission and recovery.
It does not change the active runtime composition; CI/distribution tooling must
complete the queued build before any native implementation can exist in a
release.

`ModuleControlPlane::static_distribution_worker` is a separately authorized
worker boundary. Atomic claim uses a bounded lease and an immutable attempt
record. Heartbeat and completion require the exact claim/runner pair and an
unexpired lease. Reclaim first closes the prior attempt as `lease_expired`.
Successful completion requires a digest-pinned role-bundle receipt covering the
complete selected role and surface set, per-role artifact identities, browser
asset manifests where selected, and the bound SBOM, provenance,
signature-manifest, and test evidence. Terminal replay is accepted only for the
identical completion digest. Completion remains evidence, not release
admission, serving selection, or activation.

`rustok-static-distribution-worker` is the separately deployed implementation
of the executor port. It requires a digest-pinned launcher, job configuration,
toolchain, and target; readiness and every execution re-hash the fixed files.
For one immutable claim it stages bounded create-only generated inputs in a
stable attempt directory and invokes only that launcher with fixed arguments,
an empty environment, closed standard streams, and a bounded lifetime. A
terminal receipt must bind the exact request bytes, claim, composition,
generated output, launcher, job configuration, toolchain, and target. Missing
or mismatched output is a reclaimable transport failure, never a fabricated
terminal result. The launcher is the deployment-owned CI adapter responsible
for exact CAS materialization, compilation, tests, signing, and evidence
publication; neither the owner nor the gRPC adapter receives those credentials.
The launcher and untrusted module-build worker share the single
`rustok-build-source` strict USTAR materializer. Worker-local extraction code
is prohibited so archive-safety fixes cannot diverge between trusted and
untrusted build paths.
The native launcher additionally regenerates all generated inputs, verifies
each materialized package name, version, and dependency-lock digest, rejects
dependency-alias collisions, and edits only its new job-local platform
workspace. Its digest-pinned job config is the sole source of CAS, Cargo,
Rustc, publisher, target, and resource identities. It resolves the final
workspace lock offline after composition, runs only fixed locked test and
release-build commands, and binds the raw resolved-lock digest into test
evidence and the publisher request. The digest-pinned publisher must be
idempotent by publisher-request digest. Its request enumerates the complete
canonical role plan and every exact job-local output path that belongs to it.
Its receipt is accepted only when it binds that request, the immutable
job/composition/generated output, resolved lock, complete role and surface set,
per-role artifact digests, browser asset manifests, role-bundle root digest,
and all required evidence identities. Reclaim rebuilds only the job-owned
derived workspace; immutable attempt inputs are verified, not overwritten.

The production publisher is the sole fixed role-bundle publisher in the
static-distribution worker package. It reads each selected role artifact and
browser asset manifest only from its declared job-local fixed path, publishes
the complete role bundle under one OCI root digest, and attaches CycloneDX
SBOM, SLSA provenance, raw test evidence, signatures, and the other required
evidence as digest-bound OCI referrers. It signs the exact bundle and role
digests with KMS-backed Cosign, resolves every signature manifest digest, and
writes one create-only fully bound role-bundle receipt. The receipt records raw
evidence payload digests separately from their referrer manifest digests.
Credential broker and Cosign process handling are shared with the untrusted
module publisher through `rustok-build-publication`; both programs are
deployment-pinned and re-hashed before every invocation. There is no
worker-local alternate credential, signing, per-role publisher, or independently
activated per-role release head.

Native execution identity is explicit rather than inferred from Cargo or the
compiled registry. Each immutable distribution item persists
`executor_mode = static_native`, and that value participates in composition and
generated-output digests. A release read reloads and validates the complete
succeeded build before exposing those items. Runtime catalog construction
retains ordinary compiled modules as `platform_native` and maps selected
promotions to `promoted_native` definitions carrying the exact promotion,
registry release, distribution release/revision, artifact digest, and executor
mode. Lifecycle and effective-policy services can use the same catalog while
resolving implementation handles only through the compiled registry. Rolling
the binary onto nodes and reconciling desired/observed state remain separate
deployment operations.

The rollout boundary is owner-owned and topology-bound. A trusted topology
resolver supplies canonical placement and a sorted set of assignments binding
`node_id`, failure domain, role, candidate role digest, and direct-predecessor
role digest for an automatic update. First install uses candidate-only
assignments and has no recovery target. Automatic code update requires an
identical node/failure-domain/role assignment domain; placement/count or
role/surface-shape changes are separate maintenance transitions. The owner
creates a desired rollout only for one exact admitted and verified role bundle
plus the current policy revision. Node agents report
identity-bound per-role observations with per-node revisions and health
evidence. The normal path is `preparing -> activating -> converged`; a failed
candidate transition enters the one authorized `recovering` path or
`recovery_required`, while post-convergence drift enters recoverable
`degraded`. The owner advances desired and observed rollout state under
database CAS, journals request/report idempotency, rejects stale reports, and
writes outbox events. Required role or production-surface changes create a new
role bundle and release; changes only to node placement, count, or wave weights
change rollout state. No owner operation invokes a native loader or replaces a
server process; the deployment controller, node agents, and their transport are
separate and remain available when candidate processes fail.

`ModuleControlPlane::static_distribution_release` is the only role-bundle
release-admission owner. It accepts only the current successful build and
requires separate host authorization plus an external fail-closed verification
decision for the exact bundle and requested policy revision. Signature,
provenance, SBOM, test, and dependency-policy facts must all pass. The owner
then relocks the distribution head and build, revalidates every promotion and
published role-bundle fact, stores an immutable admitted release with exact
build lineage plus admission/idempotency evidence, and writes its outbox event.
Admission never
supersedes the serving release and never changes desired or observed rollout
state. Serving selection changes only through a separately fenced production
transition that converges the complete bundle. Admission cannot load native
code or mutate the running composition.

Automatic rollback never compiles on the incident path. The canonical static
release binds the complete immutable role bundle, including every automated
server/worker role, embedded Leptos artifact, generated registry, and browser
asset. Before candidate rollout, the owner must retain and rehash the candidate
operation's then-serving direct-predecessor bundle and revalidate its admission,
security, policy, data compatibility, topology, and deployment evidence. The
production operation, not release admission, freezes that predecessor.
Missing bytes or evidence makes the transition ineligible for automatic mode.

Rollback creates a new audited owner transition to that exact retained
predecessor and converges it through the normal desired/observed rollout
boundary. It neither edits old bytes nor queues a replacement build, and it is
successful only when the predecessor role bundle is observed healthy. Rebuild
remains reproducibility evidence or a separately admitted maintenance update
through the same owner lifecycle; it is never a rollback fallback. Revocation
or quarantine preempts a stale rollback decision and cancels any transition
involving the affected release.

## Consequences

- Runtime marketplace operations cannot trigger Cargo or mutate the server
  dependency graph.
- External prebuilts and Alloy-authored releases cannot enter native promotion
  without first producing a platform-built published release from reviewed
  source.
- Source and dependency evidence cannot change between request and approval.
- Removing a promotion from a future distribution requires another complete
  build intent; approval alone never changes runtime behavior.
- Worker completion alone cannot activate native code. Verified release
  admission, production transition, retained-predecessor recovery, and
  revocation remain distinct owner transitions, while deployment agents
  execute only the exact desired/observed rollout contract.
- The trusted native-distribution worker and the untrusted sandbox-artifact
  build worker are separate processes with different launchers and credentials.

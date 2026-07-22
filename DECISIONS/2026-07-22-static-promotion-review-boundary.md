# Static Promotion Review Boundary

Status: Accepted

Date: 2026-07-22

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
immutable predecessor-linked build intent, and advances a separate CAS head.
Platform and promoted source references must exactly equal
`cas://sha256:<hex>`. Each immutable distribution item carries the reviewed
Cargo package and native entry type, so both values participate in the
composition digest and are revalidated during release activation and rollback.
It does not change the active runtime composition; CI/distribution tooling must
complete the queued build before any native implementation can exist in a
release.

`ModuleControlPlane::static_distribution_worker` is a separately authorized
worker boundary. Atomic claim uses a bounded lease and an immutable attempt
record. Heartbeat and completion require the exact claim/runner pair and an
unexpired lease. Reclaim first closes the prior attempt as `lease_expired`.
Successful completion requires digest-pinned artifact, SBOM, provenance,
signature-manifest, and test evidence; terminal replay is accepted only for the
identical completion digest. Completion remains evidence, not release activation.

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
idempotent by publisher-request digest. Its receipt is accepted only when it
binds that request, the immutable job/composition/generated output, the
resolved lock, and all five evidence identities. Reclaim rebuilds only the
job-owned derived workspace; immutable attempt inputs are verified, not
overwritten.

`ModuleControlPlane::static_distribution_release` is the only release-activation
owner. It accepts only the current successful build and requires separate host
authorization plus an external fail-closed verification decision for the exact
build and requested policy revision. Signature, provenance, SBOM, test, and
dependency-policy facts must all pass. The owner then relocks the distribution
head and build, revalidates every promotion and published build fact, and
atomically supersedes the previous release, advances a dedicated release CAS
head, stores immutable admission/idempotency evidence, and writes its outbox
event. Release activation records deployable identity; it cannot load native
code or mutate the running composition.

Rollback is rebuild-only. The owner accepts only the active release's direct,
non-revoked predecessor and only while the distribution head still identifies
the active release's build. It revalidates the target admission, terminal and
composition digests, promotion reviews, and published build facts, then queues
a new immutable build under distribution CAS. The old artifact cannot become
active directly. Standard worker completion and verified release activation are
required again, and the rebuilt artifact digest must reproduce the target. A
superseding selection or failed/cancelled build cancels the pending request.
Revocation is a separate authorized exact-replay command under
release-state CAS. It preserves immutable evidence, clears an active release
head, and cancels pending rollback requests involving the revoked release.

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
  activation, rebuild-only rollback, and revocation now exist, while deployment
  remains a separate follow-up owner operation.
- The trusted native-distribution worker and the untrusted sandbox-artifact
  build worker are separate processes with different launchers and credentials.

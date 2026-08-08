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

Host command and HTTP adapters dispatch admitted artifact bindings through
`ArtifactCommandBindingRequest` and `ArtifactHttpBindingRequest`. These
envelopes keep the immutable release, admitted binding set, installation target,
tenant, payload, and authenticated execution context together at the owner
boundary instead of spreading one operation across positional arguments.

`ControlPlaneInfrastructure` is the owner context for clock and UUID ports.
`ModuleControlPlane` creates one production context and can accept an injected
context for deterministic owner fixtures. Admission, installation lifecycle,
build, release, publication, binding-idempotency, event/schedule delivery, and
identity-allocating object-data operations use it for installation, operation,
outbox, verification, commit-evidence, governance aggregate, validation-stage,
claim, event, delivery, work-lease, upload-session, private-storage,
GC-candidate, export, secret-event, lifecycle-correlation, CAS-stage,
OCI-temporary-stage, static-promotion operation, and lease-time identities.
Schedule materialization also
uses the injected owner time. Direct system clock and random UUID access is
confined to the default infrastructure adapters outside test fixtures.
Database-expression timestamps remain owned by the transactional storage
adapter so one commit uses the database clock.

Effective availability is returned as `ModuleEffectivePolicy`, not reconstructed
by a host from a boolean set. Its `sha256:` policy revision covers the exact
ordered definition catalog, normalized platform defaults, and persisted tenant
overrides plus owner-resolved artifact runtime evidence. Each selected artifact
must resolve an exact active installation through tenant RLS, a matching durable
capability-policy revision, an available isolated executor, and an enabled
dependency closure. The policy retains only redacted installation/revision facts,
not grant contents or resolver failures. A channel owner can add a canonical
tenant-safe snapshot containing channel identity, surface, binding state, and
its own `sha256:` revision. The modules owner evaluates that snapshot without
resolving channel tables, and channel facts become part of the same policy
revision. Operational owners can additionally supply a revisioned maintenance
snapshot with a bounded reason code and global or module-scoped impact;
maintenance blocks serving without changing tenant enablement. Lifecycle writes and server guards
obtain the enabled projection from this same decision object; unknown modules
and unavailable artifact runtime state are explicitly denied instead of
appearing as absent implementation details.

Node readiness is a separate host-owned snapshot. It carries Core readiness,
artifact graph, CAS, executor ABI, and node/policy revisions. The node must
observe the base policy revision before the final policy revision is materialized;
stale observations fail closed rather than becoming a cache or runtime hint.

Revision-aware outbox consumers must use `ModulePolicyRevisionGate`: policy
revisions are opaque identities, so a transition is applied only when its
predecessor matches the durable cursor. Exact replays are acknowledged as
duplicates and divergent or out-of-order transitions are rejected as stale.
`SeaOrmModulePolicyRevisionConsumer` persists the cursor under tenant RLS and
advances it atomically with the gate result; it is consumer state, not another
event journal. Hosts obtain this durable consumer through
`ModuleControlPlane::policy_revision_consumer`, so commit-time lifecycle
serialization cannot bypass the owner facade.

The caller-supplied SeaORM connection and owner-opened transaction form the
transactional storage boundary. `ControlPlaneInfrastructure` carries the
object-safe `rustok-outbox::TransactionalEventWriter`; owner commands append
their envelope through that port in the same transaction as state and audit
facts. Redacted runtime audit remains behind `ExecutionObserver`, and domain
audit facts remain owner rows/outbox events rather than a second audit journal.
Platform-scope artifact and static-distribution events use the root event
contract's nil-tenant sentinel only through its explicit event allow-list;
tenant-scoped events fail closed if that sentinel is supplied.

Secret values never cross the artifact capability response. The sandbox-visible
`platform.secrets.acquire_handle` operation returns only logical reference and
revision. A host adapter that needs the value must use
`ModuleControlPlane::artifact_secret_use`: the owner reauthorizes the immutable
execution, reloads the exact binding revision under tenant RLS, closes the
transaction, and lends the resolved `SecretString` only to a fixed-purpose host
consumer whose result type is `()`. The resulting receipt contains only logical
reference, revision, and purpose; resolver keys, values, and consumer output are
not serializable through this boundary.
The production server registers this capability through
`ModuleControlPlane::artifact_secret_handle_policy`. The dynamic resolver first
checks the exact active installation and durable grant, and the policy repeats
that check immediately before the binding read so a lifecycle or
capability-revision change cannot leave a stale broker authorized. The repeated
check derives the tenant/module/data-contract/policy scope from owner
installation state; neither the guest nor a secret resolver supplies it.

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
The component/payload digest and the registry-returned OCI manifest digest are
separate immutable identities and are never compared for equality. Platform
build staging rehashes and matches the submitted payload against the completed
build component while preserving the receipt manifest digest for signature,
admission, and final-publication joins.
That receipt records build-service signature evidence only; author signatures
and marketplace approvals remain separate owner-governance facts.

Global artifact security is a separate owner aggregate. Registry yanking is an
ordinary discovery/install state and does not silently disable an already
admitted tenant intent. `ModuleControlPlane::artifact_security` owns explicit
quarantine, quarantine-clear, and terminal emergency-revoke commands with
revision CAS, platform authorization, exact idempotency replay, and outbox
events. The read-only security resolver feeds effective policy with redacted
status/revision/release facts; quarantine and revoke block new execution, while
a revoked state cannot be cleared.
Successful build results must carry the complete component, SBOM, provenance,
interface, and validation evidence. Failed and cancelled results reject those
success artifacts and require a structured diagnostic matching the terminal
failure, so a stale successful payload cannot be admitted through a failed result.
Before that publication window the worker obtains a repository-scoped,
short-lived lease through its deployment-owned credential broker. Credentials
never enter module contracts, descriptors, build requests, runner output, or
artifact persistence.

The internal OCI registry transport applies the complete
`OciRegistryTransportPolicy` itself: HTTPS only, verified TLS, no redirects,
no process or system proxy, bounded connection/request deadlines, bounded
retries, transfer/decompression ceilings, and one concurrent request. It
rejects non-identity content encoding, holds its concurrency permit for the
complete stream, accepts upload locations only on the original HTTPS origin,
and never forwards Basic credentials to a different host. Bearer token service
requests may be cross-host only without those credentials. `oci-distribution`
supplies only the OCI data-model and registry-auth DTOs; its HTTP client is not
used by repository-owned production code.

During admission, `ModuleInstaller` verifies the OCI package and places its
payload in an `ArtifactBlobStore` under the descriptor payload digest.
The verifier decision and durable admission evidence expose signature,
provenance, SBOM, license-policy, and vulnerability-policy results separately;
all five must be true and all five participate in governance evidence identity.
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
schema. The same bounded compiled-validator implementation is shared by the
artifact settings and installation-scoped structured-data owner paths.
`ModuleLifecycleDbWriter` keeps static
host-manifest normalization and artifact settings writes as separate
entrypoints: the artifact path resolves only the immutable settings-schema
digest retained by the active definition and rejects a caller-supplied schema
or pre-normalized bypass. `ModuleControlPlane::artifact_lifecycle` is the facade
composition entrypoint; the lower-level tenant settings store is crate-private.
Artifact persistence is limited to a revision plus a bundled schema digest for
brokered namespaced values; descriptor decoding rejects unknown fields, so an
artifact cannot attach SQL, DDL, native migrations, a bucket path, or a host
storage handle.
Dynamic artifact UI is similarly declarative-only: the current contract accepts only
`admin_settings` and `admin_actions` contribution surfaces with immutable
localization and declared permissions. Descriptor fields cannot carry a
component, URL, iframe, or native frontend package; native UI remains a static
promotion concern.
Schemas are keyed by their canonical digest, compile into a bounded node-local
LRU cache with linear-time regex limits, and use a `jsonschema` build without
filesystem or HTTP resolver features. Non-local `$ref`, `$dynamicRef`, and
`$recursiveRef` values are rejected during descriptor admission.

`ModuleControlPlane::promotion` owns the only static-promotion request and
approval path. A request is eligible only for an active platform-built release;
the owner reloads its platform build staging row and completed tenant-scoped
build request/result, then binds the exact source reference, source digest,
dependency-lock digest, Cargo package, normalized crate-local native entry type,
component digest, and OCI publication receipt. Source identity must be the exact
`cas://sha256:<hex>` reference. Approval
requires immutable ownership, dependency-audit, test, and static-review evidence
plus revision CAS, exact-replay idempotency, and separate fail-closed host
authorization for request and approval. The persisted requester cannot approve
the same promotion. `ModuleControlPlane::static_distribution` is the only owner
that can consume approved records. It replaces the complete selection under a
separate CAS head, revalidates every release/build pair, pins platform source,
toolchain and target identities, carries the Cargo package and native entry type
into every distribution item, persists the explicit `static_native` executor
mode, binds all of those facts into its composition digest, and records an
immutable predecessor-linked
build intent plus outbox evidence. Selection also requires its own fail-closed
host authorization decision. These services have no compiler,
active-composition mutation, native loader, compatibility alias, or alternate
versioned path. `ModuleControlPlane::static_distribution_worker` separately
owns bounded claim leases, heartbeats, expired-lease attempt closure, and exact
terminal replay. Successful completion requires artifact, SBOM, provenance,
signature-manifest, and test evidence, but still cannot activate a release.
`ModuleControlPlane::static_distribution_release` is the only release-activation
owner. It accepts only the current successfully completed build, calls a
mandatory external verifier before opening the mutation transaction, then
relocks and revalidates the exact build, every selected promotion, and its
published build evidence. Signature, provenance, SBOM, test, and dependency
policy decisions must all pass under the exact requested policy revision. The
owner atomically supersedes the prior release, advances a dedicated release CAS
head, stores immutable admission evidence and exact-replay idempotency, and
publishes `module.static_distribution.release_activated`. This release ledger
reloads the exact immutable build items before returning a release. A runtime
host builds its definition catalog through
`ModuleDefinitionCatalog::from_static_distribution`: compiled platform modules
remain `platform_native`, while every promoted module becomes
`promoted_native` with exact promotion, registry release, distribution release,
release revision, native artifact digest, and `static_native` executor facts.
The same catalog can be supplied to the owner lifecycle and effective-policy
services; lifecycle dispatch still resolves the implementation only from the
compiled registry. This release ledger does not deploy code or mutate the
running composition; deployment, explicit rollback, and revocation remain
separate owner operations.
`ModuleControlPlane::static_distribution_release` also owns those lifecycle
commands. Rollback is limited to the active release's direct predecessor and is
accepted only when the distribution head still matches the active release. It
revalidates the target admission, terminal build digest, complete composition,
promotion review, and publication evidence before queuing a new immutable build
and publishing both build and rollback outbox facts. The old artifact is never
reactivated, and the rebuilt artifact digest must reproduce the target before
activation. A newer selection or failed/cancelled rollback build closes the
pending request. Revocation uses release-head revision CAS and exact replay,
records actor/reason/policy, cancels pending rollback requests involving that
release, and clears the head when the revoked release was active. Neither
operation mutates a running process; deployment consumes the resulting owner
events.

`ModuleControlPlane::static_distribution_rollout` owns the next deployment
boundary. A topology resolver returns one sorted, bounded node set with a
canonical digest; the request owner pins that snapshot, the active verified
release, policy revision, artifact digest, and `static_native` executor mode in
a durable desired rollout. Node agents report only exact identity matches
through the owner port. Each node uses an observation revision and transitions
through `prepared`, `healthy`, and `active`; a healthy fleet moves the rollout
to `activating`, and only an active fleet becomes `converged`. A failed node
before convergence makes the rollout terminal; drift after convergence makes
it `degraded`, clears observed readiness, and permits an explicit
prepare/health recovery before activation again. Duplicate reports replay
their immutable receipt, while stale or out-of-order revisions fail closed.
Desired and observed rollout pointers, node observations, and operation
journals are transactional and emit outbox events. The deployment process,
node transport, and binary replacement remain outside the control-plane crate.
Activation, rollback, and revocation reserve one shared lifecycle idempotency
key namespace, so a UUID cannot be reused across command kinds.

Artifact permission descriptors carry immutable localized labels and
descriptions. Admission sends them through the shared
`ArtifactPermissionRegistrationPort` only after the installation is committed;
the installation ID makes the owner operation idempotent and a retry repeats
registration for an already admitted release. The port adds RBAC vocabulary
only and cannot assign a permission to a role or actor.

Durable artifact binding idempotency is tenant-scoped at both query and database
policy layers. `module_artifact_binding_operations` uses PostgreSQL RLS, and
claim, completion, abandonment, replay, and lease recovery set the transaction's
`rustok.tenant_id` before touching request identity or stored responses. The
tenant remains part of every unique key and mutation predicate; RLS is the
independent fail-closed boundary rather than a substitute for those predicates.

Structured-data and object-data list calls validate bounded keyset continuation
inside the requested logical prefix before invoking any capability broker. A
custom broker therefore cannot receive a continuation that escapes the admitted
namespace even if it does not repeat owner validation internally.
Structured-data deletion is a distinct granted and host-authorized operation.
It requires an exact positive record revision plus UUID idempotency key, removes
the logical record and all of its materialized indexes atomically, and persists
a policy-revision-scoped tenant-RLS replay receipt.
Structured/object put receipts use the same policy revision in their durable
idempotency identity. Export evidence and destructive namespace-purge receipts
also record that revision, preventing a result authorized under one capability
policy from being reused as evidence for another.
`ArtifactDataQuota` is selected by the host for the exact broker policy; it is
not descriptor or capability input. Structured record/byte, live object/byte,
active upload-session, and staged-chunk limits are evaluated as projected
namespace-wide usage under the owner namespace lock. Replacement subtracts the
old byte contribution, atomic batches cannot partially consume capacity, and
logical deletion releases live capacity without bypassing retention-aware
physical GC. A restore authorizer returns the exact target quota and the owner
checks the canonical snapshot manifest again inside the restore transaction.
Deployments inject stricter exact-scope limits through
`ArtifactDataQuotaPolicy`; `ModuleControlPlane` exposes matching composition
entrypoints for both structured and object capability resolvers.
Object-data deletion is a distinct granted operation. It requires the exact
positive logical-object revision and a UUID idempotency key, persists a
tenant-RLS replay receipt, and queues the now-unreachable private key for the
same retention-aware GC used by replacement and namespace purge. It never
returns or deletes the physical storage key inline.

Durable artifact-data backup is owner-only and separate from bounded export
pages. `ModuleControlPlane::artifact_data_snapshot` creates a resumable,
idempotent namespace snapshot under tenant RLS and an exact namespace revision
lock. Structured values, logical object metadata, materialized indexes, and the
index contract form the canonical manifest; object bytes are copied to private
snapshot-owned keys and re-hashed before the snapshot becomes `ready`. Restore
re-verifies that manifest and every object, then uses one transaction for the
empty-target guard, namespace CAS, restored rows, audit operation, and outbox
event. It cannot clear a purge tombstone, overwrite live data, or expose a
physical storage key to an artifact. Snapshot retention is revision-CAS state:
authorized commands can extend its deadline and apply or release legal hold,
but cannot shorten retention. The bounded collector requires separate host
authorization plus an explicit policy-snapshot rule with no audit or rollback hold before persisting a
resumable `collecting` decision. Missing policy retains data; final collection
preserves independent audit facts and emits an outbox event rather than using
implicit age-only GC.

Final registry publication revalidates localized rows loaded from the database.
Every locale must already be canonical, names and descriptions must satisfy the
bounded publication contract, and the release default locale must have an exact
translation row. Invalid database state fails closed before release or marketplace
approval facts are written.

Publisher marketplace text is always `plain_text` with
`untrusted_publisher_content` trust. The owner projection bounds names and
descriptions and rejects control, invisible, and bidirectional override
characters. Category and tags are bounded canonical identifiers with a
duplicate-free tag set. UI adapters must use framework text nodes. AI adapters
may use only
the projection's tagged structured data as non-system data and must never turn
README, metadata, source comments, test output, or artifact text into
instructions. Validation-stage and delivery-retry audit records use stable
owner-generated diagnostics rather than caller or runner output.

`ModuleMarketplaceCatalog` is the framework-neutral read port for the current
catalog. The host composes local and configured remote providers behind
`SharedModuleMarketplaceCatalog`; native and GraphQL adapters consume that same
handle and may not scan the workspace or synthesize catalog state. Registry
release projection also belongs to `SeaOrmModuleGovernanceService`: it enriches
host-supplied static facts only with durable localized active metadata, canonical
artifact references, yanked versions, and publisher identity. GraphQL and the
public registry adapter map its owner DTO without reading registry tables.
The same owner exposes one request-scoped publish-status snapshot for public
status and approval-preview paths. The status projection loads only the
addressed immutable request, includes its identity,
warnings, errors, acceptance fact, override guidance, and semantic next action,
and derives validation stages, gates, override requirements, and actor-visible
actions from durable facts. It never substitutes a newer request for the same
slug. The server supplies only authenticated principal/permission facts and
maps a semantic next action to its own route and response text; it does not
read a publish-request persistence model or recreate lifecycle policy.
The external-prebuilt and platform-build staging responses reuse that same
snapshot for request identity and status in both dry-run and committed paths;
they do not query a server SeaORM publish-request model before or after the
owner staging command.
Creation and artifact-upload responses use the same exact owner status
projection after their mutation. Artifact upload first asks the owner to
authorize and issue a digest-derived immutable slot; the host conditionally
creates or rehashes the object at that slot and then asks the owner to attach
the same metadata. The host cannot choose a storage key or delete a prior
artifact inline. Exact retries reuse the attached content-addressed object;
retention-aware owner policy, not an upload adapter, governs historical-object
cleanup. The platform-authoring producer follows this identical slot contract.
Validation enqueue, manual validation-stage reporting, and every live decision
(approve, reject, request changes, hold, resume) likewise return that exact
owner status projection after their command. HTTP no longer reconstructs
acceptance, errors, or the next action from an updated SeaORM request model.
Remote-runner heartbeat and terminal-completion adapters likewise return the
owner-issued `ModuleRemoteValidationStageTransition`; they do not read a
server validation-stage model after the lease transition. The former duplicate
registry-governance remote-transition adapter was removed, leaving one
owner-routed runner mutation path.
The same owner status projection carries authenticated `can_manage` and
`can_review` facts derived from the durable request and owner binding. Live
validate, validation-stage report, and moderation mutations authorize through
those facts rather than server-local publish-request or owner-binding reads;
an unauthenticated status projection exposes no governance actions. It also
supplies owner-derived rejected-request retry eligibility, effective publisher
identity, and the latest stage facts required for approval-override evidence,
so live adapters no longer read a publish-request persistence model after the
owner status lookup.
Artifact download uses a separate host-only owner snapshot containing only the
attached storage key and content type. It intentionally treats an absent or
unattached request as unavailable and never adds storage topology to public
publish-status DTOs.
Validation-queue and validation-stage dry-run previews likewise use the exact
owner status snapshot after authentication instead of preflighting a server
request persistence model.
Approve, reject, request-changes, hold, and resume previews follow the same
rule; approval override text and pending-stage facts remain owner-derived.
Owner-transfer adapters obtain the exact durable binding through the same owner
service before and after a transfer. They no longer read the owner-binding
table or return its persistence model from the server boundary.
Detail reads attach `ModuleGovernanceLifecycleSnapshot`, whose owner service
derives moderation policy, validation gates, events, and available governance
actions from durable registry state. A missing catalog handle or unsupported
durable artifact origin fails closed.

`module_artifact_installations` is the host-managed persistence boundary. Its
PostgreSQL migration enables RLS; tenant-scoped connections must set
`rustok.tenant_id` before querying or mutating tenant installation rows.
`SeaOrmArtifactInstallationStore` performs that setup while atomically writing
the installation, admission metadata, and `module.artifact.admitted` outbox
envelope. It stores the reference and canonical descriptor, never artifact
bytes. `StorageArtifactBlobStore` supplies the production CAS adapter over the
platform `ObjectStore` runtime: it uses private staging keys, conditional creation
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

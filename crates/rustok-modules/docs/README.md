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

For an Alloy marketplace fork, the owner can materialize one exact active Rhai
workspace only after validating its publication projection, admitted workspace
media type, source digest, and canonical verified CAS bytes. This neutral
owner contract does not depend on Alloy or expose catalog DTOs or mutable OCI
references as executable source.

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

The release-safety target gives prepared-cache entries a stable fingerprint of
the exact executor binary, WASM/Rhai engine build, engine configuration
revision, isolated-worker image/target where applicable, and runtime ABI. A
node/pool generation is a separate mandatory readiness identity: a compatible
prepared entry may be rehashed and reused, but smoke evidence must be repeated
for the new generation. Automatic transition requires both candidate and
predecessor to pass on every fingerprint/generation that may serve or recover
the operation.

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
entrypoints. The artifact path acquires the active platform/tenant activation
fences, resolves one exact admitted installation, and validates only against
that installation's immutable descriptor-bundled schema. It persists the value
under the tenant, stable data owner, and exact settings instance rather than a
slug row; caller-supplied schemas and pre-normalized bypasses are rejected.
Admission creates opaque owner/instance identities. A compatible activation
inherits them only from its direct predecessor with the same admitted registry
repository and settings-schema digest; a schema-changing activation fails
closed until the separate guarded settings-migration operation exists.
`ModuleControlPlane::artifact_lifecycle` is the facade composition entrypoint;
the lower-level settings store is crate-private. Native/static manifest settings
remain the explicitly separate `tenant_modules` contract.
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
immutable build-lineage-linked build intent plus outbox evidence. That lineage
is reproducibility history, not the production direct predecessor. Selection
also requires its own fail-closed
host authorization decision. These services have no compiler,
active-composition mutation, native loader, compatibility alias, or alternate
versioned path. The
`ModuleControlPlane::static_distribution_worker` implementation separately
owns bounded claim leases, heartbeats, expired-lease attempt closure, and exact
terminal replay. Successful completion requires artifact, SBOM, provenance,
signature-manifest, and test evidence, but still cannot activate a release.
`ModuleControlPlane::static_distribution_release` admission ledger
accepts only the current successfully completed build, calls a
mandatory external verifier before opening the mutation transaction, then
relocks and revalidates the exact build, every selected promotion, and its
published build evidence. Signature, provenance, SBOM, test, and dependency
policy decisions must all pass under the exact requested policy revision. The
owner stores the candidate as admitted without changing the serving head,
stores immutable admission evidence and exact-replay idempotency, and publishes
`module.static_distribution.release_admitted`. This release ledger
reloads the exact immutable build items before returning a release. A runtime
host builds its definition catalog through
`ModuleDefinitionCatalog::from_static_distribution`: compiled platform modules
remain `platform_native`, while every promoted module becomes
`promoted_native` with exact promotion, registry release, distribution release,
release revision, native artifact digest, and `static_native` executor facts.
The same catalog can be supplied to the owner lifecycle and effective-policy
services; lifecycle dispatch still resolves the implementation only from the
compiled registry. Admission does not deploy code or mutate the running
composition. `ModuleControlPlane::static_distribution_rollout` freezes the
then-serving direct predecessor and exact `(node, role)` assignments, persists
desired/observed rollout, and advances the serving head only after every
assignment converges healthy. Each agent claims one exact assignment under a
short owner lease; the same agent can replay an unexpired claim after losing a
response, while a different agent must wait for expiry. The lease work item is
limited to the assigned node/role identity and never includes other-node
observations. Recovery revalidates
and redeploys retained predecessor bytes; it never queues a build. Revocation
remains a separate revision-CAS security operation. Deployment agents report
evidence and never own release selection.

The accepted
[module release rollback safety decision](../../../DECISIONS/2026-08-06-module-release-rollback-safety.md)
defines the production incident-recovery boundary. The worker publishes one
complete role bundle and one receipt containing
its per-role artifact and evidence identities. Release admission stores that
immutable bundle and build lineage but does not supersede the serving release
or advance desired/observed rollout state. The production operation freezes
the direct predecessor from then-observed serving state; later unused
admissions cannot alter it. The owner retains, pre-stages, and
revalidates that complete direct-predecessor bundle before rollout, then
redeploys those exact immutable server, worker, selected Leptos,
generated-registry, and browser-asset bytes through the normal desired/observed
reconciler. Rebuild remains reproducibility evidence or a separately admitted
maintenance update through the same owner lifecycle; it is never a rollback
fallback. The duplicate `rustok-build` release head/rollback surface and the
rebuild-on-rollback request have been removed atomically.

For dynamic target admission, one governed publisher/module semantic version
binds one immutable artifact release ID and digest set. Equal replay is
idempotent; the same version with different bytes is rejected. Static identity
instead belongs to the complete distribution: one distribution lineage/version
binds one `distribution_release_id` and bundle-root digest. The same unchanged
native module artifact/version may appear in multiple later co-release bundles.
Operator projections show dynamic semver/release ID/digests or static
distribution version/release ID/root plus the complete per-module
version/digest diff for current, candidate, and direct predecessor.

The current `ModuleControlPlane::static_distribution_rollout` slice persists a
singular artifact against a sorted node set. The target replaces that shape
atomically with one role-bundle rollout whose canonical assignments bind
`node_id`, failure domain, role, candidate role digest, and predecessor role
digest. Required role or production-surface changes create a new bundle;
placement, node-count, and wave-weight changes affect only rollout state. Node
agents report exact per-role identity matches through the owner port. Each
observation is revisioned and transitions through `prepared`, `healthy`, and
`active`; a healthy wave permits activation, and only an active complete fleet
becomes `converged`. A failed candidate starts the one permitted recovery or
ends in `recovery_required`; post-convergence drift becomes `degraded` and
fails readiness. Duplicate reports replay their immutable receipt, while stale
or out-of-order revisions fail closed. Desired and observed rollout state,
node-role observations, and operation journals are transactional and emit
outbox events. The outside-candidate deployment controller, node transport,
slot replacement, and worker-generation fencing remain outside the
control-plane crate and available when candidate processes fail. Update,
rollback, disable, revocation, finalization, and collection share one
conflict-fenced operation namespace.

`degraded` is a later health/incident projection on a terminal rollout result,
not another lifecycle-operation state. It can trigger only a newly authorized
containment or direct-predecessor operation. `quarantined` and `revoked` are
separate global security projections for the same reason.

The controller and node agent come from one separately signed, digest-pinned
operations-tool release installed by the host provisioner/service supervisor,
not from the application role bundle. Owner preflight binds their exact
package/component/target digests and external protocol revision. Tool upgrades
use the `operations_tool_maintenance` class in this same canonical operation
ledger and fleet conflict fence, retain their exact predecessor, and prove
owner/controller/agent protocol compatibility. The host supervisor applies
only exact desired assignments as a narrow executor; this crate records and
revalidates lifecycle facts but does not install or self-update the tools.

Artifact permission descriptors carry immutable localized labels and
descriptions. The current installation command sends them through the shared
`ArtifactPermissionRegistrationPort` after its installation commits; the
installation ID is its idempotency identity. That shape is an atomic-cutover
gap after release admission is separated from scoped installation. In
the accepted target, admission commits only inert definitions keyed by exact
release/module/definition digest and never invents a scope or grant. Scoped
install projects those definitions idempotently under
`(scope, installation, release)`, while enablement resolves separately owned
role/actor grants against the active serving generation. Rollback reselects the
predecessor definitions; disable/remove/uninstall and retention preserve
referenced grant/audit history. The port adds RBAC vocabulary only and cannot
assign a permission to a role or actor.

The target permission preview also compares predecessor/candidate stable keys,
exact canonical authorization fingerprints, and affected roles. A grant may
carry only when stable identity and every authorization-relevant
scope/key/resource/action/binding constraint produces the same fingerprint and
an RBAC-owner continuity receipt authorizes it. Localized display text is
excluded or governed separately. The receipt and every carry/rollback commit
bind the current monotonic scope grant/role-membership epoch under the RBAC
owner fence. Any fingerprint change requires explicit approval; removed grants
become dormant. Rollback selects predecessor definitions and evaluates current
grants; it never restores a revoked grant or membership. Admission and install
never grant access implicitly.

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

The namespace purge covers structured records and private-object metadata/keys
only; it is not artifact-settings-purge evidence. Dynamic artifact settings
have a separate owner service for recovery-point creation, purge, and restore.
It requires an inactive uninstalled source installation, exact
scope/data-owner/settings-instance/revision/schema/descriptor/value identity,
an unresolved-secret-handle digest, a host-authorized retention snapshot, and
KMS-backed authenticated ciphertext before purge. Purge revalidates that
identity and ciphertext, requires its own authorization context (including
policy, retention revision, holds, and KMS key version), deletes no
structured data, and commits a monotonic settings tombstone plus outbox fact.
Restore requires that exact tombstone, creates a fresh non-serving settings
instance, and may bind it only to a compatible inactive installation under the
same owner; after uninstall/retirement it remains unbound and never resurrects
the old installation. It never snapshots or deletes role/actor grants or
external secret bytes, and cannot borrow an artifact-data snapshot as
authorization.

The target also installs an owner compatibility guard before a dynamic or
native/static settings-bearing rollout. It binds both N/N+1 schema digests and
the rollback window; every concurrent settings write CAS-revalidates the
intersection until rollback closes. Accepting a one-sided value requires a
separate confirmed maintenance command that fences writers and atomically
closes rollback eligibility. The settings recovery point persists independent
encrypted retention/hold/collection state and roots its exact KMS key version,
schema/descriptor, and lineage; purge/restore revalidate decryptability,
  target-schema compatibility, target admission revision, and secret handles. Retention mutation, KMS
rewrap, and crash-resumable terminal collection are owner-service operations:
retention uses a revision guard and can only extend expiry or add holds,
ciphertext rewrap uses the host KMS port, and collection records a durable `collecting` intent
before terminally nulling ciphertext while retaining the recovery fact.
Collection authorization is host-owned and expiry alone is never sufficient.
An unbound restore can be attached once only after host continuity
authorization and exact data-owner, registry/repository, slug, schema, and
inactive-installation checks; it cannot clear the source tombstone.

The current artifact-data scope is derived from tenant, module slug, contract
revision, and policy revision. That is an explicit cutover gap for retained
data after uninstall. The accepted target introduces a stable opaque data-owner
identity bound to scope and verified publisher/module lineage; reinstall may
attach only with its exact continuity receipt. A different publisher reusing
the slug/revision is denied. Legitimate ownership change is a separate
privileged, conflict-fenced governance transfer with old/new evidence and audit,
not implicit namespace reuse.

The target mutable-state key is
`(scope_id, data_owner_id, namespace_or_settings_instance_id, revision)` for
both platform- and tenant-scoped dynamic installations. First install creates
only mutable boundaries declared by the release; stateless or no-settings
modules persist `not_applicable`, not empty synthetic owners. Active update
inherits the exact owner/instances. `start_empty` is limited to first install or
reinstall; changing the owner/instance of an active installation is a separate
fenced maintenance migration and cutover.

Durable artifact-data backup is owner-only and separate from bounded export
pages. The target snapshot identity is exact `scope_id`, stable
`data_owner_id`, namespace-instance identity/revision, and data-contract
digest; module slug is metadata and never attach/restore authority.
`ModuleControlPlane::artifact_data_snapshot` creates a resumable, idempotent
namespace snapshot under the owner authorization/RLS domain and an exact
namespace revision lock. Structured values, logical object metadata,
materialized indexes, and the
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

Because the current empty-target restore rejects a purged namespace, pre-purge
snapshot evidence is not yet a usable post-purge recovery path. The accepted
target restores into a new isolated empty namespace instance under the same
stable data-owner identity, verifies it fully, and performs a separately
authorized active-reference CAS cutover. The old namespace remains tombstoned;
crash replay cannot clear it, attach by slug, or expose two active instances.

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
owner staging command. External-prebuilt staging carries an authenticated
`modules.manage` fact and requires the staged actor to be the recorded
quarantine approver. Platform-build staging carries the same host fact but the
owner derives whether it is allowed from that privilege, the current durable
owner binding, or the original requester before a binding exists.
Creation and artifact-upload responses use the same exact owner status
projection after their mutation. Creation carries only authenticated
principal/privilege facts; the owner checks its current binding before writing
or replaying the request. Artifact upload first asks the owner to
authorize and issue a digest-derived immutable slot; the host conditionally
creates or rehashes the object at that slot and then asks the owner to attach
the same metadata. The host cannot choose a storage key or delete a prior
artifact inline. Exact retries reuse the attached content-addressed object;
retention-aware owner policy, not an upload adapter, governs historical-object
cleanup. The platform-authoring producer follows this identical slot contract.
Release yanking is also an owner command: the owner locks the addressed
release, derives permission from the authenticated `modules.manage` fact,
durable owner binding, or release publisher, and returns only the request ID
and resulting status needed by the HTTP response.
Owner transfer is likewise owner-authorized: the host passes only the
authenticated principal and privilege fact, while the owner locks the current
binding and permits either `modules.manage` or that bound owner before writing
the immutable audit fact. No server owner/release SeaORM model remains in the
production path.
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
Owner-transfer adapters supply authenticated actor and privilege facts only.
The same owner service locks the current binding, derives authorization, and
returns the transition result; the server never reads the owner-binding table
or returns its persistence model from that boundary.
Lifecycle toggle, retry, compensation, and settings responses follow the same
rule: the modules owner returns the exact module identity plus current operation
or state facts, and the server maps them without a recovery-plan preflight or a
post-command `tenant_modules` or `module_operations` model read. Inherited
compensation availability is the owner's effective-policy decision rather than
an adapter reconstruction.
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

Admission remains inert: it does not select a serving installation or invent a
rollback predecessor. `activate_artifact` is the scoped owner transition. It
serializes one `(scope, slug)`, records only the active non-uninstalled
predecessor, makes that predecessor inactive, writes the candidate's durable
predecessor pointer and replayable operation receipt, then makes the candidate
active and emits `module.artifact.activated` in the same transaction.

File-backed admission uses `ArtifactPayloadSource::TemporaryFile` and
`DurableArtifactBlobStore::stage_file`; the storage adapter hashes the staging
file while it publishes to the durable CAS. Local storage uses an atomic
temporary copy/rename and S3 opens the file as `ByteStream`, so the owner never
materializes a downloaded OCI payload as an unbounded in-memory buffer.

## Verification

- `cargo xtask module validate modules`
- `cargo test -p rustok-modules`
- `cargo check -p rustok-server --lib`

## Related Documents

- [Implementation plan](./implementation-plan.md)
- [Neutral sandbox ADR](../../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md)
- [Module control-plane plan](../../../docs/modules/module-control-plane-consolidation-plan.md)

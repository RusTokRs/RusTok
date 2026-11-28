# rustok-modules documentation

New to the crate? Start with the [architecture guide](./architecture.md) for
the subsystem map, the main command flows, and the invariants to respect
before changing durable behavior.

## Purpose

This Core module owns the module platform control plane and artifact lifecycle.

## Responsibility Zone

Maintenance object migration requires host-supplied storage and an explicit
transaction-backed authorizer via `ModuleControlPlane::artifact_data_object_migration`.
The owner freezes requests/inventory and reserves every opaque copy key durably
before create-only byte publication, verifies SHA-256 and size, and atomically
commits target references and terminal checkpoints. Pending namespace holds block
purge, writes, and serving-reference changes; exact reconciliation retains keys and
never age-deletes uncertain objects. Targets remain non-serving. Production fences,
PostgreSQL/kill-matrix evidence, separate full-manifest cutover, and safe orphan
collection remain open in the implementation and central rollback plans.

It owns marketplace release identity, digest-pinned package admission,
tenant-policy rules and the contracts for installation, activation, rollback,
capability grants and static-promotion admission. Persistence adapters and
owner transports are still being moved from the server. It does not own sandbox
implementation or Alloy source authoring.

OCI admissions and external ingress evidence use the installation scope and
release digest as their durable identity. Tenant reads and writes set the
owner transaction's tenant scope; executor-readiness lookups also require the
exact operation and installation. The scoped migration preserves existing
release and ingress evidence in a SQLite owner migration test. A PostgreSQL 16
transactional schema check also exercised the migration SQL, duplicate digest
across two tenant scopes, and tenant reads and foreign-tenant write rejection
with a non-bypass RLS role; the
transaction was rolled back. PostgreSQL owner-service execution and concurrent
admission and crash behavior still require verification.
The ignored owner-library test
`postgres_scoped_admission_migration_enforces_tenant_rls` can be run explicitly
with `RUSTOK_MODULES_TEST_DATABASE_URL` and a PostgreSQL role allowed to create
a schema and assume `pg_read_all_data`. It creates its schema within a rolled-back
transaction and checks the non-bypass role's tenant isolation.

### Durable replay identity

An idempotent retry is identified by its explicit durable scope and
idempotency key plus the command's semantic fields. Trace, correlation, and
free-text reason values are first-attempt audit and observability evidence;
they are stored where an owner receipt requires them, but changing them does
not make the same semantic command conflict. Actor is included only where the
owner explicitly defines it as a replay-principal or authorization boundary,
either in the lookup partition or in the semantic fingerprint. Structured
facts such as `reason_code`, policy revisions, expected revisions, evidence,
and target identities remain semantic. `src/idempotency.rs` provides the
shared digest for commands whose top-level context/reason fields follow this
contract; owners with a narrower identity define an explicit fingerprint
instead of hashing arbitrary nested payload conventions.

### Static settings boundary

`rustok-modules` owns static tenant settings persistence, revision/CAS,
idempotency, localization rows, and lifecycle serialization. The declaring
module owns field semantics and runtime consumption. `tenant_modules` and
`module_static_tenant_lifecycle` are private owner persistence; consumers must
use owner read/policy ports rather than query those rows directly. A successful
static settings mutation also appends `TenantUpdated` to the canonical outbox
inside the same owner transaction as the settings, revision, and replay receipt;
outbox failure aborts the mutation.

The lifecycle writer returns one browser-safe settings snapshot for a
specific static module, as well as the bounded collection of explicit
overrides. Both read the settings and revision in one database snapshot.
The exact read exposes inherited state at revision zero when there is no
override or lifecycle aggregate. Reading it does not materialize settings.

For static normalized, exact-locale and source-locale settings writes, a database
error while claiming the aggregate leaves the admitted receipt processing rather
than recording a terminal failure. A failed rollback does not trigger claim
release or receipt finalization; failed claim cleanup also retains the receipt.
Retryable static normalized settings errors and database errors in locale writes
keep the request fingerprint in processing state even after confirmed cleanup;
the same request must wait for receipt lease expiry before retrying. The shared
receipt implementation can reclaim processing after five minutes, including
uncertain cases; retaining the receipt is not a durable reconciliation fence,
and same-key overlap after expiry still needs owner-specific recovery. A receipt
token alone cannot safely replace the aggregate key without atomic admission,
claim/reclaim and handling of existing claims. The new failure-path tests remain
unrun.

SQLite reads use the tenant owner's `tenant_identity_keys` projection in the
same statement as the module rows. The tenant migration must be applied before
these reads; an unavailable projection fails the read rather than falling back
to an unverified tenant identity. Static lifecycle and override mutations
resolve the parent's stored key in the mutation statement and reject conflicting
or orphaned identities. The watchdog resolves either valid representation inside
its claim transaction. Operation lookup by ID and the remaining cutover and
host-level verification are documented in the implementation plan.

The SQLite journal migration adds normalized uniqueness for tenant and
idempotency UUIDs across BLOB and canonical text representations. Existing collisions
prevent migration and require operator reconciliation; neither receipt is
silently selected or discarded.
Journal admission, idempotency lookup, and the tenant-scoped failure list
validate the owner identity projection within each statement. Admission stores
the parent's exact SQLite key, so a valid text parent remains compatible with
the foreign key. The reaper revalidates each operation ID and its tenant inside
its transaction before updating a journal or releasing a claim. Direct journal
status, discard, recovery and result-snapshot statements now check operation-ID
uniqueness, supported operation/key encodings and tenant owner identity in the
same SQLite statement. This is an identity-consistency fence, not caller
access authorization. Unsupported operation-ID and idempotency-key legacy
encodings require explicit reconciliation. SQLite SQL models, including a
two-connection stale-read scenario, passed; the new Rust regressions have not
been run.

Current writes normalize against the active host-resolved manifest schema, and
disable retains settings. The server supplies the active schema to the owner
for ordinary enables and enable-direction compensation; the owner validates
retained settings under the lifecycle claim and commits normalized settings
with the intent and revision. Dormant settings can be repaired while disabled
via `update_static_normalized_settings`. The standalone installer does not yet
compose the static schema for seed toggles. Static rows do not yet
persist the exact schema digest plus `not_applicable`/`ready`/`migration_required`
columns. The server manifest adapter rejects an unknown module or missing package
manifest rather than treating it as an empty schema. The package schema still
needs exact active-release evidence through the owner schema-evidence cutover.
Both static settings write paths now reject a detected unresolved
tenant or platform transition with a predecessor, rather than substituting the
candidate schema for missing predecessor evidence. This conservative read is
not atomic with transition creation and does not cover static distribution
rollout state, so N/N+1 write safety is not yet established. Lifecycle toggles,
compensation, and post-hook retry also reject detected unresolved tenant or platform transitions
after replay and claim; the read is not an atomic transition fence.
The reviewed settings CAS currently compares recorded intent and platform
default, not the complete effective policy with dependencies, quarantine,
maintenance, and runtime availability. It must not be used as evidence that
the reviewed effective-policy snapshot is unchanged.
New post-hook retry and compensation are disabled: the settings digest and
selected-intent evidence do not establish the original executable/installation
identity or prove external work stopped. Plans report `retryable = false` and
recommend no automated action; existing committed retry receipts can be replayed
without dispatch. The public executor also rejects new source-bound compensation
commands. This includes legacy operations whose aggregate claim is already
absent. No proof-based reconciliation procedure is implemented. Ordinary new
toggles and old writers are not yet comprehensively fenced from every legacy
uncertain operation. See the server lifecycle incident runbook. The new Rust
regressions, including the ignored PostgreSQL test, have not been executed.

The canonical current/target contract and gap table are in
[Settings and Configuration Architecture](../../../../docs/architecture/settings.md),
governed by the
[platform and module settings ADR](../../../../DECISIONS/2026-09-22-platform-module-settings-architecture.md).

## Migration assessment boundary

`MigrationPreflightInput` and `MigrationPreflightReceipt` carry the canonical
`rustok_core::MigrationSafetyAssessment`, not an independent safety boolean and
optional reasons. Maintenance/unknown facts deny automatic mode even without
reason text; missing settings protection, irreversible effects and
cross-revision copies also deny it. The receipt's `is_automatic_eligible()` is
the shared consumer predicate used by governed transitions, the coordinator and
data-upgrade evaluation. The coordinator rejects a foreign operation/module
receipt before creating a checkpoint. Receipt bodies are not persisted in those
checkpoints; incomplete wire bodies fail decoding rather than reconstructing
safe facts from old flags.

The [assessment ADR](../../../../DECISIONS/2026-11-05-migration-safety-assessment.md)
does not complete native execution or consolidation admission. Dynamic
descriptor facts and SQL-operation summaries retain their different evidence
scope. Production authorization remains owner-derived.

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
capability-policy revision, an affirmative executor-availability input, and an
enabled dependency closure. This input is not necessarily a health probe. The
policy retains only redacted installation/revision facts,
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
Deserializing a full `ModuleEffectivePolicy` revalidates its digest revision,
decision keys and revision bindings, enabled/denied consistency, denial
explanations, and exact agreement between enabled modules and enabled
decisions; catalog membership and revision provenance still require the policy
owner or an exact current cache identity.

The policy revision identifies durable intent and durable runtime evidence.
It deliberately excludes the volatile executor-liveness flag: decisions still
factor executor availability in, so two policies sharing one revision can
differ in the enabled set while workers flap. That is an accepted trade, not
an oversight — churning the durable revision on every worker flap would
invalidate every revision-keyed cache and transition consumer cluster-wide.
A cached decision can lag both recovery and loss; a revision match does not
prove current executor availability. Dispatch checks do not protect earlier
authorization consumers such as server guards and RBAC. The static server
policy path deliberately remains independent of sandbox availability. Artifact
HTTP/command/UI/audit admission and non-lifecycle execution now share the
uncached mixed-catalog owner policy. It combines compiled definitions and the
bounded RLS-scoped active artifact selection, never overriding a native slug,
and treats active tenant-enabled installations as dynamic activation intent
while honoring explicit tenant override denials. The actual composed executor's
payload-kind support supplies the availability input; it is not a worker
health probe. Exact durable node readiness is still required for the resulting
deployment readiness revision, independently derived from the deployment intent,
topology digest and frozen owner assignment identities. Tenant effective-policy
revision remains independently bound to execution admission; see the
[deployment readiness authority ADR](../../../../DECISIONS/2026-11-27-artifact-deployment-readiness-authority.md). Readiness now covers the root and every selected artifact in its dependency
and co-requisite closure. Each dependency lock node must match the selected
artifact version, payload and manifest digest, and dependency edges; a native
definition without immutable artifact evidence cannot satisfy a lock node.
Every required artifact must match the policy's installation/graph/capability
facts and active observed assignment on the same node and reconciliation
revision. The owner checks the observed-head revision before and after the
closure read, not an atomic execution lease. Storage failures remain
infrastructure errors; unavailable or stale readiness is a policy denial.
The singleton ledger now carries a deployment-scoped revision rather than a
tenant policy digest. Its owner reconstructs the digest from stored intent,
topology and exact assignments, requires desired/observed equality, and rejects
legacy rows whose deployment intent is NULL. The tenant-specific policy is still
required and the sandbox compares its admission revision separately. Fresh
reconciliation is mandatory at cutover; historical observations are not promoted.
Contextual channel/maintenance owner inputs,
the reconciliation producer's full revision parity and atomic runtime fences
remain outstanding.
The active installation resolver chooses the effective tenant/platform scope by
slug before checking release identity, including when the two scopes have
selected different versions. Disabled and uninstalled installations do not
participate; an ambiguous winning scope fails closed. The single and batched
resolvers use the same selection. The multi-chunk read is not a repeatable-read
snapshot and needs further transactional fencing. Lifecycle-hook dispatch also
uses this resolver, so a hook targeting a shadowed release fails closed; its
rollout/transition behavior needs end-to-end verification. Lifecycle hook
dispatch under the static tenant aggregate renews the exact live claim
immediately before starting and again when the hook succeeds; a heartbeat
cannot revive an expired or missing lease. Revision advance checks the live
claim in the state transaction; successful lifecycle and post-hook-retry
completion recheck ownership and release the claim in the same transaction as
their journal changes. An expired or replaced claim rolls those changes back.
The lease check is at statement time, not commit time. PostgreSQL heartbeat and
advance now materialize a row lock before evaluating the lease deadline. The
equivalent SQL was exercised with separate sessions against a disposable
PostgreSQL 18.6 server, including lock waits past expiry and a rolled-back
settings write. That SQL experiment does not execute Rust or SeaORM; the Rust
lock-wait regressions and application integration remain unrun. These checks
only reject an observed stale
holder: a lease may expire while external work still runs,
and dropping a hook future does not prove its side effects stopped. Expired
journal claims are no longer reclaimed or released by the watchdog; hook and
uncertain commit failures retain the fence. This deliberately blocks recovery
until external quiescence is established and the operation is reconciled. No
safe automated reconciliation procedure exists yet; an operator flag or lease
expiry is not proof of quiescence. Other node actions, existing deployments
running older writers, and external effects remain outside this guarantee.
PostgreSQL concurrency tests are still required. The server lifecycle reaper now retains an ID-ordered page
cursor across polls, including no-op pages, and wraps at the end. This avoids
repeatedly scanning the same full batch of live or non-releasable rows; the
one-shot `reap_stale` API does not retain a cursor. Each page still locks journal
rows before claims and performs SQLite identity checks. The cursor is process-local
and advances only after commit; restarts begin at the first page, errors retry the
same page, and persistently locked or corrupt rows can still impede progress.
The page limit bounds returned rows, not database scan/sort cost; SQLite also
performs per-candidate identity validation. ID ordering no longer prioritizes
the oldest or nonterminal journal entries. Eventual coverage
assumes successful scans, unlocked rows, a running worker and a finite traversal
(not an indefinitely growing tail). Rust and real PostgreSQL concurrency tests
remain unrun. Event and schedule
projections now share the order-independent scope and ambiguity selector with
runtime and select before descriptor parsing: an invalid winning descriptor
cannot restore the platform fallback. Both projections validate the winning
descriptor's slug, version and digest against the installation and its payload
digest against admission, even if no binding matches the event. The schedule materializer warns and omits
an ambiguous or invalid slug while continuing other bindings. For events, an
incomplete subscription projection is an error before any enqueue or downstream
publish, rather than silently acknowledging potential lost deliveries. This
can block unrelated events for the tenant (and an invalid platform winner can
affect multiple tenants); relay attempts are finite and failed outbox rows need
operator recovery after repair. A delivery insert conflict is replayable only
when the existing row belongs to the requesting tenant and has the same source
digest; a global source-ID collision in another tenant fails closed even without
RLS. The unique identity remains global, so such a collision requires repair
rather than creating a second delivery. Replay resolves the then-current subscriptions,
not a snapshot of those at the original event time. Schedule omissions do not
promise replay of missed slots: an unseen binding starts its cursor when first
materialized. Both projection queries return at most 10,001 eligible rows to
Rust: 10,000 is the supported limit and the extra row detects overflow. Event
overflow fails the relay projection before publication; schedule overflow fails
the poll before cursor or slot writes. This is a row-count bound, not a bound on
descriptor bytes or database scan work. Event and schedule enqueue and schedule
claim also compare descriptor identity with installation and admission. Event
claim repeats the comparison in its work-item read. A corrupt descriptor or
identity mismatch returns a storage error instead of cancelling/dead-lettering
the item; the claim transaction rolls back and pending work requires repair.
This can stall a tenant queue. This is not full parity: runtime validates the
complete installed artifact and exact release, whereas these checks cover the
descriptor and selected identity columns only. Projection, claim and dispatch
are not transactionally fenced against later changes. Rust queue/projection
tests and PostgreSQL/RLS and concurrency verification remain outstanding.
Artifact binding admission requires an enabled artifact definition for the exact
release and a matching installation fact; the non-lifecycle executor rechecks
both against its resolved installation. A static definition with the same slug
cannot grant artifact execution. Host and sandbox now use the same
artifact-aware serving resolver. A prior effective-policy admission is rejected
if the freshly resolved policy revision differs, even when the target remains
enabled. Owner-revalidation work has no prior revision and must independently
pass the current policy. Lifecycle hooks have a separate admission path.
These checks still depend on owner provenance, and are not a transactional
lock over subsequent execution.
The server's `get_latest` cache hit does not check an independently obtained
current owner revision. It exposes only policy; consumers requiring both policy
and defaults use an uncached resolution that supplies the same manifest defaults
to policy construction. Owner reads are not a single database transaction.
Observed invalidations fence local in-flight cache fills, but delayed or missed
events, concurrent database changes, and post-check changes remain freshness
gaps; TTL is not a transactional authorization guarantee. Browser transports receive only
`rustok_api::ModuleEffectivePolicyView`: its
policy revision, per-module effective result, and stable redacted denial
taxonomy. The host composes one `SharedModuleEffectivePolicyReader` with the
active manifest's normalized co-requisites, and native Admin and GraphQL
transports consume that exact projection rather than recomputing availability.
Core identity is part of the definition catalog, not a transport-side bypass of
an unavailable owner decision.

Installed static composition reaches browser transports only through
`rustok_api::StaticInstalledModuleView`. The server adapts its private active
manifest once, supplies `SharedStaticInstalledModuleReader` to native Admin,
and GraphQL maps the same view. Source-control and filesystem locators remain
host-private manifest facts.

Static tenant-lifecycle browser reads similarly pass through the
host-composed `SharedStaticModuleLifecycleReader`. Its server adapter uses the
same active-manifest normalization and co-requisites as GraphQL, projects
browser-safe lifecycle revisions, and fails closed rather than allowing native
Admin to deserialize defaults or invent a missing revision.

The full browser-safe static module registry is a separate
`rustok_api::StaticModuleRegistryView` read contract. The host composes one
`SharedStaticModuleRegistryReader`, resolves one active manifest, and applies
catalog metadata, effective policy, lifecycle revisions, and locale projection
before GraphQL or native Admin receives it. Transports cannot rebuild the view
from build-time manifest metadata, direct lifecycle rows, or UI defaults.

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

`SeaOrmModuleTransitionService` is the sole host-facing transition query and
convergence boundary. Its reads enforce exact tenant scope, including the
platform `None` scope, and host lifecycle/settings guards propagate owner read
failures instead of treating missing evidence as permission to mutate. SQLite
stores the service's UUID identities as BLOB values consistently across parent
and foreign-key columns; revisions, security epochs, and retry counters use
checked conversions at both persistence edges.

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
checks the exact active installation and durable grant; the handle read then
locks the binding row and the same execution-authority fence in one short
transaction. Secret value use reloads the exact revision, closes the transaction
before calling the external resolver, then re-locks and compares the binding
facts and reauthorizes immediately before the fixed-purpose consumer. No SQL
row lock spans the resolver or consumer. The repeated checks derive
installation-backed capability authority and the independently persisted
opaque secret instance from owner installation state. Neither the guest nor a
secret resolver supplies this identity; no data contract is required.

`OciDistributionArtifactRegistry` resolves only digest-pinned references. It
requires the returned manifest digest to match the requested reference, reads
the descriptor from the manifest config, and downloads exactly one payload
layer whose digest and media type match that descriptor. Admission limits reject
an oversized descriptor and the OCI-declared layer size before `pull_blob`, then
stream the received bytes into a private temporary file while enforcing the
same size limit and SHA-256 digest.

`OciDistributionArtifactPublisher` publishes the descriptor-selected payload
and hands its verified source SBOM/provenance evidence to the shared publication
boundary. The source-side SLSA v1 statement remains payload-bound while the
boundary validates it, then supplies only its predicate to Cosign. Cosign owns
the final in-toto envelope and binds that signed attestation to the returned
digest-pinned OCI manifest; package admission separately re-fetches and
verifies the descriptor-selected payload. The isolated build worker uses a
deployment-owned KMS provider reference to sign the manifest and emit current
SLSA v1 and CycloneDX signed attestations, then resolves Cosign's compatible
OCI signature manifest to a digest-pinned publication receipt. The standard
Cosign tag is used only while resolving the signature manifest and never
becomes installation identity.
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
events. The owner calls the host authorizer inside the transition transaction,
before replay and before taking the platform slug activation lock; the host
must retain direct credential and current RBAC row fences through commit.
The owner then calls the host's final authority check immediately before both
new-state and exact-replay commits, after lock waits and all writes, so a
time-bounded direct credential cannot expire while the transaction is pending.
Therefore an exact idempotency replay still requires current authorization
and a still-live credential at its commit boundary.
The read-only security resolver feeds effective policy with redacted
status/revision/release facts; quarantine and revoke block new execution, while
a revoked state cannot be cleared.
Successful build results must carry the complete component, SBOM, provenance,
interface, and validation evidence. Failed and cancelled results reject those
success artifacts and require a structured diagnostic matching the terminal
failure, so a stale successful payload cannot be admitted through a failed result.
`ModuleBuildResultReader` is the sole cross-owner read port for a completed
immutable request/result pair. Alloy uses it to bind a reviewed Component
candidate to its matching source-CAS digest, scenario comparison, publication
receipt, and build-result revision; it never receives a caller-provided worker
result or queries `module_build_requests` directly. The read port establishes
owner completion facts only. Deployment proof for the hardened OCI job remains
an operational evidence requirement and is not inferred from this API.
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
Dynamic artifact UI is similarly declarative-only: the current contract accepts
`admin_settings`, `admin_actions`, `admin_status`, `admin_help`,
`admin_navigation`, `admin_table`, `admin_form`, and `storefront_slot` surfaces
with immutable localization and declared permissions. Descriptor fields cannot
carry a component, URL, iframe, or native frontend package. The owner projects
an admitted contribution only into `rustok_api::ArtifactUiContributionView`.
Its redacted execution evidence uses the same framework-neutral
`rustok_api::ArtifactBindingExecutionAuditEntry` contract. Those DTOs exclude
catalogs, localization keys, permissions, binding IDs, executable UI material,
inputs, outputs, actors, traces, credentials, and grants. Native UI remains a
static promotion concern.
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
[module release rollback safety decision](../../../../DECISIONS/2026-08-06-module-release-rollback-safety.md)
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
fails readiness. A degraded rollout stays claimable: its failed assignments
may be re-claimed under fresh leases and re-prepared until the fleet is
healthy and active again, reconverging on the release already committed as
the active head without a second release activation; an operator recovery
to the direct predecessor remains available throughout. Duplicate reports
replay their immutable receipt, while stale
or out-of-order revisions fail closed. Desired and observed rollout state,
node-role observations, and operation journals are transactional and emit
outbox events. The outside-candidate deployment controller, node transport,
slot replacement, and worker-generation fencing remain outside the
control-plane crate and available when candidate processes fail. Update,
rollback, disable, revocation, finalization, and collection share one
conflict-fenced operation namespace.

`ModuleDesiredObservedState`, `ModuleReconciliationPhase`,
`ModuleReconciliationEvidence`, and `ModuleReconciliationFailure` are the
single reusable desired/observed vocabulary. The static rollout retains its
native release, topology, role, and activation policy while using that shared
contract. `ModuleControlPlane::artifact_node_reconciliation` is the dynamic
artifact/sandbox consumer: it persists a complete owner-selected node and
admitted-installation set, including release/payload digests, payload kind,
admitted payload media type, admission, dependency-graph and capability
revisions, executor ABI, policy revision,
fenced claims, observations, and outbox events. The owner alone turns a fully
healthy set active and advances the observed head. A changed installation
lifecycle identity makes an old desired set stale rather than allowing a node
cache or sandbox worker to continue it. `SeaOrmArtifactNodeReadiness` is the
read-only runtime gate for a configured server node: it permits only an active
assignment from the converged observed head whose desired, observed, and live
installation/admission identities (including payload kind and media type) plus
the current canonical policy revision still match exactly. It does not consult
a node cache or add database, policy, or
product/AI ownership to the sandbox.
The server composes that gate before every non-lifecycle artifact dispatch.
It also uses a bounded digest-keyed `VerifiedArtifactNodeCache` before durable
CAS and rehashes every hit, so neither a corrupt cache entry nor a cache miss
can supply readiness. `rustok-artifact-node-transport` consumes the narrow
`ModuleControlPlane::artifact_node_agent()` port through certificate-bound
claim/heartbeat/report framing. The independent
`rustok-artifact-node-controller` owns deployment composition of the immutable
certificate-to-agent/node map without receiving topology, reconciliation
authoring, CAS, sandbox, or policy capabilities. The separate
`rustok-artifact-node-reconciler` composes the other mTLS service: a verified
operator certificate maps to one audit actor and an explicit allowed-node
scope, while its bounded topology request has no caller-selected actor or
artifact identity. The owner validates topology and reloads every selected
admitted installation under its own transaction. The canonical topology digest
is bound into the owner request idempotency identity and must match resolver
output. Neither process is a second source of desired state. Node-local materialization is performed only by
`rustok-artifact-node-agent` through the narrow agent port and durable CAS.

`degraded` is a later health/incident projection on a terminal rollout result,
not another lifecycle-operation state. It can trigger only a newly authorized
containment or direct-predecessor operation. `quarantined` and `revoked` are
separate global security projections for the same reason.

The controller, reconciler, and node agent come from one separately signed,
digest-pinned operations-tool release installed by the host provisioner/service
supervisor, not from the application role bundle. Owner preflight binds their
exact package/component/target digests and external protocol revision. Tool
upgrades use the `operations_tool_maintenance` class in this same canonical
operation ledger and fleet conflict fence, retain their exact predecessor, and
prove owner/controller/reconciler/agent protocol compatibility. The host
supervisor applies only exact desired assignments as a narrow executor; this
crate records and revalidates lifecycle facts but does not install or
self-update the tools.

Artifact permission descriptors carry immutable localized labels and
descriptions. The current installation command sends them through the shared
`ArtifactPermissionRegistrationPort` after its installation commits; the
installation ID is its idempotency identity. The admission row and result
record that projection separately as `not_required`, `pending`, or
`registered`: a failed projection leaves the durable admission acknowledged
as `pending`, and an idempotent retry loads the stored descriptor and
completes the projection without consulting the registry again. That durable
state does not by itself complete the accepted separation of release
admission from scoped installation. In
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
claim, completion, reconciliation marking, and replay set the transaction's
`rustok.tenant_id` before touching request identity or stored responses. The
tenant remains part of every unique key and mutation predicate; RLS is the
independent fail-closed boundary rather than a substitute for those predicates.
Each binding receipt is partitioned by tenant and actor and carries a canonical
semantic request digest. Trace and correlation values are retained only as
first-attempt audit evidence; changing them on retry does not turn replay of
the same semantic binding into a conflict.

**Recovery boundary:** the 60-second lease is advisory metadata and its expiry
never authorizes a new execution. A live pending receipt returns `InProgress`;
expired pending receipts transition to `reconciliation_required`, and execution
errors do the same rather than deleting them. This prevents automatic
same-key re-execution. The exact original executor can still commit its proven
response after advisory expiry; its operation ID and full request identity must
match. Ambiguous operations with no proven response remain blocked until an
owner-specific reconciliation workflow is implemented. That workflow must fence
late completion before it authorizes any competing action. The stable
host idempotency key is propagated through `ArtifactBindingExecutionContext`,
`SandboxContext`, and Rhai's read-only `IDEMPOTENCY_KEY` constant. That identity
is useful to effect owners but does not itself make guest effects idempotent or
provide exactly-once semantics. Installer recovery and capability-owner
idempotency/reconciliation remain incomplete; do not clear or take over these
receipts based only on elapsed time.

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
only; it is not artifact-settings-purge evidence. Its destructive request names
only an exact installation; the owner derives the current namespace scope from
that installation's admitted descriptor and capability grant, binds the
durable receipt to `(tenant_id, installation_id, idempotency_key)`, and repeats
the inactive-plus-uninstalled lifecycle fence inside its write transaction.
The server authorizer binds tenant and installation to owner-derived facts.
Before receipt lookup or installation/namespace locks, the host's RBAC mutation
authority acquires the exact direct-session credential, module policy and role
fences on the supplied owner write transaction and requires admitted and current
`modules:manage`. Every commit, including either receipt replay path, checks the
credential deadline and request scope again. Exact terminal replay remains
independent of mutable target eligibility. The owner retains authority through
commit and never opens an independent authorization transaction. This is not a
proof of global lock ordering, operational traffic/job/write drain or retention
and legal-hold admission; current source regressions remain unrun.
Transport errors hide storage details and receipt integers are checked.
The matching preview is an owner read projection, not a GraphQL-side database
join. The in-progress physical cutover binds serving collisions to the exact
tenant/owner/instance; an active different owner with the same slug has separate
bytes. The updated purge integration verifies this storage boundary, terminal
replay, and permanent root tombstones. Remaining cutover and recovery gates
stay open. Dynamic artifact settings
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

The plaintext-tag cipher and permissive settings authorizer were deleted.
Settings mutations now consume a host-composed owner service through
`GraphqlRuntimeInputs`; absent policy/encryption ports return unavailable, and
the matching preview cannot indicate apply readiness. The owner accepts shared
ports without default implementations. Real encryption/KMS and owner-backed
retention/secret-handle/hold/fence composition remain incomplete. Owner-port
tests do not prove encrypted recovery, KMS rotation, or production hold safety.

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

Post-purge recovery derives the exact retired installation, stable owner,
contract, and current tombstoned namespace. Prepare requires the exact purge
receipt and reference/tombstone revisions, reserves a fresh empty non-serving
instance and recovery hold, then invokes actual snapshot restore and verification.
A separate cutover command authorizes on the transaction, re-verifies target
rows and bytes, and CASes the owner reference. It keeps the source permanently
tombstoned, releases only its own recovery hold, and never activates code.
Complete prepare/cutover digests govern terminal replay before mutable lifecycle
checks. Schema guards preserve command identity, verification facts, and terminal
receipts. No permissive host authorizer is supplied.

The purge/recovery integration passes 1/1 with actual record/object restore,
denied actors/revisions, corrupt target bytes rejecting CAS, permanent source
tombstones, and terminal replay after later lifecycle changes. This is bounded
SQLite/local-storage evidence with explicit fixture policies. Host runtime/
transport composition, recovery lifecycle outbox facts, remaining callers/
fixtures, and production policy/traffic/job/write/recovery/retention fences
remain open. No FFA/FBA status is promoted.

The physical cutover is in progress. Pending schemas and broker SQL now use
tenant/owner/opaque instance identities, with immutable namespace metadata and
a revisioned tenant/owner serving reference. Data broker resolution initializes
only the exact installation's first empty instance and reads this reference;
ordinary writes never initialize namespaces. Private object/upload/snapshot/
restore keys include the same owner/instance identities. Restore selects a
distinct staging instance without a serving reference. Snapshot/restore copies
reserve one durable key before publication and re-read actual target bytes.
Restore holds its source snapshot, verifies all target rows and object bytes,
then seals the target as verified with a source/target fingerprint. Verified
metadata cannot be changed or deleted; verification evidence cannot be removed,
and the instance cannot return to staging. Collection checks active holds at
admission and resume. Missing copy parents remain unresolved, with no age-based
orphan deletion. The focused real-storage test passes 1/1. Remaining callers,
fixtures, secret runtime/fence evidence, actual migration-object copy, and
production recovery composition and lifecycle outbox facts are not yet closed. This worktree is
not a production-ready post-purge recovery path.

Secrets use independent installation-backed capability authority and the
persisted opaque `secret_instance_id`, including stateless installations.
Catalogs and immutable full-request-digest receipts are keyed by tenant, stable
owner, and secret instance. Handle use pins both instance identity and revision.
No data contract, artifact-data namespace, or slug/revision storage lookup is
used. Secrets runtime fixtures pass 9/9, including the stateless same-slug
owner-isolation route; the full owner library passes 308/308. Host management/revocation fences
and production value-use authorization remain required.

MCP now consumes the shared installation-backed `ArtifactCapabilityScope`,
binding tenant, stable owner, exact release, installation, and grant revision.
It does not require a data contract or namespace. Its broker and server invoker
match the complete admitted subject instead of only the slug. The artifact
server exposes a separate explicit read-only registry tool allowlist, rather
than inheriting every tool later added to the general MCP registry. Calls use
a short execution-authority admission transaction and the result is revalidated
before release. Stateless MCP runtime verification remains pending. Secrets
add only their independently persisted opaque secret instance to this
authority; they do not require a data persistence scope.

Owner maintenance commands share `ArtifactDataNamespace` facts and the same
serialized root reader. Checked tenant/owner/instance/contract/revision facts
are policy input, not a grant. The object-specific duplicate DTO/parser is
removed; structured-copy authorization uses this owner-backed boundary.

`ArtifactDataCopier` requires current transaction-backed authorization and exact
source/target namespace revisions. It reads a sealed non-serving source under
sorted root locks and copies records into a staging non-serving target. Target
schema, create-only value/revision/size equality, declared index projections,
quota, revision CAS, and immutable exact-response receipts commit in one database
transaction after independently durable admission of the full request and frozen
page. Preparing and latest non-terminal pages hold both namespaces through shared
database guards; authorized continuation hands off that page hold atomically.
`reserve_page` exposes admission without target writes. `reconcile_pending_pages`
loads the original request and frozen evidence and rechecks current authorization,
descriptor, schema, and quota. Failed receipt commit preserves preparing evidence
and rolls back all target effects. Receipts record the actual commit policy revision.
Replay preserves the original continuation before mutable checks; skipped
continuations and terminal restarts are rejected. Revised file-backed SQLite
runtime tests pass 11/11 with persisted fixture policy. Whole-operation source retention, production maintenance
and fleet fences, PostgreSQL crash/race evidence, outbox composition, full
snapshot verification, and separate serving CAS remain required.

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
catalog. Its public list and detail operations return
`rustok_api::MarketplaceModule`; `ModuleMarketplaceEntry` remains an
owner-private intermediate projection. The host composes local and configured
remote providers behind `SharedModuleMarketplaceCatalog`. Its server adapter
maps each owner entry once, reduces registry-principal JSON to browser-safe
display labels, and gives GraphQL and native Admin the same canonical view.
Neither transport may scan the workspace, synthesize catalog state, or rebuild
the projection. Registry-release projection also belongs to
`SeaOrmModuleGovernanceService`: it enriches host-supplied static facts only with
durable localized active metadata, canonical artifact references, yanked
versions, and publisher identity. GraphQL and the public registry adapter consume
those owner projections without reading registry tables.
The same owner exposes one request-scoped publish-status snapshot for public
status and approval-preview paths. The status projection loads only the
addressed immutable request, includes its identity,
warnings, errors, acceptance fact, override guidance, and semantic next action,
and derives validation stages, gates, override requirements, and actor-visible
actions from durable facts. It never substitutes a newer request for the same
slug. The server supplies only authenticated principal/permission facts and
maps a semantic next action to its own route and response text; it does not
read a publish-request persistence model or recreate lifecycle policy.
The browser-visible status and generic governance mutation envelopes are the
strict shared `rustok_api::RegistryPublishStatus` and `RegistryMutationResult`
contracts. The server carries the owner-issued gate, action, and complete
validation-stage snapshots through to that boundary, including execution mode,
runnable/manual-confirmation policy, terminal reason codes, and suggested
outcomes; no server or Admin-local snapshot may drop those fields or supply
response-version defaults.
Automated worker evidence uses the owner-owned
`ModuleGovernanceAutomatedCheck` contract. The owner rejects blank or duplicate
check identities, normalizes the accepted evidence before persistence, and
projects it from the newest lifecycle event as browser-safe
`RegistryAutomatedCheckLifecycle` values. Check detail is intentionally
optional; no browser consumer reads raw governance-event JSON.
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
or state facts, and the server maps them without a post-command
`tenant_modules` or `module_operations` model read. New retry and compensation
are currently denied by the owner due to missing execution and quiescence evidence.
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
predecessor pointer and replayable operation receipt, makes the candidate
active, and emits `module.artifact.activated` in the same transaction. A plain
activation deliberately does not synthesize a governed-transition checkpoint,
preflight result, conflict-fence decision, observation window, or rollout
hold: those records exist only when a governed transition is created
separately for the exact activation operation through the transition owner.
Plain activation, deactivation, uninstall, tenant lifecycle and admission
reverification reject unresolved governed checkpoints in overlapping scopes:
platform changes check every tenant; tenant changes check their own tenant and
platform. `InterventionRequired` remains unresolved even though automatic
progression is terminal. Rollback may resolve its own checkpoint, but rejects
other overlapping checkpoints. Governed staging and these serving mutations
acquire the same platform slug lock, then the tenant lock when required.
Staging rechecks conflicting checkpoints and the exact predecessor installation
under that lock; normalized, localized and source-locale settings commits also
reject unresolved checkpoints under the platform lock, including first-install
transitions without a predecessor. Settings transactions lock their receipt
before the platform lock and hold both through commit; on a domain conflict
they roll back, release the aggregate claim, and record a replayable failure.
PostgreSQL transactions use READ COMMITTED so post-lock checks see commits made
while waiting. The platform lock serializes all tenants of a slug, a deliberate
concurrency cost. On PostgreSQL, migration 000098 requires the transaction
advisory gate before the permanent platform activation row. The registry
BEFORE ROW guard tries the same gate and uses NOWAIT on an existing row;
a competing late writer fails with SQLSTATE 55P03 and must roll back.
The AFTER trigger advances only the exact registry witness. Migration 000101
also makes witness identities permanent and their integer revisions strictly
successive; deletion, truncation, regression, and SQLite REPLACE of existing
witnesses fail closed. SQLite release INSERT must not conflict with an existing
primary key, slug/version or rowid; releases and witnesses cannot change their
rowid. This prevents REPLACE from silently deleting another release or witness
and cascading receipt deletion without a continuity update, even with recursive
triggers disabled. Canonical publication uses plain INSERT and later changes use
UPDATE. Direct monotonic witness advancement can only invalidate
old evidence; it is not a supported release writer. PostgreSQL raw witness
writes share the nonblocking slug advisory gate. The release guards also
acquire their required table locks and existing witness row without waiting.
Privileged DDL, disabled triggers and restoration are outside this DML guard.
Multi-slug SQL
maintenance must pre-acquire gates and rows in sorted slug order, and TRUNCATE
of releases is rejected. No witness changes on metadata-only updates,
failed writes or rollback. Drain old writers/leases before the coordinated
migration and application rollout. PostgreSQL regressions are unrun.

Governed checkpoint staging, activation and finalization retain the durable
security epoch through commit: platform/tenant slug fences precede the epoch
(FOR SHARE on PostgreSQL, SQLite writer reservation), followed by domain
writes. Security operations take the platform slug before inspecting a receipt;
a completed exact replay does not reserve the epoch. New quarantine/revoke
operations acquire the exclusive epoch before reserving their receipt or
writing security state. Clear-quarantine does not advance the epoch. A
conflicting receipt key without a visible exact receipt fails closed. All
replays still require the transaction-bound host authority at commit.
Finalization rechecks exact receipt replay after waiting on the platform
fence, then acquires the epoch before the FK-bearing receipt. Epoch advancement
returns the exact assigned successor and fails closed on exhaustion.
Migration 000102 preserves the singleton and accepts only an exact successor
or an unchanged row (including SQLite writer reservation). Direct insertion,
deletion, PostgreSQL truncation, epoch reset, skipped values and metadata-only
changes are rejected. Existing invalid or absent baselines block migration;
no automatic reseed. Ordinary DML integrity does not cover privileged DDL or
restoration of an older database. Drain writers before migration.
This is not a guarantee of global deadlock freedom or of immediate interruption
of an admitted transaction. PostgreSQL and SQLite regressions are authored,
not executed.

Lifecycle toggle and post-hook recovery admission now take that same platform
lock while checking checkpoints and acquiring the durable journal aggregate
claim. For an artifact target they also resolve the tenant/platform winning
installation under that lock and compare its admitted descriptor to the target
catalog definition before committing the claim. Staging and artifact
serving-lock users check overlapping journal claims while holding the lock; even
an expired claim blocks them until it is cleared. PostgreSQL fails closed when
row security is active for the lifecycle aggregate or, for artifact claim
admission, any of the installation, admission, tenant-lifecycle or uninstall
tables under the current role. Checkpoint fence lookups also fail closed under
active checkpoint-table RLS, since a policy might hide a platform or overlapping
tenant checkpoint. The visibility checks acquire transaction-held table locks
before inspecting RLS so policy DDL cannot race those reads. Equivalent SQL
experiments on disposable PostgreSQL 18.6 confirmed the checkpoint visibility
check detects an RLS-hidden platform row and holds its table lock against DDL;
they also checked the four artifact table locks. They do not run Rust or prove
application role policies. Filtered rows could hide claims or a higher
priority selection; an auditable cross-tenant policy has not yet been
established. These checks may make lifecycle admission, settings, serving or
staging unavailable for RLS-restricted roles. SQLite normalizes tenant UUID storage forms for the claim
check.

This is not yet a complete lifecycle fence: the new journal path retains
uncertain claims, but older writers or other execution paths can still overlap
external effects. The target descriptor check does not pin the precomputed dependency or
policy snapshot, nor distinguish two installations with identical descriptors;
post-hook recovery lacks durable evidence of the original artifact release and
is now disabled for new attempts.
Native catalog/distribution identity is not revalidated by this check. Static
distribution rollouts do not participate, and staging does not validate
existing static settings against the candidate schema. Rollout items alone are
not a complete set of affected native modules: the platform binary and removed
predecessor selections also matter. Converged or superseded status and assignment
lease expiry do not prove an already issued node action has stopped. A complete
fence needs a fleet execution boundary and reconciliation of uncertain node
actions, not just a per-slug status check. Static rollout and artifact node
reconciliation assignment claim, heartbeat, and report use the database wall
clock and check leases after locking. Rollout locks the release head before the
rollout head; artifact reconciliation locks its state and validates installation
identities before heartbeat. These changes have not been compiled or tested
against real PostgreSQL. SQLite only compares canonical millisecond UTC lease
timestamps; active claims in older timestamp formats fail closed instead of being
replayed, renewed, or reclaimed. Before deployment, stop old agents and writers,
reconcile any uncertain node actions, and repair or clear legacy claims only
after confirming their work has stopped. The lease changes do not fence already
issued artifact node actions: on heartbeat failure the agent waits for an
in-flight materializer and does not start a report, but that materializer can
continue work and cancellation alone would not prove external or filesystem
effects stopped. Shutdown can interrupt the wait. Its processor handles pending
and prepared work only, although the owner also offers active assignments for
drift observation; that mismatch remains unresolved. The low-level checkpoint, static lifecycle
claim, tenant module state writers, lifecycle journal mutations, and raw toggle executor
are crate-private. The public lifecycle writer constructs the execution request and
performs the owner validations. Raw journal, checkpoint and aggregate methods exposed
by `test-support` are only for fixtures; do not enable this Cargo feature in any
production build (including through feature unification). Journal maintenance and
other low-level APIs still require a separate boundary review. This visibility change
is an API break for callers of the raw executor and journal mutations; migrate to
owner commands, not test fixtures. Old binaries and direct SQL also bypass the
protocol; quiesce old writers before relying on it. It does not establish hook
identity or quiescence, or block legacy uncertain operations without a claim. SQLite
tenant-scoped checkpoint queries normalize native BLOB and legacy TEXT UUID
representations; an undecodable selected row fails the read. Normalization can
scan checkpoint rows. New Rust regressions and real PostgreSQL race tests have
not been run.

`SeaOrmModuleTransitionService::start_governed_automatic_artifact_transition`
is the governed start saga. It loads the exact admitted candidate and the
serving predecessor, derives migration/data preflight from their admitted
descriptors (an unchanged persistence contract and data schema, an unchanged
settings schema, and idempotency-`Required` lifecycle bindings are the
automatic-mode conditions — anything else is maintenance-classified and
refused before any checkpoint exists), records the checkpoint in
`PreStaging`, executes candidate and predecessor smoke readiness through the
host-composed `ExecutorReadinessService` pools outside any database
transaction, requires automatic-mode eligibility on every required pool, and
only then calls the governed activation. The governed activation shares the
checkpoint operation identity and, under the activation lock, revalidates
the checkpoint's candidate/predecessor pair and the current security epoch,
records predecessor payload/manifest rollout-window retention holds, advances
the checkpoint to `Observing`, and changes serving selection in the same
transaction. A retry discovers and resumes the staged checkpoint; a staged
pair or epoch that has gone stale is moved to `InterventionRequired` instead
of activating.

Executor-pool generations are durable deployment facts owned by
`SeaOrmExecutorPoolRegistry` (`module_executor_pools`): a host registers
each pool it actually composed, re-registering identical fingerprint,
placement, and attestation resumes the current generation, and any change
advances the generation by one so earlier smoke evidence goes stale by
construction. The server composes one readiness service per pool it runs:
the in-process WASM pool fingerprint is derived from the running server
binary, and the isolated Rhai pool fingerprint is built only from the
worker's own identity evidence (its binary digest, admitted image digest,
and isolation-envelope digest, reported over the authenticated readiness
RPC) plus the mTLS trust anchor the host pinned for it. The operator
placement policy (`RUSTOK_GOVERNED_PLACEMENT_POLICY_REVISION`,
`RUSTOK_GOVERNED_REQUIRED_PLACEMENT`,
`RUSTOK_GOVERNED_ALLOW_IN_PROCESS_FALLBACK`) must be configured before the
tenant `startGovernedTenantArtifactTransition` transport can start a
governed transition; without it, or when a configured worker cannot prove
its identity, the composition fails closed and no governed transition can
start. Required pools are the pools of the executing node.

`rollback_artifact` is the only dynamic serving recovery command. It verifies
that the retained target is the direct predecessor recorded by the activation
operation and atomically changes both admission selections, records its
idempotency receipt, releases any retention holds for that activation
operation, and emits `module.artifact.rolled_back`. When a governed checkpoint
exists for the exact activation operation, the same transaction also verifies
the checkpoint slug and tenant, records its single predecessor recovery, and
CAS-saves the advanced checkpoint; a converged, already-recovered,
point-of-no-return, or recovery-exhausted checkpoint aborts the rollback. No
transport may mark a checkpoint recovered without completing this serving
transition.

`SeaOrmModuleTransitionService::finalize` owns explicit convergence. The
authenticated command carries tenant scope, `modules:manage` evidence,
`expected_revision`, and an idempotency key. Its checkpoint CAS, retention-hold
release, operation receipt, and `module.transition.finalized` event commit in
one transaction. The watchdog never treats an elapsed observation window as
convergence evidence: stale-epoch and expired-window checkpoints move to
`InterventionRequired` without changing serving selection, retaining recovery
evidence for an explicit operator rollback or deactivation instead of reusing
an obsolete capability grant automatically.

File-backed admission uses `ArtifactPayloadSource::TemporaryFile` and
`DurableArtifactBlobStore::stage_file`; the storage adapter hashes the staging
file while it publishes to the durable CAS. Local storage uses an atomic
temporary copy/rename and S3 opens the file as `ByteStream`, so the owner never
materializes a downloaded OCI payload as an unbounded in-memory buffer.

## Operations

### Background maintenance

The server runs a `module_artifact_maintenance` background worker (60 s
poll, `runtime.background_workers`) for the control-plane work no request
path can trigger. Each leg is fail-isolated per pass and fail-closed
inside its crate owner:

- **Published-blob GC.** Every pass reloads
  `SeaOrmArtifactBlobRetentionPolicy` from the durable retention-hold
  ledger (the single owner of "these bytes must survive" facts) and runs
  `ArtifactAdmissionReconciler::reconcile_published_blobs`. A digest
  under any active CAS hold (rollout window, predecessor standby,
  investigation, audit, legal) is retained; a snapshot load failure
  aborts the pass with no deletions. Policy eligibility is only one
  gate — deletion still requires the reconciler's two-phase
  mark/grace/claim over the admission reference set.
- **Tenant data-object GC.** `tenants_with_candidates_after` discovers a
  bounded page of tenants holding queued candidates in ascending UUID order.
  The host retains the exclusive cursor across maintenance passes and wraps
  after a short or empty page, so persistent retention-held candidates or a
  large backlog in early tenants cannot starve later tenants. A discovery or
  retention-snapshot failure preserves the cursor and retries that page;
  an individual tenant-sweep failure does not block later tenants and is
  retried after the cursor wraps. The cursor is process-local, while candidate
  rows remain durable across restart. When a page has work, the worker loads
  one `ArtifactDataObjectLedgerRetentionSnapshot` and
  `sweep_tenant_with_ledger_retention_snapshot` reuses it for every tenant in
  that page (a snapshot load failure deletes nothing). The sweep derives
  per-object rules from the queue timestamp plus a fixed 7-day grace window
  and retains any namespace under an active data-object hold wholesale
  (over-retention is the safe direction while a namespace is under
  investigation).

- **Admission reverification.** Enabled only when
  `RUSTOK_MODULE_MAINTENANCE_VERIFICATION_ENDPOINT` (https, mTLS via the
  `RUSTOK_MODULE_MAINTENANCE_VERIFICATION_*` client-identity variables)
  is configured. This snapshot deliberately has **no server-settings
  counterpart** for the endpoint, mTLS material, policy revisions, or
  maintenance actor: those deployment-owned inputs are currently the
  `RUSTOK_MODULE_MAINTENANCE_*` environment contract read by the worker,
  because the snapshot does not yet expose a typed server setting for
  this maintenance leg.
  `RUSTOK_MODULE_MAINTENANCE_TRUST_POLICY_REVISION` and
  `RUSTOK_MODULE_MAINTENANCE_CAPABILITY_POLICY_REVISION` name the
  current policy; admissions whose durable evidence carries different
  revisions (or undecodable evidence) are listed by
  `list_stale_candidates_after` and re-verified as owner commands under
  `RUSTOK_MODULE_MAINTENANCE_ACTOR_ID`, with a deterministic per-candidate
  idempotency key so retries replay instead of re-verifying. The owner returns
  the last installation ID examined, not just the last stale candidate, so the
  host can continue beyond both a 32-candidate page and the 8,192-row scan
  budget. Failed candidates remain eligible but do not pin the first page:
  the host advances after a successful scan and retries them after wrap. A
  scan error preserves the cursor. Invalid configuration disables the leg
  loudly at startup; it never lets stale evidence pass for fresh.

Without initialized durable storage the worker is not spawned and
startup logs an error: blob and data-object garbage collection are off.

### Delivery parking on policy denial

A policy denial (`DeniedByPolicy`) is raised before any guest
execution, and policy toggles are routine state. Event and schedule
deliveries therefore treat `denied_by_policy` as *held*: the completion
returns the delivery to `pending`, reverts the claim's attempt
increment, and reschedules with the queue's retry backoff. Only genuine
execution failures spend the `max_attempts` dead-letter budget, so a
temporarily disabled module receives its queued work after re-enable
instead of losing it permanently. A permanently disabled module's held
deliveries remain until the module is re-enabled or its work is purged
through uninstall. Each claim now stores a fresh UUID independently of the
failure-attempt budget. Every completion requires that exact token and a live
lease according to the database clock; a held delivery can therefore reuse its
attempt number without accepting the previous completion. PostgreSQL creates
claim deadlines and retry delays using `clock_timestamp()`, rather than the
transaction-start `NOW()`, so time spent waiting before the statement does not
silently consume a fresh lease. PostgreSQL completion locks the delivery row
before checking expiry, avoiding a check performed before a lock wait. The
guard is at the database statement, not a fence for guest side effects already
in flight; the lock-wait regression has not yet run on real PostgreSQL.
Missing or out-of-scope installations, missing admission identity, and corrupt
release descriptors are repair errors rather than proof of unavailability;
claim rolls back rather than terminally cancelling or dead-lettering such work.

Migration `000092` requires a **stop-the-world worker upgrade**: stop all old
event and schedule delivery workers before migration and do not restart them.
They do not check claim tokens. Existing running rows get a NULL token and
cannot be completed by new workers; they expire and are retried under normal
queue policy. The migration refuses automatic downgrade. Downstream callers of
the queue completion API must carry the claim token from the work item.

The delivery adapters rotate their starting tenant and log per-tenant claim or
materialization failures while continuing to other tenants. If no item is
claimed, they return the first error; when another tenant supplies work, the
error remains observable in logs. A poison item still blocks later work within
its own tenant, and tenant enumeration failures remain global.

Static settings, localized settings and source-locale writes lock the current
receipt before touching the lifecycle aggregate. A tenant-scoped fence binds
that aggregate claim to the exact receipt and lease; reclaim can transfer only
its own recorded claim, while a journal or unrecognized claim remains busy
until its owner reconciles it. Completion and cleanup lock the same receipt
before the aggregate, preventing a stale worker from cleaning up the new claim.
The aggregate row also records the owner domain (`journal` or `settings`).
Journal heartbeat, advance and release check that domain on the aggregate
itself; the stale reaper no longer releases claims. These paths cannot take or
release a settings claim even if UUIDs match. Settings writes use settings-only advance and release. This
avoids relying on a tenant-RLS-filtered fence lookup in journal workers. The
cross-owner changes and their Rust regression tests remain unverified on the
real application and require PostgreSQL concurrency/lock-wait testing.

Migration `000093` requires stopping **all old static lifecycle and settings
writers** and reconciling active aggregate claims before upgrade. Legacy claims
are not distinguishable by owner: the migration fails if any remain, and it
refuses automatic downgrade. Do not run mixed old and new writers. An unrelated
expired journal claim is not automatically stolen by settings; investigate it.
There is no implemented proof-based lifecycle reconciliation procedure yet. Fence rows can remain after a claim is superseded
by another owner and need an owner-verified retention procedure; they cannot by
themselves authorize a claim transfer. PostgreSQL/RLS and concurrency evidence
for this protocol remains required before production.

### UI resource projection

Activation-time navigation-route/storefront-slot collision checks read
`module_artifact_ui_resources`, an admission-time projection written in
the same transaction that creates each immutable installation (and
backfilled by migration `000085`). The check is an indexed lookup by
resource identity; descriptor validity is an admission-time invariant
and is no longer re-proven for every active installation inside the
activation transaction.

### Schedule digest upgrade note (R3-41)

Schedule binding digests are computed over the canonical six-field cron
expression. Stored five-field digests from earlier builds no longer
match, so on the first poll after upgrade each binding's schedule cursor
is re-initialized at `now` (no backlog replay) and any pending
pre-upgrade slots under the old digest stop being claimable. This is a
one-time, self-healing re-initialization, visible as
`initialized_cursors` in the materialization report.

### Transition checkpoint scope on PostgreSQL

`module_transition_checkpoints` intentionally has no RLS policy: the
tenant-lifecycle fence must see platform-scope transitions for the same
slug from inside a tenant transaction. Scope discrimination for this
table is application-level (`tenant_id` predicate plus the
Platform-before-Tenant activation lock order), consistent with the
transition watchdog reading all checkpoints.

## Verification

- `cargo xtask module validate modules`
- `cargo test -p rustok-modules`
- `cargo check -p rustok-server --lib`

## Related Documents

- [Implementation plan](./implementation-plan.md)
- [Neutral sandbox ADR](../../../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md)
- [Module control-plane plan](../../../../docs/modules/module-control-plane-consolidation-plan.md)

### Host execution capability lease

The server-composed artifact binding executor registers a host-only live lease
for each generated execution ID. It binds the exact sandbox subject, tenant,
actor/trace/idempotency context, phase, binding and admitted capability set.
The non-renewable monotonic deadline is bounded by the sandbox wall-clock
budget and, for direct HTTP/command execution, the original credential expiry.
The request scope capture shares its invalidation flag and never serializes
bearer evidence to a worker. Initial preflight and each capability call check
current direct-session/RBAC authority, tenant serving policy and readiness
(non-lifecycle), and exact current installation/grant evidence (capability
calls). Only declared binding capabilities are eligible. Owner lifecycle and
durable work retain their own admission; audit actor attribution does not
create a direct credential. Scope and lease expiry are checked before launch
and before returning output. The guard removes the lease on completion,
failure or cancellation; the broker checks membership again after async reads.
This is process-local admission, not an atomic external-effect fence or a
continuous interrupt for capability-free computation. In-flight calls may
complete, and a post-launch denial preserves ambiguous-outcome semantics.
Operational maintenance operator authorization/transport and native serving, deferred/background channel context, external owner transactional authority,
performance and actual worker callback verification remain open. See
[execution capability lease ADR](../../../../DECISIONS/2026-11-27-artifact-execution-capability-lease.md).

### Outbound HTTP capability SSRF boundary

The `platform.http` broker disables process/system proxies on both its shared
and DNS-pinned clients. Hostname resolution is bounded by the capability
deadline, every returned address is screened before the first validated address
is pinned, and the remaining deadline is shared by connection, request and
response-body consumption. Redirects remain disabled and response bytes are
streamed under an overflow-safe configured ceiling. Zero timeouts and zero
response limits are rejected during construction. Host matching is exact,
method grants are canonical uppercase, path prefixes respect segment
boundaries, and ports default to HTTPS 443/HTTP 80 unless the grant explicitly
lists custom ports.

Rhai `http_post(url, body, idempotency_key)` and
`http_request(method, url, body, headers, idempotency_key)` overloads allow
an explicit per-call UUID. When omitted, the broker derives a stable key from
the semantic request digest; identical requests in one execution therefore
replay the same effect. Use distinct explicit UUIDs for intentionally repeated
identical requests.

Mutating methods (POST, PUT, PATCH and DELETE) require a stable host-issued
execution idempotency key. Each request also has an explicit or deterministic
per-request key. A tenant/installation-scoped receipt is committed before
dispatch; completed responses are persisted and replayed, while a conflicting
key is rejected and a dispatching/ambiguous outcome is never automatically
resent. The composite key is also sent as the remote `Idempotency-Key` header
for services that support it. Event and schedule deliveries propagate their
stable delivery ID into the execution context, so retries address the same
receipt. GET, HEAD and OPTIONS use a short SQL admission fence but do not create
effect receipts.

The network request remains outside the SQL transaction. A timeout or process
failure after dispatch can leave an ambiguous receipt; that is deliberately
not treated as permission to retry the remote side effect. It requires
reconciliation rather than automatic replay.

### SQL capability effect authority

The server composition injects an explicit SQL effect authorizer into the
structured-data, private-object, event-outbox, HTTP, MCP and secret-handle
brokers. It captures the live execution lease before opening the owner
transaction, reuses Auth/RBAC tenant/user/session/role fences on that
transaction for direct user execution, and checks the locked credential
deadline and lease membership before commit. Structured data, private objects
and event outbox use this guard through their owner commits. HTTP dispatch,
MCP registry calls and secret-handle reads use short fenced admission
transactions that are closed before network, MCP or secret-provider I/O. All structured CRUD,
list/index reads and the delete-receipt replay path use the same commit gate. Object metadata
reads/lists, object put/delete receipts, upload-session creation/claim, chunk
append, completion transitions and their replay paths use the same guard.
An error rolls back the owner transaction and its receipt; no new database
connection or guest invocation is permitted under the fences. The events resolver also
binds the complete installation subject and tenant, rejecting mismatched
capability identity. Owner-only/background work preserves its separate
admission contract and checks lease lifetime, without inventing user evidence.
The SQL fence also acquires the existing platform-then-tenant artifact
activation locks, reselects the exact installation after the lock wait and
resolves its durable sandbox policy on that transaction. It rejects changed
installation facts, capability revision, missing grants and changed grant
constraints relative to the execution lease. Activation, deactivation,
rollback and tenant artifact lifecycle writers use these same selection
locks. Lifecycle hooks do not use the serving-mutation journal rejection.
The lease also captures the exact release's clear security revision (zero
for implicit clear). Every host capability rechecks it, including lifecycle
calls. SQL effects verify it inside the transaction under the platform slug
activation fence, shared by quarantine, clear and revoke writers. This fences
first quarantine even without a security row, and quarantine followed by
clear cannot revive an older lease. Registry release status has an independent
per-(slug, version) monotonic authority witness (migration 000096). The permanent
witness advances on row insert/delete or status/checksum changes, including
unlisted-to-listed-to-unlisted ABA; no-op metadata changes and rolled-back
transactions do not advance it. Publication, yank and unyank acquire the platform
slug fence before registry row locks; mutation triggers share that fence. Yank
remains a discovery/install fact: it invalidates old leases but does not prevent
a fresh lease solely because the release is yanked. Unavailable status denies
admission. For ordinary serving, the same owner closure used by node readiness is
captured in the lease, including exact artifact members reached through
dependencies and co-requisites and their security and registry authority revisions. Before any root
selection lock, SQL admission takes all artifact slug locks in ascending
order (platform then tenant per slug), reselects the complete artifact set,
validates each durable sandbox policy, security revision and registry witness, and keeps the
locks through commit. Non-SQL calls compare fresh closure evidence but remain
preflight-only. The private-object owner separately fences its SQL
metadata/session/chunk transitions and revalidates the execution lease after
object bytes have been read. Lifecycle retains root-only owner authority. For serving,
every visited module (native and artifact) also captures its canonical
StaticTenantLifecycleStore revision and exact optional tenant override,
including absent intent. Core overrides remain irrelevant to policy.
The SQL effect takes platform slug locks for the entire mixed closure in
ascending order (and the tenant selection lock for each artifact), then
compares lifecycle revision and override on the same transaction. Both
override commit paths take the same platform lock before writing intent,
journal and aggregate revision; static settings commits already use it.
A concurrent first override cannot bypass the absent-row fence, and
disable/clear or disable/enable cannot revive the prior lifecycle revision.
Settings changes conservatively invalidate leases because they advance the
same owner revision. Active claims alone are not treated as committed
intent changes; no lock spans guest hooks. The serving policy is now derived
from one verified owner composition snapshot and carries that snapshot's
revision/hash into the lease; a final nonlocking check rejects composition
changes during derivation. Each fresh capability preflight compares the
composition identity as well as closure evidence. SQL effects acquire the
existing shared active-composition row lock after the mixed closure fences
(SQLite already holds a write reservation) and compare both revision and
hash without bootstrapping missing state. Owner CAS replacement serializes
with this lock; returning to an identical manifest does not restore the
old revision. The global composition fence conservatively invalidates a
serving lease even for unrelated manifest edits. No extra migration is
needed. This covers composition-derived defaults and co-requisites, not
changes to an in-process native registry or independent deployment inputs.
The serving closure also binds the converged reconciliation state revision,
head IDs, owner-derived readiness digest, node ID and exact observations for
every selected artifact. Capture all readiness evidence in a single owner
read snapshot, then reject a changed head. SQL effects take the shared
reconciliation state row lock after mixed-selection fences (SQLite already
holds its write reservation) and before the composition row lock. All
reconciliation requests, claims, heartbeats and reports acquire the exclusive
state row lock before changing ledger evidence. Recompute the persisted
deployment digest and recheck every required live/observed assignment identity
and observation revision under that lock. An incomplete, legacy, changed
or missing set denies the effect; even a restored head cannot renew a lease
whose state revision changed. Fresh non-SQL preflight compares the same
readiness evidence without an effect lock. Lifecycle hooks retain their
separate root-only admission. This protects the durable ledger, not live
executor availability or independent worker liveness. Contextual
deferred/background channel context is not transactionally fenced. Operational serving maintenance now has a Modules-owned singleton revision and history (migration 000097), with global, tenant, module and tenant/module scopes combined by union. The owner replacement service requires an injected transaction-bound platform authorizer and predecessor CAS. Its normalized scope changes and command history commit together. The artifact resolver captures active or inactive maintenance evidence, includes it in the mixed policy and checks the exact revision through every serving lease and structured SQL effect. The shared row lock follows composition and Channel fences. Native/static serving, concrete operator authority and transport remain open; recovery/lifecycle remain separate owner paths. Drain old serving processes and leases before relying on the migration; downgrade is denied. HTTP channel resolution is now re-read from the owner without the TTL cache, bracketed by the durable generation and matched to the middleware-selected channel. The mixed serving policy evaluates its exact bindings for every closure member; unresolved HTTP context retains the generation as well. Capability preflight reuses the captured input and compares the generation. Structured SQL effects hold the Channel-owned generation row after readiness/composition fences until commit. No new migration is needed. Registry authority
is checked under the sorted platform/tenant slug fences, before readiness and
composition locks. Non-SQL effects remain preflight-only; lifecycle checks only
the root. Drain old writers and leases before applying migration 000096; witness
downgrade is denied. Direct SQL maintenance must follow the canonical lock order;
DDL/TRUNCATE, witness tampering and complete cross-owner ordering are outside
this guarantee. Each artifact lease also captures
both platform and tenant selection generations on the permanent slug fences.
Successful activation, deactivation, rollback, uninstall and explicit artifact
tenant enable/disable advance the affected scope in the same transaction as
the selection change and receipt. Lock acquisition and receipt replay do not
advance generations. This rejects disable/enable, replacement/rollback and
temporary tenant shadow ABA even if the exact installation returns unchanged.
Capture exact selections and generations in one read snapshot; SQL effects
compare generations after all closure locks. Lifecycle captures its root
generation separately. This deliberately invalidates leases conservatively
on scope mutations even when the effective winner does not change.
Migration 000095 adds the generation column with baseline zero. Drain old
executions, stop old selection/security writers, migrate and deploy all
writers and effect consumers together; mixed-version writers can bypass the
fence. No transport change is required. Cross-owner lock-order verification
remains outstanding. Independent non-server broker
compositions still need an explicitly supplied authority. Physical object-store
bytes, outbound HTTP exchanges and host-only secret-value consumers remain
outside SQL transactions. Object reads and all capability responses are
revalidated before release. HTTP mutations use the durable receipt protocol
above; a completed response is replayed and an unresolved dispatch is never
automatically repeated. The current artifact MCP surface is an explicit
read-only registry allowlist; adding a mutating MCP tool requires a durable
per-call receipt before it can be exposed. Secret handles are read under the
execution fence, and host-only value use rechecks the exact binding revision
after resolution and reauthorizes immediately before the consumer.
See [execution capability lease ADR](../../../../DECISIONS/2026-11-27-artifact-execution-capability-lease.md).

### Private object capability effect boundary

The server-composed `platform.data.objects` resolver receives the same
execution SQL effect authorizer as structured data and events. The owner locks
the exact execution authority on every metadata transaction, including reads,
lists, put/delete receipts, upload-session creation and completion claims,
chunk append, completed-upload replay, and the final session transition. The
guard rechecks installation selection, capability grant constraints, security
revision, direct credential/RBAC fences, and lease liveness before commit. Every
object operation also locks the serving namespace and rejects an active object
or record-copy migration hold before reading or publishing namespace state.

Object-store bytes are not part of the SQL transaction. A normal object put
writes to a private generated key before publishing metadata; chunk append
writes a private staging key before opening the owner transaction. The owner
transaction then revalidates session state and quota before publishing the
chunk row, so no SQL authority lock spans object-store I/O. A known denied
commit rolls back SQL and compensates the just-written bytes. An ambiguous
storage/database failure does not delete bytes whose metadata commit may have
succeeded. Object reads commit a fenced metadata snapshot, fetch bytes outside
the transaction, and revalidate the exact execution authority before returning
the response. Upload completion is resumable through the durable object
idempotency receipt and the session's original idempotency key.

This is not a distributed transaction or a complete orphan-recovery guarantee.
Production fleet/write/retention cutover, ambiguous-outcome reconciliation and
safe orphan collection remain open.

### Cross-owner transaction ordering

For the inspected Auth/RBAC and artifact serving/security/lifecycle boundaries,
acquire credentials before composition, lifecycle policy cursor (when needed),
roles, sorted module slugs (platform then tenant), readiness, Channel and
maintenance. Lifecycle state commit validates the cursor predecessor before
acquiring its slug fence; no-transition commits do not acquire a cursor.
Direct serving SQL checks composition before roles; background serving checks
composition before closure locks. SQLite composition reserves its writer
itself. See the [execution authority ADR](../../../../DECISIONS/2026-11-27-artifact-execution-capability-lease.md) for the scoped ordering and remaining audit gaps.

Reconciliation request/claim/heartbeat/report now acquire sorted distinct
platform slug fences before the readiness state row and read installation
and admission identity under PostgreSQL FOR SHARE (compatible with FK KEY
SHARE). Agent paths recheck the exact desired state and target installation
set after waiting; either convergence branch revalidates all target identities.
Migration checkpoint writes now acquire platform/tenant slug fences before
receipt and identity writes. No new migration; coordinate owner writer rollout.
Migration 000098 removes the blocking registry AFTER-trigger platform edge.
Migration 000099 adds PostgreSQL BEFORE guards for new release request references
and publication/yank/unyank receipt parents. The guard takes KEY SHARE NOWAIT
in request-before-release order where both are referenced, rejects missing or
invisible parents, and retains existing FK constraints. SET NULL cascades and
unchanged release request references need no new request parent lock. Retry
55P03 from a new transaction. Source regressions are authored, unrun. Other
implicit FK/receipt/generation ordering remains open.

Validation claim, remote completion and expired-claim requeue acquire the
publish request before locking its job/stage, matching manual stage reports
and enqueue. Locate without locking a child, then re-read it under the exact
request fence and child row lock; use PostgreSQL read committed and a live
clock for remote lease predicates. The lock-order regression is authored,
unrun; broader owner/worker contention evidence remains open.

Migration 000100 adds request-child and owner-receipt FK guards, including
review/create/upload/validation receipts, evidence and its receipt, job/stage,
build/external/Alloy staging, platform admission and release artifact identity.
The guards run on INSERT or explicit FK-column UPDATE, taking request before
evidence/release with KEY SHARE NOWAIT. Metadata-only heartbeat updates do not
acquire a parent fence. Existing FK actions are retained. Retry SQLSTATE 55P03
from a new transaction; missing/invisible parents fail closed. Source-only
regressions cover all listed parent classes; no runtime proof yet.

### Durable lifecycle cursor in request module authority (2026-11-27)

The host retains the Modules owner's exact optional `module.lifecycle`
cursor state (digest and monotonic sequence), plus the active composition
revision and verified manifest hash. An uncached policy observation brackets
its evaluation with composition and cursor reads; drift or unreadable evidence
fails closed. The read-only cursor port sets transaction-local tenant scope,
never inserts a cursor or locks a writer row, and distinguishes missing from
issued zero state. The original authenticated scope, including WebSocket lease
and captured clones, compares the complete owner observation. A canonical
lifecycle transition/resync ABA is detected even if the final policy digest and
grants equal those admitted earlier. Conservative invalidation is intentional
for unrelated tenant lifecycle transitions and first cursor creation.

This is not a transaction-spanning revocation fence. Raw SQL bypass of owner
transition writers or deletion/recreation of owner generations is not covered
by the sequence contract; database-level enforcement and PostgreSQL contention
verification remain open. SQLite source regressions cover read-only absence,
rollback and cursor ABA without notification; execution is pending.

### Lifecycle cursor continuity integrity

The tenant `module.lifecycle` cursor is durable security authority, not
disposable consumer progress. The Modules migration prevents ordinary DML
from deleting, replacing, moving or resetting that cursor, including SQLite
implicit REPLACE deletes through rowid or alternate UUID representations.
New lifecycle cursors start at zero with no digest; existing legacy
sequence-zero digests are preserved. An advance requires the exact next
integer sequence and a different valid digest. Exact no-op writes remain
legal. PostgreSQL TRUNCATE is rejected for the cursor table because it would
bypass per-row history; other consumers retain normal row mutation semantics.
Install with old writers drained, validate existing rows and reject
inconsistent identity/sequence evidence rather than reconstruct history.
Lifecycle cursors persist even after tenant removal; do not reuse tenant IDs.
The owner cursor remains the source of truth. No new generation table.
This does not couple direct tenant_modules edits to the cursor or prove that
a manually supplied successor digest describes the current policy; the
existing content observation remains necessary. Privileged DDL, restores
and pre-cutover lost history are not covered. Source regressions unrun.

### Static intent physical change witness (2026-11-27)

Migration `m20261127_000105_static_intent_authority_generation` installs a permanent per-tenant generation maintained transactionally by `tenant_modules` DML triggers. It complements, rather than changes, the lifecycle event cursor: INSERT/DELETE and changed enablement advance the witness; settings-only updates do not. SQLite additionally advances on override ID changes and rejects cross-tenant implicit REPLACE, including rowid conflicts. PostgreSQL forbids TRUNCATE of intent or witness history; both backends reject witness reset, deletion, identity replacement and overflow. Zero is the cutover baseline, not reconstructed history. No tenant FK removes witness history on deletion. RBAC uncached observations bracket policy evaluation with this owner-issued generation in addition to the lifecycle cursor and composition identity. A completed raw-DML ABA invalidates admitted scope and WebSocket leases, without event delivery. The witness is not a policy engine or event producer. DDL/privileged trigger bypass, database restore, composition raw-DML ABA and commit-time authorization fences remain distinct boundaries. Deployment must apply migrations before the new auth reader. Regression sources authored, not executed.

### Artifact security receipt key admission

Migration 000106 adds a PostgreSQL nonwaiting transaction advisory gate for
each global artifact-security receipt UUID, including ordinary INSERT,
UPDATE and DELETE. New reservations take the receipt relation write mode
with NOWAIT after the epoch (if needed); replay reads take ACCESS SHARE
NOWAIT without taking the epoch. A busy key aborts the statement (55P03);
roll back the entire owner transaction before retrying. SQLite keeps its
serialized writer. This does not protect arbitrary explicit tuple locks,
privileged DDL or other receipt tables. Drain old writers for cutover;
source contention regressions are authored, not executed.

# Module release rollback safety

- Date: 2026-08-06
- Status: Accepted, amended on 2026-08-09
- Supersedes in part:
  - the rebuild-on-rollback rule in
    [Static Promotion Review Boundary](./2026-07-22-static-promotion-review-boundary.md);
  - caller-selected migration compatibility in the current artifact rollback
    command; and
  - any direct operator rollback path owned by `rustok-build`.

## Context

RusToK needs an operator-friendly production module update experience: users
must be able to identify the serving, candidate, and previous releases; start
one update; understand rejection or failure; and return to a verified direct
predecessor when that is safe.

The platform is a compiled, distributed application with dynamic artifacts,
not a plugin directory whose files can be overwritten. Dynamic rollback is
scoped to an installation. Native code, the server, embedded Leptos surfaces,
generated registries, and browser assets are deployed as a complete compiled
distribution and cannot be rolled back as isolated files.

The repository currently has overlapping static release state:

- `rustok-build` exposes an active release and direct rollback mutation;
- `rustok-modules` owns the platform composition projection;
- `rustok-modules` also owns static-distribution release and desired/observed
  rollout state.

The `rustok-build` rollback changes database statuses and emits an event but
does not prove deployment or health convergence. The static-distribution
rollback queues a new build, which makes compiler, worker, source CAS,
toolchain, signing, and verifier availability part of the incident path.
Neither is the required bounded automatic recovery contract.

Existing control-plane contracts retain immutable artifacts, predecessor
lineage, audit/outbox facts, migration checkpoints, artifact-data snapshots,
security state, and desired/observed native rollout facts. They do not justify
automatic restoration of committed database data. Sandbox, build, migration
transaction, or rollout evidence alone cannot prove that the predecessor
remains correct after live writes or an irreversible effect.

## Decision

### Canonical Owner and Release Units

Production versioning, update safety, rollback eligibility, and incident
outcome are one `rustok-modules`-owned lifecycle. A production release is an
immutable identity binding source, dependency lock, build/test evidence,
artifact/UI and role digests, declared migration/data-contract facts, and
declared executor kind/runtime ABI/capability requirements. Immutable admission
test evidence is attached to the release
record, but current policy/security revisions, topology, controller authority,
exact runtime fingerprint, pool generation, node observations, and deployment
receipts belong to an update/rollout readiness operation. Changing those live
facts does not create a new artifact release.

`rustok-modules` is the sole operator-level owner of:

- module update intent and exact preflight decision;
- selected/serving/candidate/direct-predecessor facts;
- rollback unit, compatibility and observation policy;
- durable operation, complete cross-scope conflict fence set, and one
  automatic attempt;
- rollback selection, desired/observed rollout, retention hold, and incident
  outcome; and
- the shared operator projection and command result.

`rustok-build` owns canonical role-plan construction/validation and shared
non-operator build primitives. `rustok-static-distribution-worker` is the sole
trusted executor/publisher for an owner-authorized complete static role bundle
and returns one canonical static role-bundle receipt. There is no second static
publisher. `rustok-migrations` remains the neutral trusted migration executor.
Sandboxes and deployment agents execute narrow owner-authorized work. None owns
a second update, rollback, or incident decision.

The atomic implementation cutover removes the public `rustok-build` active
release/rollback mutation, its duplicate mutable head, and every direct
GraphQL/native/CLI caller. No dual-write or compatibility path is retained.

Release admission may retain immutable build/selection lineage, but it cannot
name the production direct predecessor. The owner freezes the direct
predecessor from the exact observed serving state only when a production
transition begins. Building or admitting any number of unused candidates
therefore cannot change a later rollback target.

The release and rollback unit is:

- one exact platform- or tenant-scoped installation for a dynamic artifact; if
  dependency resolution changes other installations, the complete changed
  lock graph joins the unit; or
- the complete immutable role distribution for static/native code. The
  recovery operation separately binds its exact topology snapshot.

A static rollback can therefore return every module co-released in the direct
predecessor composition. Operator preflight shows that complete blast radius.
Unchanged dynamic dependencies and active dependents are eligibility evidence,
not mutation targets.

### Artifact Planes and Physical Deployment

Source preparation, distribution, runtime execution, control state, persistent
data, and node placement are separate boundaries:

- `rustok-modules` preparation is the sole logical source-CAS owner through the
  unversioned `SourceObjectStore` port and one physical adapter. It publishes a
  globally deduplicated create-only blob under `source_digest`, while each
  authorization domain gets an RLS-scoped `source_receipt_id` over
  owner/preparation, source digest, media type, length, and manifest. Equal
  same-request replay converges; divergent bytes or metadata under that request
  reject. Identical private-owner bytes share only the blob, never receipt
  authority or metadata. The owner holds/collects a blob only after every
  receipt releases it. The CAS is
  never mounted into an application runtime. Platform/native/WASM trees use
  deterministic archives through the archive codec/client, while a reviewed
  Rhai release uses its canonical bounded-workspace bytes without an invented
  tar wrapper;
- OCI is the digest-pinned distribution and trust-verification boundary for
  dynamic packages and the single complete static role bundle;
- platform-controlled CAS is the only runtime byte source for admitted dynamic
  WASM/Rhai payloads; runtime and rollback never fall back to OCI or source;
- PostgreSQL stores release/admission/selection, operation, migration,
  desired/observed, retention, and typed receipt state, but not executable
  bytes or raw logs; and
- node-local caches, materialized releases, slots, and journals are owned by a
  deployment agent and never become release authority.

The default local/monolith physical layout is rooted at one trusted
operator-selected `<instance-root>` and uses only canonical relative
subdirectories. It may be located anywhere supported by the operating system;
no Linux FHS path, root privilege, container, drive, or separator is part of
the product contract. Advanced adapters may map logical subtrees to volumes,
OCI layers, object storage, logging, or runtime facilities. Those mappings are
host placement evidence only and never alter release IDs, digests, logical
object keys, migration identity, or rollback semantics. Independent
installations use independent roots and data-plane authority; replicas share
owner state but keep node-local cache/runtime placement.

A dynamic descriptor declares executor kind and ABI requirements but cannot
choose trust placement. Release admission validates only immutable kind/ABI and
globally applicable policy constraints; it owns no installation scope or live
capacity decision. Scoped install/enable/update resolves and persists exact
placement, policy revision, worker/node attestation, executor fingerprint, and
live capacity. A language sandbox is not proof of process isolation. Missing
required isolated capacity rejects scoped readiness/transition while the shared
release remains inert/admitted; runtime never falls back to another placement.

Dynamic prepared-cache identity uses a stable `runtime_fingerprint` over the
exact executor binary, WASM/Rhai engine build, engine configuration revision,
isolated-worker image digest where applicable, target/CPU contract, and runtime
ABI. Node/pool generation is a separate monotonic readiness identity, not part
of that stable fingerprint. A changed fingerprint invalidates prepared bytes;
a new pool generation may reuse only rehashed compatible bytes and must repeat
smoke readiness. Automatic mode requires current-generation candidate and
predecessor receipts for every fingerprint that may serve or recover the
operation; a friendly version or stale pool receipt has no authority.

The static deployable identity is one OCI root digest that binds the exact role
and surface set, each role artifact/image digest, generated registries, every
present Leptos SSR/CSR/hydrate and browser asset, migration/data-contract
declaration, and other deterministic deployable metadata. The OCI root cannot
contain its own later signature or referrer digests. One create-only canonical
publication receipt binds that root and its role digests to every exact SBOM,
provenance, test, signature, and referrer payload/manifest identity; release
admission verifies and retains the receipt. Nodes, traffic weights, secrets,
and live configuration are rollout facts rather than bundle bytes. Changing
node count, placement, or the selected assignment of roles already present in
the bundle creates/revises a maintenance rollout; requiring a role artifact
absent from the bundle creates a new bundle/release. A first install has candidate-only
assignments and no recovery target. Automatic code update requires an identical
node/failure-domain/role assignment domain carrying both candidate and
predecessor digests; placement/count or role/surface-shape changes are separate
maintenance transitions.

OCI is the only production static publication path. A container-runtime layer
or bare-metal release directory is a verified materialization of that digest,
not a second publisher or release ledger. The current `rustok-build`
filesystem/HTTP/container publishers, arbitrary rollout command, active release
head, public rollback, and installer per-role release activation are replaced
atomically rather than retained as fallbacks.

Before predecessor capacity changes, candidate and predecessor bytes are
pre-staged, rehash-verified, and locally startable on every affected assignment.
Registry or network access is not an automatic-recovery dependency. Browser
assets use release-qualified or content-addressed URLs, remain available for
the declared mixed-fleet/client lifetime, and return not-found rather than
current HTML when an immutable asset is absent.

The outside-candidate controller and node agents are operations infrastructure,
not role-bundle contents. An agent accepts only exact operation/node/role/bundle
and role-digest work, has no build, signing, DDL, restore, release-selection,
arbitrary-command, or Next.js authority, and reports authenticated monotonic
receipts. Application/module processes cannot write the deployment root or
access the agent control socket.

Those components are supplied as one independently deployable signed
operations-tool release. Installer bootstrap and every affected operation bind
its exact package/component digests, target, signer/evidence identity, and
external controller/agent protocol revision. `rustok-modules` revalidates those
facts but neither installs nor self-updates the tools. Subsequent tool upgrade
is the `operations_tool_maintenance` operation class in the same canonical
`rustok-modules` operation/receipt ledger and state vocabulary, with a fleet
conflict key, exact host/component desired/observed assignments, protocol
matrix, idempotent reports, and one predecessor recovery authorization. The
host provisioner/service supervisor is the narrow executor: it applies exact
owner assignments, retains the predecessor tools, and cannot select versions
or own another ledger. Candidate application code is never a dependency of
this tool lifecycle.

On a single HTTP/SSR node, automatic mode preserves predecessor capacity while
the candidate starts on a non-serving slot, switches traffic outside the
candidate only after readiness, and keeps the predecessor locally recoverable
through observation. Worker handoff fences the old claim generation before the
new one can claim work. If capacity cannot support that or another measured
predecessor-preserving path, the transition is maintenance. Multi-node rollout
uses sorted role/failure-domain assignments, pre-stages both releases
everywhere, mutates one bounded wave at a time, and retains a predecessor/control
cohort until convergence.

Retention marks generic source objects, media-type receipts/manifests and locks
(including Rhai workspace objects), OCI roots/roles/referrers, dynamic
executable CAS payloads, live artifact-data objects/logical metadata,
upload/session staging, logical-delete candidates and tombstones,
snapshot/restore copy intents and created object keys, browser assets, build
inputs/receipts, node-ready predecessor slots, exact queued-work executors,
operations-tool packages/components/signatures/evidence/local predecessor
slots, encrypted settings recovery-point ciphertext and exact KMS-key/schema/
descriptor roots, diagnostics, and recovery points. Holds
outlive time windows while an operation, incident, recovery, client lifetime,
audit, or legal condition remains. Physical collection uses owner-authorized
tombstone, grace, and final reference/hold recheck; uninstall or age alone is
never deletion authority.

### Operation Classes and Installation Semantics

Release preparation and production transition are separate durable operations
with separate correlation and idempotency domains. Every preparation owns one
`preparation_id` plus an explicit platform-public or tenant-private
authorization/RLS domain. Immutable CAS bytes may deduplicate globally without
exposing private release metadata, evidence, or logs; only a
platform-authorized public catalog release may be referenced across tenants.
Every platform- or tenant-scoped transition creates its own RLS-scoped
`operation_id`, receives only authorized facts and sanitized evidence
references, and read-only references the admitted release and preparation. Two
tenants never share transition authority, raw logs, receipts, or idempotency
identity.
Source/build/sandbox/publication/admission failure rejects a candidate and is
never described as production rollback.
Quarantine is a separate authorized artifact-security decision with its own
revision, evidence, and idempotency; ordinary preparation/readiness failure
cannot create it.

For a dynamic artifact, `(publisher lineage, module identity, semantic
version)` maps to exactly one immutable artifact release ID and digest set.
Equal-version/equal-digest replay is idempotent; equal-version/different-byte
admission is rejected and requires a new semantic version.

For a static composition, `(distribution lineage, human distribution
version/label)` maps to exactly one `distribution_release_id` and bundle-root
digest. It is not an embedded native module's semantic version. The same
unchanged native artifact/version may appear in multiple later bundles when
the platform, another module, selected role set, toolchain, or generated
composition changes. Operator projection shows the distribution
version/release ID/root plus the complete per-module version/digest diff.

A friendly UI may abbreviate a digest visually, but details, receipts, logs,
and machine output retain the unit-appropriate human version/label, immutable
release ID, and full authoritative digest set. Neither a tag nor version string
is mutation authority.

- The first platform install deploys one admitted base role bundle. It has no
  static predecessor; failure uses bounded fresh-install cleanup before durable
  state, exact restart resume after it, or separately authorized restore. It
  never rolls back to an invented empty platform release.
- A platform update and native module add/update/remove always build and deploy
  a new complete static role bundle. Recovery returns the complete direct-
  predecessor bundle, including co-released modules, while compatible expanded
  schema/data remains.
- Tenant enable/disable of native code already present in the bundle changes
  owner activation intent and does not rebuild the distribution. Physically
  adding, changing, or excluding native code does.
- Activation-only changes still use owner preflight and durable coordination.
  They fence new execution claims, account for in-flight work, and bind
  lifecycle-hook and external-effect receipts. Returning to the previous
  activation state is automatic only when every intermediate effect is
  compatible and reconcilable; irreversible or uncertain effects require
  maintenance and become recovery-required after their gate.
- Dynamic `admit`, `install`, `enable`, `update`, `disable`, `remove`,
  `uninstall`, `rollback`, `finalize`, `dynamic_artifact_data_purge`, and
  `dynamic_artifact_settings_purge` are distinct actions. `purge` alone is a
  non-callable category.
  Admission stores an immutable release; installation is scoped and initially
  non-executing; enablement selects serving bindings/work only after readiness.
  Product-level add is a compound install+enable operation.
- Admission commits immutable, inert permission definitions keyed by exact
  release/module/digest and never fabricates a scope or grant. Scoped install
  projects those definitions idempotently under the installation; enablement
  resolves separate scope-owned grants against the active serving generation.
  Rollback reselects predecessor definitions, while uninstall and collection
  preserve referenced grant/audit history. The installation-keyed registrar is
  therefore an install-phase port, not post-admission work.
- Permission preview compares exact predecessor/candidate definitions and
  affected roles/keys. A stable permission identity may carry a scope grant
  only when its exact canonical authorization fingerprint also matches and an
  RBAC-owner continuity receipt authorizes the carry against the current
  monotonic scope grant/role-membership epoch. The fingerprint includes
  every authorization-relevant scope/key/resource/action/binding constraint;
  localized labels/descriptions are excluded or governed separately. Any
  fingerprint change requires explicit grant approval; removed grants become
  dormant. Carry and transition/rollback commit acquire the RBAC-owner conflict
  key and CAS-revalidate the epoch. Rollback selects predecessor definitions
  and evaluates current grants; it never restores a revoked grant row or role
  membership from an old receipt. Neither
  admission nor install assigns access implicitly.
- A first enable/add has an explicit absent/disabled serving baseline. Failure
  before activation leaves the installation disabled without a rollback
  attempt. An eligible regression after activation may contain it back to that
  exact baseline while preserving candidate bytes, data, and diagnostics.
  Updating an already disabled installation creates another inactive
  installation, leaves serving state absent, claims no serving predecessor or
  automatic rollback, and requires a fresh enable preflight later.
- Dynamic removal is a reversible production transition to an absent target;
  it retains the inactive installation as the exact direct predecessor through
  the rollback window. Disable keeps the selected installation inactive for a
  later enable. Uninstall is a later fenced action allowed only after that
  installation's code-rollback eligibility is closed, no dependent or pinned
  work can select it, and it is observed absent/disabled. Uninstall retires the
  installation identity but deletes no CAS bytes, settings, data, objects,
  snapshots, schema, migration history, audit, or diagnostics. When it starts
  from disabled-selected state, one fenced transaction first moves
  selected/desired state and tenant intent to absent and advances the
  binding/work generation; delayed enable/outbox work cannot select the retired
  identity. An already absent installation proceeds directly to retirement.
  Compatibility finalization, `dynamic_artifact_data_purge`,
  `dynamic_artifact_settings_purge`, and physical garbage collection are four
  separate actions after their respective dependent, work, client, rollback,
  incident, recovery, audit, and legal holds close.
- Reinstall is a new scoped audited install/transition referencing a currently
  admitted and revalidated release. It may reuse immutable CAS bytes but must
  inventory retained data and work; identity by slug cannot silently resurrect
  an old installation.

The same payload digest may share one global CAS object across scopes, but each
scope retains distinct authorization, policy, installation, operation,
evidence, and tenant-RLS state.

Retained settings/data/object namespaces use a stable opaque data-owner
identity bound to scope and verified module ownership/publisher lineage. Slug,
contract revision, package digest, or a new installation ID alone cannot attach
retained state. Reinstall requires an exact continuity receipt; an authorized
owner/publisher change is a separate privileged conflict-fenced governance
transfer with old/new evidence and audit. A foreign publisher reusing a slug is
denied and the prior namespace remains retained and inaccessible.

### Static Incident Path

Automatic static recovery redeploys the retained immutable direct-predecessor
role bundle. Before candidate rollout begins, the owner must prove that all
required server, worker, Leptos, generated-registry, and browser-asset bytes
and receipts are retained and pre-staged for each affected role assignment;
rehash them; and revalidate current admission, security, policy, data
compatibility, topology, and deployment authority.

Rollback creates a new audited transition but neither edits artifact bytes nor
compiles a replacement. It uses the normal desired/observed rollout reconciler,
and succeeds only when the predecessor role bundle is observed healthy.
Rebuild remains release-admission/reproducibility evidence or a separately
admitted maintenance update through the same owner lifecycle. It is never a
rollback fallback. Missing predecessor bytes or evidence makes automatic mode
ineligible.

### Executable Transition Decision

Update mode is computed for one exact predecessor-to-candidate update and its
candidate-to-predecessor recovery within one live scope. Documentation,
semantic versions, a sandbox run, a module
declaration, or caller input cannot authorize it.

`rustok-modules` persists an immutable decision bound to the operation class,
exact from/target states, applicable exact/absent/not-applicable release facts,
rollback unit, dependency and active-dependent closure, configuration/data/schema and
migration checkpoint, security/policy/topology revisions, health policy,
retention, recovery evidence, and evidence digests. The owner reloads these
facts under revision/fence checks before every state-changing transition.
Missing, stale, contradictory, or unverifiable evidence fails closed into
maintenance.

Preflight requires no conflicting nonterminal operation and, wherever serving
or activation participates, converged selected, desired, and observed state
across the conflict set. First install, remove-to-absent, activation-only, and
purge never fabricate an inapplicable release fact: first install has a target
bundle but no predecessor, while remove/activation/purge may have no candidate
release. Preflight returns an
immutable preview of mode, mutation unit, blast radius, denial/eligibility
reasons, rollback-window effect, migration/point-of-no-return facts, fences,
and recovery action. Apply binds that exact receipt; changed evidence requires
a fresh preview. Static composition and maintenance updates require explicit
confirmation.

The current caller-selected `migration_rollback_mode` authority is removed.
Migration policy retains the existing `reversible`, `compensating`, and
`prohibited` values as owner evidence, but `reversible` is necessary rather
than sufficient.

An update has one operator-visible mode:

- **Automatic** is available only when the direct predecessor and exact
  dependency closure remain admitted, unquarantined, unrevoked, retained, and
  compatible with every intermediate live state. A bounded observation window
  permits one candidate-attributed recovery attempt. Data is never restored.
- **Maintenance** applies to unproven compatibility, non-transactional or
  mixed-fleet-incompatible DDL, compensation, destructive cleanup,
  irreversible conversion, unsafe durable work, or unsafe external side
  effects. Maintenance never performs automatic rollback.

A failure before the desired rollout or any deployment/serving mutation is an
update rejection: predecessor capacity is unchanged and no rollback attempt is
consumed. Once rollout has displaced, stopped, or reduced predecessor capacity,
a candidate startup/readiness failure is a rollout failure and may reserve the
single recovery attempt even before the candidate serves traffic. The
observation window still begins only with the first candidate traffic. An
arbitrary older release is a new fully preflighted update, not rollback.

Candidate acceptance requires runtime/topology health plus terminal
update-owned migration/backfill checkpoints and invariants, activation or
migration hooks, serving binding/work-generation materialization, required
outbox delivery, and external-effect reconciliation. An optional compatible
backfill is a separate visible durable owner operation/hold, never hidden under
an accepted update. An unknown remote-effect outcome remains running/observing
until reconciled or becomes recovery-required; it cannot be accepted.

A later update starts only after the preceding operation is terminal and its
selected, desired, and observed-serving state is converged across the conflict
set. Starting it atomically closes the previous code-rollback eligibility and
establishes the then-serving release as the new direct predecessor. Outstanding
compatibility, backfill, finalization, retention, recovery-point, durable-work,
client-lifetime, incident, audit, and legal-hold obligations remain durable
under their owners and are included in the new decision and conflict set. The
new update cannot release or forget them, and an earlier release never remains
a hidden two-hop rollback target.

### Durable Operation and Failure Attribution

One owner operation derives the complete conflict-key set for the rollback
unit, schema/data owners, dependency and active-dependent installations,
topology, and affected namespaces. It acquires or fences that set atomically in
the fixed release-unit, data/migration-owner, namespace, and topology order
before mutation; a scope-local lease cannot authorize a cross-scope change.
The set serializes update, rollback,
disable/deactivate/uninstall, quarantine/revoke,
migration/backfill/finalization, restore/purge, and retention collection. Every
external phase has immutable request binding, monotonic checkpoint, fenced
lease, idempotent terminal receipt, and restart reconciliation. Process or node
loss cannot create a second automatic attempt. Transactional phases use CAS and
idempotency; leases are limited to asynchronous or external work.

Before the first compensating or irreversible effect, the owner durably closes
automatic eligibility and establishes required traffic, job, and write fences.
A crash never reopens that gate. Failure after it creates a
recovery-required outcome.

Maintenance execution proceeds from that gate through the exact authorized
migration/effect and candidate rollout to observed serving health. It is not
accepted at the gate. Any migration, rollout, or health failure after the gate
is recovery-required. Cancellation before the gate is safe; a failed candidate
is never automatically retried and requires a fresh update/preflight.

Trusted observations are fresh and bound to the exact release, rollout scope,
topology, and pinned health policy. Module self-report, ordinary business/input
errors, missing telemetry, or a platform-wide database, broker, network, or
provider outage cannot alone authorize module rollback. A dependency symptom
counts only when a bounded predecessor/control cohort remains healthy and the
pinned policy attributes the regression to the candidate. Quarantine,
revocation, policy, topology, migration, and retention changes preempt stale
decisions. Quarantine/revocation commits one global monotonic release-security
epoch/fence without enumerating tenant operations or waiting behind their
external leases. Every later claim, activation, transition, and external-result
commit rechecks that epoch; bounded per-scope reconcilers independently
contain/drain affected installations. No post-epoch action gains authority,
while the security command itself remains bounded.

A single-node topology cannot use statistical attribution that requires a
control cohort. If trusted telemetry remains missing after candidate traffic
until the pinned deadline, candidate traffic is fenced and the operation runs
its one recovery when eligibility remains proven; otherwise it becomes
recovery-required.

Static recovery authority and its bounded evaluator remain available outside
the candidate application and embedded UI. The deployment controller receives
only the exact operation, candidate, predecessor, topology, policy, deadline,
and single-operation recovery authorization. Reservation/consumption is
persisted atomically: exact same-operation replay resumes idempotently, while a
divergent request or second operation is denied. The controller cannot select
releases, run DDL, or restore data.

### Data, Durable Work, and Finalization

The normal strategy is forward-compatible `expand -> migrate -> contract`.
Every platform/native release declares an owner-scoped desired-schema manifest;
before production mutation, `rustok-modules` obtains from `rustok-migrations`
one normalized current-to-desired diff and a plan-only, no-production-write
dry-run receipt bound to the release, current/desired schema digests, and exact
migration-plan digest. The receipt is classification evidence, not automatic
authorization. Only a no-op or proven transactional additive schema diff that
keeps one canonical representation valid for N and N+1 may pass the schema
gate into automatic mode; compatible data backfill remains subject to its
checkpoint rules. Unknown, destructive, irreversible, non-transactional,
unsafe-locking, or mixed-fleet-incompatible changes require explicit
maintenance confirmation;
the operator projection labels this **Database update required**, without adding another
operation type or state. Apply executes only the digest-bound plan, and
destructive cleanup remains separate finalization. Automatic mode must not
introduce old/new adapters, fallback decoders, dual read/write paths, or
parallel internal contracts.

Dynamic artifacts supply no SQL, DDL, physical storage keys, or native
migrations. They use host-owned live settings and brokered structured/object
data. Code recovery restores neither. A change to a dynamic persistence
contract revision is maintenance-only until the broker proves one canonical
namespace and representation that N and N+1 both read/write at every
checkpoint and after return to N. Independent source/target namespace copies,
dual read/write, fallback decoding, mirrored data, or best-effort reverse copy
cannot authorize automatic mode. Migration phase, checkpoint, and
irreversibility are owner-derived; caller-supplied rollback or irreversible
flags have no authority.

For automatic settings compatibility, the owner first verifies that the
current value is accepted by both predecessor and candidate while the
predecessor serves. It CAS-writes only when an operator explicitly selects a
different value in that intersection; `not_applicable` creates no settings
phase. Before selection, the owner installs a durable compatibility guard on
the dynamic settings instance or native/static settings revision, bound to both
schema digests and rollback-window identity. Every concurrent write
CAS-revalidates that guard through mixed fleet, acceptance, and rollback-window
closure. A one-sided value requires a separately confirmed maintenance command
that fences writers, proves predecessor serving/work is gone, atomically closes
rollback eligibility, advances the guard, and only then writes. Disjoint
settings schemas require a separate fenced
maintenance transition with a protected recovery point, exact canonical
normalized target value, idempotent receipt, and monotonic point-of-no-return
before mutation. Any transformation occurs offline; lifecycle receives no
transform executable. After that gate code rollback is unavailable
unless the predecessor is proven compatible with the resulting value; an
arbitrary artifact script or caller assertion has no authority.

A structured-record revision copier is not complete object-data migration
evidence. When live objects exist, maintenance preflight rejects the revision
change unless a broker-owned bounded plan inventories exact source digests,
persists per-copy intents/idempotency/checkpoints, and verifies the target
manifest/count before selection. Unknown object coverage fails closed.

For automatic mode, both N and N+1 must correctly read, write, validate, index,
and serialize every intermediate database/configuration state. The same
compatibility proof covers public/native transports, artifact bindings, events,
outbox payloads, schedules, queued jobs, retries, caches/indexes, and active
dependents. N+1 work remaining after return to N must be safely consumable,
drained under the bounded authority below, cancelled, or visibly dead-lettered
and reconciled.

Retention never grants execution eligibility. An N+1-pinned work item may run
after rollback only under a bounded item-specific drain authorization that
creates no new work, serves no traffic, revalidates capability, security, and
policy state, and is cancelled by quarantine or revocation. Otherwise the item
is cancelled or visibly dead-lettered for reconciliation.

Code rollback does not undo payments, emails, webhooks, published events, or
other external mutations. Such effects must be compatible, idempotent, fenced,
or covered by a tested reconciliation procedure for automatic mode. Mutating
effects require stable owner idempotency and unknown-outcome reconciliation;
unresolved outcomes prevent a successful recovery result.

Destructive compatibility cleanup is a separate maintenance finalization.
It may remove only obsolete schema/contracts and derived or reconstructible
compatibility projections; current authoritative rows, objects, and settings
are never finalization targets.
Dynamic artifact-data purge and dynamic artifact-settings purge are two
separately privileged
preview/apply operations with distinct recovery evidence, points of no return,
receipts, and restart semantics; combined apply is rejected. An operator UI may
group read-only previews but reports each result and any partial completion
explicitly. Either purge is allowed only after the installation is absent and
retired, selected/desired/re-enable authority is gone, attach/reinstall/restore
is not in flight, work is terminal, and traffic/job/write fences are proven.
Reset while installed is not purge. Native/static authoritative domain rows,
objects, and settings remain retained in the initial product; no generic native
domain purge is promised. A future native owner-specific deletion requires its
own accepted maintenance/recovery contract. Background GC deletes only
already-unreferenced physical
identities after tombstone, grace, and final recheck; neither finalization nor
elapsed time authorizes data purge. Finalization requires an accepted/converged candidate, explicit
rollback-window closure, completed backfills and invariants, no old nodes/work,
and expiry of the maximum declared client/cache lifetime since the last N
response while N assets remained retained. No incident/recovery/rollback or
unsatisfied retention/recovery/legal/audit condition may remain.

### Recovery Points

Database restoration is a separately authorized recovery operation and never
automatically overwrites live production data.

Dynamic artifact-data and artifact settings are separate purge/recovery
boundaries. An artifact-data snapshot cannot authorize settings deletion.
Dynamic artifact-settings purge
requires its own protected recovery point binding exact scope, stable data
owner/installation, settings revision, admitted schema digest, canonical
validated value, unresolved secret handles, and policy/retention evidence;
purge first commits a monotonic tombstone/revision. Restore requires that exact
tombstone, creates a new non-serving settings instance, and may bind it only to
an explicitly named non-retired inactive installation under the same stable
data owner. If the installation was retired, restore leaves the instance
unbound for a later continuity-authorized reinstall and never clears retirement
or the old tombstone. Role/actor grants and external secret bytes are neither
snapshotted nor implicitly purged.

A settings recovery point has its own monotonic retention revision, deadline,
legal/audit/incident holds, and `ready`, `collecting`, and `collected` states.
It roots encrypted ciphertext, exact KMS key version, schema/descriptor
digests, settings/data-owner lineage, policy, and audit receipt. Purge apply and
restore revalidate decryptability, target-schema compatibility, unresolved
secret handles, tombstone, and current holds; KMS rotation preserves or
explicitly rewraps a retained point. Collection persists its decision before
deleting bytes, resumes after crash, preserves audit facts, and makes later
restore ineligible. Purge/status explicitly reports the retained recovery-copy
expiry and holds.

Module-scoped recovery exists only for an explicit data ownership boundary
with a complete, bounded, tested snapshot/restore procedure. Artifact-data
restore retains its empty-target rule. Cross-module native data normally
requires platform PostgreSQL recovery.

After artifact-data purge, restore never clears or reuses the old namespace
tombstone. It creates and verifies a new isolated empty namespace instance
under the same stable data-owner identity, then uses a separately authorized
CAS cutover of the active namespace reference. The operation binds snapshot,
old tombstone, new instance, data owner, and cutover revision so crash replay
cannot expose two active namespaces or attach retained data by slug.

Recovery fences traffic/writes/workers, preserves the failed live state,
restores into an isolated or empty target, verifies identities, security,
domain invariants, objects, projections, outbox/offsets and external effects,
records measured RPO/RTO, and only then performs a separately authorized
cutover. The platform defines no generic merge into live data; any merge is an
owner-specific, tested, separately authorized recovery contract.

### Frontend Boundary

Every Leptos artifact of a selected static role, including rendering code and
any present CSR/hydrate JS, WASM, CSS, and browser assets, is part of the role
bundle and rollback evidence. Dynamic declarative UI, localization,
permissions, and bindings move with their admitted artifact.

Next.js build, deployment, health, and rollback remain optional, external, and
manual. The lifecycle neither operates nor observes an individual Next.js
deployment. Generic public/headless N/N+1 contract compatibility remains
backend preflight evidence and may deny automatic mode; Next.js cannot
authorize or claim lifecycle success.

## Consequences

- Every updateable module, including a stateless module, records a local
  release/data readiness block. Stateless modules still account for
  dependencies, durable work, contracts, health, and external effects.
- The central readiness board reports evidence but never grants production
  eligibility.
- The shared operator projection distinguishes selected from serving state,
  shows the rollback unit and blast radius, update mode/reason, observation and
  rollback windows, migration/point-of-no-return facts, fence state,
  eligibility/denial reasons, recovery progress, and sanitized diagnostics.
- Preparation rejection is distinct from a production transition, and every
  terminal result reports target/predecessor, code/serving outcome, database
  outcome, retained changes, and whether incident reconciliation is terminal.
- Automatic recovery succeeds only after the direct predecessor is observed
  healthy and every required durable-work/external-effect reconciliation is
  terminal. A pointer write, queued build, process launch, or restored traffic
  with unresolved reconciliation is intermediate.
- Manual rollback uses a fresh executable decision, direct-predecessor rule,
  fence set, and convergence definition. Its window begins at candidate
  acceptance, after retention started before rollout, and closes at
  finalization or the next converged update; current configuration, security,
  dependency, data, migration, and retention facts can make it ineligible
  sooner.
- Full logs remain protected, tenant-isolated, bounded, redacted, and separate
  from typed owner receipts and fixed-cardinality metrics.
- Verification includes mixed N/N+1 reads/writes and durable work, process loss
  at every phase, concurrency and security races, external-outage
  non-triggering, retention/GC holds, outside-candidate static recovery,
  operations-tool predecessor/protocol recovery, structured-plus-object data
  denial/migration, irreversible-gate races, tenant isolation, finalization
  denial, and measured restore drills.

## Related Documents

- [Module Release and Rollback Plan](../docs/modules/module-release-rollback-plan.md)
- [Module artifact rollback boundary](./2026-07-13-module-artifact-rollback-boundary.md)
- [Static promotion review boundary](./2026-07-22-static-promotion-review-boundary.md)
- [Installer topology composition identity](./2026-07-12-installer-topology-composition-identity.md)
- [Platform-owned OCI registry transport boundary](./2026-08-06-oci-registry-transport-boundary.md)
- [Durable artifact-data snapshot and guarded restore](./2026-07-22-artifact-data-snapshot-restore.md)
- [Artifact security state boundary](./2026-07-22-artifact-security-state-boundary.md)
- [Shared owner-operation receipt ledger](./2026-08-03-owner-operation-receipts.md)
- [Neutral sandbox foundation](./2026-07-11-neutral-sandbox-foundation.md)

# rustok-rbac

## Purpose

`rustok-rbac` owns the tenant-scoped policy authorization runtime for RusToK.

## Cache consistency failures

Generation-aware cached permission resolution discards relation reads rejected by
every bounded publication attempt and returns the typed
`PermissionCacheResolutionError::Invalidated`. It does not grant those permissions
or fabricate an empty successful grant set. Store errors remain distinct. This
local check is not a commit-time revocation fence or a guarantee for cache adapters using default non-generation-aware methods.
See [the cache-consistency decision](../../../DECISIONS/2026-11-02-rbac-cache-resolution-consistency.md).

The persisted SeaORM reader resolves the entire tenant/user membership and
static-grant graph in one SQL statement, for both cache misses and uncached
authorization. A statement snapshot prevents mixed membership/grant reads; it
does not fence subsequent revocation. A cached candidate is validated against
the exact persisted permission pairs in a fresh statement before returning a
hit, followed by a local token recheck. This handles direct SQL and missed
notifications without adding a global write lock. Cache hits still incur a DB
query; a mismatch triggers the typed relation read and conditional publication.
The durable invalidation generation remains the owner mutation/recovery
watermark, not a complete raw-DML change feed.

## Responsibilities

- Provide `RbacModule` metadata for the runtime registry.
- Resolve effective permissions from relation data.
- Own persisted relation reads and tenant membership deletion; server adapters
  delegate mutations inside the host transaction.
- Own transaction-backed role-mutation fact collection, exact assignment checks,
  and active-super-administrator continuity for role/status/account changes.
- Evaluate permission checks through the single live policy engine.
- Publish the typed `settings:*` and `logs:*` platform-admin surface used by server adapters.
- Own explicit tenant-role grants for immutable artifact permission keys without
  extending the static `Permission` enum or writing `role_permissions`.
- Own the RBAC relation-integrity, durable invalidation-generation and user
  membership revision migrations. The membership revision is distinct from the
  cache generation.

## Interactions

- Depends on `rustok-core` for permission vocabulary and module contracts.
- Used by `apps/server` through `RbacService`, RBAC extractors, and permission-aware
  `SecurityContext` creation.
- Owns its GraphQL role query/mutation/types under `rustok_rbac::graphql`; `apps/server`
  only composes those roots and provides a role-writer adapter to the runtime persistence layer.
- Exposes a module-owned Leptos admin overview through `rustok-rbac-admin`.
- Other runtime modules do not need a direct dependency on `rustok-rbac`; they publish typed
  permissions via `rustok-core`, and server transport layers enforce them through this module.
- Manual role-based authorization in `apps/server` is not part of the live contract.

## Entry points

- `RbacModule`
- `RuntimePermissionResolver`
- `SeaOrmRelationPermissionStore`
- `authorize_current_permission`
- `resolve_persisted_permissions_on`
- `PermissionResolver`
- `authorize_permission`
- `authorize_any_permission`
- `authorize_all_permissions`
- `has_effective_permission_in_set`
- `RbacArtifactPermissionAssignmentService`
- `SeaOrmArtifactPermissionAuthorizer`
- `graphql::RbacQuery` / `graphql::RbacMutation` (feature `graphql`)

## Docs

- [CRATE_API.md](CRATE_API.md)
- [Module docs](./docs/README.md)
- [Platform docs index](../../../docs/index.md)

## Authorization model (canonical)

- The persisted relation graph (`roles`, `permissions`, `user_roles`,
  `role_permissions`) is the only source of truth for grants. Static built-in
  role permission sets in `rustok-core` are bootstrap defaults for new tenants
  only; after seeding, tenant grant changes are legitimate persisted state.
  System-role repair (an explicit operator action) restores missing default
  roles with their bootstrap grants and never rewrites an existing role's
  grants, including when a concurrent initializer wins the missing-role
  insert. Only a successfully inserted role is seeded; single-role vocabulary
  materialization uses the same ordered (resource, action) keys as control-plane
  grant writes. Repair is not part of the normal request flow, so roles deleted through
  the control plane stay deleted. GraphQL and native role overview use the same
  read-only owner query; listing an empty tenant never runs repair. Role rows,
  tenant-scoped grants and localized display names share one SQL statement.
  Both transports order system roles first, then by slug.
- The decision engine is the platform's own set-based evaluator over the
  resolved permission set (see
  `DECISIONS/2026-02-26-rbac-relation-source-of-truth-cutover.md`).
- Default roles (`super_admin`, `admin`, `manager`, `customer`) are seeded
  as database records per tenant from the bootstrap defaults in `rustok-core`
  and are thereafter owned by the database. Only the super administrator
  role can never be deleted (break-glass account protection; its grant set
  is also immutable through the GraphQL control plane). Every other role —
  the seeded defaults and custom roles alike — can be renamed, have its
  grants edited within the actor's authority ceiling, and be deleted while
  it is not assigned to any user. Role mutations validate every grant
  against the canonical permission vocabulary and the actor's own
  effective authority ceiling.

## GraphQL role-grant invalidation

A supplied `updateRole.permissions` replacement advances the durable RBAC
permission generation in the same transaction as the grant write. After commit,
a generation-aware host callback supports cache clearing and best-effort fan-out;
its compatibility default calls the legacy membership callback. Recovery is
eventual and depends on healthy generation watchers; this is not a transactional
outbox or a revocation fence. Metadata-only edits do not advance the generation.
The reservation is a late transaction-held row write returning its exact
successor and rejecting invalid or exhausted integer storage. PostgreSQL
reserves its table and singleton with NOWAIT: contention aborts the owner
transaction instead of waiting below earlier domain locks. Driver errors remain
typed for transaction owners. The standalone committed replacement retries
only classified pre-commit failures after explicit rollback; it is not a
request credential re-admission protocol. Never acquire
new earlier-ranked tenant/credential/user/role/grant/receipt identities after
reserving it. Bulk lock-order verification remains open.
[ADR](../../../DECISIONS/2026-11-02-rbac-graphql-grant-invalidation.md).

## Module availability (dormant grants)

Modules own slices of the permission vocabulary; roles are tenant records
that mix grants across modules. Disabling a module for a tenant never
deletes or hides roles or their grants — the module's grants become
*dormant*:

- **Enforcement:** the host strips dormant grants from the request
  permission snapshot at authentication time (`PermissionAvailability`), so
  disabling a module immediately revokes its capabilities everywhere, while
  the persisted grants reactivate unchanged when the module is re-enabled.
  Platform-core permissions (no registered owner module) are always active.
- **Control plane:** `platformPermissions` annotates every permission with
  `moduleSlug` / `moduleStatus` (`core` / `enabled` / `disabled`), and
  `RoleInfo.inactivePermissions` lists a role's dormant grants so operators
  see the state instead of discovering it. The admin UI renders dormant
  grants greyed out.
- **Mutation guard:** creating a role with dormant grants, or adding a new
  dormant grant to an existing role, is rejected. Dormant grants already on
  a role are preserved across saves — removing them is an explicit operator
  decision, never a side effect of saving.
- Hosts that register no `RbacModuleAvailabilityProvider` treat every
  permission as platform-core; availability resolution failures on the
  authentication path fail closed.

## Known Limitations / Pending Implementation

The GraphQL role-assignment adapter binds the exact admitted direct-session
credential to the typed Auth user-administration context. The host
corroborates it with the valid request scope; grant-only production
administration is denied. Role
definition create/update/delete and the host REST artifact grant/revoke
routes now require the host transaction authority port on the RBAC owner
transaction. Lock the direct session, current static module policy and
the actor/target role union before evaluating live grants; require the
admitted permission ceiling and a final credential deadline before commit.
The returned locked permission set is the effective intersection of admitted
and live grants (including manage wildcard narrowing). At least one identical
management alternative must survive both checks; a newly acquired alternative
cannot replace a revoked one. The GraphQL owner also applies its admitted
ceiling to the adapter result. Missing host authority denies writes. Artifact operations retain their
exact registered scope, idempotency receipt and transactional event. Every
actual artifact grant or revoke advances the existing durable RBAC generation
in the same transaction; exact retries and state no-ops do not. Missing,
negative or exhausted generation state denies changes. The owner protects
actor identity before roles on PostgreSQL and retains the exact immutable
permission definition through commit. This
does not establish artifact-runtime entitlement or global inter-module
lock ordering; compile and contention evidence remain pending.

- **Role assignment replaces all tenant memberships with a single role.**
  `assign_user_role` accepts a built-in role enum (`role`) or any
  tenant-persisted role slug (`roleSlug`), including custom roles. Built-in
  assignments enforce the canonical role hierarchy; custom-role assignments
  have no platform rank, so the owner requires every grant of the custom
  role to fit inside the actor's own effective permissions (authority
  ceiling), plus target-management and last-super-administrator continuity
  rules. Multi-role membership is not exposed as an assignment model: the
  read path unions permissions across all persisted memberships, but
  assignment always converges the user to exactly one tenant role.
- **`BuiltinTenantRbacCatalog` publishes platform defaults, not tenant
  state.** The synchronous `TenantRbacCatalog` contract cannot read the
  database, so it exposes the built-in role vocabulary. Tenant-persisted
  roles (including custom roles) are available through the GraphQL `roles`
  query. A database-backed tenant catalog is pending.
- **`SecurityContext::system()` is a trusted-runtime-only constructor.** It
  carries the static super-admin bootstrap set and must only be built by
  platform composition code, never from request data.

### Read-only static authority revisions

The owner exposes a single-statement membership/role-grant revision snapshot
for admission checks that span multiple relation reads. This is an optimistic
change detector, not an acquisition of the role/user mutation locks or an
authorization decision. Missing/invalid epochs fail closed; membership and
role-grant ABA remain visible. Artifact grants and module policy are separate
authority domains.

### Standalone role writer ordering

PostgreSQL standalone replacement takes the target user fence, then the
existing target membership, destination and SuperAdmin continuity roles in
canonical UUID order before per-role checks. Bootstrap assignment fences
the user and exact system role before touching shared permission vocabulary;
a late role conflict fails via NOWAIT. Lazy missing-role creation and bulk
repair remain outside a complete cross-owner ordering proof. PostgreSQL
regression evidence is authored but unrun.

### User administration role lock set

The host user-admin transaction can lock actor and target membership roles,
requested destination and existing SuperAdmin continuity role in one
UUID-ordered owner query. Existing users must be locked first; SQLite callers
must hold the write fence. The result indicates whether the requested role
exists in the locked set. This is not itself authorization and does not
establish a universal lock order for all RBAC or artifact writers.

Migration 000012 protects the permanent RBAC cache watermark from direct SQL
insertion, deletion, replacement and non-increasing or non-integer updates.
Normal reservations advance by one; deliberate forward-only recovery may skip
values. Missing/corrupt predecessor storage requires offline recovery.
Privileged DDL and database restoration remain separate. Tests unrun.

### Permanent static authority identities

Migration 000013 backfills and retains an append-only UUID witness for each
existing user, role and permission and for every subsequent successful insert.
A deleted UUID cannot be reused, including SQLite same-ID REPLACE and alternate
text/blob UUID encodings; UUID updates are forbidden. Conflict-do-nothing and
rolled-back births consume nothing. Witnesses contain only kind and UUID, no
email, role slug or permission payload, and have no cascading foreign keys.
The SQLite witness table uses WITHOUT ROWID, preventing replacement through
an unrelated hidden row identity. PostgreSQL uses a nonwaiting per-identity
advisory fence for witness inserts.
The migration cannot reconstruct identities deleted before cutover, and does
not protect privileged DDL/restore, other authority kinds, or SQLite
natural-key REPLACE with a different UUID. Source regressions are unrun.

Artifact catalog admission orders permission keys and normalized localization
keys; projection reads the corresponding persisted keys in order. Migration
000016 rejects late PostgreSQL catalog FK parent waits and adds nonwaiting
relation admission to artifact grant/receipt guards. SQLite retains its
serialized writer and existing FKs. PostgreSQL regression authored, unrun.

Migration 000017 couples actual artifact grant SQL changes to the existing
RBAC watermark, including direct INSERT/DELETE and changed grant identity.
PostgreSQL defers advancement to commit and rejects late role or watermark
contention without waiting; SQLite advances under its writer reservation.
The owner no longer explicitly reserves a second generation for artifact
assignment. Cutover advances once to invalidate older continuity receipts.
Static direct relation generation coupling and runtime verification remain open.

### Tenant-bound role labels and administrator continuity

Persisted role membership reads join the subject's tenant and the role's
tenant, excluding malformed cross-tenant links. The persisted role planner
distinguishes effective hierarchy rank from actual canonical system
`super_admin` membership: only the latter participates in last-active-admin
continuity. Custom-role replacement and status/removal checks use the same
membership predicate. Existing hierarchy and permission-ceiling checks remain.
Source regressions cover a malformed foreign membership and the distinction
between inferred rank and canonical membership; compilation and execution
remain pending.

### Administrator continuity under snapshot isolation (source only)

The continuity role is now a write fence, not just a row lock: the canonical
super_admin row receives an exact no-op update before counting remaining
active members. PostgreSQL REPEATABLE READ/SERIALIZABLE transactions whose
snapshot predates a committed continuity check cannot both authorize removals
from the old active set; serialization failure requires whole-transaction
retry. The no-op does not change grant_revision or the role's timestamp.
SQLite retains its existing writer fence. Persisted role planners and the
standalone status/removal continuity boundary reserve the existing target
membership, destination and continuity roles in canonical UUID order before
reading hierarchy or membership facts. Missing custom destination fails
closed. Callers must retain the transaction through the effect. Raw user
status DML bypassing the owner, privileged database maintenance, and complete
cross-owner lock ordering are not covered by this change. PostgreSQL
regression source authored; compilation and execution remain pending.

### Continuous static module-policy observation

Composed JWT admissions retain the owner-issued tenant policy identity,
composition revision and availability alongside the original grant ceiling.
Scope-aware async checks re-observe those facts without the TTL cache;
drift or unavailable evidence terminally invalidates the matching scope.
Final admission and existing WebSocket lease revalidation do not bootstrap
missing composition. This is observation-based, not a commit-time fence;
undetected tenant lifecycle ABA, raw-DML continuity and sync-only consumers
remain open. Source regression authored; compilation and execution pending.

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

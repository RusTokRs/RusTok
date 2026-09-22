# Canonical native module reference contract

- Date: 2026-09-18
- Decision status: Accepted
- Implementation status: Blog reference v1 source-certified; maintainer execution evidence remains pending
- Owners: platform architecture / module owners
- Extends: [Canonical native module source layout](./2026-09-18-canonical-native-module-source-layout.md)
- Extends: [Multilingual DB storage via parallel localized records](./2026-04-05-multilingual-db-storage-parallel-localized-records.md)
- Extends: [Truthful locale provenance](./2026-07-21-language-agnostic-legacy-locale-provenance.md)

## Context

A common directory layout is not enough to make a module safe to copy. During the
first strict audit of `rustok-blog`, several defects existed behind an otherwise
clean physical layout: fallback locale leaking into writes, ambiguous nullable
patches, optional optimistic concurrency, a domain state machine bypassed by the
production service, derived counters changing business revision, transport error
leakage, tenant override asymmetry, and integrations duplicating owner persistence
policy.

Because the Blog module is the reference used to migrate the rest of the
repository, these are platform architecture concerns rather than Blog-only bugs.

## Decision

A module may be called a **canonical native reference module** only when both its
physical layout and these semantic invariants are machine-guarded.

### Owner and persistence boundary

- Persistence entities are private implementation details. Other crates consume
  owner DTOs, services, or explicit ports.
- `lib.rs` is a facade; runtime composition stays in `module.rs`.
- Integrations adapt owner services/ports. They do not query owner entities to
  recreate business policy.

### Tenant authority

The request/runtime `TenantContext` is authoritative. Optional tenant arguments
may confirm the current tenant but may never switch it. Authenticated actor tenant
and current tenant must match on both read and write paths.

Authorization must use the owner module's registered permission resource consistently.
For Blog post owner state that resource is `BlogPosts`; generic `Posts` authority must
not silently replace or supplement it on privileged reads.

### Multilingual provenance

- Base rows remain language-agnostic.
- Localized copy lives in parallel localized rows.
- Runtime fallback is read policy only; it never supplies write provenance.
- Creating a new locale requires that locale's own required localized fields.
  Text from another locale must never be copied and relabeled.
- UI translation bundles such as `en`/`ru` describe bundled interface copy,
  not the set of content locales tenants may store.

### Mutation and concurrency semantics

- Nullable patches use explicit `Keep / Set / Clear` semantics.
- If an owner aggregate exposes a revision/version, mutable edit commands require
  the predecessor revision. CAS is not optional.
- A public input field must have observable owner behavior. Ignored compatibility
  fields are forbidden.
- Arbitrary metadata must not shadow canonical typed state.

### Lifecycle

The domain transition table is the single lifecycle policy. Persistence-backed
commands must consult that same policy, and transports/UI expose only valid
commands. Restore is explicit rather than disguised as another transition.

### Derived state and events

Derived projections (counts, summaries, caches) do not change the owner's
business revision or business `updated_at` unless the derived state is itself
defined as part of that aggregate revision. Locale-neutral derived changes use
locale-neutral refresh/reindex signals.

### Public errors

Owner errors keep internal context, but every public transport maps them through
one owner-controlled safe descriptor. Database, connector, host and SQL details
must not be rendered through `Display` at HTTP, GraphQL, or native UI boundaries.

Persisted-state decoding failures, missing canonical cross-owner projections, broken
host composition and unsupported storage/runtime state are internal invariants, not
client validation. Typed dependency boundaries preserve semantic error classes such
as not-found, conflict, forbidden, validation and internal failure instead of
collapsing them into a generic 400 response.

Native server-function adapters resolve host context fallibly and use the same safe
owner descriptor; they must not panic on missing composition state or expose runtime
type/dependency names.

### Reads and pagination

- Violated storage invariants fail closed; readers do not manufacture empty valid
  objects.
- Canonical cross-owner projections are part of the storage contract: missing or
  duplicate canonical identities fail as internal invariants rather than validation.
- Offset pagination uses deterministic ordering with a stable unique tie-breaker.
- Sort/filter fields are typed or validated; unknown values are not silently
  reinterpreted.

### FBA boundaries

- FBA status describes evidence actually obtained. Static boundary evidence does
  not imply `transport_verified`.
- Retryable write commands carry stable logical command identity/idempotency
  across retries.
- Provider errors remain typed across the port boundary.
- Degraded/fallback behavior is explicitly described and separately evidenced.

### Module-owned UI adapter profile

Module-owned `admin/` and `storefront/` crates are adapters, not second domain
owners. Their canonical responsibilities are:

- `model`: transport-neutral UI DTOs;
- `core/`: UI commands/state/presentation policy with tests;
- `transport/`: GraphQL/native adapters over the same owner contract;
- `ui/`: render/controller code and reusable components;
- `i18n`: interface-copy lookup only;
- thin `lib.rs`: package facade/composition.

For the Blog reference profile, Rust UI source files are capped at 40 KiB. Large
files split by commands, presentation, components, tests, or transport
responsibility rather than arbitrary line ranges.

## Reference implementation

`crates/modules/rustok-blog` is reference v1. Its backend source layout is
guarded by `npm run verify:module-source-layout`; its semantic invariants are
guarded by `npm run verify:module-reference-contract`.

The final post-#4072 source audit classifies Blog reference v1 as source-certified.
That certification does not claim maintainer-owned compile/runtime evidence and does
not change any FBA evidence level.

A module must not be migrated by mechanically copying Blog files. It should copy
the responsibility vocabulary and invariants, then implement its own owner
semantics.

## FBA status of the reference

Blog Comments remains `boundary_ready`. This ADR does not promote it to
`transport_verified`; compiled/live remote transport and fallback evidence are
still required for that promotion.

## Rollout

1. Make Blog green under both reference guards.
2. Keep its docs/evidence synchronized with production behavior.
3. Migrate Product as the large-owner reference.
4. Migrate Pages, Forum, Taxonomy and remaining native modules in bounded slices.
5. Enroll each migrated module into the same guards after conformance.

Any defect found in the Blog reference blocks propagation until either corrected
or explicitly resolved by a superseding architecture decision.

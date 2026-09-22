# Canonical native module source layout

- Date: 2026-09-18
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform architecture / module owners
- Extends: [Categorized workspace layout for crates](./2026-09-06-crates-workspace-layout-split.md)
- Supersedes: None
- Superseded by: None

## Context

RusToK already defines semantic module ownership and backend boundaries, but native
module crates have accumulated different physical source shapes. Some modules are
mostly flat, others organize around transport names, and large owners can grow
single service files that become shared merge-conflict hotspots. That drift makes
navigation, review, automated agent work, and future extraction harder even when
runtime ownership is otherwise correct.

The repository already has canonical module-authoring and backend guides. This
decision standardizes their physical source vocabulary instead of introducing a
parallel architecture document.

## Decision

Native platform modules use one canonical responsibility-oriented source layout.
Only responsibilities that actually exist are materialized; empty scaffolding is
forbidden.

The canonical slots are:

- `src/lib.rs`: crate documentation, module declarations, and deliberate public
  re-exports only;
- `src/module.rs`: `RusToKModule`, `MigrationSource`, runtime-extension
  registration, event-listener registration, and module metadata;
- `src/domain/`: transport- and persistence-independent state machines, value
  objects, validation policy, and domain invariants;
- `src/dto/`: owner-facing request/response and command/query data contracts;
- `src/entities/`: owner persistence mappings;
- `src/services/`: application/domain use cases and transactional orchestration;
- `src/ports/`: owner-defined cross-boundary ports when a real consumer exists;
- `src/integrations/`: adapters to capabilities owned elsewhere, such as SEO,
  reactions, snapshots, indexing, or translation;
- `src/graphql/`: owner GraphQL adapters;
- `src/controllers/` (or `src/rest/`): owner HTTP adapters;
- `src/migrations/`: owner migrations and ordering descriptors;
- `src/runtime.rs`: optional narrow reusable runtime state when the module has one;
- `src/tests/`: crate-internal contract tests that need crate-private access;
  integration tests that only use the public API remain in top-level `tests/`.

A responsibility slot may become a directory of feature submodules as it grows.
Large files are split by stable responsibility rather than by arbitrary line
ranges. The initial canonical reference profile treats 32 KiB as a hard source-file
ceiling; an exception requires an explicit verifier exception and documented
architectural rationale.

`rustok-blog` is the first canonical native module reference implementation.
Its public API remains stable through deliberate re-exports even when physical
source files move.

Shared/support library crates follow the same principles—thin `lib.rs`, explicit
models/types, services/runtime, ports, and adapters—but they do not invent
`module.rs`, migrations, manifests, or runtime registration when those
responsibilities do not exist.

The standalone WASI Component Model template in `rustok-module-template` is not a
native server-module template and is not changed by this decision.

## Sources of truth and ownership

- This ADR owns the physical-layout decision.
- `docs/backend/module-backend-implementation.md` is the canonical implementation
  guide for the layout.
- `docs/modules/module-authoring.md` remains the canonical module-authoring entry
  point and links to the backend guide.
- Module-local README/docs own the live responsibility map for each module.
- `scripts/verify/verify-module-source-layout.mjs` is the machine guard for
  enrolled reference modules.
- Domain state and persistence ownership remain with the existing module owners;
  moving a file never transfers semantic ownership.

## Invariants

### Allowed states

- Small modules omit unused slots.
- Large responsibilities become nested submodules under the owning canonical slot.
- Public compatibility is preserved by explicit re-export when a physical move is
  not intended to change the owner contract.
- Adoption across existing modules is incremental; a module becomes strict once it
  is enrolled in the layout verifier.

### Forbidden states

- Business logic in `lib.rs`.
- Module/runtime registration logic outside `module.rs` for enrolled modules.
- Transport dependencies in `domain/`.
- Axum, GraphQL, Leptos, or `rustok-web` dependencies in application services.
- Capability adapters scattered at the crate root.
- Generic root dumping grounds such as `common.rs`, `misc.rs`, or `utils.rs`
  that combine unrelated ownership.
- Empty directories created only to resemble the reference tree.
- Treating the standalone WASI module template as the native module source template.

## Non-goals

This decision does not change runtime module taxonomy, persisted schemas, API wire
contracts, FFA/FBA readiness, or cross-module ownership. It does not require a
repository-wide mechanical move in one PR.

## Data, transaction, and concurrency boundary

Not applicable. Physical source placement does not change data, transaction,
revision, idempotency, or destructive-operation semantics. Refactors adopting the
layout must preserve the existing owner transaction boundaries.

## Context dimensions

Tenant, channel, locale, principal/auth, policy, trace, timezone, currency, and
other runtime dimensions are unchanged. Their typed contracts remain owned by the
existing module and platform boundaries.

## Events and projections

Event/outbox and projection ownership is unchanged. Moving event listeners or
integration adapters into canonical slots must not alter event identity, delivery,
rebuild, or reconciliation semantics.

## Failure semantics

Layout verification fails closed for enrolled reference modules. A violation must
be fixed by restoring the canonical placement or by adding a narrowly documented
exception; silently weakening the verifier is not an accepted migration path.

## Migration and cutover

Adoption is incremental:

1. accept the physical-layout vocabulary;
2. make `rustok-blog` the first reference implementation;
3. enforce Blog with the source-layout verifier;
4. migrate other modules from fresh `main` in bounded mechanical PRs;
5. enroll each module in the verifier only after its source layout conforms.

Physical moves must preserve public contracts unless a separate architectural
decision explicitly changes them.

## Alternatives considered

- **Keep per-module conventions.** Rejected because it leaves navigation and
  review behavior dependent on historical accident and agent preference.
- **Force every slot into every crate.** Rejected because empty layers are
  speculative scaffolding and obscure real ownership.
- **Split responsibilities into many crates immediately.** Rejected because
  physical organization does not justify new dependency/runtime ownership
  boundaries.
- **Create a second module-standard document.** Rejected because the repository
  already has canonical backend and module-authoring guides.

## Verification

For the initial reference implementation:

- `npm run verify:module-source-layout`;
- `cargo xtask module validate blog`;
- `cargo check -p rustok-blog`;
- `cargo test -p rustok-blog --lib`;
- existing Blog boundary verifiers appropriate to touched adapters.

The layout verifier checks root cleanliness, required Blog source slots,
`lib.rs`/ `module.rs` separation, a bounded Rust file size, and forbidden
transport/persistence dependencies in lower layers.

## Consequences

Developers and agents get one predictable vocabulary for native modules.
Large modules such as Product can be decomposed without creating new crates merely
for navigability. Existing modules require bounded follow-up refactors before
strict enrollment; this is intentional to avoid a repository-wide churn commit.

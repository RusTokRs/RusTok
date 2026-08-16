## Scope

Reactions ownership.

## Current state

Reactions module implementation.

## Milestones

1. Foundation

## Verification

- `cargo test -p rustok-reactions`

## Change rules

Standard rules.

---
id: doc://crates/rustok-reactions/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-reactions
last_reviewed: 2026-08-07
---

# `rustok-reactions` implementation plan

## Program ledger

| Task | Status | Deliverable |
| --- | --- | --- |
| `REACTIONS-00` | `in_progress` | Neutral API, optional owner module, distribution/server feature selection and provider registries are source-ready; maintainer Cargo/Node verification remains. |
| `REACTIONS-01` | `in_progress` | Tenant-composite owner schema, immutable catalog snapshots, actor uniqueness and shared Outbox command receipts are source-ready. Regenerate `Cargo.lock` and retain SQLite/PostgreSQL execution evidence. |
| `REACTIONS-02` | `in_progress` | Actor state, aggregate deltas, typed semantic events and completed shared Outbox receipts now share one transaction. Bounded inspect/repair reconciliation is source-ready; retain rollback, replay, concurrency and repair evidence. |
| `REACTIONS-03` | `in_progress` | Forum topic/reply provider, optional host materialization and executable composition profile tests are source-ready. Reactions stays outside defaults; retained execution evidence remains pending. |
| `REACTIONS-04` | `in_progress` | Blog `post` producer and Blog+Reactions composition profile are source-ready over Blog-owned publication, channel visibility and owner version. The neutral-contract review now covers Forum and Blog producers; retain Blog provider/host runtime evidence before freezing shared transport or presentation contracts. |
| `REACTIONS-05` | `in_progress` | Manifest-composed bounded GraphQL read/write transport, producer-neutral module-owned Leptos reaction-bar foundation, bounded Forum selected-topic/selected-reply host composition and Rust Playwright browser-evidence source are ready. Retain schema/runtime/UI/browser execution evidence and add thin Blog storefront composition before freezing the presentation contract; no HTTP transport is frozen by this slice. |
| `REACTIONS-06` | `planned` | Runtime evidence, FBA contracts, import/reconciliation and release profiles. |

## Ownership

Reactions owns catalog snapshots, actor state, shared-receipt-bound command
execution, aggregate projections, semantic reaction events and aggregate repair.
Producer modules own subject existence, current revision, visibility, lifecycle
and reaction policy. Profiles owns actor presentation. Reputation and
achievements consume semantic facts but do not mutate reaction state.
Notifications may consume future events but are not part of command correctness.

## Persistence invariants

- Every row is tenant-scoped.
- Subject identity is `(tenant, source, kind, subject UUID)`.
- Subject and catalog revisions are positive and monotonic.
- One catalog revision is immutably bound to one catalog payload.
- One actor state exists per tenant/subject/actor.
- Selection keys remain inside the authorized bounded catalog.
- Aggregate counts never become negative and change atomically with actor state.
- Command UUID equals the shared Outbox idempotency key.
- Producer authorization happens before owner storage access.
- Reactions never reads producer-private tables.

The initial catalog revision is the producer subject revision. Independent
catalog revisioning is a later explicit API/migration change.

## Semantic event boundary

`rustok-events::ReactionsEvent` defines two sealed schema-v1 facts:

- `reactions.actor_state.changed` records one committed actor-state transition,
  the resulting bounded selection and exact added/removed aggregate keys;
- `reactions.subject.reconciled` records one committed bounded aggregate repair
  and a bounded/truncated changed-key sample.

The admitted `owner_operation_receipts.id` is the exact typed envelope UUID. A
changed reaction command writes actor state, aggregate deltas, the semantic event
and completed shared receipt in one owner transaction. Event conflict or
unavailability aborts the transaction. Idempotent no-op commands and completed
receipt replays do not publish another event.

Payloads expose stable tenant-scoped identities through envelope/payload fields,
positive revisions and bounded reaction keys only. They do not expose producer
content, visibility denial reasons, profile presentation, claims, roles, locale,
channel or free-form repair diagnostics.

The typed family is registered in the central closed `ContractEventPayload` and
schema registry. The committed event-contract digest artifact must be regenerated
by the maintainer before `--locked` or release evidence is accepted.

## Bounded reconciliation boundary

`ReactionsService` exposes owner-only inspect and repair methods for one exact
subject. Both require tenant equality and `reactions:reconcile`; repair also
requires a non-nil command UUID equal to the write idempotency key.

The request supplies an explicit actor-state bound capped at 1,000. Aggregate
rows are hard-capped at 128 and reported issues at 64. Inspection is read-only.
Repair is admitted through the shared Outbox receipt ledger, serializes the owner
subject row and reconstructs `reaction_aggregates` only from valid persisted
actor selections under the immutable current catalog.

Repair fails closed when the current catalog is missing/corrupt or an actor state
has a non-positive revision, corrupt/duplicate selection, selection-limit
violation or key outside the current catalog. It never mutates actor selections,
rewrites catalog snapshots, reads producer-private tables or guesses producer
visibility/lifecycle policy. A clean repair is an idempotent receipt-only no-op;
a drift repair replaces only aggregate rows and publishes one transactional
`reactions.subject.reconciled` event.

## Forum producer boundary

Forum registers a neutral `ReactionSubjectProviderFactory` for `topic` and
`reply` without depending on the Reactions owner. The provider:

- validates exact tenant/source/kind identity;
- hides missing, deleted, lifecycle-denied and audience-denied subjects behind
  one unavailable result;
- checks existing Forum rich-audience visibility before returning revision
  conflicts;
- uses `latest captured Forum revision id + 1` as the current subject revision;
- allows only approved replies whose parent topic is currently open and visible;
- publishes one bounded single-selection `like` catalog for the initial contract;
- resolves delegated actors through the existing exact recipient-context port;
- does not read or reinterpret existing Forum vote state.

Forum remains fully usable when Reactions is absent. The neutral factory may be
registered without materializing a Reactions owner runtime.

## Blog producer boundary

Blog is the second real producer for the neutral contract. It registers a
`ReactionSubjectProviderFactory` for `post` while depending only on
`rustok-reactions-api`. The provider:

- validates exact tenant/source/kind identity;
- uses the Blog-owned positive `blog_posts.version` as subject revision;
- allows only owner-state `published` posts;
- reuses Blog's typed `blog_post_channel_visibility` relation and existing
  channel visibility helper before exposing a subject;
- returns the same unavailable result for missing, unpublished and
  channel-denied posts, and returns revision conflict only after visibility;
- publishes one bounded single-selection `like` catalog;
- does not persist actor reaction state, aggregates or presentation in Blog.

The Blog module dependency list does not add the Reactions owner. Blog remains
usable when Reactions is absent, while a selected Reactions owner can materialize
the Blog factory through the same generic host registry as Forum.

## Neutral-contract review

Forum and Blog now exercise the same producer SPI with materially different owner
models: Forum derives revision from captured topic/reply history and rich audience
facts, while Blog uses an explicit owner version plus publication/channel scope.
No API expansion was required for the second producer. This is the source-level
neutral-contract review gate for `REACTIONS-04`; retained Blog composition and
provider authorization execution evidence remains required before shared
transport or presentation contracts are frozen.

## Optional host composition boundary

`mod-reactions` is an explicit optional feature in `rustok-distribution` and
`rustok-server`. It registers `ReactionsModule`, contributes the owner migration
source and materializes the reaction subject registry only after host providers
have been composed. The optional distribution/server host materialization is
source-ready. The enabled/disabled runtime evidence now has executable test
source, but retained runs remain pending.

The executable host fails closed when the feature is selected but
`ReactionsModule` is absent from the supplied `ModuleRegistry`. When Forum is
also selected, materialization happens after Forum audience facts and exact
recipient-context providers exist, so the Forum factory cannot be built with a
broader or incomplete authority boundary. Blog needs no extra host authority
provider because its reaction scope is derived from Blog-owned publication,
channel visibility and version state.

Supported profiles are explicit:

- Forum without Reactions: Forum remains available; the neutral factory is not
  materialized into an owner registry.
- Reactions without Forum or Blog: the owner registry materializes with no
  producer source.
- Forum with Reactions: the registry materializes the `forum` source with
  `topic` and `reply` kinds.
- Blog with Reactions: the registry materializes the `blog` source with the
  `post` kind.
- Selected Reactions feature without `ReactionsModule`: startup fails with the
  stable owner-missing error before publishing a false runtime.

Neither `mod-forum` nor `mod-blog` implies `mod-reactions`, server defaults do
not select it and `modules.toml` keeps Reactions outside `default_enabled` and
outside default profiles.

## Bounded GraphQL transport boundary

`REACTIONS-05` starts with one manifest-composed GraphQL transport over the
existing neutral owner ports. The transport is deliberately narrow:

- `reactionSnapshot` accepts only source, kind, subject UUID and positive subject
  revision; tenant scope comes from `TenantContext`, never caller input;
- anonymous reads use a system transport actor with no actor-state request, so
  producer authorization resolves public visibility only;
- authenticated human reads derive actor state from the trusted `AuthContext`;
  service principals do not impersonate a user through GraphQL read input;
- `applyReaction` requires a human-user principal and derives the actor UUID from
  `AuthContext`; there is no caller-supplied actor identity;
- the mutation command UUID is also the `PortContext` idempotency key, preserving
  the owner receipt invariant without a transport-local receipt store;
- producer source/kind/revision checks, catalog authorization, actor state,
  aggregate mutation and event/receipt atomicity remain inside
  `ReactionsService` and producer providers rather than GraphQL resolvers;
- subject revisions, actor-state revisions and aggregate counts are rendered as
  decimal strings so GraphQL integer width cannot truncate owner `u64` values;
- the mutation returns only the stable command UUID and `changed` flag; clients
  may issue the read query for canonical post-write state instead of creating a
  second transport-owned representation.

The manifest declares query, mutation and a runtime-data factory. The factory
fails closed unless the host has already materialized
`Arc<ReactionSubjectRegistry>`, then constructs `ReactionsService` from the host
DB plus that registry. `rustok-server` enables the crate `graphql` feature only
when `mod-reactions` is selected. This slice adds no producer-table reads and no
HTTP-specific policy. The GraphQL contract remains source-ready rather than
frozen until maintainer schema/runtime evidence is retained alongside the
Forum+Blog provider evidence.

## Storefront presentation foundation

`rustok-reactions-storefront` is the first module-owned presentation slice over
the neutral GraphQL transport. It is a reusable `ReactionBar`, not a standalone
route and not a producer-specific persistence adapter.

The UI accepts only an exact positive revisioned subject reference consisting of
source, kind, subject UUID and subject revision. It never discovers or guesses a
Forum/Blog revision and therefore cannot bypass producer authorization. Tenant
and actor identity remain outside component/GraphQL input and come from the
trusted UI auth context. Anonymous snapshots are read-only because the owner
returns no actor state; each authenticated click creates a fresh command UUID,
then reloads `reactionSnapshot` after success instead of maintaining a shadow
aggregate or actor-state projection.

The package intentionally has no dependency on `rustok-reactions`,
`rustok-forum` or `rustok-blog`. `apps/storefront` now owns the Forum cross-module
composition and imports both the neutral Forum storefront facts and the separate
Reactions presentation package. The Forum host path composes the selected topic
or, when a valid explicit `reply` query is present on that topic route, the one
selected reply. It calls only Forum's generic dual-path current-revision facade,
constructs `ReactionSubjectUiRef` in the host and mounts at most one
`ReactionBar`; it never asks for every visible reply revision and never moves
catalog, actor state, aggregate or command behavior into Forum. Blog storefront
composition remains pending. The source guards are
`scripts/verify/verify-reactions-storefront-ui.mjs`,
`scripts/verify/verify-forum-topic-reactions-storefront-composition.mjs` and
`scripts/verify/verify-forum-reply-reactions-storefront-composition.mjs`.

The Forum host composition now also has browser-evidence source in
`tests/e2e-rust/tests/leptos_storefront_forum_reactions.rs`, backed by
`crates/rustok-forum/contracts/forum-reactions-storefront-browser-evidence.json`
and `scripts/verify/verify-forum-reactions-storefront-browser-evidence.mjs`.
Maintainer-supplied canonical topic and selected-reply URLs are observed through
the existing Rust `playwright-rs` E2E crate. The topic document must mount only
the topic host-composition marker and the explicit reply selection must mount
only the reply marker. The harness does not seed producer state, call Reactions
transport directly or claim execution merely because the source exists.

## Executable composition evidence

`apps/server/tests/reactions_composition_profiles.rs` provides executable
composition profile tests over the public server host-composition entrypoint and
an isolated SQLite in-memory connection. The target has source coverage for the
three established Forum/Reactions optional profiles, Blog+Reactions producer
materialization and the selected-feature/missing-owner failure. It does not query
Forum or Blog domain rows or execute reaction commands.

The Blog producer also adds source-level provider and contract evidence in
`crates/rustok-blog/contracts/blog-reaction-subject-provider.json` and
`scripts/verify/verify-blog-reaction-subject-provider.mjs`. Retained execution of
the Blog+Reactions host profile and Blog authorization behavior remains part of
`REACTIONS-04` completion.

The contract remains `source_ready_maintainer_execution_pending`: retained
execution evidence remains pending until a maintainer runs and stores the
profiles and browser harness. Source presence alone does not promote
`REACTIONS-03`, `REACTIONS-04` or `REACTIONS-05` to done.

## Immediate next action

Regenerate `Cargo.lock` and the `rustok-events` digest artifact, then retain
SQLite/PostgreSQL evidence covering changed/no-op/replay event cardinality,
rollback on event failure, concurrent actor updates, clean/blocked/drift
reconciliation and receipt replay. Retain Blog provider authorization and
Blog+Reactions host composition evidence. Retain manifest-composed GraphQL schema
and runtime evidence for anonymous/authenticated reads, human-user writes,
tenant mismatch, idempotent replay and stale/denied subjects. Retain the Reactions
storefront package and bounded Forum selected-topic/selected-reply host runtime
evidence, execute and retain the Rust Playwright browser harness, then add thin
Blog storefront composition that supplies its exact authorized producer version
without moving Blog storage or presentation ownership into Reactions.

Before release, execute SQLite and PostgreSQL migrations, retain replay,
concurrency and rollback evidence, and retain bounded repair evidence for catalog
and aggregate drift.

## Verification

```bash
cargo test -p rustok-events reactions
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo test -p rustok-reactions --features graphql graphql
cargo test -p rustok-reactions-storefront
cargo test -p rustok-forum reaction_subject
cargo test -p rustok-blog reaction_subject
cargo test -p rustok-e2e-rust --test leptos_storefront_forum_reactions -- --nocapture
cargo check -p rustok-reactions-api --all-targets
cargo check -p rustok-reactions --all-targets
cargo check -p rustok-reactions --features graphql --all-targets
cargo check -p rustok-reactions-storefront --all-targets
cargo check -p rustok-reactions-storefront --features hydrate --all-targets
cargo check -p rustok-forum --all-targets
cargo check -p rustok-blog --all-targets
cargo check -p rustok-distribution --features "mod-forum mod-reactions"
cargo check -p rustok-server --no-default-features --features mod-reactions
cargo test -p rustok-server --no-default-features --features mod-forum --test reactions_composition_profiles forum_without_reactions_keeps_forum_host_composition_available
cargo test -p rustok-server --no-default-features --features mod-reactions --test reactions_composition_profiles reactions_without_forum_materializes_an_empty_subject_registry
cargo test -p rustok-server --no-default-features --features mod-reactions --test reactions_composition_profiles selected_reactions_feature_fails_when_owner_module_is_missing
cargo test -p rustok-server --no-default-features --features "mod-forum mod-reactions" --test reactions_composition_profiles forum_with_reactions_materializes_topic_and_reply_provider
cargo test -p rustok-server --no-default-features --features "mod-blog mod-reactions" --test reactions_composition_profiles blog_with_reactions_materializes_post_provider
cargo run -p rustok-events --example event_contract_digests -- --write
node scripts/verify/verify-reactions-foundation.mjs
node scripts/verify/verify-reactions-owner-persistence.mjs
node scripts/verify/verify-forum-reaction-subject-provider.mjs
node scripts/verify/verify-blog-reaction-subject-provider.mjs
node scripts/verify/verify-reactions-host-composition.mjs
node scripts/verify/verify-reactions-composition-profiles.mjs
node scripts/verify/verify-reactions-events-reconciliation.mjs
node scripts/verify/verify-reactions-storefront-ui.mjs
node scripts/verify/verify-forum-topic-reactions-storefront-composition.mjs
node scripts/verify/verify-forum-reply-reactions-storefront-composition.mjs
node scripts/verify/verify-forum-reactions-storefront-browser-evidence.mjs
git diff --check
```

Tests, checks, lockfile/digest generation and retained runtime/browser evidence are maintainer-run.

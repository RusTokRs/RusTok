# rustok-reactions-storefront / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA storefront UI components for the shared Reactions capability (`ReactionBar`).
- Integrates with `rustok-ui-core`, `rustok-ui-i18n`, and `rustok-graphql`.

## Domain Collaborations
- Collaborates with `rustok-reactions` GraphQL API for snapshots and reaction mutations.
- Collaborates with content producers (`rustok-blog`, `rustok-forum`) via `ReactionSubjectUiRef`.

## Public surface
- `pub mod i18n`: Project Fluent localization helpers.
- `pub mod model`: Public view models (`ReactionAction`, `ReactionSnapshotView`, `ReactionSubjectUiRef`).
- `pub mod transport`: GraphQL reaction transport adapter.
- `pub mod ui`: Leptos presentation components (`ReactionBar`).
- `pub use model::{ReactionAction, ReactionSnapshotView, ReactionSubjectUiRef}`
- `pub use ui::ReactionBar`

## Minimum Contract Set

### Input DTOs/Commands
- `ReactionSubjectUiRef`: Subject reference carrying source, kind, subject UUID, and positive owner revision.
- `ReactionAction`: Reaction mutation action (Add, Remove, Toggle).
- `ReactionSnapshotView`: Snapshot view model carrying reactions, counts, and active actor states.

### Domain Invariants
- Producer neutrality: does not depend directly on producer storage or internal entity state.
- Tenant and actor identity derived from trusted UI auth context, never accepted as variables.
- Write mutations generate fresh UUID command identity for owner idempotency.

### Events / Outbox Side Effects
- Storefront UI adapter; reactions mutations delegate event dispatch to `rustok-reactions`.

### Errors / Failure Codes
- Network and GraphQL execution errors are safely captured and do not crash UI rendering.

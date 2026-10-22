# rustok-reactions-storefront

Module-owned Leptos presentation for the shared `rustok-reactions` capability.

## Ownership boundary

This package is deliberately producer-neutral. It does not depend on
`rustok-reactions`, `rustok-forum`, `rustok-blog`, producer persistence, or any
producer-private table. A producer composition surface must pass an already
resolved exact `ReactionSubjectUiRef` containing source, kind, subject UUID and
the positive owner revision expected by the Reactions GraphQL contract.

The component never discovers or guesses a producer revision. In particular,
Forum and Blog remain authoritative for subject existence, lifecycle,
visibility and revision.

## Transport behavior

`ReactionBar` reads the canonical `reactionSnapshot` query and applies one
`applyReaction` mutation through the existing manifest-composed GraphQL
transport.

- tenant and authenticated actor identity are never accepted as component or
  GraphQL variables;
- auth token and tenant slug come from the trusted UI auth context;
- anonymous snapshots remain read-only because the owner returns no actor state;
- each click gets a fresh UUID command identity, which the GraphQL adapter also
  uses as the owner idempotency key;
- successful writes trigger a canonical snapshot reload instead of maintaining a
  shadow aggregate or actor-state model in the UI;
- owner error text is not rendered by this package;
- aggregate counts and revisions stay decimal strings end to end.

The initial Forum and Blog producers currently expose only `like`, but the UI
renders the bounded catalog returned by the owner and does not hard-code a
single storage model.

## Deliberate limits

This source slice does not mount the package into Forum or Blog, add a standalone
Reactions route, expose producer revisions through unrelated DTOs, add HTTP
transport, change Reactions storage, change existing Forum votes, or enable
Reactions by default. Producer composition is a later thin slice once each
surface has the exact authorized subject revision available at its public UI
boundary.

## Maintainer verification

No commands are executed by the implementation agent. Suggested checks:

```bash
cargo test -p rustok-reactions-storefront
cargo check -p rustok-reactions-storefront --all-targets
cargo check -p rustok-reactions-storefront --features hydrate --all-targets
node scripts/verify/verify-reactions-storefront-ui.mjs
git diff --check
```

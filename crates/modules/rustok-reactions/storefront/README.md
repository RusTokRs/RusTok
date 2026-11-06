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

## Producer composition and write settlement

Blog now composes this package from its owner-maintained discussion island and
passes the post detail's exact positive revision. Forum composition remains a
separate consumer boundary; this package never discovers producer identity.

`core.rs` owns subject validation, action/count presentation and asynchronous
write/read settlement. The interaction lock spans both the mutation and its
canonical read. A mutation failure may represent a write committed before a
network timeout, so it still triggers a read. If that read fails, stale controls
are removed and the read failure is shown. No failed mutation is automatically
retried and no local aggregate is incremented.

## Authentication lifecycle

Leptos reads the canonical reactive `AuthContext` session and user signals.
Each credential/tenant/principal binding owns a keyed render scope. Its pending
mutation continuation is cancelled with the Leptos scope, and a current-binding
fence runs before writes and around the follow-up read. A late response cannot
restore a previous actor's selected reactions after login, logout or a switch.

The Next Blog consumer uses the existing storefront host `useStorefrontAuth`
binding and its synchronous fresh-snapshot fence. Request state is scoped to
that binding, tenant and subject, including the render before an effect runs.
No token is decoded to invent an actor identity. Token rotation invalidates
request freshness and refetches the canonical owner state; it is not treated as
proof that a different person has signed in. Transport debug output redacts tokens.

Pure settlement tests cover scope changes before a write, after a write and
during reload. Mounted auth-transition/browser verification remains pending.

## Deliberate limits

There is no standalone Reactions route or HTTP transport in this UI package.
Its source-level Blog integration and pure policy tests do not establish
mounted browser authorization, delivery/recovery or accessibility evidence.

## Documentation

- [CRATE_API.md](CRATE_API.md)

## Maintainer verification

The current continuation passes the Reactions source boundary check. Rust compilation/tests and mounted runtime verification remain pending integration; run the following scoped checks:

```bash
cargo test -p rustok-reactions-storefront
cargo check -p rustok-reactions-storefront --all-targets
cargo check -p rustok-reactions-storefront --features hydrate --all-targets
node scripts/verify/verify-reactions-storefront-ui.mjs
git diff --check
```

# Reactions module contract

`rustok-reactions` is an optional first-party module. It depends on Outbox for
its future transactional event boundary but this foundation does not yet publish
or consume events.

## Runtime composition

Module registration initializes two empty runtime-extension registries:

- immediate `ReactionSubjectProvider` instances;
- deferred `ReactionSubjectProviderFactory` instances materialized after
  `HostRuntimeContext` exists.

A source slug is unique across both the materialized provider set and each
registry. Missing providers are explicit capability-unavailable states and do
not authorize a subject.

## Data boundary

No reaction tables, migrations or owner commands exist in this source slice.
The eventual owner must persist complete tenant/source/kind/subject/revision
identity, actor identity, canonical reaction key, action and command UUID. It
must never use a subject label or route as identity.

## UI and transports

No GraphQL, REST, native server functions, Leptos or Next package is registered.
UI remains hidden while the owner persistence boundary is absent.

## Verification

```bash
cargo test -p rustok-reactions
cargo check -p rustok-reactions --all-targets
node scripts/verify/verify-reactions-foundation.mjs
```

No successful execution is claimed by the implementation source.

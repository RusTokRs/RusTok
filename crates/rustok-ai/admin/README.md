# rustok-ai-admin

Leptos admin UI package for the `rustok-ai` capability crate.

## Responsibilities

- Exposes the AI operator/admin surface used by `apps/admin`.
- Stays capability-owned: AI business UI does not live in `apps/admin`.
- Owns the provider profile, tool policy, chat session, trace, and approval flows for the AI
  control plane.
- Uses native-first Leptos `#[server]` functions while keeping GraphQL in parallel.

## Entry Points

- `AiAdmin` — root admin page component for the AI control plane.

## Interactions

- Consumed by `apps/admin` as a host/composition-root dependency.
- Talks to `apps/server` through `rustok-ai` server functions and the parallel GraphQL contract.
- Depends on `rustok-ai` for typed runtime/service contracts and on `rustok-mcp` indirectly through
  the server-side orchestration path.

## Documentation

- See [platform docs](../../../docs/index.md).

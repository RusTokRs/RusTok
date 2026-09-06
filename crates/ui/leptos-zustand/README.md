# leptos-zustand

## Purpose

`leptos-zustand` owns small serializable store snapshot/update contracts for Leptos state synchronization in RusToK.

## Responsibilities

- Represent serializable store snapshots.
- Represent state transitions in a transport-friendly shape.
- Keep lightweight shared store DTOs reusable across Leptos hosts and UI packages.

## Entry points

- `StoreSnapshot`
- `StoreUpdate`

## Interactions

- Can be used by Leptos applications and shared UI packages that synchronize client state.
- Works alongside app-local stores and API clients without owning business logic.
- Stays intentionally small and generic rather than becoming a full state-management framework.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)

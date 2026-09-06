# leptos-zustand docs

`leptos-zustand` currently exposes serializable snapshot and update DTOs. It
is not a state-management runtime: it owns no subscriptions, persistence, or
business-state policy.

Use it only after a host has demonstrated a cross-component state workflow
that cannot remain an app-local Leptos signal. The active adoption decision is
tracked in the [implementation plan](./implementation-plan.md).

# Fly UI

`fly-ui` contains framework-neutral visual-editor behaviour. It depends only on `fly` and standard
serialization/error crates. It does not contain DOM code, Leptos or Dioxus types, RusTok transport,
routing, tenant loading, RBAC implementation, or module-specific widgets.

The crate defines:

- full, inline, preview, and read-only presentations;
- panel, viewport, toolbar, selection, overlay, and dirty-state models;
- editor intents and effects;
- drag-and-drop candidate and insertion contracts;
- contribution, renderer, and property-editor descriptors;
- capability and policy evaluation;
- deterministic admin/storefront registry factories;
- a framework-neutral contribution adapter contract;
- a deterministic state machine suitable for mock-adapter tests.

## Capability policy

`EditorCapabilityPolicy` evaluates the intersection of requested product features, tenant policy,
user permissions, and provider health. Its result is the editable capability ceiling for a
`FlyUiStateMachine`.

Presentation modes only mask that ceiling. Switching to preview or read-only temporarily exposes a
read-only effective profile; switching back to full or inline restores the previously evaluated
ceiling rather than `CapabilityState::full()`. Revoking edit or drag-and-drop capabilities also
cleans up transient drag, insertion, and resize state immediately.

Framework and host packages may resolve authentication, RBAC, tenant rollout, and provider health,
but they pass only the evaluated `CapabilityState` into `fly-ui`. Browser/SSR adapters must apply the
same profile before accepting remote authoring intents.

## Contribution ownership

`ContributionDescriptor.provider` identifies the provider of the component contract being extended.
It is not automatically the module that owns lifecycle, permissions, rollout, or health.
`ModuleContributionManifest.owner_provider` owns those concerns. Cross-provider extensions are
rejected unless `target_providers` explicitly pins the target provider and version.

This lets a consumer such as Pages expose existing `fly.builtin@1` blocks without pretending to own
the built-in renderer. It also prevents a module from silently replacing another provider's
renderer or property editor.

Registry discovery, dependency ordering, tenant/policy filtering, duplicate detection, and contract
resolution stay in `fly-ui`. Framework adapters only execute an already resolved descriptor. Project
mutation always returns to `fly::EditorCommand` and `FlyEditor` history/revision handling.

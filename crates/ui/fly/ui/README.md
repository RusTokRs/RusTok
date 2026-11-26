# Fly UI

For full public interface and state contract definitions, see [CRATE_API.md](CRATE_API.md).

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

## Keyboard shortcut parity

`fixtures/keyboard-shortcuts.json` is shared by the Rust shortcut resolver tests and the Page
Builder iframe Playwright scenarios. It records the expected semantic shortcut (or no shortcut);
the browser checks admission and the exact forwarded stroke. Native composition and editable DOM
boundaries are exercised separately in the browser. Extend the shared cases whenever the shortcut
contract changes.

## Capability policy

`EditorCapabilityPolicy` evaluates the intersection of requested product features, tenant policy,
user permissions, and provider health. Its result is the editable capability ceiling for a
`FlyUiStateMachine`.

Presentation modes only mask that ceiling. Switching to preview or read-only temporarily exposes a
read-only effective profile; switching back to full or inline restores the previously evaluated
ceiling rather than `CapabilityState::full()`. Revoking edit or drag-and-drop capabilities also
cleans up transient drag, insertion, and resize state immediately. Command admission intersects
the public capability projection with the host-evaluated ceiling and normalizes edit-dependent
permissions again; a restored or externally modified projection cannot grant a denied operation.
Resize handles also require the effective style-edit capability.

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

## Save and drag transition guarantees

The state machine validates a drop before consuming the active drag, so an invalid candidate or
unsupported clipboard source can be retried or cancelled without losing its overlay. Save starts
capture both the command sequence and a unique request ID. Success, failure and conflict
transitions must carry the active request ID; a stale response cannot acknowledge or fail a newer
attempt even when its sequence and project hash repeat after controller rehydration. Acknowledging
an earlier snapshot leaves subsequent edits dirty and does not overwrite their project hash.
Concurrent save requests and retries after an unresolved revision conflict are rejected by
`RequestSave`. Resolving a conflict requires the host to reload/rebase the draft; Fly does not
implement automatic merge. Host transports must carry the `SaveAttempt` end-to-end. The local
request ID is not a server idempotency key. Like the core revision state, the UI rejects
mutating intents when the command sequence is exhausted instead of saturating the
counter and emitting an untrackable write. Rejection leaves the state (including an
active drag and insertion overlay) unchanged. Non-mutating intents and saving the
existing document remain available subject to the usual policy.
Drop rechecks the current presentation and drag capability at commit time, including when
restored state contains a drag started under a different policy. Command construction also
rejects illegal or malformed candidates, so direct callers and restored state cannot bypass
candidate validation. Starting another drag clears the preceding insertion overlay.
Selection and hover transitions invalidate the previous component's rectangle, and selection
changes also clear resize handles and the property-editor identity. A rectangle without a
corresponding selection or hover is discarded. Hosts must invalidate measured geometry after
document changes and supply a fresh canvas snapshot.

A save success is accepted only during an active `SaveStarted` transition and only when its
request ID matches the active attempt. A stale success or failure, uncorrelated failure, or
unrelated operation never implicitly clears a standing revision conflict. The conflict also
forces the effective capabilities to read-only and clears drag/resize affordances. Refreshing
host capabilities or switching presentation cannot lift this fence; the host capability ceiling
is retained separately. The conflict remains authoritative after deserialization.

Viewport dimensions and zoom are normalized at the intent boundary using the same limits as
custom preview presets; invalid scroll offsets become zero. Non-finite overlay/drop geometry is
rejected without changing editor state, and a resize event with malformed or overflowing pointer
coordinates never emits non-finite CSS dimensions. Direct patch construction also rejects invalid
rectangles and ignores changes on axes not controlled by the active handle. The CSS patch
preserves the checked finite dimensions without a second rounding step that could violate a
fractional bound or alter a locked aspect ratio.

RequestSave and SaveStarted share admission: edit permission, blocking document diagnostics,
unresolved conflicts and an existing attempt. SaveSucceeded rejects blank revision labels
without retiring the active request, and a changed working hash remains dirty even if the
sequence has not changed. Page Builder's generic controller dispatch cannot accept lifecycle
events; its correlated save APIs update both Fly revision and UI state. If the host has
synchronized a hash, SaveSucceeded must match both the submitted hash and request ID rather than
merely the latest server revision; a mismatched acknowledgement leaves the active save untouched.
The request ID and active sequence are transient and are not restored from serialized UI state.

Pointer movement over one million CSS pixels is rejected as malformed input at the resize
boundary, preventing otherwise finite but impractically huge CSS sizes. Viewport preset
application continues clamping horizontal scroll to the selected width.

Resize gestures leave untouched dimensions unchanged, including off-grid dimensions and
perpendicular arrow keys. A zero-displacement gesture produces no patch. Corner gestures with
aspect-ratio preservation snap the driving dimension and constrain a single scale to both axes'
size bounds; the aspect ratio and bounds take precedence over snapping the second dimension.
Incompatible bounds, collapsed geometry or an unrepresentable locked aspect ratio leave the
original geometry unchanged. Preview and patch generation apply the same relative aspect-ratio
check; subnormal quotients and inconsistent locked change flags fail closed. An unlocked gesture
can still expand a collapsed element. West and north handles anchor
their opposite edges to the initial rectangle.

# fly-ui / CRATE_API

## Public surface

### Primary State Machine & Engine
- `FlyUiStateMachine`: deterministic editor state machine processing `UiIntent` inputs and emitting `UiEffect` actions.
- `FlyUiState`: complete UI state aggregate including viewport, panels, selection, overlays, and capabilities.
- `UiIntent`: user and host interaction intents (`SelectComponent`, `DragStart`, `DropCandidate`, `UpdateProperty`, etc.).
- `UiEffect`: side-effect descriptors produced by transitions (`CommandApplied`, `SelectionChanged`, `ScrollRequested`, etc.).

### Capability Policies & Manifests
- `EditorCapabilityPolicy`: evaluates the effective capability ceiling from tenant policy, permissions, and provider health.
- `EditorCapability`: capability flags (`Edit`, `DragDrop`, `Resize`, `CustomStyle`, etc.).
- `CapabilityState`: effective permissions state governing UI feature availability.
- `ContributionRegistry`, `ContributionDescriptor`: registry of extensible component types, renderers, and property editors.
- `ModuleContributionManifest`: declarative manifest enabling modules to register visual blocks without framework coupling.

### Viewport, Layout & Drag-and-Drop
- `DragState`, `DropPosition`, `HitTestCandidate`: drag-and-drop hit testing and insertion calculations.
- `ResizeSession`, `ResizeHandle`, `ResizePolicy`: element resizing state and constraints.
- `ResponsiveBreakpoint`, `ViewportPreset`, `DeviceClass`: responsive preview dimensions and zoom normalizers.

## Invariants

- **Safe Rust Only**: The entire crate strictly forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Framework Independence**: Zero DOM, Leptos, Dioxus, or React dependencies; all UI logic operates as a pure data-driven state machine.
- **Capability Ceiling**: Switching to preview or read-only modes masks capabilities temporarily; returning to full or inline mode restores the verified capability ceiling rather than granting unconditional full permissions.

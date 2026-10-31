# Crate: `fly-ui`

Framework-neutral visual editor state machine, user intents, capability ceilings, layer hierarchies, and contribution contracts for Fly editors.

## 1. Public API Surface

### Primary State Machine & Engine
- `FlyUiStateMachine`: Deterministic editor state machine processing `UiIntent` inputs and emitting `UiEffect` actions.
- `FlyUiState`: Complete UI state aggregate including viewport, panels, selection, overlays, and capabilities.
- `UiIntent`: User and host interaction intents (`SelectComponent`, `DragStart`, `DropCandidate`, `UpdateProperty`, etc.).
- `UiEffect`: Side-effect descriptors produced by transitions (`CommandApplied`, `SelectionChanged`, `ScrollRequested`, etc.).

### Capability Policies & Manifests
- `EditorCapabilityPolicy`: Evaluates the effective capability ceiling from tenant policy, permissions, and provider health.
- `EditorCapability`: Capability flags (`Edit`, `DragDrop`, `Resize`, `CustomStyle`, etc.).
- `CapabilityState`: Effective permissions state governing UI feature availability.
- `ContributionRegistry`, `ContributionDescriptor`: Registry of extensible component types, renderers, and property editors.
- `ModuleContributionManifest`: Declarative manifest enabling modules to register visual blocks without framework coupling.

### Viewport, Layout & Drag-and-Drop
- `DragState`, `DropPosition`, `HitTestCandidate`: Drag-and-drop hit testing and insertion calculations.
- `ResizeSession`, `ResizeHandle`, `ResizePolicy`: Element resizing state and constraints.
- `ResponsiveBreakpoint`, `ViewportPreset`, `DeviceClass`: Responsive preview dimensions and zoom normalizers.

## 2. Invariants & Business Rules
- **Safe Rust Only**: The entire crate strictly forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Framework Independence**: Zero DOM, Leptos, Dioxus, or React dependencies; all UI logic operates as a pure data-driven state machine.
- **Capability Ceiling**: Switching to preview or read-only modes masks capabilities temporarily; returning to full or inline mode restores the verified capability ceiling rather than granting unconditional full permissions.
- **Atomic Contribution Assembly**: Duplicate renderer or property editor registrations fail atomically without leaving partial or corrupted contracts.

## 3. Error Handling & Edge Cases
- Fail-closed capability evaluation: missing, degraded, or unhealthy providers remove mutating permissions automatically.
- Save conflict detection (`save_conflict_is_a_distinct_lost_update_state`): prevents silent lost updates.
- Typed lookup and dispatch errors without panic or stringly-typed failure suppression.

## 4. Security & Isolation Invariants
- Cross-provider extension isolation: only allowlisted target providers and tenant-enabled modules can register extensions.
- Hostile keyboard injection defense: text editing does not intercept native copy/paste, and physical key fallback rules respect layout boundaries.
- Read-only sessions strictly reject any mutating command dispatch.

## 5. Contract Tests & Verification
- Unit test suite: 74 comprehensive tests covering capability intersection, command requirements, contribution manifests, keyboard shortcuts, and state transitions.
- `src/contract_tests.rs`: Direct verification of package metadata, state machine invariants, capability intersection, and viewport presets.

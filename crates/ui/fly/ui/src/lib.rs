//! Framework-neutral visual-editor state, intents, policies, and contribution contracts.

// The entire Fly stack is safe Rust. `forbid` (not `deny`) so it cannot be re-enabled
// locally with an `allow` attribute.
#![forbid(unsafe_code)]

mod capability_policy;
mod command_capability;
mod contribution;
mod contribution_adapter;
mod contribution_factory;
mod contribution_manifest;
mod dnd;
mod error;
mod keyboard;
mod layers;
mod machine;
mod messages;
mod palette_access;
mod resize;
mod state;
mod style;
mod viewport;

pub use capability_policy::{
    EditorCapability, EditorCapabilityEvaluation, EditorCapabilityPolicy, EditorProviderState,
};
pub use command_capability::CommandCapabilityRequirement;
pub use contribution::{
    AccessibilityMetadata, ContributionDescriptor, ContributionRegistry, PropertyEditorDescriptor,
    RendererDescriptor, ResolvedPropertyEditor, ResolvedRenderer,
};
pub use contribution_adapter::{
    ContributionAdapter, PropertyEditorRequest, RendererRequest, edit_contribution_properties,
    render_contribution,
};
pub use contribution_factory::{
    ContributionAssemblyDiagnostic, ContributionAssemblyPolicy, ContributionAssemblyResult,
    ContributionAssemblySeverity, ContributionProviderHealth, ContributionSurface,
    ModuleContributionMetadata, assemble_contribution_registry, build_admin_contribution_registry,
    build_storefront_contribution_registry,
};
pub use contribution_manifest::{
    ModuleContributionManifest, assemble_contribution_manifests,
    build_admin_contribution_registry_from_manifests,
    build_storefront_contribution_registry_from_manifests,
};
pub use dnd::{DragSource, DragState, DropPosition, HitTestCandidate, command_for_drop};
pub use error::UiError;
pub use keyboard::{EditorShortcut, KeyStroke, ModifierState, resolve_editor_shortcut};
pub use layers::{
    ComponentPropertyChange, ComponentPropertyKind, ComponentPropertyView, EditorLayerItem,
    PaletteBlock, PaletteBlockItem, ScopedStyleChange, ThemeTokenItem, component_property_view,
    editor_layer_items, editor_layer_items_for_page, palette_block, palette_block_items,
    palette_block_items_with_access, palette_blocks, preview_viewport_for_breakpoint,
    scoped_style_value, theme_token_items,
};
pub use machine::{FlyUiStateMachine, UiEffect, UiIntent};
pub use messages::{EditorMessages, PagePropertiesUpdate};
pub use palette_access::PaletteBlockAccess;
pub use resize::{ResizeHandle, ResizePolicy, ResizeResult, ResizeSession};
pub use state::{
    CanvasRect, CapabilityState, DirtyState, EditorPolicy, FlyUiState, OverlayState,
    PageNavigationState, PanelKind, PanelState, Presentation, SelectionState, ViewportState,
};
pub use style::{
    StyleEntry, StyleGroup, StyleInputError, StylePropertyDescriptor, StyleStateDescriptor,
    StyleValueKind, builtin_style_properties, builtin_style_states, style_entries, style_patch,
};
pub use viewport::{
    DeviceClass, ResponsiveBreakpoint, ViewportPreset, builtin_responsive_breakpoints,
    builtin_viewport_presets, custom_breakpoint, custom_viewport, normalize_zoom,
    responsive_breakpoint, viewport_preset,
};

pub type UiResult<T> = Result<T, UiError>;

#[cfg(test)]
mod tests;

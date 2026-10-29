//! # Fly Web Browser Runtime & Geometry
//!
//! Framework-neutral browser runtime, geometry, hit-testing, and iframe bridge for Fly visual editors.
//!
//! This crate owns browser geometry, pointer interactions, drop target resolution, iframe protocol
//! envelopes, event listeners, and authenticated real-DOM inline editing.
//! It is completely independent of any specific UI framework (zero Leptos or Dioxus dependencies).

// No hand-written unsafe in this crate. `deny` rather than `forbid` because framework
// proc-macros (`#[component]`, wasm-bindgen glue) may expand to generated unsafe guarded by their
// own `#[allow(unsafe_code)]`, which `forbid` would reject.
#![deny(unsafe_code)]

mod foundation;
pub use foundation::{
    AutoScrollDelta, AutoScrollPolicy, BrowserDropTarget, BrowserHitTarget, BrowserPoint,
    BrowserRect, CleanupRegistry, CoordinateTransform, DropAxis, DropZonePolicy,
    FLY_IFRAME_PROTOCOL, IframeBridgeEnvelope, IframeBridgeMessage, PointerKind, PointerSample,
    auto_scroll_delta, hit_test_drop_targets, normalize_hit_targets, resolve_drop_position,
};

mod real_dom_inline;
pub use real_dom_inline::{
    AuthenticatedInlineEditGrant, AuthenticatedInlineEditRequest, FLY_REAL_DOM_COMPONENT_ATTRIBUTE,
    FLY_REAL_DOM_INLINE_ATTRIBUTE, InlineEditContractError, InlineEditableField,
    MAX_INLINE_TEXT_BYTES,
};
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use real_dom_inline::{RealDomInlineEditSubscription, attach_real_dom_inline_editing};

mod origin;
pub use origin::{OPAQUE_ORIGIN, normalize_expected_origin, outbound_target_origin};

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser_interaction;
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser_interaction::{
    BrowserResizeSession, key_stroke_from_event, prevent_editor_shortcut_default,
    shortcut_from_event,
};

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
mod browser_runtime;
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
pub use browser_runtime::{
    BrowserRuntimeError, EventListenerHandle, IframeJsonSubscription, IframeMessagePort,
    ResizeObserverHandle, WindowMessageSubscription, release_pointer_capture, set_pointer_capture,
};

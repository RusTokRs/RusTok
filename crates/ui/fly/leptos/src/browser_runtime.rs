#[expect(
    unused_imports,
    reason = "contracted facade re-export for wasm-client surface"
)]
pub use fly_web::{
    BrowserRuntimeError, EventListenerHandle, IframeJsonSubscription, IframeMessagePort,
    ResizeObserverHandle, WindowMessageSubscription, release_pointer_capture, set_pointer_capture,
};

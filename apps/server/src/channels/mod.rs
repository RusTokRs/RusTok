//! Server-side WebSocket channels.
//!
//! Each channel corresponds to a WebSocket endpoint. Implement [`RustokChannel`]
//! to add a new channel with a consistent lifecycle contract.
//!
//! ## Current channels
//!
//! | Channel | Path | Description |
//! |---------|------|-------------|
//! | [`builds`] | `/ws/builds` | Streams build events to admin clients |
//!
//! ## Adding a new channel
//!
//! 1. Create `src/channels/<name>.rs` implementing [`RustokChannel`].
//! 2. Register the Axum route in `src/app.rs` → `routes()`.
//! 3. Add a row to the table above.

use async_trait::async_trait;
use axum::extract::ws::WebSocket;
use loco_rs::app::AppContext;

/// Contract for server-side WebSocket channels.
///
/// Implementors receive an upgraded WebSocket and are responsible for the full
/// connection lifecycle (auth handshake, message loop, cleanup on disconnect).
///
/// Unlike Loco Channels, `RustokChannel` uses the server's own auth mechanism
/// (Bearer JWT validated before the upgrade) and does not depend on
/// `loco_rs::controller::channels`.
#[async_trait]
pub trait RustokChannel: Send + Sync {
    /// Axum route path for this channel, e.g. `"/ws/builds"`.
    fn path(&self) -> &'static str;

    /// Handle an upgraded WebSocket connection.
    ///
    /// Called after the HTTP → WebSocket upgrade succeeds. The implementation
    /// is responsible for reading/writing frames and closing cleanly.
    async fn handle(&self, socket: WebSocket, ctx: AppContext);
}

pub mod builds;

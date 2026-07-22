//! Search admin native transport composition.
//!
//! The included parts own the `#[server]` endpoints, resolve `HostRuntimeContext`,
//! and obtain the typed event bus through
//! `shared_get::<rustok_outbox::TransactionalEventBus>()`.

include!("native_server_adapter/api.rs");
include!("native_server_adapter/read_bootstrap.rs");
include!("native_server_adapter/read_diagnostics.rs");
include!("native_server_adapter/read_analytics.rs");
include!("native_server_adapter/write_runtime.rs");
include!("native_server_adapter/write_dictionary.rs");
include!("native_server_adapter/normalization.rs");
include!("native_server_adapter/pipeline.rs");
include!("native_server_adapter/mapping.rs");
include!("native_server_adapter/support.rs");

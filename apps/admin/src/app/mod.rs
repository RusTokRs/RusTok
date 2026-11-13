#[cfg(feature = "ssr")]
pub mod auth_ssr;
#[cfg(feature = "ssr")]
pub mod draft_retention;
pub mod modules;
pub mod page_builder_contributions;
pub mod providers;
pub mod router;
#[cfg(feature = "ssr")]
pub mod security;
#[cfg(feature = "ssr")]
pub mod shell;

#[cfg(feature = "ssr")]
pub use auth_ssr::request_auth_snapshot;
#[cfg(feature = "ssr")]
pub use draft_retention::{DRAFT_SESSION_PRUNE_INTERVAL, spawn_draft_session_prune};
pub use router::App;
#[cfg(feature = "ssr")]
pub use security::{admin_security_headers, request_csp_nonce, validate_admin_security_profile};
#[cfg(feature = "ssr")]
pub use shell::shell;

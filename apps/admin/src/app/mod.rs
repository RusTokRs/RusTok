#[cfg(feature = "ssr")]
pub mod auth_ssr;
pub mod modules;
pub mod providers;
pub mod router;
#[cfg(feature = "ssr")]
pub mod security;
#[cfg(feature = "ssr")]
pub mod shell;

#[cfg(feature = "ssr")]
pub use auth_ssr::{AuthCookieBootstrap, request_auth_snapshot};
pub use router::App;
#[cfg(feature = "ssr")]
pub use security::{admin_security_headers, request_csp_nonce, validate_admin_security_profile};
#[cfg(feature = "ssr")]
pub use shell::shell;

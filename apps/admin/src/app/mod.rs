#[cfg(feature = "ssr")]
pub mod auth_ssr;
pub mod modules;
pub mod providers;
pub mod router;
#[cfg(feature = "ssr")]
pub mod shell;

#[cfg(feature = "ssr")]
pub use auth_ssr::{request_auth_snapshot, AuthCookieBootstrap};
pub use router::App;
#[cfg(feature = "ssr")]
pub use shell::shell;

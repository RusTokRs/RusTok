//! Framework-neutral client identity and session contracts.
//!
//! The shared value definitions remain in `rustok-ui-auth`; Auth owns their
//! lifecycle, transport, and framework bindings in this module package.

pub use rustok_ui_auth::{AuthError, AuthSession, AuthUser};

mod scope;

pub use scope::ClientAuthScope;

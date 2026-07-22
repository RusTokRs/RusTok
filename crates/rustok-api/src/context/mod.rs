#[cfg(feature = "server")]
mod auth;
mod channel;
#[cfg(feature = "server")]
mod oauth_scope;
#[cfg(feature = "server")]
mod principal;
#[cfg(feature = "server")]
mod tenant;

#[cfg(feature = "server")]
pub use auth::{
    AuthContext, AuthContextExtension, OptionalAuthContext, has_any_effective_permission,
    has_effective_permission, restrict_permissions_to_scopes,
};
pub use channel::{
    ChannelContext, ChannelResolutionOutcome, ChannelResolutionSource, ChannelResolutionStage,
    ChannelResolutionTraceStep,
};
#[cfg(feature = "server")]
pub use channel::{ChannelContextExt, ChannelContextExtension, OptionalChannel};
#[cfg(feature = "server")]
pub use oauth_scope::scope_matches;
#[cfg(feature = "server")]
pub use tenant::{
    OptionalTenant, TenantContext, TenantContextExt, TenantContextExtension, TenantError,
};

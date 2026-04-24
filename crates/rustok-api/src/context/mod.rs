#[cfg(feature = "server")]
mod auth;
mod channel;
#[cfg(feature = "server")]
mod tenant;

#[cfg(feature = "server")]
pub use auth::{
    has_any_effective_permission, has_effective_permission, infer_user_role_from_permissions,
    scope_matches, AuthContext, AuthContextExtension, OptionalAuthContext,
};
pub use channel::{
    ChannelContext, ChannelResolutionOutcome, ChannelResolutionSource, ChannelResolutionStage,
    ChannelResolutionTraceStep,
};
#[cfg(feature = "server")]
pub use channel::{ChannelContextExt, ChannelContextExtension, OptionalChannel};
#[cfg(feature = "server")]
pub use tenant::{
    OptionalTenant, TenantContext, TenantContextExt, TenantContextExtension, TenantError,
};

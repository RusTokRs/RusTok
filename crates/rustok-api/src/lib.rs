pub mod context;
pub mod graphql;
pub mod loco;
pub mod request;
pub mod route_selection;
pub mod ui;

pub use context::{
    has_any_effective_permission, has_effective_permission, infer_user_role_from_permissions,
    scope_matches, AuthContext, AuthContextExtension, ChannelContext, ChannelContextExt,
    ChannelContextExtension, ChannelResolutionOutcome, ChannelResolutionSource,
    ChannelResolutionStage, ChannelResolutionTraceStep, OptionalAuthContext, OptionalChannel,
    OptionalTenant, TenantContext, TenantContextExt, TenantContextExtension, TenantError,
};
pub use request::RequestContext;
pub use route_selection::{
    admin_route_query_schema, is_legacy_admin_query_key, sanitize_admin_route_query,
    AdminQueryDependency, AdminQueryKey, AdminRouteQuerySchema,
};
pub use ui::{
    build_ui_message_catalog, resolve_ui_message, resolve_ui_message_or_fallback, UiMessageCatalog,
    UiRouteContext,
};

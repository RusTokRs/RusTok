/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod context;
#[cfg(feature = "server")]
pub mod graphql;
#[cfg(feature = "loco-adapter")]
pub mod loco;
pub mod manifest_hash;
pub mod module_registry_contract;
pub mod ports;
#[cfg(feature = "server")]
pub mod request;
pub mod route_selection;
pub mod ui;
pub mod write_path_feedback;

#[cfg(feature = "server")]
pub use context::{
    AuthContext, AuthContextExtension, ChannelContextExt, ChannelContextExtension,
    OptionalAuthContext, OptionalChannel, OptionalTenant, TenantContext, TenantContextExt,
    TenantContextExtension, TenantError, has_any_effective_permission, has_effective_permission,
    infer_user_role_from_permissions, scope_matches,
};
pub use context::{
    ChannelContext, ChannelResolutionOutcome, ChannelResolutionSource, ChannelResolutionStage,
    ChannelResolutionTraceStep,
};
pub use ports::{
    PortActor, PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind,
    PortOperationKind,
};
#[cfg(feature = "server")]
pub use request::RequestContext;
pub use route_selection::{
    AdminQueryDependency, AdminQueryKey, AdminRouteQuerySchema, admin_route_query_schema,
    is_legacy_admin_query_key, sanitize_admin_route_query,
};
pub use ui::{
    UiMessageCatalog, UiRouteContext, UiRouteQueryUpdate, build_ui_message_catalog,
    normalize_ui_text, parse_ui_csv, resolve_ui_message, resolve_ui_message_or_fallback,
    route_query_update_for_text,
};
pub use write_path_feedback::{WritePathIssue, WritePathIssueKind, classify_write_path_issue};

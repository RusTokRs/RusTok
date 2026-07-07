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
pub mod locale;
pub mod manifest_hash;
pub mod module_registry_contract;
pub mod permissions;
pub mod ports;
#[cfg(feature = "server")]
pub mod request;
pub mod route_selection;
#[cfg(feature = "server")]
pub mod runtime;
pub mod ui;
pub mod write_path_feedback;

#[cfg(feature = "server")]
pub use context::{
    has_any_effective_permission, has_effective_permission, scope_matches, AuthContext,
    AuthContextExtension, ChannelContextExt, ChannelContextExtension, OptionalAuthContext,
    OptionalChannel, OptionalTenant, TenantContext, TenantContextExt, TenantContextExtension,
    TenantError,
};
pub use context::{
    ChannelContext, ChannelResolutionOutcome, ChannelResolutionSource, ChannelResolutionStage,
    ChannelResolutionTraceStep,
};
pub use locale::{
    build_locale_candidates, extract_locale_tag_from_header, is_valid_locale_tag,
    locale_primary_language, locale_tags_match, normalize_locale_tag, push_locale_candidate,
    PLATFORM_FALLBACK_LOCALE,
};
pub use permissions::{Action, Permission, Resource};
pub use ports::{
    PortActor, PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind,
    PortOperationKind,
};
#[cfg(feature = "server")]
pub use request::RequestContext;
pub use route_selection::{
    admin_route_query_schema, is_legacy_admin_query_key, sanitize_admin_route_query,
    AdminQueryDependency, AdminQueryKey, AdminRouteQuerySchema,
};
#[cfg(feature = "server")]
pub use runtime::HostRuntimeContext;
pub use ui::{
    normalize_ui_text, parse_ui_csv, route_query_update_for_text, UiRouteContext,
    UiRouteQueryUpdate,
};
pub use write_path_feedback::{classify_write_path_issue, WritePathIssue, WritePathIssueKind};

/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod artifact_permissions;
pub mod context;
pub mod event_delivery;
#[cfg(feature = "server")]
pub mod graphql;
pub mod locale;
pub mod manifest_hash;
pub mod module_registry_contract;
pub mod module_work;
pub mod permissions;
pub mod platform_build;
pub mod ports;
#[cfg(feature = "server")]
pub mod request;
pub mod richtext;
#[cfg(feature = "runtime")]
pub mod runtime;
pub mod tenant_rbac;
pub mod write_path_feedback;

pub use artifact_permissions::{
    ArtifactPermissionLocalization, ArtifactPermissionRegistration,
    ArtifactPermissionRegistrationPort, ArtifactPermissionRegistrationRequest,
    ArtifactPermissionScope,
};
#[cfg(feature = "server")]
pub use context::{
    AuthContext, AuthContextExtension, ChannelContextExt, ChannelContextExtension,
    OptionalAuthContext, OptionalChannel, OptionalTenant, TenantContext, TenantContextExt,
    TenantContextExtension, TenantError, has_any_effective_permission, has_effective_permission,
    scope_matches,
};
pub use context::{
    ChannelContext, ChannelResolutionOutcome, ChannelResolutionSource, ChannelResolutionStage,
    ChannelResolutionTraceStep,
};
pub use event_delivery::{
    EventDeliveryConfigurationSnapshot, EventDeliveryControl, EventDeliveryUpdateOutcome,
    SharedEventDeliveryControl,
};
pub use locale::{
    PLATFORM_FALLBACK_LOCALE, build_locale_candidates, extract_locale_tag_from_header,
    is_valid_locale_tag, locale_primary_language, locale_tags_match, normalize_locale_tag,
    push_locale_candidate,
};
pub use module_work::{
    ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource,
};
pub use permissions::{Action, Permission, Resource};
pub use platform_build::{
    PlatformBuildSnapshot, PlatformBuildStage, PlatformBuildStatus, PlatformDeploymentProfile,
    PlatformReleaseSnapshot, PlatformReleaseStatus,
};
pub use ports::{
    PortActor, PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind,
    PortOperationKind,
};
#[cfg(feature = "server")]
pub use request::RequestContext;
pub use richtext::{
    RichTextDocument, RichTextMark, RichTextNode, RichTextProfileId, RichTextProfileIdError,
    RichTextView, document_json_schema,
};
#[cfg(feature = "runtime")]
pub use runtime::{HostRuntimeContext, HostSettingsSnapshot};
pub use tenant_rbac::{
    SharedTenantRbacCatalog, TenantRbacCatalog, TenantRbacCatalogError, TenantRbacPermission,
    TenantRbacRole,
};
pub use write_path_feedback::{WritePathIssue, WritePathIssueKind, classify_write_path_issue};

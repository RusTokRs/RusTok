use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    MarketplaceSellerAdminCommand, MarketplaceSellerAdminCommandResult,
    MarketplaceSellerAdminDetail, MarketplaceSellerAdminDirectory, MarketplaceSellerAdminFilters,
};
#[cfg(feature = "ssr")]
use crate::model::{
    MarketplaceSellerAdminListItem, MarketplaceSellerAdminMember, MarketplaceSellerAdminRecord,
};

#[derive(Debug, Clone)]
pub struct NativeMarketplaceSellerAdminError(pub String);

impl Display for NativeMarketplaceSellerAdminError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeMarketplaceSellerAdminError {}

impl From<ServerFnError> for NativeMarketplaceSellerAdminError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_directory(
    filters: MarketplaceSellerAdminFilters,
) -> Result<MarketplaceSellerAdminDirectory, NativeMarketplaceSellerAdminError> {
    marketplace_seller_directory_native(filters)
        .await
        .map_err(Into::into)
}

pub async fn load_detail(
    seller_id: String,
) -> Result<MarketplaceSellerAdminDetail, NativeMarketplaceSellerAdminError> {
    marketplace_seller_detail_native(seller_id)
        .await
        .map_err(Into::into)
}

pub async fn execute_command(
    idempotency_key: String,
    command: MarketplaceSellerAdminCommand,
) -> Result<MarketplaceSellerAdminCommandResult, NativeMarketplaceSellerAdminError> {
    marketplace_seller_command_native(idempotency_key, command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "marketplace-seller/directory")]
async fn marketplace_seller_directory_native(
    filters: MarketplaceSellerAdminFilters,
) -> Result<MarketplaceSellerAdminDirectory, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, Permission, TenantContext, request::RequestContext,
        };
        use rustok_marketplace_seller::{
            ListMarketplaceSellersInput, MarketplaceSellerReadPort, MarketplaceSellerService,
        };

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_permission(
            &auth,
            &[
                Permission::MARKETPLACE_SELLERS_LIST,
                Permission::MARKETPLACE_SELLERS_READ,
            ],
        )?;
        ensure_tenant(&auth, &tenant)?;

        let page = filters.page.max(1);
        let per_page = filters.per_page.clamp(1, 100);
        let service = MarketplaceSellerService::new(runtime.db_clone());
        let response = MarketplaceSellerReadPort::list_sellers(
            &service,
            port_context(&auth, &tenant, &request, None),
            ListMarketplaceSellersInput {
                page,
                per_page,
                status: parse_seller_status(filters.status.as_deref())?,
                onboarding_status: parse_onboarding_status(filters.onboarding_status.as_deref())?,
                search: normalize_optional_text(filters.search),
            },
        )
        .await
        .map_err(map_port_error)?;

        Ok(MarketplaceSellerAdminDirectory {
            items: response
                .items
                .into_iter()
                .map(|seller| MarketplaceSellerAdminListItem {
                    id: seller.id.to_string(),
                    handle: seller.handle,
                    resolved_locale: seller.resolved_locale,
                    display_name: seller.display_name,
                    status: seller.status.as_str().to_string(),
                    onboarding_status: seller.onboarding_status.as_str().to_string(),
                })
                .collect(),
            total: response.total,
            page,
            per_page,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = filters;
        Err(ServerFnError::new(
            "marketplace seller directory requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "marketplace-seller/detail")]
async fn marketplace_seller_detail_native(
    seller_id: String,
) -> Result<MarketplaceSellerAdminDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, Permission, TenantContext, request::RequestContext,
        };
        use rustok_marketplace_seller::{
            ListMarketplaceSellerMembersRequest, MarketplaceSellerReadPort,
            MarketplaceSellerService, ReadMarketplaceSellerRequest,
        };

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_permission(&auth, &[Permission::MARKETPLACE_SELLERS_READ])?;
        ensure_tenant(&auth, &tenant)?;
        let seller_id = parse_uuid(seller_id.as_str(), "seller_id")?;
        let service = MarketplaceSellerService::new(runtime.db_clone());
        let context = port_context(&auth, &tenant, &request, None);
        let seller = MarketplaceSellerReadPort::read_seller(
            &service,
            context.clone(),
            ReadMarketplaceSellerRequest { seller_id },
        )
        .await
        .map_err(map_port_error)?;
        let members = MarketplaceSellerReadPort::list_members(
            &service,
            context,
            ListMarketplaceSellerMembersRequest { seller_id },
        )
        .await
        .map_err(map_port_error)?;

        Ok(MarketplaceSellerAdminDetail {
            seller: map_seller(seller),
            members: members.into_iter().map(map_member).collect(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = seller_id;
        Err(ServerFnError::new(
            "marketplace seller detail requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "marketplace-seller/command")]
async fn marketplace_seller_command_native(
    idempotency_key: String,
    command: MarketplaceSellerAdminCommand,
) -> Result<MarketplaceSellerAdminCommandResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, Permission, TenantContext, request::RequestContext,
        };
        use rustok_marketplace_seller::{
            AddMarketplaceSellerMemberInput, AddMarketplaceSellerMemberRequest,
            CreateMarketplaceSellerInput, MarketplaceSellerCommandPort, MarketplaceSellerService,
            ReactivateMarketplaceSellerRequest, ReviewMarketplaceSellerOnboardingInput,
            ReviewMarketplaceSellerOnboardingRequest, SubmitMarketplaceSellerOnboardingInput,
            SubmitMarketplaceSellerOnboardingRequest, SuspendMarketplaceSellerInput,
            SuspendMarketplaceSellerRequest, UpdateMarketplaceSellerMemberInput,
            UpdateMarketplaceSellerMemberRequest, UpdateMarketplaceSellerProfileInput,
            UpdateMarketplaceSellerProfileRequest,
        };

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let required = match &command {
            MarketplaceSellerAdminCommand::Create { .. } => Permission::MARKETPLACE_SELLERS_CREATE,
            MarketplaceSellerAdminCommand::ReviewOnboarding { .. }
            | MarketplaceSellerAdminCommand::Suspend { .. }
            | MarketplaceSellerAdminCommand::Reactivate { .. } => {
                Permission::MARKETPLACE_SELLERS_MANAGE
            }
            MarketplaceSellerAdminCommand::UpdateProfile { .. }
            | MarketplaceSellerAdminCommand::SubmitOnboarding { .. }
            | MarketplaceSellerAdminCommand::AddMember { .. }
            | MarketplaceSellerAdminCommand::UpdateMember { .. } => {
                Permission::MARKETPLACE_SELLERS_UPDATE
            }
        };
        ensure_permission(&auth, &[required])?;
        ensure_tenant(&auth, &tenant)?;
        let context = port_context(&auth, &tenant, &request, Some(idempotency_key));
        let service = MarketplaceSellerService::new(runtime.db_clone());

        match command {
            MarketplaceSellerAdminCommand::Create { draft } => {
                let seller = MarketplaceSellerCommandPort::create_seller(
                    &service,
                    context,
                    CreateMarketplaceSellerInput {
                        handle: draft.handle,
                        display_name: draft.display_name,
                        legal_name: draft.legal_name,
                        owner_user_id: parse_uuid(draft.owner_user_id.as_str(), "owner_user_id")?,
                        metadata: object_or_empty(draft.metadata, "metadata")?,
                    },
                )
                .await
                .map_err(map_port_error)?;
                Ok(seller_result(seller))
            }
            MarketplaceSellerAdminCommand::UpdateProfile { seller_id, draft } => {
                let seller = MarketplaceSellerCommandPort::update_seller_profile(
                    &service,
                    context,
                    UpdateMarketplaceSellerProfileRequest {
                        seller_id: parse_uuid(seller_id.as_str(), "seller_id")?,
                        input: UpdateMarketplaceSellerProfileInput {
                            display_name: normalize_optional_text(draft.display_name),
                            legal_name: normalize_optional_text(draft.legal_name),
                            metadata: draft
                                .metadata
                                .map(|value| object_or_empty(value, "metadata"))
                                .transpose()?,
                        },
                    },
                )
                .await
                .map_err(map_port_error)?;
                Ok(seller_result(seller))
            }
            MarketplaceSellerAdminCommand::SubmitOnboarding { seller_id, note } => {
                let seller = MarketplaceSellerCommandPort::submit_seller_onboarding(
                    &service,
                    context,
                    SubmitMarketplaceSellerOnboardingRequest {
                        seller_id: parse_uuid(seller_id.as_str(), "seller_id")?,
                        input: SubmitMarketplaceSellerOnboardingInput {
                            note: normalize_optional_text(note),
                        },
                    },
                )
                .await
                .map_err(map_port_error)?;
                Ok(seller_result(seller))
            }
            MarketplaceSellerAdminCommand::ReviewOnboarding {
                seller_id,
                approved,
                note,
            } => {
                let seller = MarketplaceSellerCommandPort::review_seller_onboarding(
                    &service,
                    context,
                    ReviewMarketplaceSellerOnboardingRequest {
                        seller_id: parse_uuid(seller_id.as_str(), "seller_id")?,
                        input: ReviewMarketplaceSellerOnboardingInput {
                            approved,
                            note: normalize_optional_text(note),
                        },
                    },
                )
                .await
                .map_err(map_port_error)?;
                Ok(seller_result(seller))
            }
            MarketplaceSellerAdminCommand::Suspend { seller_id, reason } => {
                let seller = MarketplaceSellerCommandPort::suspend_seller(
                    &service,
                    context,
                    SuspendMarketplaceSellerRequest {
                        seller_id: parse_uuid(seller_id.as_str(), "seller_id")?,
                        input: SuspendMarketplaceSellerInput { reason },
                    },
                )
                .await
                .map_err(map_port_error)?;
                Ok(seller_result(seller))
            }
            MarketplaceSellerAdminCommand::Reactivate { seller_id } => {
                let seller = MarketplaceSellerCommandPort::reactivate_seller(
                    &service,
                    context,
                    ReactivateMarketplaceSellerRequest {
                        seller_id: parse_uuid(seller_id.as_str(), "seller_id")?,
                    },
                )
                .await
                .map_err(map_port_error)?;
                Ok(seller_result(seller))
            }
            MarketplaceSellerAdminCommand::AddMember { seller_id, draft } => {
                let member = MarketplaceSellerCommandPort::add_seller_member(
                    &service,
                    context,
                    AddMarketplaceSellerMemberRequest {
                        seller_id: parse_uuid(seller_id.as_str(), "seller_id")?,
                        input: AddMarketplaceSellerMemberInput {
                            user_id: parse_uuid(draft.user_id.as_str(), "user_id")?,
                            role: parse_member_role(draft.role.as_str())?,
                            metadata: object_or_empty(draft.metadata, "metadata")?,
                        },
                    },
                )
                .await
                .map_err(map_port_error)?;
                Ok(member_result(member))
            }
            MarketplaceSellerAdminCommand::UpdateMember {
                seller_id,
                member_id,
                draft,
            } => {
                let member = MarketplaceSellerCommandPort::update_seller_member(
                    &service,
                    context,
                    UpdateMarketplaceSellerMemberRequest {
                        seller_id: parse_uuid(seller_id.as_str(), "seller_id")?,
                        member_id: parse_uuid(member_id.as_str(), "member_id")?,
                        input: UpdateMarketplaceSellerMemberInput {
                            role: draft.role.as_deref().map(parse_member_role).transpose()?,
                            status: draft
                                .status
                                .as_deref()
                                .map(parse_member_status)
                                .transpose()?,
                            metadata: draft
                                .metadata
                                .map(|value| object_or_empty(value, "metadata"))
                                .transpose()?,
                        },
                    },
                )
                .await
                .map_err(map_port_error)?;
                Ok(member_result(member))
            }
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (idempotency_key, command);
        Err(ServerFnError::new(
            "marketplace seller commands require the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn ensure_permission(
    auth: &rustok_api::AuthContext,
    required: &[rustok_api::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_any_effective_permission(&auth.permissions, required) {
        return Err(ServerFnError::new(
            "Permission denied: marketplace seller permission required",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_tenant(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
) -> Result<(), ServerFnError> {
    if auth.tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "Permission denied: marketplace seller tenant mismatch",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn port_context(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request: &rustok_api::request::RequestContext,
    idempotency_key: Option<String>,
) -> rustok_api::PortContext {
    let mut context = rustok_api::PortContext::new(
        tenant.id.to_string(),
        rustok_api::PortActor::user(auth.user_id.to_string()),
        request.locale.clone(),
        format!("native-marketplace-seller-{}", uuid::Uuid::new_v4()),
    )
    .with_deadline(std::time::Duration::from_secs(5));
    if let Some(channel) = request.channel_slug.clone() {
        context = context.with_channel(channel);
    }
    if let Some(key) = idempotency_key {
        context = context.with_idempotency_key(key);
    }
    context
}

#[cfg(feature = "ssr")]
fn map_port_error(error: rustok_api::PortError) -> ServerFnError {
    use rustok_api::PortErrorKind;
    let message = match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            error.message
        }
        PortErrorKind::Forbidden => "Permission denied: marketplace seller operation".to_string(),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            "Marketplace seller service is temporarily unavailable".to_string()
        }
        PortErrorKind::InvariantViolation => {
            "Marketplace seller command requires operator review".to_string()
        }
    };
    ServerFnError::new(message)
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new(format!("Invalid {field}")))
}

#[cfg(feature = "ssr")]
fn parse_seller_status(
    value: Option<&str>,
) -> Result<Option<rustok_marketplace_seller::MarketplaceSellerStatus>, ServerFnError> {
    value
        .and_then(normalize_text)
        .map(|value| {
            rustok_marketplace_seller::MarketplaceSellerStatus::parse(value.as_str())
                .ok_or_else(|| ServerFnError::new("Invalid seller status"))
        })
        .transpose()
}

#[cfg(feature = "ssr")]
fn parse_onboarding_status(
    value: Option<&str>,
) -> Result<Option<rustok_marketplace_seller::MarketplaceSellerOnboardingStatus>, ServerFnError> {
    value
        .and_then(normalize_text)
        .map(|value| {
            rustok_marketplace_seller::MarketplaceSellerOnboardingStatus::parse(value.as_str())
                .ok_or_else(|| ServerFnError::new("Invalid onboarding status"))
        })
        .transpose()
}

#[cfg(feature = "ssr")]
fn parse_member_role(
    value: &str,
) -> Result<rustok_marketplace_seller::MarketplaceSellerMemberRole, ServerFnError> {
    rustok_marketplace_seller::MarketplaceSellerMemberRole::parse(value.trim())
        .ok_or_else(|| ServerFnError::new("Invalid marketplace seller member role"))
}

#[cfg(feature = "ssr")]
fn parse_member_status(
    value: &str,
) -> Result<rustok_marketplace_seller::MarketplaceSellerMemberStatus, ServerFnError> {
    rustok_marketplace_seller::MarketplaceSellerMemberStatus::parse(value.trim())
        .ok_or_else(|| ServerFnError::new("Invalid marketplace seller member status"))
}

#[cfg(feature = "ssr")]
fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| normalize_text(value.as_str()))
}

#[cfg(feature = "ssr")]
fn normalize_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(feature = "ssr")]
fn object_or_empty(
    value: serde_json::Value,
    field: &str,
) -> Result<serde_json::Value, ServerFnError> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(ServerFnError::new(format!(
            "Invalid {field}: expected JSON object"
        ))),
    }
}

#[cfg(feature = "ssr")]
fn map_seller(
    value: rustok_marketplace_seller::MarketplaceSellerResponse,
) -> MarketplaceSellerAdminRecord {
    MarketplaceSellerAdminRecord {
        id: value.id.to_string(),
        tenant_id: value.tenant_id.to_string(),
        handle: value.handle,
        resolved_locale: value.resolved_locale,
        display_name: value.display_name,
        legal_name: value.legal_name,
        status: value.status.as_str().to_string(),
        onboarding_status: value.onboarding_status.as_str().to_string(),
        onboarding_note: value.onboarding_note,
        suspension_reason: value.suspension_reason,
        metadata: value.metadata,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
        activated_at: value.activated_at.map(|date| date.to_rfc3339()),
        suspended_at: value.suspended_at.map(|date| date.to_rfc3339()),
    }
}

#[cfg(feature = "ssr")]
fn map_member(
    value: rustok_marketplace_seller::MarketplaceSellerMemberResponse,
) -> MarketplaceSellerAdminMember {
    MarketplaceSellerAdminMember {
        id: value.id.to_string(),
        seller_id: value.seller_id.to_string(),
        user_id: value.user_id.to_string(),
        role: value.role.as_str().to_string(),
        status: value.status.as_str().to_string(),
        invited_by_actor_id: value.invited_by_actor_id.map(|id| id.to_string()),
        accepted_at: value.accepted_at.map(|date| date.to_rfc3339()),
        metadata: value.metadata,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn seller_result(
    seller: rustok_marketplace_seller::MarketplaceSellerResponse,
) -> MarketplaceSellerAdminCommandResult {
    MarketplaceSellerAdminCommandResult {
        seller: Some(map_seller(seller)),
        member: None,
    }
}

#[cfg(feature = "ssr")]
fn member_result(
    member: rustok_marketplace_seller::MarketplaceSellerMemberResponse,
) -> MarketplaceSellerAdminCommandResult {
    MarketplaceSellerAdminCommandResult {
        seller: None,
        member: Some(map_member(member)),
    }
}

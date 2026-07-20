#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::model::{
    MarketplaceSellerAdminCommand, MarketplaceSellerAdminCommandResult,
    MarketplaceSellerAdminDetail, MarketplaceSellerAdminDirectory, MarketplaceSellerAdminFilters,
    MarketplaceSellerAdminListItem, MarketplaceSellerAdminMember, MarketplaceSellerAdminRecord,
};

pub type GraphqlMarketplaceSellerAdminError = String;

const DIRECTORY_QUERY: &str = "query MarketplaceSellerAdminDirectory($page: Int, $perPage: Int, $status: MarketplaceSellerStatusGql, $onboardingStatus: MarketplaceSellerOnboardingStatusGql, $search: String) { marketplaceSellers(page: $page, perPage: $perPage, status: $status, onboardingStatus: $onboardingStatus, search: $search) { total page per_page: perPage items { id handle resolved_locale: resolvedLocale display_name: displayName status onboarding_status: onboardingStatus } } }";
const DETAIL_QUERY: &str = "query MarketplaceSellerAdminDetail($id: UUID!) { seller: marketplaceSeller(id: $id) { id tenant_id: tenantId handle resolved_locale: resolvedLocale display_name: displayName legal_name: legalName status onboarding_status: onboardingStatus onboarding_note: onboardingNote suspension_reason: suspensionReason metadata created_at: createdAt updated_at: updatedAt activated_at: activatedAt suspended_at: suspendedAt } members: marketplaceSellerMembers(sellerId: $id) { id seller_id: sellerId user_id: userId role status invited_by_actor_id: invitedByActorId accepted_at: acceptedAt metadata created_at: createdAt updated_at: updatedAt } }";
const CREATE_MUTATION: &str = "mutation MarketplaceSellerAdminCreate($idempotencyKey: String!, $input: MarketplaceSellerCreateInputGql!) { result: createMarketplaceSeller(idempotencyKey: $idempotencyKey, input: $input) { id tenant_id: tenantId handle resolved_locale: resolvedLocale display_name: displayName legal_name: legalName status onboarding_status: onboardingStatus onboarding_note: onboardingNote suspension_reason: suspensionReason metadata created_at: createdAt updated_at: updatedAt activated_at: activatedAt suspended_at: suspendedAt } }";
const UPDATE_PROFILE_MUTATION: &str = "mutation MarketplaceSellerAdminUpdateProfile($idempotencyKey: String!, $sellerId: UUID!, $input: MarketplaceSellerProfileInputGql!) { result: updateMarketplaceSellerProfile(idempotencyKey: $idempotencyKey, sellerId: $sellerId, input: $input) { id tenant_id: tenantId handle resolved_locale: resolvedLocale display_name: displayName legal_name: legalName status onboarding_status: onboardingStatus onboarding_note: onboardingNote suspension_reason: suspensionReason metadata created_at: createdAt updated_at: updatedAt activated_at: activatedAt suspended_at: suspendedAt } }";
const SUBMIT_MUTATION: &str = "mutation MarketplaceSellerAdminSubmit($idempotencyKey: String!, $sellerId: UUID!, $note: String) { result: submitMarketplaceSellerOnboarding(idempotencyKey: $idempotencyKey, sellerId: $sellerId, note: $note) { id tenant_id: tenantId handle resolved_locale: resolvedLocale display_name: displayName legal_name: legalName status onboarding_status: onboardingStatus onboarding_note: onboardingNote suspension_reason: suspensionReason metadata created_at: createdAt updated_at: updatedAt activated_at: activatedAt suspended_at: suspendedAt } }";
const REVIEW_MUTATION: &str = "mutation MarketplaceSellerAdminReview($idempotencyKey: String!, $sellerId: UUID!, $approved: Boolean!, $note: String) { result: reviewMarketplaceSellerOnboarding(idempotencyKey: $idempotencyKey, sellerId: $sellerId, approved: $approved, note: $note) { id tenant_id: tenantId handle resolved_locale: resolvedLocale display_name: displayName legal_name: legalName status onboarding_status: onboardingStatus onboarding_note: onboardingNote suspension_reason: suspensionReason metadata created_at: createdAt updated_at: updatedAt activated_at: activatedAt suspended_at: suspendedAt } }";
const SUSPEND_MUTATION: &str = "mutation MarketplaceSellerAdminSuspend($idempotencyKey: String!, $sellerId: UUID!, $reason: String!) { result: suspendMarketplaceSeller(idempotencyKey: $idempotencyKey, sellerId: $sellerId, reason: $reason) { id tenant_id: tenantId handle resolved_locale: resolvedLocale display_name: displayName legal_name: legalName status onboarding_status: onboardingStatus onboarding_note: onboardingNote suspension_reason: suspensionReason metadata created_at: createdAt updated_at: updatedAt activated_at: activatedAt suspended_at: suspendedAt } }";
const REACTIVATE_MUTATION: &str = "mutation MarketplaceSellerAdminReactivate($idempotencyKey: String!, $sellerId: UUID!) { result: reactivateMarketplaceSeller(idempotencyKey: $idempotencyKey, sellerId: $sellerId) { id tenant_id: tenantId handle resolved_locale: resolvedLocale display_name: displayName legal_name: legalName status onboarding_status: onboardingStatus onboarding_note: onboardingNote suspension_reason: suspensionReason metadata created_at: createdAt updated_at: updatedAt activated_at: activatedAt suspended_at: suspendedAt } }";
const ADD_MEMBER_MUTATION: &str = "mutation MarketplaceSellerAdminAddMember($idempotencyKey: String!, $sellerId: UUID!, $input: MarketplaceSellerMemberCreateInputGql!) { result: addMarketplaceSellerMember(idempotencyKey: $idempotencyKey, sellerId: $sellerId, input: $input) { id seller_id: sellerId user_id: userId role status invited_by_actor_id: invitedByActorId accepted_at: acceptedAt metadata created_at: createdAt updated_at: updatedAt } }";
const UPDATE_MEMBER_MUTATION: &str = "mutation MarketplaceSellerAdminUpdateMember($idempotencyKey: String!, $sellerId: UUID!, $memberId: UUID!, $input: MarketplaceSellerMemberUpdateInputGql!) { result: updateMarketplaceSellerMember(idempotencyKey: $idempotencyKey, sellerId: $sellerId, memberId: $memberId, input: $input) { id seller_id: sellerId user_id: userId role status invited_by_actor_id: invitedByActorId accepted_at: acceptedAt metadata created_at: createdAt updated_at: updatedAt } }";

#[derive(Debug, Serialize)]
struct DirectoryVariables {
    page: i32,
    #[serde(rename = "perPage")]
    per_page: i32,
    status: Option<String>,
    #[serde(rename = "onboardingStatus")]
    onboarding_status: Option<String>,
    search: Option<String>,
}

#[derive(Debug, Serialize)]
struct IdVariables {
    id: String,
}

#[derive(Debug, Deserialize)]
struct DirectoryResponse {
    #[serde(rename = "marketplaceSellers")]
    directory: DirectoryWire,
}

#[derive(Debug, Deserialize)]
struct DirectoryWire {
    items: Vec<ListItemWire>,
    total: u64,
    page: u64,
    per_page: u64,
}

#[derive(Debug, Deserialize)]
struct ListItemWire {
    id: String,
    handle: String,
    resolved_locale: String,
    display_name: String,
    status: String,
    onboarding_status: String,
}

#[derive(Debug, Deserialize)]
struct DetailResponse {
    seller: SellerWire,
    members: Vec<MemberWire>,
}

#[derive(Debug, Deserialize)]
struct SellerMutationResponse {
    result: SellerWire,
}

#[derive(Debug, Deserialize)]
struct MemberMutationResponse {
    result: MemberWire,
}

#[derive(Debug, Deserialize)]
struct SellerWire {
    id: String,
    tenant_id: String,
    handle: String,
    resolved_locale: String,
    display_name: String,
    legal_name: Option<String>,
    status: String,
    onboarding_status: String,
    onboarding_note: Option<String>,
    suspension_reason: Option<String>,
    metadata: serde_json::Value,
    created_at: String,
    updated_at: String,
    activated_at: Option<String>,
    suspended_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MemberWire {
    id: String,
    seller_id: String,
    user_id: String,
    role: String,
    status: String,
    invited_by_actor_id: Option<String>,
    accepted_at: Option<String>,
    metadata: serde_json::Value,
    created_at: String,
    updated_at: String,
}

pub async fn load_directory(
    token: Option<String>,
    tenant_slug: Option<String>,
    filters: MarketplaceSellerAdminFilters,
) -> Result<MarketplaceSellerAdminDirectory, GraphqlMarketplaceSellerAdminError> {
    let page = filters.page.max(1);
    let per_page = filters.per_page.clamp(1, 100);
    let response: DirectoryResponse = request(
        DIRECTORY_QUERY,
        DirectoryVariables {
            page: page.min(i32::MAX as u64) as i32,
            per_page: per_page.min(i32::MAX as u64) as i32,
            status: graphql_enum(filters.status),
            onboarding_status: graphql_enum(filters.onboarding_status),
            search: normalize_optional_text(filters.search),
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(MarketplaceSellerAdminDirectory {
        items: response
            .directory
            .items
            .into_iter()
            .map(|item| MarketplaceSellerAdminListItem {
                id: item.id,
                handle: item.handle,
                resolved_locale: item.resolved_locale,
                display_name: item.display_name,
                status: normalize_enum_output(item.status),
                onboarding_status: normalize_enum_output(item.onboarding_status),
            })
            .collect(),
        total: response.directory.total,
        page: response.directory.page,
        per_page: response.directory.per_page,
    })
}

pub async fn load_detail(
    token: Option<String>,
    tenant_slug: Option<String>,
    seller_id: String,
) -> Result<MarketplaceSellerAdminDetail, GraphqlMarketplaceSellerAdminError> {
    let response: DetailResponse = request(
        DETAIL_QUERY,
        IdVariables { id: seller_id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(MarketplaceSellerAdminDetail {
        seller: response.seller.into(),
        members: response.members.into_iter().map(Into::into).collect(),
    })
}

pub async fn execute_command(
    token: Option<String>,
    tenant_slug: Option<String>,
    idempotency_key: String,
    command: MarketplaceSellerAdminCommand,
) -> Result<MarketplaceSellerAdminCommandResult, GraphqlMarketplaceSellerAdminError> {
    match command {
        MarketplaceSellerAdminCommand::Create { draft } => {
            let response: SellerMutationResponse = request(
                CREATE_MUTATION,
                serde_json::json!({
                    "idempotencyKey": idempotency_key,
                    "input": {
                        "handle": draft.handle,
                        "displayName": draft.display_name,
                        "legalName": draft.legal_name,
                        "ownerUserId": draft.owner_user_id,
                        "metadata": object_or_empty(draft.metadata)?,
                    }
                }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(seller_result(response.result))
        }
        MarketplaceSellerAdminCommand::UpdateProfile { seller_id, draft } => {
            let response: SellerMutationResponse = request(
                UPDATE_PROFILE_MUTATION,
                serde_json::json!({
                    "idempotencyKey": idempotency_key,
                    "sellerId": seller_id,
                    "input": {
                        "displayName": normalize_optional_text(draft.display_name),
                        "legalName": normalize_optional_text(draft.legal_name),
                        "metadata": draft.metadata.map(object_or_empty).transpose()?,
                    }
                }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(seller_result(response.result))
        }
        MarketplaceSellerAdminCommand::SubmitOnboarding { seller_id, note } => {
            seller_command(
                SUBMIT_MUTATION,
                serde_json::json!({
                    "idempotencyKey": idempotency_key,
                    "sellerId": seller_id,
                    "note": normalize_optional_text(note),
                }),
                token,
                tenant_slug,
            )
            .await
        }
        MarketplaceSellerAdminCommand::ReviewOnboarding {
            seller_id,
            approved,
            note,
        } => {
            seller_command(
                REVIEW_MUTATION,
                serde_json::json!({
                    "idempotencyKey": idempotency_key,
                    "sellerId": seller_id,
                    "approved": approved,
                    "note": normalize_optional_text(note),
                }),
                token,
                tenant_slug,
            )
            .await
        }
        MarketplaceSellerAdminCommand::Suspend { seller_id, reason } => {
            seller_command(
                SUSPEND_MUTATION,
                serde_json::json!({
                    "idempotencyKey": idempotency_key,
                    "sellerId": seller_id,
                    "reason": reason,
                }),
                token,
                tenant_slug,
            )
            .await
        }
        MarketplaceSellerAdminCommand::Reactivate { seller_id } => {
            seller_command(
                REACTIVATE_MUTATION,
                serde_json::json!({
                    "idempotencyKey": idempotency_key,
                    "sellerId": seller_id,
                }),
                token,
                tenant_slug,
            )
            .await
        }
        MarketplaceSellerAdminCommand::AddMember { seller_id, draft } => {
            let response: MemberMutationResponse = request(
                ADD_MEMBER_MUTATION,
                serde_json::json!({
                    "idempotencyKey": idempotency_key,
                    "sellerId": seller_id,
                    "input": {
                        "userId": draft.user_id,
                        "role": graphql_enum_required(draft.role, "member role")?,
                        "metadata": object_or_empty(draft.metadata)?,
                    }
                }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(member_result(response.result))
        }
        MarketplaceSellerAdminCommand::UpdateMember {
            seller_id,
            member_id,
            draft,
        } => {
            let response: MemberMutationResponse = request(
                UPDATE_MEMBER_MUTATION,
                serde_json::json!({
                    "idempotencyKey": idempotency_key,
                    "sellerId": seller_id,
                    "memberId": member_id,
                    "input": {
                        "role": draft.role.map(|value| graphql_enum_required(value, "member role")).transpose()?,
                        "status": draft.status.map(|value| graphql_enum_required(value, "member status")).transpose()?,
                        "metadata": draft.metadata.map(object_or_empty).transpose()?,
                    }
                }),
                token,
                tenant_slug,
            )
            .await?;
            Ok(member_result(response.result))
        }
    }
}

async fn seller_command(
    query: &str,
    variables: serde_json::Value,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MarketplaceSellerAdminCommandResult, GraphqlMarketplaceSellerAdminError> {
    let response: SellerMutationResponse = request(query, variables, token, tenant_slug).await?;
    Ok(seller_result(response.result))
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, GraphqlMarketplaceSellerAdminError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

impl From<SellerWire> for MarketplaceSellerAdminRecord {
    fn from(value: SellerWire) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            handle: value.handle,
            resolved_locale: value.resolved_locale,
            display_name: value.display_name,
            legal_name: value.legal_name,
            status: normalize_enum_output(value.status),
            onboarding_status: normalize_enum_output(value.onboarding_status),
            onboarding_note: value.onboarding_note,
            suspension_reason: value.suspension_reason,
            metadata: value.metadata,
            created_at: value.created_at,
            updated_at: value.updated_at,
            activated_at: value.activated_at,
            suspended_at: value.suspended_at,
        }
    }
}

impl From<MemberWire> for MarketplaceSellerAdminMember {
    fn from(value: MemberWire) -> Self {
        Self {
            id: value.id,
            seller_id: value.seller_id,
            user_id: value.user_id,
            role: normalize_enum_output(value.role),
            status: normalize_enum_output(value.status),
            invited_by_actor_id: value.invited_by_actor_id,
            accepted_at: value.accepted_at,
            metadata: value.metadata,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

fn seller_result(value: SellerWire) -> MarketplaceSellerAdminCommandResult {
    MarketplaceSellerAdminCommandResult {
        seller: Some(value.into()),
        member: None,
    }
}

fn member_result(value: MemberWire) -> MarketplaceSellerAdminCommandResult {
    MarketplaceSellerAdminCommandResult {
        seller: None,
        member: Some(value.into()),
    }
}

fn graphql_enum(value: Option<String>) -> Option<String> {
    normalize_optional_text(value).map(|value| value.to_ascii_uppercase())
}

fn graphql_enum_required(
    value: String,
    label: &str,
) -> Result<String, GraphqlMarketplaceSellerAdminError> {
    normalize_optional_text(Some(value))
        .map(|value| value.to_ascii_uppercase())
        .ok_or_else(|| format!("{label} is required"))
}

fn normalize_enum_output(value: String) -> String {
    value.to_ascii_lowercase()
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn object_or_empty(
    value: serde_json::Value,
) -> Result<serde_json::Value, GraphqlMarketplaceSellerAdminError> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err("metadata must be a JSON object".to_string()),
    }
}

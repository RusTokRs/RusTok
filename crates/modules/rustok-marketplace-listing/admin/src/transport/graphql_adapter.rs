#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::model::{
    MarketplaceListingAdminCommand, MarketplaceListingAdminCommandResult,
    MarketplaceListingAdminDetail, MarketplaceListingAdminDirectory, MarketplaceListingAdminEvent,
    MarketplaceListingAdminFilters, MarketplaceListingAdminListItem, MarketplaceListingAdminRecord,
    MarketplaceListingAdminTerms,
};

pub type GraphqlMarketplaceListingAdminError = String;

const DIRECTORY_QUERY: &str = "query MarketplaceListingAdminDirectory($page: Int, $perPage: Int, $sellerId: UUID, $masterVariantId: UUID, $marketSlug: String, $channelSlug: String, $status: MarketplaceListingStatusGql, $approvalStatus: MarketplaceListingApprovalStatusGql, $search: String) { marketplaceListings(page: $page, perPage: $perPage, sellerId: $sellerId, masterVariantId: $masterVariantId, marketSlug: $marketSlug, channelSlug: $channelSlug, status: $status, approvalStatus: $approvalStatus, search: $search) { total page per_page: perPage items { id seller_id: sellerId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion } } }";
const DETAIL_QUERY: &str = "query MarketplaceListingAdminDetail($id: UUID!) { listing: marketplaceListing(id: $id) { id tenant_id: tenantId seller_id: sellerId master_product_id: masterProductId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion current_terms: currentTerms { id listing_id: listingId version pricing_reference: pricingReference inventory_reference: inventoryReference fulfillment_profile_slug: fulfillmentProfileSlug metadata created_at: createdAt } metadata published_at: publishedAt approved_at: approvedAt created_at: createdAt updated_at: updatedAt } events: marketplaceListingEvents(listingId: $id, limit: 200) { id listing_id: listingId actor_id: actorId event_kind: eventKind locale provenance note metadata created_at: createdAt } }";
const CREATE_MUTATION: &str = "mutation MarketplaceListingAdminCreate($idempotencyKey: String!, $input: MarketplaceListingCreateInputGql!) { result: createMarketplaceListing(idempotencyKey: $idempotencyKey, input: $input) { id tenant_id: tenantId seller_id: sellerId master_product_id: masterProductId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion current_terms: currentTerms { id listing_id: listingId version pricing_reference: pricingReference inventory_reference: inventoryReference fulfillment_profile_slug: fulfillmentProfileSlug metadata created_at: createdAt } metadata published_at: publishedAt approved_at: approvedAt created_at: createdAt updated_at: updatedAt } }";
const UPDATE_TERMS_MUTATION: &str = "mutation MarketplaceListingAdminUpdateTerms($idempotencyKey: String!, $listingId: UUID!, $input: MarketplaceListingTermsInputGql!) { result: updateMarketplaceListingTerms(idempotencyKey: $idempotencyKey, listingId: $listingId, input: $input) { id tenant_id: tenantId seller_id: sellerId master_product_id: masterProductId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion current_terms: currentTerms { id listing_id: listingId version pricing_reference: pricingReference inventory_reference: inventoryReference fulfillment_profile_slug: fulfillmentProfileSlug metadata created_at: createdAt } metadata published_at: publishedAt approved_at: approvedAt created_at: createdAt updated_at: updatedAt } }";
const SUBMIT_MUTATION: &str = "mutation MarketplaceListingAdminSubmit($idempotencyKey: String!, $listingId: UUID!) { result: submitMarketplaceListingForReview(idempotencyKey: $idempotencyKey, listingId: $listingId) { id tenant_id: tenantId seller_id: sellerId master_product_id: masterProductId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion current_terms: currentTerms { id listing_id: listingId version pricing_reference: pricingReference inventory_reference: inventoryReference fulfillment_profile_slug: fulfillmentProfileSlug metadata created_at: createdAt } metadata published_at: publishedAt approved_at: approvedAt created_at: createdAt updated_at: updatedAt } }";
const REVIEW_MUTATION: &str = "mutation MarketplaceListingAdminReview($idempotencyKey: String!, $listingId: UUID!, $approved: Boolean!, $note: String) { result: reviewMarketplaceListing(idempotencyKey: $idempotencyKey, listingId: $listingId, approved: $approved, note: $note) { id tenant_id: tenantId seller_id: sellerId master_product_id: masterProductId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion current_terms: currentTerms { id listing_id: listingId version pricing_reference: pricingReference inventory_reference: inventoryReference fulfillment_profile_slug: fulfillmentProfileSlug metadata created_at: createdAt } metadata published_at: publishedAt approved_at: approvedAt created_at: createdAt updated_at: updatedAt } }";
const PUBLISH_MUTATION: &str = "mutation MarketplaceListingAdminPublish($idempotencyKey: String!, $listingId: UUID!) { result: publishMarketplaceListing(idempotencyKey: $idempotencyKey, listingId: $listingId) { id tenant_id: tenantId seller_id: sellerId master_product_id: masterProductId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion current_terms: currentTerms { id listing_id: listingId version pricing_reference: pricingReference inventory_reference: inventoryReference fulfillment_profile_slug: fulfillmentProfileSlug metadata created_at: createdAt } metadata published_at: publishedAt approved_at: approvedAt created_at: createdAt updated_at: updatedAt } }";
const SUSPEND_MUTATION: &str = "mutation MarketplaceListingAdminSuspend($idempotencyKey: String!, $listingId: UUID!, $reason: String!) { result: suspendMarketplaceListing(idempotencyKey: $idempotencyKey, listingId: $listingId, reason: $reason) { id tenant_id: tenantId seller_id: sellerId master_product_id: masterProductId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion current_terms: currentTerms { id listing_id: listingId version pricing_reference: pricingReference inventory_reference: inventoryReference fulfillment_profile_slug: fulfillmentProfileSlug metadata created_at: createdAt } metadata published_at: publishedAt approved_at: approvedAt created_at: createdAt updated_at: updatedAt } }";
const REACTIVATE_MUTATION: &str = "mutation MarketplaceListingAdminReactivate($idempotencyKey: String!, $listingId: UUID!) { result: reactivateMarketplaceListing(idempotencyKey: $idempotencyKey, listingId: $listingId) { id tenant_id: tenantId seller_id: sellerId master_product_id: masterProductId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion current_terms: currentTerms { id listing_id: listingId version pricing_reference: pricingReference inventory_reference: inventoryReference fulfillment_profile_slug: fulfillmentProfileSlug metadata created_at: createdAt } metadata published_at: publishedAt approved_at: approvedAt created_at: createdAt updated_at: updatedAt } }";
const ARCHIVE_MUTATION: &str = "mutation MarketplaceListingAdminArchive($idempotencyKey: String!, $listingId: UUID!) { result: archiveMarketplaceListing(idempotencyKey: $idempotencyKey, listingId: $listingId) { id tenant_id: tenantId seller_id: sellerId master_product_id: masterProductId master_variant_id: masterVariantId seller_sku: sellerSku market_slug: marketSlug channel_slug: channelSlug status approval_status: approvalStatus current_terms_version: currentTermsVersion current_terms: currentTerms { id listing_id: listingId version pricing_reference: pricingReference inventory_reference: inventoryReference fulfillment_profile_slug: fulfillmentProfileSlug metadata created_at: createdAt } metadata published_at: publishedAt approved_at: approvedAt created_at: createdAt updated_at: updatedAt } }";

#[derive(Debug, Serialize)]
struct DirectoryVariables {
    page: i32,
    #[serde(rename = "perPage")]
    per_page: i32,
    #[serde(rename = "sellerId")]
    seller_id: Option<String>,
    #[serde(rename = "masterVariantId")]
    master_variant_id: Option<String>,
    #[serde(rename = "marketSlug")]
    market_slug: Option<String>,
    #[serde(rename = "channelSlug")]
    channel_slug: Option<String>,
    status: Option<String>,
    #[serde(rename = "approvalStatus")]
    approval_status: Option<String>,
    search: Option<String>,
}

#[derive(Debug, Serialize)]
struct IdVariables {
    id: String,
}

#[derive(Debug, Deserialize)]
struct DirectoryResponse {
    #[serde(rename = "marketplaceListings")]
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
    seller_id: String,
    master_variant_id: String,
    seller_sku: String,
    market_slug: String,
    channel_slug: String,
    status: String,
    approval_status: String,
    current_terms_version: i32,
}

#[derive(Debug, Deserialize)]
struct DetailResponse {
    listing: ListingWire,
    events: Vec<EventWire>,
}

#[derive(Debug, Deserialize)]
struct MutationResponse {
    result: ListingWire,
}

#[derive(Debug, Deserialize)]
struct ListingWire {
    id: String,
    tenant_id: String,
    seller_id: String,
    master_product_id: String,
    master_variant_id: String,
    seller_sku: String,
    market_slug: String,
    channel_slug: String,
    status: String,
    approval_status: String,
    current_terms_version: i32,
    current_terms: TermsWire,
    metadata: serde_json::Value,
    published_at: Option<String>,
    approved_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct TermsWire {
    id: String,
    listing_id: String,
    version: i32,
    pricing_reference: Option<String>,
    inventory_reference: Option<String>,
    fulfillment_profile_slug: Option<String>,
    metadata: serde_json::Value,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct EventWire {
    id: String,
    listing_id: String,
    actor_id: Option<String>,
    event_kind: String,
    locale: Option<String>,
    provenance: String,
    note: Option<String>,
    metadata: serde_json::Value,
    created_at: String,
}

pub async fn load_directory(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
    filters: MarketplaceListingAdminFilters,
) -> Result<MarketplaceListingAdminDirectory, GraphqlMarketplaceListingAdminError> {
    let page = filters.page.max(1);
    let per_page = filters.per_page.clamp(1, 100);
    let response: DirectoryResponse = request(
        DIRECTORY_QUERY,
        DirectoryVariables {
            page: page.min(i32::MAX as u64) as i32,
            per_page: per_page.min(i32::MAX as u64) as i32,
            seller_id: normalize_optional_text(filters.seller_id),
            master_variant_id: normalize_optional_text(filters.master_variant_id),
            market_slug: normalize_optional_text(filters.market_slug),
            channel_slug: normalize_optional_text(filters.channel_slug),
            status: graphql_enum(filters.status),
            approval_status: graphql_enum(filters.approval_status),
            search: normalize_optional_text(filters.search),
        },
        token,
        tenant_slug,
        locale,
    )
    .await?;
    Ok(MarketplaceListingAdminDirectory {
        items: response
            .directory
            .items
            .into_iter()
            .map(|item| MarketplaceListingAdminListItem {
                id: item.id,
                seller_id: item.seller_id,
                master_variant_id: item.master_variant_id,
                seller_sku: item.seller_sku,
                market_slug: item.market_slug,
                channel_slug: item.channel_slug,
                status: normalize_enum_output(item.status),
                approval_status: normalize_enum_output(item.approval_status),
                current_terms_version: item.current_terms_version,
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
    locale: Option<String>,
    listing_id: String,
) -> Result<MarketplaceListingAdminDetail, GraphqlMarketplaceListingAdminError> {
    let response: DetailResponse = request(
        DETAIL_QUERY,
        IdVariables { id: listing_id },
        token,
        tenant_slug,
        locale,
    )
    .await?;
    Ok(MarketplaceListingAdminDetail {
        listing: response.listing.into(),
        events: response.events.into_iter().map(Into::into).collect(),
    })
}

pub async fn execute_command(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
    idempotency_key: String,
    command: MarketplaceListingAdminCommand,
) -> Result<MarketplaceListingAdminCommandResult, GraphqlMarketplaceListingAdminError> {
    let (query, variables) = match command {
        MarketplaceListingAdminCommand::Create { draft } => (
            CREATE_MUTATION,
            serde_json::json!({
                "idempotencyKey": idempotency_key,
                "input": {
                    "sellerId": draft.seller_id,
                    "masterVariantId": draft.master_variant_id,
                    "sellerSku": draft.seller_sku,
                    "marketSlug": draft.market_slug,
                    "channelSlug": draft.channel_slug,
                    "pricingReference": normalize_optional_text(draft.pricing_reference),
                    "inventoryReference": normalize_optional_text(draft.inventory_reference),
                    "fulfillmentProfileSlug": normalize_optional_text(draft.fulfillment_profile_slug),
                    "metadata": object_or_empty(draft.metadata)?,
                }
            }),
        ),
        MarketplaceListingAdminCommand::UpdateTerms { listing_id, draft } => (
            UPDATE_TERMS_MUTATION,
            serde_json::json!({
                "idempotencyKey": idempotency_key,
                "listingId": listing_id,
                "input": {
                    "pricingReference": normalize_optional_text(draft.pricing_reference),
                    "inventoryReference": normalize_optional_text(draft.inventory_reference),
                    "fulfillmentProfileSlug": normalize_optional_text(draft.fulfillment_profile_slug),
                    "metadata": object_or_empty(draft.metadata)?,
                }
            }),
        ),
        MarketplaceListingAdminCommand::SubmitForReview { listing_id } => {
            (SUBMIT_MUTATION, id_variables(idempotency_key, listing_id))
        }
        MarketplaceListingAdminCommand::Review {
            listing_id,
            approved,
            note,
        } => (
            REVIEW_MUTATION,
            serde_json::json!({
                "idempotencyKey": idempotency_key,
                "listingId": listing_id,
                "approved": approved,
                "note": normalize_optional_text(note),
            }),
        ),
        MarketplaceListingAdminCommand::Publish { listing_id } => {
            (PUBLISH_MUTATION, id_variables(idempotency_key, listing_id))
        }
        MarketplaceListingAdminCommand::Suspend { listing_id, reason } => (
            SUSPEND_MUTATION,
            serde_json::json!({
                "idempotencyKey": idempotency_key,
                "listingId": listing_id,
                "reason": reason,
            }),
        ),
        MarketplaceListingAdminCommand::Reactivate { listing_id } => (
            REACTIVATE_MUTATION,
            id_variables(idempotency_key, listing_id),
        ),
        MarketplaceListingAdminCommand::Archive { listing_id } => {
            (ARCHIVE_MUTATION, id_variables(idempotency_key, listing_id))
        }
    };
    let response: MutationResponse = request(query, variables, token, tenant_slug, locale).await?;
    Ok(MarketplaceListingAdminCommandResult {
        listing: response.result.into(),
    })
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: Option<String>,
) -> Result<T, GraphqlMarketplaceListingAdminError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        locale,
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

impl From<ListingWire> for MarketplaceListingAdminRecord {
    fn from(value: ListingWire) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            seller_id: value.seller_id,
            master_product_id: value.master_product_id,
            master_variant_id: value.master_variant_id,
            seller_sku: value.seller_sku,
            market_slug: value.market_slug,
            channel_slug: value.channel_slug,
            status: normalize_enum_output(value.status),
            approval_status: normalize_enum_output(value.approval_status),
            current_terms_version: value.current_terms_version,
            current_terms: value.current_terms.into(),
            metadata: value.metadata,
            published_at: value.published_at,
            approved_at: value.approved_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<TermsWire> for MarketplaceListingAdminTerms {
    fn from(value: TermsWire) -> Self {
        Self {
            id: value.id,
            listing_id: value.listing_id,
            version: value.version,
            pricing_reference: value.pricing_reference,
            inventory_reference: value.inventory_reference,
            fulfillment_profile_slug: value.fulfillment_profile_slug,
            metadata: value.metadata,
            created_at: value.created_at,
        }
    }
}

impl From<EventWire> for MarketplaceListingAdminEvent {
    fn from(value: EventWire) -> Self {
        Self {
            id: value.id,
            listing_id: value.listing_id,
            actor_id: value.actor_id,
            event_kind: normalize_enum_output(value.event_kind),
            locale: value.locale,
            provenance: normalize_enum_output(value.provenance),
            note: value.note,
            metadata: value.metadata,
            created_at: value.created_at,
        }
    }
}

fn id_variables(idempotency_key: String, listing_id: String) -> serde_json::Value {
    serde_json::json!({
        "idempotencyKey": idempotency_key,
        "listingId": listing_id,
    })
}

fn graphql_enum(value: Option<String>) -> Option<String> {
    normalize_optional_text(value).map(|value| value.to_ascii_uppercase())
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
) -> Result<serde_json::Value, GraphqlMarketplaceListingAdminError> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err("metadata must be a JSON object".to_string()),
    }
}

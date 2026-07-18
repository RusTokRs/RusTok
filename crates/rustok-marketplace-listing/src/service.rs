use std::sync::Arc;

use rustok_api::{PortContext, PortErrorKind};
use rustok_marketplace_seller::{
    MarketplaceSellerReadPort, MarketplaceSellerStatus, ReadMarketplaceSellerRequest,
};
use rustok_outbox::TransactionalEventBus;
use rustok_product::ProductCatalogReadPort;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::dto::{
    ListMarketplaceListingsInput, MarketplaceListingApprovalStatus,
    MarketplaceListingEligibilityProjection, MarketplaceListingEligibilityRequest,
    MarketplaceListingListResponse, MarketplaceListingResponse, MarketplaceListingStatus,
    MarketplaceListingTermsResponse,
};
use crate::entities::{listing, listing_terms};
use crate::error::{MarketplaceListingError, MarketplaceListingResult};

const MAX_LISTINGS_PER_PAGE: u64 = 100;

pub struct MarketplaceListingService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    seller_reader: Arc<dyn MarketplaceSellerReadPort>,
    product_reader: Arc<dyn ProductCatalogReadPort>,
}

impl MarketplaceListingService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        seller_reader: Arc<dyn MarketplaceSellerReadPort>,
        product_reader: Arc<dyn ProductCatalogReadPort>,
    ) -> Self {
        Self {
            db,
            event_bus,
            seller_reader,
            product_reader,
        }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub(crate) fn event_bus(&self) -> &TransactionalEventBus {
        &self.event_bus
    }

    pub(crate) fn seller_reader(&self) -> &dyn MarketplaceSellerReadPort {
        self.seller_reader.as_ref()
    }

    pub(crate) fn product_reader(&self) -> &dyn ProductCatalogReadPort {
        self.product_reader.as_ref()
    }

    pub async fn get_listing(
        &self,
        tenant_id: Uuid,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        load_listing_response(&self.db, tenant_id, listing_id).await
    }

    pub async fn list_listings(
        &self,
        tenant_id: Uuid,
        input: ListMarketplaceListingsInput,
    ) -> MarketplaceListingResult<MarketplaceListingListResponse> {
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, MAX_LISTINGS_PER_PAGE);
        let mut query = listing::Entity::find().filter(listing::Column::TenantId.eq(tenant_id));
        if let Some(seller_id) = input.seller_id {
            query = query.filter(listing::Column::SellerId.eq(seller_id));
        }
        if let Some(variant_id) = input.master_variant_id {
            query = query.filter(listing::Column::MasterVariantId.eq(variant_id));
        }
        if let Some(market_slug) = normalize_optional_text(input.market_slug) {
            query = query.filter(listing::Column::MarketSlug.eq(market_slug));
        }
        if let Some(channel_slug) = normalize_optional_text(input.channel_slug) {
            query = query.filter(listing::Column::ChannelSlug.eq(channel_slug));
        }
        if let Some(status) = input.status {
            query = query.filter(listing::Column::Status.eq(status.as_str()));
        }
        if let Some(status) = input.approval_status {
            query = query.filter(listing::Column::ApprovalStatus.eq(status.as_str()));
        }
        if let Some(search) = normalize_optional_text(input.search) {
            query = query.filter(
                Condition::any()
                    .add(listing::Column::SellerSku.contains(search.as_str()))
                    .add(listing::Column::MarketSlug.contains(search.as_str()))
                    .add(listing::Column::ChannelSlug.contains(search.as_str())),
            );
        }
        let paginator = query
            .order_by_desc(listing::Column::UpdatedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let models = paginator.fetch_page(page.saturating_sub(1)).await?;
        let mut items = Vec::with_capacity(models.len());
        for model in models {
            items.push(load_response_for_model(&self.db, model).await?);
        }
        Ok(MarketplaceListingListResponse { items, total })
    }

    pub async fn list_eligibility(
        &self,
        context: PortContext,
        request: MarketplaceListingEligibilityRequest,
    ) -> MarketplaceListingResult<Vec<MarketplaceListingEligibilityProjection>> {
        let tenant_id = parse_tenant_id(&context)?;
        let market_slug = required_slug(request.market_slug, "market_slug")?;
        let channel_slug = required_slug(request.channel_slug, "channel_slug")?;
        let models = listing::Entity::find()
            .filter(listing::Column::TenantId.eq(tenant_id))
            .filter(listing::Column::MasterVariantId.eq(request.master_variant_id))
            .filter(listing::Column::MarketSlug.eq(market_slug))
            .filter(listing::Column::ChannelSlug.eq(channel_slug))
            .order_by_asc(listing::Column::SellerId)
            .order_by_asc(listing::Column::Id)
            .all(&self.db)
            .await?;
        let mut output = Vec::with_capacity(models.len());
        for model in models {
            let response = load_response_for_model(&self.db, model).await?;
            let mut reasons = listing_reason_codes(&response);
            match self
                .seller_reader
                .read_seller(
                    context.clone(),
                    ReadMarketplaceSellerRequest {
                        seller_id: response.seller_id,
                    },
                )
                .await
            {
                Ok(seller) if seller.status == MarketplaceSellerStatus::Active => {}
                Ok(_) => reasons.push("seller_not_active".to_string()),
                Err(_) => reasons.push("seller_unavailable".to_string()),
            }
            output.push(MarketplaceListingEligibilityProjection {
                eligible: reasons.is_empty(),
                listing: response,
                reason_codes: reasons,
            });
        }
        Ok(output)
    }
}

pub(crate) async fn ensure_listing_identity_available<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    variant_id: Uuid,
    market_slug: &str,
    channel_slug: &str,
    seller_sku: &str,
) -> MarketplaceListingResult<()> {
    if listing::Entity::find()
        .filter(listing::Column::TenantId.eq(tenant_id))
        .filter(listing::Column::SellerId.eq(seller_id))
        .filter(listing::Column::MasterVariantId.eq(variant_id))
        .filter(listing::Column::MarketSlug.eq(market_slug))
        .filter(listing::Column::ChannelSlug.eq(channel_slug))
        .one(connection)
        .await?
        .is_some()
    {
        return Err(MarketplaceListingError::DuplicateScope);
    }
    if listing::Entity::find()
        .filter(listing::Column::TenantId.eq(tenant_id))
        .filter(listing::Column::SellerId.eq(seller_id))
        .filter(listing::Column::SellerSku.eq(seller_sku))
        .one(connection)
        .await?
        .is_some()
    {
        return Err(MarketplaceListingError::DuplicateSellerSku(
            seller_sku.to_string(),
        ));
    }
    Ok(())
}

pub(crate) async fn find_listing<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    listing_id: Uuid,
) -> MarketplaceListingResult<listing::Model> {
    listing::Entity::find_by_id(listing_id)
        .filter(listing::Column::TenantId.eq(tenant_id))
        .one(connection)
        .await?
        .ok_or(MarketplaceListingError::ListingNotFound(listing_id))
}

pub(crate) async fn find_current_terms<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    listing_id: Uuid,
    version: i32,
) -> MarketplaceListingResult<listing_terms::Model> {
    listing_terms::Entity::find()
        .filter(listing_terms::Column::TenantId.eq(tenant_id))
        .filter(listing_terms::Column::ListingId.eq(listing_id))
        .filter(listing_terms::Column::Version.eq(version))
        .one(connection)
        .await?
        .ok_or(MarketplaceListingError::TermsNotFound {
            listing_id,
            version,
        })
}

pub(crate) async fn load_listing_response<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    listing_id: Uuid,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let model = find_listing(connection, tenant_id, listing_id).await?;
    load_response_for_model(connection, model).await
}

pub(crate) async fn load_response_for_model<C: ConnectionTrait>(
    connection: &C,
    model: listing::Model,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let terms = find_current_terms(
        connection,
        model.tenant_id,
        model.id,
        model.current_terms_version,
    )
    .await?;
    map_listing(model, terms)
}

pub(crate) fn map_listing(
    model: listing::Model,
    terms: listing_terms::Model,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let status = MarketplaceListingStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplaceListingError::Validation(format!(
            "unknown marketplace listing status `{}`",
            model.status
        ))
    })?;
    let approval_status = MarketplaceListingApprovalStatus::parse(model.approval_status.as_str())
        .ok_or_else(|| {
        MarketplaceListingError::Validation(format!(
            "unknown marketplace listing approval status `{}`",
            model.approval_status
        ))
    })?;
    Ok(MarketplaceListingResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        seller_id: model.seller_id,
        master_product_id: model.master_product_id,
        master_variant_id: model.master_variant_id,
        seller_sku: model.seller_sku,
        market_slug: model.market_slug,
        channel_slug: model.channel_slug,
        status,
        approval_status,
        current_terms_version: model.current_terms_version,
        current_terms: MarketplaceListingTermsResponse {
            id: terms.id,
            listing_id: terms.listing_id,
            version: terms.version,
            pricing_reference: terms.pricing_reference,
            inventory_reference: terms.inventory_reference,
            fulfillment_profile_slug: terms.fulfillment_profile_slug,
            metadata: terms.metadata,
            created_at: terms.created_at,
        },
        metadata: model.metadata,
        published_at: model.published_at,
        approved_at: model.approved_at,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

pub(crate) fn listing_reason_codes_without_lifecycle(
    listing: &MarketplaceListingResponse,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if listing.approval_status != MarketplaceListingApprovalStatus::Approved {
        reasons.push("listing_not_approved".to_string());
    }
    if listing.current_terms.pricing_reference.is_none() {
        reasons.push("pricing_reference_missing".to_string());
    }
    if listing.current_terms.inventory_reference.is_none() {
        reasons.push("inventory_reference_missing".to_string());
    }
    reasons
}

pub(crate) fn map_seller_port_error(error: rustok_api::PortError) -> MarketplaceListingError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            MarketplaceListingError::Validation(error.message)
        }
        _ => MarketplaceListingError::SellerUnavailable(error.code),
    }
}

pub(crate) fn map_product_port_error(error: rustok_api::PortError) -> MarketplaceListingError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            MarketplaceListingError::Validation(error.message)
        }
        _ => MarketplaceListingError::ProductUnavailable(error.code),
    }
}

pub(crate) fn map_listing_insert_error(error: sea_orm::DbErr) -> MarketplaceListingError {
    if matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    ) {
        MarketplaceListingError::DuplicateScope
    } else {
        error.into()
    }
}

fn listing_reason_codes(listing: &MarketplaceListingResponse) -> Vec<String> {
    let mut reasons = listing_reason_codes_without_lifecycle(listing);
    if listing.status != MarketplaceListingStatus::Active {
        reasons.push("listing_not_active".to_string());
    }
    if listing.published_at.is_none() {
        reasons.push("listing_not_published".to_string());
    }
    reasons
}

fn parse_tenant_id(context: &PortContext) -> MarketplaceListingResult<Uuid> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        MarketplaceListingError::Validation(
            "PortContext.tenant_id must be a UUID for marketplace listing ports".to_string(),
        )
    })
}

fn required_slug(value: String, field: &str) -> MarketplaceListingResult<String> {
    let value = value.trim().to_ascii_lowercase().replace('_', "-");
    if value.is_empty()
        || value.starts_with('-')
        || value.ends_with('-')
        || value
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || character == '-'))
    {
        return Err(MarketplaceListingError::Validation(format!(
            "{field} must contain lowercase ASCII letters, digits, or internal hyphens"
        )));
    }
    Ok(value)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

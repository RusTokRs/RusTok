use std::sync::Arc;

use chrono::Utc;
use rustok_api::{PortContext, PortErrorKind};
use rustok_core::generate_id;
use rustok_marketplace_seller::{
    MarketplaceSellerReadPort, MarketplaceSellerStatus, ReadMarketplaceSellerRequest,
};
use rustok_product::{ProductCatalogReadPort, VariantProductProjectionRequest};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;
use validator::Validate;

use crate::command_receipts::{
    admit, complete, normalize_idempotency_key, replay, request_hash, rollback,
    ListingCommandAdmission,
};
use crate::dto::{
    CreateMarketplaceListingInput, ListMarketplaceListingsInput,
    MarketplaceListingApprovalStatus, MarketplaceListingEligibilityProjection,
    MarketplaceListingEligibilityRequest, MarketplaceListingListResponse,
    MarketplaceListingResponse, MarketplaceListingStatus, MarketplaceListingTermsResponse,
    ReviewMarketplaceListingInput, SuspendMarketplaceListingInput,
    UpdateMarketplaceListingTermsInput,
};
use crate::entities::{listing, listing_terms};
use crate::error::{MarketplaceListingError, MarketplaceListingResult};

const MAX_LISTINGS_PER_PAGE: u64 = 100;

pub struct MarketplaceListingService {
    db: DatabaseConnection,
    seller_reader: Arc<dyn MarketplaceSellerReadPort>,
    product_reader: Arc<dyn ProductCatalogReadPort>,
}

impl MarketplaceListingService {
    pub fn new(
        db: DatabaseConnection,
        seller_reader: Arc<dyn MarketplaceSellerReadPort>,
        product_reader: Arc<dyn ProductCatalogReadPort>,
    ) -> Self {
        Self {
            db,
            seller_reader,
            product_reader,
        }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
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

    pub async fn create_listing(
        &self,
        context: PortContext,
        input: CreateMarketplaceListingInput,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        context.require_write_semantics().map_err(port_validation_error)?;
        input
            .validate()
            .map_err(|error| MarketplaceListingError::Validation(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let key = normalize_idempotency_key(required_idempotency_key(&context)?)?;
        let seller_sku = required_text(input.seller_sku, "seller_sku")?;
        let market_slug = required_slug(input.market_slug, "market_slug")?;
        let channel_slug = required_slug(input.channel_slug, "channel_slug")?;
        let pricing_reference = normalize_optional_text(input.pricing_reference);
        let inventory_reference = normalize_optional_text(input.inventory_reference);
        let fulfillment_profile_slug = normalize_optional_text(input.fulfillment_profile_slug);
        let metadata = object_or_empty(input.metadata, "metadata")?;
        let normalized = serde_json::json!({
            "seller_id": input.seller_id,
            "master_variant_id": input.master_variant_id,
            "seller_sku": seller_sku,
            "market_slug": market_slug,
            "channel_slug": channel_slug,
            "pricing_reference": pricing_reference,
            "inventory_reference": inventory_reference,
            "fulfillment_profile_slug": fulfillment_profile_slug,
            "metadata": metadata,
        });
        let hash = request_hash("create_listing", actor_id, &normalized)?;

        let seller = self
            .seller_reader
            .read_seller(
                context.clone(),
                ReadMarketplaceSellerRequest {
                    seller_id: input.seller_id,
                },
            )
            .await
            .map_err(map_seller_port_error)?;
        if seller.status == MarketplaceSellerStatus::Closed {
            return Err(MarketplaceListingError::Validation(
                "closed seller cannot create listings".to_string(),
            ));
        }
        let product = self
            .product_reader
            .read_variant_product_projection(
                context.clone(),
                VariantProductProjectionRequest {
                    variant_id: input.master_variant_id,
                    locale: None,
                    fallback_locale: None,
                },
            )
            .await
            .map_err(map_product_port_error)?;

        match admit(
            &self.db,
            tenant_id,
            actor_id,
            key,
            "create_listing",
            hash.as_str(),
        )
        .await?
        {
            ListingCommandAdmission::Replay(receipt) => {
                replay(receipt, "create_listing", hash.as_str())
            }
            ListingCommandAdmission::New(receipt) => {
                let result: MarketplaceListingResult<MarketplaceListingResponse> = async {
                    ensure_listing_identity_available(
                        &receipt.transaction,
                        tenant_id,
                        input.seller_id,
                        input.master_variant_id,
                        market_slug.as_str(),
                        channel_slug.as_str(),
                        seller_sku.as_str(),
                    )
                    .await?;
                    let listing_id = generate_id();
                    let terms_id = generate_id();
                    let now = Utc::now();
                    let listing_model = listing::ActiveModel {
                        id: Set(listing_id),
                        tenant_id: Set(tenant_id),
                        seller_id: Set(input.seller_id),
                        master_product_id: Set(product.id),
                        master_variant_id: Set(input.master_variant_id),
                        seller_sku: Set(seller_sku.clone()),
                        market_slug: Set(market_slug.clone()),
                        channel_slug: Set(channel_slug.clone()),
                        status: Set(MarketplaceListingStatus::Draft.as_str().to_string()),
                        approval_status: Set(
                            MarketplaceListingApprovalStatus::Draft.as_str().to_string(),
                        ),
                        approval_note: Set(None),
                        suspension_reason: Set(None),
                        current_terms_version: Set(1),
                        metadata: Set(metadata.clone()),
                        published_at: Set(None),
                        approved_at: Set(None),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    }
                    .insert(&receipt.transaction)
                    .await
                    .map_err(map_listing_insert_error)?;
                    let terms_model = listing_terms::ActiveModel {
                        id: Set(terms_id),
                        tenant_id: Set(tenant_id),
                        listing_id: Set(listing_id),
                        version: Set(1),
                        pricing_reference: Set(pricing_reference.clone()),
                        inventory_reference: Set(inventory_reference.clone()),
                        fulfillment_profile_slug: Set(fulfillment_profile_slug.clone()),
                        metadata: Set(serde_json::json!({})),
                        created_at: Set(now.into()),
                    }
                    .insert(&receipt.transaction)
                    .await?;
                    map_listing(listing_model, terms_model)
                }
                .await;
                match result {
                    Ok(response) => complete(receipt, &response).await,
                    Err(error) => rollback(receipt, error).await,
                }
            }
        }
    }

    pub async fn update_terms(
        &self,
        context: PortContext,
        input: UpdateMarketplaceListingTermsInput,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        context.require_write_semantics().map_err(port_validation_error)?;
        input
            .validate()
            .map_err(|error| MarketplaceListingError::Validation(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let key = normalize_idempotency_key(required_idempotency_key(&context)?)?;
        let pricing_reference = normalize_optional_text(input.pricing_reference);
        let inventory_reference = normalize_optional_text(input.inventory_reference);
        let fulfillment_profile_slug = normalize_optional_text(input.fulfillment_profile_slug);
        let metadata = object_or_empty(input.metadata, "metadata")?;
        let normalized = serde_json::json!({
            "listing_id": input.listing_id,
            "pricing_reference": pricing_reference,
            "inventory_reference": inventory_reference,
            "fulfillment_profile_slug": fulfillment_profile_slug,
            "metadata": metadata,
        });
        let hash = request_hash("update_terms", actor_id, &normalized)?;
        match admit(&self.db, tenant_id, actor_id, key, "update_terms", hash.as_str()).await? {
            ListingCommandAdmission::Replay(receipt) => {
                replay(receipt, "update_terms", hash.as_str())
            }
            ListingCommandAdmission::New(receipt) => {
                let result: MarketplaceListingResult<MarketplaceListingResponse> = async {
                    let model = find_listing(&receipt.transaction, tenant_id, input.listing_id).await?;
                    if model.status == MarketplaceListingStatus::Archived.as_str() {
                        return Err(MarketplaceListingError::InvalidTransition {
                            from: model.status,
                            to: "terms_updated".to_string(),
                        });
                    }
                    let next_version = model.current_terms_version.checked_add(1).ok_or_else(|| {
                        MarketplaceListingError::Validation(
                            "listing terms version overflow".to_string(),
                        )
                    })?;
                    let terms_model = listing_terms::ActiveModel {
                        id: Set(generate_id()),
                        tenant_id: Set(tenant_id),
                        listing_id: Set(input.listing_id),
                        version: Set(next_version),
                        pricing_reference: Set(pricing_reference.clone()),
                        inventory_reference: Set(inventory_reference.clone()),
                        fulfillment_profile_slug: Set(fulfillment_profile_slug.clone()),
                        metadata: Set(metadata.clone()),
                        created_at: Set(Utc::now().into()),
                    }
                    .insert(&receipt.transaction)
                    .await?;
                    let mut active: listing::ActiveModel = model.into();
                    active.current_terms_version = Set(next_version);
                    active.status = Set(MarketplaceListingStatus::Draft.as_str().to_string());
                    active.approval_status = Set(
                        MarketplaceListingApprovalStatus::Draft
                            .as_str()
                            .to_string(),
                    );
                    active.approval_note = Set(None);
                    active.approved_at = Set(None);
                    active.published_at = Set(None);
                    active.updated_at = Set(Utc::now().into());
                    let listing_model = active.update(&receipt.transaction).await?;
                    map_listing(listing_model, terms_model)
                }
                .await;
                match result {
                    Ok(response) => complete(receipt, &response).await,
                    Err(error) => rollback(receipt, error).await,
                }
            }
        }
    }

    pub async fn submit_for_review(
        &self,
        context: PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        self.transition(
            context,
            "submit_for_review",
            listing_id,
            serde_json::json!({"listing_id": listing_id}),
            &[MarketplaceListingStatus::Draft],
            MarketplaceListingStatus::PendingReview,
            Some(MarketplaceListingApprovalStatus::Pending),
            None,
        )
        .await
    }

    pub async fn review_listing(
        &self,
        context: PortContext,
        input: ReviewMarketplaceListingInput,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceListingError::Validation(error.to_string()))?;
        let note = normalize_optional_text(input.note);
        let next_approval = if input.approved {
            MarketplaceListingApprovalStatus::Approved
        } else {
            MarketplaceListingApprovalStatus::Rejected
        };
        self.transition(
            context,
            "review_listing",
            input.listing_id,
            serde_json::json!({
                "listing_id": input.listing_id,
                "approved": input.approved,
                "note": note,
            }),
            &[MarketplaceListingStatus::PendingReview],
            MarketplaceListingStatus::Draft,
            Some(next_approval),
            note,
        )
        .await
    }

    pub async fn publish_listing(
        &self,
        context: PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        context.require_write_semantics().map_err(port_validation_error)?;
        let tenant_id = parse_tenant_id(&context)?;
        let current = self.get_listing(tenant_id, listing_id).await?;
        let seller = self
            .seller_reader
            .read_seller(
                context.clone(),
                ReadMarketplaceSellerRequest {
                    seller_id: current.seller_id,
                },
            )
            .await
            .map_err(map_seller_port_error)?;
        if seller.status != MarketplaceSellerStatus::Active {
            return Err(MarketplaceListingError::Validation(
                "listing cannot be published while seller is not active".to_string(),
            ));
        }
        let reasons = listing_reason_codes_without_lifecycle(&current);
        if !reasons.is_empty() {
            return Err(MarketplaceListingError::Validation(format!(
                "listing cannot be published: {}",
                reasons.join(",")
            )));
        }
        self.transition(
            context,
            "publish_listing",
            listing_id,
            serde_json::json!({"listing_id": listing_id}),
            &[MarketplaceListingStatus::Draft],
            MarketplaceListingStatus::Active,
            None,
            None,
        )
        .await
    }

    pub async fn suspend_listing(
        &self,
        context: PortContext,
        input: SuspendMarketplaceListingInput,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceListingError::Validation(error.to_string()))?;
        let reason = required_text(input.reason, "reason")?;
        self.transition(
            context,
            "suspend_listing",
            input.listing_id,
            serde_json::json!({"listing_id": input.listing_id, "reason": reason}),
            &[MarketplaceListingStatus::Active],
            MarketplaceListingStatus::Suspended,
            None,
            Some(reason),
        )
        .await
    }

    pub async fn reactivate_listing(
        &self,
        context: PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        context.require_write_semantics().map_err(port_validation_error)?;
        let tenant_id = parse_tenant_id(&context)?;
        let current = self.get_listing(tenant_id, listing_id).await?;
        let seller = self
            .seller_reader
            .read_seller(
                context.clone(),
                ReadMarketplaceSellerRequest {
                    seller_id: current.seller_id,
                },
            )
            .await
            .map_err(map_seller_port_error)?;
        if seller.status != MarketplaceSellerStatus::Active {
            return Err(MarketplaceListingError::Validation(
                "listing cannot be reactivated while seller is not active".to_string(),
            ));
        }
        let reasons = listing_reason_codes_without_lifecycle(&current);
        if !reasons.is_empty() {
            return Err(MarketplaceListingError::Validation(format!(
                "listing cannot be reactivated: {}",
                reasons.join(",")
            )));
        }
        self.transition(
            context,
            "reactivate_listing",
            listing_id,
            serde_json::json!({"listing_id": listing_id}),
            &[MarketplaceListingStatus::Suspended],
            MarketplaceListingStatus::Active,
            None,
            None,
        )
        .await
    }

    pub async fn archive_listing(
        &self,
        context: PortContext,
        listing_id: Uuid,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        self.transition(
            context,
            "archive_listing",
            listing_id,
            serde_json::json!({"listing_id": listing_id}),
            &[
                MarketplaceListingStatus::Draft,
                MarketplaceListingStatus::PendingReview,
                MarketplaceListingStatus::Active,
                MarketplaceListingStatus::Suspended,
            ],
            MarketplaceListingStatus::Archived,
            None,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn transition(
        &self,
        context: PortContext,
        command_kind: &'static str,
        listing_id: Uuid,
        normalized_request: serde_json::Value,
        expected_statuses: &[MarketplaceListingStatus],
        next_status: MarketplaceListingStatus,
        next_approval: Option<MarketplaceListingApprovalStatus>,
        note_or_reason: Option<String>,
    ) -> MarketplaceListingResult<MarketplaceListingResponse> {
        context.require_write_semantics().map_err(port_validation_error)?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let key = normalize_idempotency_key(required_idempotency_key(&context)?)?;
        let hash = request_hash(command_kind, actor_id, &normalized_request)?;
        match admit(&self.db, tenant_id, actor_id, key, command_kind, hash.as_str()).await? {
            ListingCommandAdmission::Replay(receipt) => replay(receipt, command_kind, hash.as_str()),
            ListingCommandAdmission::New(receipt) => {
                let result: MarketplaceListingResult<MarketplaceListingResponse> = async {
                    let model = find_listing(&receipt.transaction, tenant_id, listing_id).await?;
                    let current_status = MarketplaceListingStatus::parse(model.status.as_str())
                        .ok_or_else(|| MarketplaceListingError::Validation(
                            "stored listing status is invalid".to_string(),
                        ))?;
                    if !expected_statuses.contains(&current_status) {
                        return Err(MarketplaceListingError::InvalidTransition {
                            from: model.status,
                            to: next_status.as_str().to_string(),
                        });
                    }
                    if matches!(next_status, MarketplaceListingStatus::Active)
                        && model.approval_status
                            != MarketplaceListingApprovalStatus::Approved.as_str()
                    {
                        return Err(MarketplaceListingError::InvalidTransition {
                            from: model.approval_status,
                            to: next_status.as_str().to_string(),
                        });
                    }
                    let terms = find_current_terms(
                        &receipt.transaction,
                        tenant_id,
                        listing_id,
                        model.current_terms_version,
                    )
                    .await?;
                    let mut active: listing::ActiveModel = model.into();
                    active.status = Set(next_status.as_str().to_string());
                    if let Some(approval) = next_approval {
                        active.approval_status = Set(approval.as_str().to_string());
                        active.approval_note = Set(note_or_reason.clone());
                        active.approved_at = Set(
                            (approval == MarketplaceListingApprovalStatus::Approved)
                                .then(|| Utc::now().into()),
                        );
                    }
                    match next_status {
                        MarketplaceListingStatus::Active => {
                            active.published_at = Set(Some(Utc::now().into()));
                            active.suspension_reason = Set(None);
                        }
                        MarketplaceListingStatus::Suspended => {
                            active.suspension_reason = Set(note_or_reason.clone());
                        }
                        MarketplaceListingStatus::Archived => {
                            active.published_at = Set(None);
                        }
                        _ => {}
                    }
                    active.updated_at = Set(Utc::now().into());
                    let listing_model = active.update(&receipt.transaction).await?;
                    map_listing(listing_model, terms)
                }
                .await;
                match result {
                    Ok(response) => complete(receipt, &response).await,
                    Err(error) => rollback(receipt, error).await,
                }
            }
        }
    }
}

async fn ensure_listing_identity_available<C: ConnectionTrait>(
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

async fn find_listing<C: ConnectionTrait>(
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

async fn find_current_terms<C: ConnectionTrait>(
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

async fn load_listing_response<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    listing_id: Uuid,
) -> MarketplaceListingResult<MarketplaceListingResponse> {
    let model = find_listing(connection, tenant_id, listing_id).await?;
    load_response_for_model(connection, model).await
}

async fn load_response_for_model<C: ConnectionTrait>(
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

fn map_listing(
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
        .ok_or_else(|| MarketplaceListingError::Validation(format!(
            "unknown marketplace listing approval status `{}`",
            model.approval_status
        )))?;
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
        approval_note: model.approval_note,
        suspension_reason: model.suspension_reason,
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

fn listing_reason_codes_without_lifecycle(listing: &MarketplaceListingResponse) -> Vec<String> {
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

fn parse_tenant_id(context: &PortContext) -> MarketplaceListingResult<Uuid> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        MarketplaceListingError::Validation(
            "PortContext.tenant_id must be a UUID for marketplace listing ports".to_string(),
        )
    })
}

fn parse_actor_id(context: &PortContext) -> MarketplaceListingResult<Uuid> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        MarketplaceListingError::Validation(
            "write PortContext.actor.id must be a UUID for marketplace listing audit".to_string(),
        )
    })
}

fn required_idempotency_key(context: &PortContext) -> MarketplaceListingResult<String> {
    context
        .idempotency_key
        .clone()
        .ok_or_else(|| MarketplaceListingError::Validation(
            "marketplace listing write requires an idempotency key".to_string(),
        ))
}

fn port_validation_error(error: rustok_api::PortError) -> MarketplaceListingError {
    MarketplaceListingError::Validation(error.message)
}

fn map_seller_port_error(error: rustok_api::PortError) -> MarketplaceListingError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            MarketplaceListingError::Validation(error.message)
        }
        _ => MarketplaceListingError::SellerUnavailable(error.code),
    }
}

fn map_product_port_error(error: rustok_api::PortError) -> MarketplaceListingError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            MarketplaceListingError::Validation(error.message)
        }
        _ => MarketplaceListingError::ProductUnavailable(error.code),
    }
}

fn map_listing_insert_error(error: sea_orm::DbErr) -> MarketplaceListingError {
    if matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    ) {
        MarketplaceListingError::DuplicateScope
    } else {
        error.into()
    }
}

fn required_text(value: String, field: &str) -> MarketplaceListingResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(MarketplaceListingError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    Ok(value.to_string())
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

fn object_or_empty(
    value: serde_json::Value,
    field: &str,
) -> MarketplaceListingResult<serde_json::Value> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(MarketplaceListingError::Validation(format!(
            "{field} must be a JSON object"
        ))),
    }
}

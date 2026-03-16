use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::Error;
use loco_rs::Result;
use rustok_telemetry::metrics;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use rustok_commerce::dto::{CreateVariantInput, PriceInput, UpdateVariantInput, VariantResponse};
use rustok_commerce::{entities, PricingService};
use rustok_core::generate_id;
use rustok_events::DomainEvent;

use crate::common::PaginationParams;
use crate::context::TenantContext;
use crate::extractors::rbac::{
    RequireProductsCreate, RequireProductsDelete, RequireProductsRead, RequireProductsUpdate,
};
use crate::services::event_bus::transactional_event_bus_from_context;

#[derive(Debug, serde::Deserialize, IntoParams, ToSchema)]
pub struct ListVariantsParams {
    #[serde(flatten)]
    pub pagination: Option<PaginationParams>,
}

/// List product variants
#[utoipa::path(
    get,
    path = "/api/commerce/products/{product_id}/variants",
    tag = "commerce",
    params(
        ("product_id" = Uuid, Path, description = "Product ID"),
        ListVariantsParams
    ),
    responses(
        (status = 200, description = "List of variants", body = Vec<VariantResponse>),
        (status = 404, description = "Product not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(super) async fn list_variants(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _user: RequireProductsRead,
    Path(product_id): Path<Uuid>,
    Query(params): Query<ListVariantsParams>,
) -> Result<Json<Vec<VariantResponse>>> {
    use rustok_commerce::entities::{price, product_variant};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

    let requested_limit = params
        .pagination
        .as_ref()
        .map(|pagination| pagination.per_page);
    let pagination = params.pagination.unwrap_or_default();

    let product = entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant.id))
        .one(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;

    let variants = product_variant::Entity::find()
        .filter(product_variant::Column::ProductId.eq(product.id))
        .order_by_asc(product_variant::Column::Position)
        .offset(pagination.offset())
        .limit(pagination.limit())
        .all(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let variant_ids: Vec<Uuid> = variants.iter().map(|variant| variant.id).collect();
    let prices = if variant_ids.is_empty() {
        Vec::new()
    } else {
        price::Entity::find()
            .filter(price::Column::VariantId.is_in(variant_ids))
            .all(&ctx.db)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?
    };

    let mut prices_map: std::collections::HashMap<Uuid, Vec<_>> = std::collections::HashMap::new();
    for price in prices {
        prices_map.entry(price.variant_id).or_default().push(price);
    }

    let response = variants
        .into_iter()
        .map(|variant| {
            let variant_prices = prices_map.get(&variant.id).cloned().unwrap_or_default();
            build_variant_response(variant, variant_prices)
        })
        .collect::<Vec<_>>();

    metrics::record_read_path_budget(
        "http",
        "commerce.list_variants",
        requested_limit,
        pagination.limit(),
        response.len(),
    );

    Ok(Json(response))
}

/// Create a new product variant
#[utoipa::path(
    post,
    path = "/api/commerce/products/{product_id}/variants",
    tag = "commerce",
    params(
        ("product_id" = Uuid, Path, description = "Product ID")
    ),
    request_body = CreateVariantInput,
    responses(
        (status = 201, description = "Variant created successfully", body = VariantResponse),
        (status = 404, description = "Product not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(super) async fn create_variant(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireProductsCreate(user): RequireProductsCreate,
    Path(product_id): Path<Uuid>,
    Json(input): Json<CreateVariantInput>,
) -> Result<(StatusCode, Json<VariantResponse>)> {
    use chrono::Utc;
    use rustok_commerce::entities::{price, product_variant};
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set,
        TransactionTrait,
    };

    let product = entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant.id))
        .one(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;

    let txn = ctx
        .db
        .begin()
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let max_position = product_variant::Entity::find()
        .filter(product_variant::Column::ProductId.eq(product.id))
        .count(&txn)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let variant_id = generate_id();
    let now = Utc::now();

    let variant = product_variant::ActiveModel {
        id: Set(variant_id),
        product_id: Set(product.id),
        tenant_id: Set(tenant.id),
        sku: Set(input.sku.clone()),
        barcode: Set(input.barcode.clone()),
        ean: Set(None),
        upc: Set(None),
        inventory_policy: Set(input.inventory_policy.clone()),
        inventory_management: Set("manual".into()),
        inventory_quantity: Set(input.inventory_quantity),
        weight: Set(input.weight),
        weight_unit: Set(input.weight_unit.clone()),
        option1: Set(input.option1.clone()),
        option2: Set(input.option2.clone()),
        option3: Set(input.option3.clone()),
        position: Set(max_position as i32),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };

    let variant = variant
        .insert(&txn)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let mut created_prices = Vec::new();
    for price_input in &input.prices {
        let price_model = price::ActiveModel {
            id: Set(generate_id()),
            variant_id: Set(variant_id),
            currency_code: Set(price_input.currency_code.clone()),
            amount: Set(price_input.amount),
            compare_at_amount: Set(price_input.compare_at_amount),
        };
        let price = price_model
            .insert(&txn)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?;
        created_prices.push(price);
    }

    transactional_event_bus_from_context(&ctx)
        .publish_in_tx(
            &txn,
            tenant.id,
            Some(user.user.id),
            DomainEvent::VariantCreated {
                variant_id,
                product_id,
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    txn.commit()
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(build_variant_response(variant, created_prices)),
    ))
}

/// Get variant details
#[utoipa::path(
    get,
    path = "/api/commerce/variants/{id}",
    tag = "commerce",
    params(
        ("id" = Uuid, Path, description = "Variant ID")
    ),
    responses(
        (status = 200, description = "Variant details", body = VariantResponse),
        (status = 404, description = "Variant not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(super) async fn show_variant(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    _user: RequireProductsRead,
    Path(id): Path<Uuid>,
) -> Result<Json<VariantResponse>> {
    use rustok_commerce::entities::{price, product_variant};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let variant = product_variant::Entity::find_by_id(id)
        .filter(product_variant::Column::TenantId.eq(tenant.id))
        .one(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;

    let prices = price::Entity::find()
        .filter(price::Column::VariantId.eq(id))
        .all(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(build_variant_response(variant, prices)))
}

/// Update an existing variant
#[utoipa::path(
    put,
    path = "/api/commerce/variants/{id}",
    tag = "commerce",
    params(
        ("id" = Uuid, Path, description = "Variant ID")
    ),
    request_body = UpdateVariantInput,
    responses(
        (status = 200, description = "Variant updated successfully", body = VariantResponse),
        (status = 404, description = "Variant not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(super) async fn update_variant(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireProductsUpdate(user): RequireProductsUpdate,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateVariantInput>,
) -> Result<Json<VariantResponse>> {
    use chrono::Utc;
    use rustok_commerce::entities::{price, product_variant};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};

    let variant = product_variant::Entity::find_by_id(id)
        .filter(product_variant::Column::TenantId.eq(tenant.id))
        .one(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;

    let product_id = variant.product_id;

    let txn = ctx
        .db
        .begin()
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let mut variant_active: product_variant::ActiveModel = variant.into();
    variant_active.updated_at = Set(Utc::now().into());

    if let Some(sku) = input.sku {
        variant_active.sku = Set(Some(sku));
    }
    if let Some(barcode) = input.barcode {
        variant_active.barcode = Set(Some(barcode));
    }
    if let Some(inventory_quantity) = input.inventory_quantity {
        variant_active.inventory_quantity = Set(inventory_quantity);
    }
    if let Some(inventory_policy) = input.inventory_policy {
        variant_active.inventory_policy = Set(inventory_policy);
    }
    if let Some(weight) = input.weight {
        variant_active.weight = Set(Some(weight));
    }
    if let Some(weight_unit) = input.weight_unit {
        variant_active.weight_unit = Set(Some(weight_unit));
    }

    let variant = variant_active
        .update(&txn)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    transactional_event_bus_from_context(&ctx)
        .publish_in_tx(
            &txn,
            tenant.id,
            Some(user.user.id),
            DomainEvent::VariantUpdated {
                variant_id: id,
                product_id,
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    txn.commit()
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let prices = price::Entity::find()
        .filter(price::Column::VariantId.eq(id))
        .all(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(build_variant_response(variant, prices)))
}

/// Delete a variant
#[utoipa::path(
    delete,
    path = "/api/commerce/variants/{id}",
    tag = "commerce",
    params(
        ("id" = Uuid, Path, description = "Variant ID")
    ),
    responses(
        (status = 204, description = "Variant deleted successfully"),
        (status = 404, description = "Variant not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(super) async fn delete_variant(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireProductsDelete(user): RequireProductsDelete,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    use rustok_commerce::entities::product_variant;
    use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter, TransactionTrait};

    let variant = product_variant::Entity::find_by_id(id)
        .filter(product_variant::Column::TenantId.eq(tenant.id))
        .one(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;

    let product_id = variant.product_id;

    let txn = ctx
        .db
        .begin()
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    variant
        .delete(&txn)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    transactional_event_bus_from_context(&ctx)
        .publish_in_tx(
            &txn,
            tenant.id,
            Some(user.user.id),
            DomainEvent::VariantDeleted {
                variant_id: id,
                product_id,
            },
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    txn.commit()
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Update variant prices
#[utoipa::path(
    put,
    path = "/api/commerce/variants/{id}/prices",
    tag = "commerce",
    params(
        ("id" = Uuid, Path, description = "Variant ID")
    ),
    request_body = Vec<PriceInput>,
    responses(
        (status = 200, description = "Prices updated successfully", body = VariantResponse),
        (status = 404, description = "Variant not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(super) async fn update_prices(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireProductsUpdate(user): RequireProductsUpdate,
    Path(id): Path<Uuid>,
    Json(prices): Json<Vec<PriceInput>>,
) -> Result<Json<VariantResponse>> {
    let pricing = PricingService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    pricing
        .set_prices(tenant.id, user.user.id, id, prices)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    show_variant(State(ctx), tenant, RequireProductsRead(user), Path(id)).await
}

fn build_variant_response(
    variant: rustok_commerce::entities::product_variant::Model,
    prices: Vec<rustok_commerce::entities::price::Model>,
) -> VariantResponse {
    let title = generate_variant_title(&variant);
    let price_responses = prices
        .into_iter()
        .map(|price| rustok_commerce::dto::PriceResponse {
            currency_code: price.currency_code,
            amount: price.amount,
            compare_at_amount: price.compare_at_amount,
            on_sale: price
                .compare_at_amount
                .map(|value| value > price.amount)
                .unwrap_or(false),
        })
        .collect();

    VariantResponse {
        id: variant.id,
        product_id: variant.product_id,
        sku: variant.sku,
        barcode: variant.barcode,
        title,
        option1: variant.option1,
        option2: variant.option2,
        option3: variant.option3,
        prices: price_responses,
        inventory_quantity: variant.inventory_quantity,
        inventory_policy: variant.inventory_policy.clone(),
        in_stock: variant.inventory_quantity > 0 || variant.inventory_policy == "continue",
        weight: variant.weight,
        weight_unit: variant.weight_unit,
        position: variant.position,
    }
}

fn generate_variant_title(variant: &rustok_commerce::entities::product_variant::Model) -> String {
    let options: Vec<&str> = [
        variant.option1.as_deref(),
        variant.option2.as_deref(),
        variant.option3.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();

    if options.is_empty() {
        "Default".to_string()
    } else {
        options.join(" / ")
    }
}

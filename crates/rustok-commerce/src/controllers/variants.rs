use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{loco::transactional_event_bus_from_context, AuthContext, TenantContext};
use rustok_core::{generate_id, Permission};
use rustok_events::DomainEvent;
use rustok_telemetry::metrics;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    dto::{CreateVariantInput, PriceInput, PriceResponse, UpdateVariantInput, VariantResponse},
    entities, PricingService,
};

use super::common::{ensure_permissions, PaginationParams};

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
pub async fn list_variants(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(product_id): Path<Uuid>,
    Query(params): Query<ListVariantsParams>,
) -> Result<Json<Vec<VariantResponse>>> {
    use crate::entities::{price, product_variant};

    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_READ],
        "Permission denied: products:read required",
    )?;

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

    let variant_ids = variants
        .iter()
        .map(|variant| variant.id)
        .collect::<Vec<_>>();
    let prices = if variant_ids.is_empty() {
        Vec::new()
    } else {
        price::Entity::find()
            .filter(price::Column::VariantId.is_in(variant_ids))
            .all(&ctx.db)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?
    };

    let mut prices_map: std::collections::HashMap<Uuid, Vec<crate::entities::price::Model>> =
        std::collections::HashMap::new();
    for price in prices {
        prices_map.entry(price.variant_id).or_default().push(price);
    }

    let response = variants
        .into_iter()
        .map(|variant| {
            let variant_prices = prices_map.remove(&variant.id).unwrap_or_default();
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
    params(("product_id" = Uuid, Path, description = "Product ID")),
    request_body = CreateVariantInput,
    responses(
        (status = 201, description = "Variant created successfully", body = VariantResponse),
        (status = 404, description = "Product not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_variant(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(product_id): Path<Uuid>,
    Json(input): Json<CreateVariantInput>,
) -> Result<(StatusCode, Json<VariantResponse>)> {
    use crate::entities::{price, product_variant};
    use chrono::Utc;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set,
        TransactionTrait,
    };

    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_CREATE],
        "Permission denied: products:create required",
    )?;

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
    }
    .insert(&txn)
    .await
    .map_err(|err| Error::BadRequest(err.to_string()))?;

    let mut created_prices = Vec::new();
    for price_input in &input.prices {
        let price = price::ActiveModel {
            id: Set(generate_id()),
            variant_id: Set(variant_id),
            price_list_id: Set(None),
            currency_code: Set(price_input.currency_code.clone()),
            region_id: Set(None),
            amount: Set(price_input.amount),
            compare_at_amount: Set(price_input.compare_at_amount),
            legacy_amount: Set(decimal_to_cents(price_input.amount)),
            legacy_compare_at_amount: Set(
                price_input.compare_at_amount.and_then(decimal_to_cents),
            ),
            min_quantity: Set(None),
            max_quantity: Set(None),
        }
        .insert(&txn)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
        created_prices.push(price);
    }

    transactional_event_bus_from_context(&ctx)
        .publish_in_tx(
            &txn,
            tenant.id,
            Some(auth.user_id),
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
    params(("id" = Uuid, Path, description = "Variant ID")),
    responses(
        (status = 200, description = "Variant details", body = VariantResponse),
        (status = 404, description = "Variant not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn show_variant(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<VariantResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_READ],
        "Permission denied: products:read required",
    )?;

    let variant = load_variant_response(&ctx.db, tenant.id, id).await?;
    Ok(Json(variant))
}

/// Update an existing variant
#[utoipa::path(
    put,
    path = "/api/commerce/variants/{id}",
    tag = "commerce",
    params(("id" = Uuid, Path, description = "Variant ID")),
    request_body = UpdateVariantInput,
    responses(
        (status = 200, description = "Variant updated successfully", body = VariantResponse),
        (status = 404, description = "Variant not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_variant(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateVariantInput>,
) -> Result<Json<VariantResponse>> {
    use crate::entities::{price, product_variant};
    use chrono::Utc;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};

    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

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
            Some(auth.user_id),
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
    params(("id" = Uuid, Path, description = "Variant ID")),
    responses(
        (status = 204, description = "Variant deleted successfully"),
        (status = 404, description = "Variant not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_variant(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    use crate::entities::product_variant;
    use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter, TransactionTrait};

    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_DELETE],
        "Permission denied: products:delete required",
    )?;

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
            Some(auth.user_id),
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
    params(("id" = Uuid, Path, description = "Variant ID")),
    request_body = Vec<PriceInput>,
    responses(
        (status = 200, description = "Prices updated successfully", body = VariantResponse),
        (status = 404, description = "Variant not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_prices(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(prices): Json<Vec<PriceInput>>,
) -> Result<Json<VariantResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    let pricing = PricingService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    pricing
        .set_prices(tenant.id, auth.user_id, id, prices)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    let variant = load_variant_response(&ctx.db, tenant.id, id).await?;
    Ok(Json(variant))
}

async fn load_variant_response(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> Result<VariantResponse> {
    use crate::entities::{price, product_variant};

    let variant = product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?
        .ok_or(Error::NotFound)?;

    let prices = price::Entity::find()
        .filter(price::Column::VariantId.eq(variant_id))
        .all(db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(build_variant_response(variant, prices))
}

fn build_variant_response(
    variant: crate::entities::product_variant::Model,
    prices: Vec<crate::entities::price::Model>,
) -> VariantResponse {
    let title = generate_variant_title(&variant);
    let price_responses = prices
        .into_iter()
        .map(|price| PriceResponse {
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
        translations: Vec::new(),
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

fn decimal_to_cents(amount: rust_decimal::Decimal) -> Option<i64> {
    use rust_decimal::prelude::ToPrimitive;

    (amount * rust_decimal::Decimal::from(100))
        .round_dp(0)
        .to_i64()
}

fn generate_variant_title(variant: &crate::entities::product_variant::Model) -> String {
    let options = [
        variant.option1.as_deref(),
        variant.option2.as_deref(),
        variant.option3.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if options.is_empty() {
        "Default".to_string()
    } else {
        options.join(" / ")
    }
}

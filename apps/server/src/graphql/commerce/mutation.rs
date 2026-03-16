use async_graphql::{Context, FieldError, Object, Result};
use rust_decimal::Decimal;
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::common::require_module_enabled;
use crate::graphql::errors::GraphQLError;
use crate::graphql::schema::module_slug;
use crate::services::rbac_service::RbacService;
use rustok_commerce::CatalogService;
use rustok_core::Permission;
use rustok_outbox::TransactionalEventBus;

use super::types::*;

#[derive(Default)]
pub struct CommerceMutation;

#[Object]
impl CommerceMutation {
    async fn create_product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        input: CreateProductInput,
    ) -> Result<GqlProduct> {
        require_module_enabled(ctx, module_slug::COMMERCE).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::PRODUCTS_CREATE, Permission::PRODUCTS_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: products:create required",
            ));
        }

        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        let domain_input = convert_create_product_input(input)?;
        let product = catalog
            .create_product(tenant_id, user_id, domain_input)
            .await?;

        Ok(product.into())
    }

    async fn update_product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
        input: UpdateProductInput,
    ) -> Result<GqlProduct> {
        require_module_enabled(ctx, module_slug::COMMERCE).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: products:update required",
            ));
        }

        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        let domain_input = rustok_commerce::dto::UpdateProductInput {
            translations: input.translations.map(|translations| {
                translations
                    .into_iter()
                    .map(
                        |translation| rustok_commerce::dto::ProductTranslationInput {
                            locale: translation.locale,
                            title: translation.title,
                            handle: translation.handle,
                            description: translation.description,
                            meta_title: translation.meta_title,
                            meta_description: translation.meta_description,
                        },
                    )
                    .collect()
            }),
            vendor: input.vendor,
            product_type: input.product_type,
            metadata: None,
            status: input.status.map(Into::into),
        };

        let product = catalog
            .update_product(tenant_id, user_id, id, domain_input)
            .await?;

        Ok(product.into())
    }

    async fn publish_product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
    ) -> Result<GqlProduct> {
        require_module_enabled(ctx, module_slug::COMMERCE).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: products:update required",
            ));
        }

        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        let product = catalog.publish_product(tenant_id, user_id, id).await?;

        Ok(product.into())
    }

    async fn delete_product(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
        id: Uuid,
    ) -> Result<bool> {
        require_module_enabled(ctx, module_slug::COMMERCE).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::PRODUCTS_DELETE, Permission::PRODUCTS_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: products:delete required",
            ));
        }

        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        catalog.delete_product(tenant_id, user_id, id).await?;

        Ok(true)
    }
}

fn convert_create_product_input(
    input: CreateProductInput,
) -> Result<rustok_commerce::dto::CreateProductInput> {
    let translations = input
        .translations
        .into_iter()
        .map(
            |translation| rustok_commerce::dto::ProductTranslationInput {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
                meta_title: translation.meta_title,
                meta_description: translation.meta_description,
            },
        )
        .collect();

    let options = input
        .options
        .unwrap_or_default()
        .into_iter()
        .map(|option| rustok_commerce::dto::ProductOptionInput {
            name: option.name,
            values: option.values,
        })
        .collect();

    let variants = input
        .variants
        .into_iter()
        .map(|variant| {
            let prices = variant
                .prices
                .into_iter()
                .map(|price| {
                    let amount = parse_decimal(&price.amount)?;
                    let compare_at_amount = match price.compare_at_amount {
                        Some(value) => Some(parse_decimal(&value)?),
                        None => None,
                    };

                    Ok(rustok_commerce::dto::PriceInput {
                        currency_code: price.currency_code,
                        amount,
                        compare_at_amount,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(rustok_commerce::dto::CreateVariantInput {
                sku: variant.sku,
                barcode: variant.barcode,
                option1: variant.option1,
                option2: variant.option2,
                option3: variant.option3,
                prices,
                inventory_quantity: variant.inventory_quantity.unwrap_or(0),
                inventory_policy: variant
                    .inventory_policy
                    .unwrap_or_else(|| "deny".to_string()),
                weight: None,
                weight_unit: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(rustok_commerce::dto::CreateProductInput {
        translations,
        options,
        variants,
        vendor: input.vendor,
        product_type: input.product_type,
        metadata: serde_json::Value::Object(Default::default()),
        publish: input.publish.unwrap_or(false),
    })
}

fn parse_decimal(value: &str) -> Result<Decimal> {
    Decimal::from_str(value).map_err(|_| async_graphql::Error::new("Invalid decimal value"))
}

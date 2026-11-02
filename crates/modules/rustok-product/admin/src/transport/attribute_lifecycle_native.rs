/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Host-selected schema read/write capabilities, never a locally constructed schema service.

use crate::model::{
    AttributeDefinitionOutcome, AttributeLifecycleRequest, AttributeLifecycleWorkspace,
    UpdateAttributeDefinitionDraft,
};
use leptos::prelude::*;

#[cfg(feature = "ssr")]
fn project<T: serde::Serialize, U: serde::de::DeserializeOwned>(
    value: T,
) -> Result<U, ServerFnError> {
    // The owner JSON projection is the same contract as GraphQL's JSON scalar. Fail closed on
    // a shape mismatch; do not silently drop fields in a hand-maintained partial mapper.
    serde_json::from_value(
        serde_json::to_value(value)
            .map_err(|_| ServerFnError::new("Definition projection is unavailable"))?,
    )
    .map_err(|_| ServerFnError::new("Definition projection is unavailable"))
}

#[server(prefix = "/api/fn", endpoint = "product/admin/attribute-lifecycle", input = leptos::server_fn::codec::Json)]
pub(super) async fn read_definition_native(
    tenant_id: String,
    actor_id: String,
    locale: String,
    request: AttributeLifecycleRequest,
) -> Result<AttributeLifecycleWorkspace, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::native_server_adapter::{catalog_actor_scope, map_product_internal_error};
        let (host, auth, tenant, locale) = catalog_actor_scope(
            &tenant_id,
            &actor_id,
            &locale,
            &[
                rustok_api::Permission::PRODUCTS_READ,
                rustok_api::Permission::PRODUCTS_MANAGE,
            ],
        )
        .await?;
        let runtime = host
            .shared_get::<rustok_product::ProductCatalogReadRuntime>()
            .ok_or_else(map_product_internal_error)?;
        let port = runtime
            .schema_read_port()
            .ok_or_else(map_product_internal_error)?;
        let mut context = rustok_api::PortContext::new(
            tenant.id.to_string(),
            rustok_api::PortActor::user(auth.user_id.to_string()),
            locale,
            uuid::Uuid::new_v4().to_string(),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        for permission in &auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = port
            .read_attribute_lifecycle(context, project(request)?)
            .await
            .map_err(|_| map_product_internal_error())?;
        project(result)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "Native attribute lifecycle requires the SSR host",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/update-attribute-definition", input = leptos::server_fn::codec::Json)]
pub(super) async fn update_definition_native(
    tenant_id: String,
    actor_id: String,
    idempotency_key: String,
    input: UpdateAttributeDefinitionDraft,
) -> Result<AttributeDefinitionOutcome, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::native_server_adapter::{
            catalog_actor_scope, map_product_internal_error, write_context,
        };
        let (host, auth, tenant, locale) = catalog_actor_scope(
            &tenant_id,
            &actor_id,
            &input.locale,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
        )
        .await?;
        if !crate::core::attribute_lifecycle::valid_input(&input)
            || !crate::core::write_journal::valid_uuid(&idempotency_key)
        {
            return Err(ServerFnError::new("Invalid definition request"));
        }
        let runtime = host
            .shared_get::<rustok_product::ProductCatalogCommandRuntime>()
            .ok_or_else(map_product_internal_error)?;
        let port = runtime
            .schema_write_port()
            .ok_or_else(map_product_internal_error)?;
        let context = write_context(
            &auth,
            &tenant,
            locale,
            "update_attribute_definition",
            None,
            &idempotency_key,
        )?;
        let result = port
            .update_attribute_definition(context, project(input)?)
            .await;
        Ok(match result {
            Ok(result) => AttributeDefinitionOutcome::Decision {
                result: project(result)?,
            },
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "product.validation" | "product.schema_validation"
                ) && rustok_product::product_catalog_command_error_is_terminal(&error) =>
            {
                AttributeDefinitionOutcome::Rejected {
                    code: "validation".into(),
                }
            }
            Err(_) => AttributeDefinitionOutcome::Unknown,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "Native attribute lifecycle requires the SSR host",
        ))
    }
}

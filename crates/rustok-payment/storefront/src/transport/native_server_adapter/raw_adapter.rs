use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde_json::json;
#[cfg(feature = "ssr")]
use uuid::Uuid;

use super::super::{
    PaymentCollection, PaymentCollectionCreateRequest, PaymentCollectionTransportError,
};

pub async fn create_payment_collection_server(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, PaymentCollectionTransportError> {
    storefront_payment_create_collection_native(request)
        .await
        .map_err(|error| PaymentCollectionTransportError::ServerFn(error.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "payment/create-payment-collection")]
async fn storefront_payment_create_collection_native(
    request: PaymentCollectionCreateRequest,
) -> Result<PaymentCollection, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_commerce::storefront_checkout_runtime::{
            self, StorefrontPaymentCollectionCommand,
        };

        let app_ctx = expect_context::<AppContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let cart_id = Uuid::parse_str(request.cart_id.trim())
            .map_err(|_| ServerFnError::new("cart_id must be a valid UUID"))?;
        let metadata = request.metadata;

        let collection = storefront_checkout_runtime::create_storefront_payment_collection(
            &app_ctx,
            &tenant,
            &request_context,
            auth,
            StorefrontPaymentCollectionCommand {
                cart_id,
                metadata: json!({
                    "source_module": metadata.source_module,
                    "source_surface": metadata.source_surface,
                    "command": metadata.command,
                    "owner_module": metadata.owner_module,
                }),
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;

        Ok(map_payment_collection(collection))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "payment/create-payment-collection requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_payment_collection(
    value: rustok_payment::dto::PaymentCollectionResponse,
) -> PaymentCollection {
    PaymentCollection {
        id: value.id.to_string(),
        status: value.status,
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        authorized_amount: value.authorized_amount.normalize().to_string(),
        captured_amount: value.captured_amount.normalize().to_string(),
        order_id: value.order_id.map(|value| value.to_string()),
        provider_id: value.provider_id,
        payment_count: value.payments.len() as u64,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(crate::controllers::ingest_provider_webhook),
    components(schemas(crate::controllers::PaymentWebhookIngressResponse)),
    tags((
        name = "payment-webhooks",
        description = "Signature-verified payment provider webhook ingress"
    ))
)]
pub struct PaymentApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    PaymentApiDoc::openapi()
}

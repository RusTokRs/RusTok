/// Explicit test intent/actor. Production entrypoints must use the caller's canonical context.
pub fn shipment_context(tenant_id: uuid::Uuid) -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        tenant_id.to_string(),
        rustok_api::PortActor::user(uuid::Uuid::new_v4().to_string()),
        "en",
        uuid::Uuid::new_v4().to_string(),
    )
    .with_idempotency_key(uuid::Uuid::new_v4().to_string())
    .with_deadline(std::time::Duration::from_secs(30))
}


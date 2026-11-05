use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_commerce_foundation::error::CommerceError;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    CreatePriceListOwnerInput, PriceListOwnerService, PriceListOwnerSnapshot,
    UpdatePriceListOwnerInput,
};

#[async_trait]
pub trait PriceListOwnerPort: Send + Sync {
    async fn create_price_list(
        &self,
        context: PortContext,
        input: CreatePriceListOwnerInput,
    ) -> Result<PriceListOwnerSnapshot, PortError>;

    async fn update_price_list(
        &self,
        context: PortContext,
        price_list_id: Uuid,
        input: UpdatePriceListOwnerInput,
    ) -> Result<PriceListOwnerSnapshot, PortError>;

    async fn delete_price_list(
        &self,
        context: PortContext,
        price_list_id: Uuid,
    ) -> Result<(), PortError>;
}

pub fn in_process_price_list_owner_port(db: DatabaseConnection) -> Arc<dyn PriceListOwnerPort> {
    Arc::new(PriceListOwnerService::new(db))
}

#[async_trait]
impl PriceListOwnerPort for PriceListOwnerService {
    async fn create_price_list(
        &self,
        context: PortContext,
        input: CreatePriceListOwnerInput,
    ) -> Result<PriceListOwnerSnapshot, PortError> {
        let tenant_id = authorize_write(&context)?;
        if input.channel_id == Some(Uuid::nil()) {
            return Err(PortError::validation(
                "pricing.price_list_owner_channel_id_nil",
                "channel_id must not be a nil UUID",
            ));
        }
        tracing::debug!(
            tenant_id = %tenant_id,
            channel_id = ?input.channel_id,
            list_type = %input.list_type,
            "price_list_owner_port create_price_list entered"
        );
        PriceListOwnerService::create_price_list(self, tenant_id, input)
            .await
            .map_err(owner_error_to_port_error)
    }

    async fn update_price_list(
        &self,
        context: PortContext,
        price_list_id: Uuid,
        input: UpdatePriceListOwnerInput,
    ) -> Result<PriceListOwnerSnapshot, PortError> {
        if price_list_id.is_nil() {
            return Err(PortError::validation(
                "pricing.price_list_id_nil",
                "price_list_id must not be a nil UUID",
            ));
        }
        let tenant_id = authorize_write(&context)?;
        tracing::debug!(
            tenant_id = %tenant_id,
            price_list_id = %price_list_id,
            "price_list_owner_port update_price_list entered"
        );
        PriceListOwnerService::update_price_list(self, tenant_id, price_list_id, input)
            .await
            .map_err(owner_error_to_port_error)
    }

    async fn delete_price_list(
        &self,
        context: PortContext,
        price_list_id: Uuid,
    ) -> Result<(), PortError> {
        if price_list_id.is_nil() {
            return Err(PortError::validation(
                "pricing.price_list_id_nil",
                "price_list_id must not be a nil UUID",
            ));
        }
        let tenant_id = authorize_write(&context)?;
        tracing::debug!(
            tenant_id = %tenant_id,
            price_list_id = %price_list_id,
            "price_list_owner_port delete_price_list entered"
        );
        PriceListOwnerService::delete_price_list(self, tenant_id, price_list_id)
            .await
            .map_err(owner_error_to_port_error)
    }
}

fn authorize_write(context: &PortContext) -> Result<Uuid, PortError> {
    context.require_policy(PortCallPolicy::write())?;
    let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "pricing.price_list_owner_tenant_id_invalid",
            "price list owner context must carry a UUID tenant_id",
        )
    })?;
    if tenant_id.is_nil() {
        return Err(PortError::validation(
            "pricing.price_list_owner_tenant_id_nil",
            "price list owner context must carry a non-nil UUID tenant_id",
        ));
    }
    Ok(tenant_id)
}

fn owner_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "pricing.price_list_owner_database_unavailable",
            "price list owner storage is temporarily unavailable",
        ),
        CommerceError::Validation(message) if message == "price_list_id was not found" => {
            PortError::not_found(
                "pricing.price_list_owner_not_found",
                "price list was not found",
            )
        }
        CommerceError::Validation(_) => PortError::validation(
            "pricing.price_list_owner_validation",
            "price list owner request is invalid",
        ),
        _ => PortError::invariant_violation(
            "pricing.price_list_owner_invariant",
            "price list owner operation failed",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    #[test]
    fn authorize_write_accepts_valid_and_rejects_nil_or_invalid() {
        let valid_tenant = Uuid::new_v4();
        let valid_ctx = PortContext::new(
            valid_tenant.to_string(),
            PortActor::system(),
            "en-US",
            Uuid::new_v4(),
        )
        .with_idempotency_key("test-key")
        .with_deadline(std::time::Duration::from_millis(5000));
        assert_eq!(authorize_write(&valid_ctx).unwrap(), valid_tenant);

        let nil_ctx = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::system(),
            "en-US",
            Uuid::new_v4(),
        )
        .with_idempotency_key("test-key")
        .with_deadline(std::time::Duration::from_millis(5000));
        let err = authorize_write(&nil_ctx).unwrap_err();
        assert_eq!(err.code, "pricing.price_list_owner_tenant_id_nil");

        let invalid_ctx =
            PortContext::new("not-a-uuid", PortActor::system(), "en-US", Uuid::new_v4())
                .with_idempotency_key("test-key")
                .with_deadline(std::time::Duration::from_millis(5000));
        let err = authorize_write(&invalid_ctx).unwrap_err();
        assert_eq!(err.code, "pricing.price_list_owner_tenant_id_invalid");
    }
}

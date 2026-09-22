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
        let tenant_id = authorize_write(&context)?;
        PriceListOwnerService::update_price_list(self, tenant_id, price_list_id, input)
            .await
            .map_err(owner_error_to_port_error)
    }

    async fn delete_price_list(
        &self,
        context: PortContext,
        price_list_id: Uuid,
    ) -> Result<(), PortError> {
        let tenant_id = authorize_write(&context)?;
        PriceListOwnerService::delete_price_list(self, tenant_id, price_list_id)
            .await
            .map_err(owner_error_to_port_error)
    }
}

fn authorize_write(context: &PortContext) -> Result<Uuid, PortError> {
    context.require_policy(PortCallPolicy::write())?;
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "pricing.price_list_owner_tenant_id_invalid",
            "price list owner context must carry a UUID tenant_id",
        )
    })
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

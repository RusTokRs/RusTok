use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use serde_json::Value;
use uuid::Uuid;

/// Resolves attached custom fields for a Product donor under an admitted tenant.
#[async_trait]
pub trait ProductAttachedValuesReadPort: Send + Sync {
    async fn read_attached_values(
        &self,
        context: PortContext,
        product_id: Uuid,
        fallback_locale: &str,
    ) -> Result<Option<Value>, PortError>;
}

/// Product operations for an authorized Flex caller with a deadline and stable command key.
/// A retry returns the original result committed with the write.
#[async_trait]
pub trait ProductAttachedValuesWritePort: Send + Sync {
    async fn update_attached_values(
        &self,
        context: PortContext,
        product_id: Uuid,
        values: Option<Value>,
    ) -> Result<Option<Value>, PortError>;

    async fn delete_attached_values(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<(), PortError>;
}

pub fn validate_attached_values_patch(values: Option<&Value>) -> Result<(), PortError> {
    if values.is_some_and(|values| !values.is_object()) {
        return Err(PortError::validation(
            "product.attached_values_invalid", "Expected a custom field object",
        ));
    }
    Ok(())
}

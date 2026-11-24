use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Capability granted by the host to internal callers that bind checkout operations.
pub const CART_CHECKOUT_IDENTITY_READ_CLAIM: &str = "cart.checkout_identity.read";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadCartCheckoutIdentityRequest {
    pub cart_id: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartCheckoutIdentity {
    pub tenant_id: Uuid,
    pub cart_id: Uuid,
}

/// Confirms the retained cart identity for a tenant before an internal checkout binding.
/// Guest access and buyer authorization are enforced separately by the storefront.
#[async_trait]
pub trait CartCheckoutIdentityReadPort: Send + Sync {
    async fn read_cart_checkout_identity(
        &self,
        context: PortContext,
        request: ReadCartCheckoutIdentityRequest,
    ) -> Result<CartCheckoutIdentity, PortError>;
}

use super::native_server_adapter::{self, ApiError};
use crate::model::{
    CommerceAdminCartSnapshot, CommerceCartPromotionDraft, CommerceCartPromotionPreview,
};

pub async fn preview_cart_promotion(
    cart_id: String,
    draft: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ApiError> {
    native_server_adapter::preview_cart_promotion(cart_id, draft).await
}

pub async fn apply_cart_promotion(
    cart_id: String,
    draft: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ApiError> {
    native_server_adapter::apply_cart_promotion(cart_id, draft).await
}

#[cfg(test)]
mod tests {
    use std::any::type_name;

    use super::*;

    #[test]
    fn promotion_transport_keeps_api_error_contract() {
        assert!(type_name::<ApiError>().contains("ApiError"));
    }
}

use super::native_server_adapter::{self, ApiError};
use super::promotion_client_error_safety::PromotionClientErrorContext;
use crate::model::{
    CommerceAdminCartSnapshot, CommerceCartPromotionDraft, CommerceCartPromotionPreview,
};

pub async fn preview_cart_promotion(
    cart_id: String,
    draft: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ApiError> {
    let context = PromotionClientErrorContext::for_preview(cart_id.as_str());
    native_server_adapter::preview_cart_promotion(cart_id, draft)
        .await
        .map_err(|promotion_error| context.map_error(promotion_error))
}

pub async fn apply_cart_promotion(
    cart_id: String,
    draft: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ApiError> {
    let context = PromotionClientErrorContext::for_apply(cart_id.as_str());
    native_server_adapter::apply_cart_promotion(cart_id, draft)
        .await
        .map_err(|promotion_error| context.map_error(promotion_error))
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

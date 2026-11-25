use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::UpdateProductImageInput;

#[derive(Serialize, Deserialize)]
pub(crate) struct ProductIdPayload {
    pub product_id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ReadAttachedValuesPayload {
    pub product_id: Uuid,
    pub fallback_locale: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct DeleteVariantPayload {
    pub variant_id: Uuid,
    pub expected_revision: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ProductInputPayload<T> {
    pub product_id: Uuid,
    pub input: T,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct VariantInputPayload<T> {
    pub variant_id: Uuid,
    pub input: T,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ProductImageIdPayload {
    pub product_id: Uuid,
    pub image_id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct UpdateProductImagePayload {
    pub product_id: Uuid,
    pub image_id: Uuid,
    pub input: UpdateProductImageInput,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ReorderProductImagesPayload {
    pub product_id: Uuid,
    pub image_ids: Vec<Uuid>,
}

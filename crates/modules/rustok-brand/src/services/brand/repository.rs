use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    dto::{BrandDto, BrandTranslationDto},
    entities::{brand, brand_translation},
    error::BrandResult,
};

/// Shared by read queries and by commands that need the snapshot before commit.
pub(super) async fn load_translations(
    db: &impl ConnectionTrait,
    brand_id: Uuid,
) -> BrandResult<Vec<brand_translation::Model>> {
    Ok(brand_translation::Entity::find()
        .filter(brand_translation::Column::BrandId.eq(brand_id))
        .all(db)
        .await?)
}

/// Batch loads translations for multiple brand IDs in a single query to eliminate N+1 roundtrips.
pub(super) async fn load_translations_batch(
    db: &impl ConnectionTrait,
    brand_ids: &[Uuid],
) -> BrandResult<HashMap<Uuid, Vec<brand_translation::Model>>> {
    if brand_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let all = brand_translation::Entity::find()
        .filter(brand_translation::Column::BrandId.is_in(brand_ids.to_vec()))
        .all(db)
        .await?;
    let mut map: HashMap<Uuid, Vec<brand_translation::Model>> =
        HashMap::with_capacity(brand_ids.len());
    for t in all {
        map.entry(t.brand_id).or_default().push(t);
    }
    Ok(map)
}

pub(super) fn assemble_brand_dto(
    model: brand::Model,
    translations: Vec<brand_translation::Model>,
    locale: Option<&str>,
) -> BrandDto {
    let translation_dtos: Vec<BrandTranslationDto> = translations
        .iter()
        .map(|t| BrandTranslationDto {
            id: t.id,
            locale: t.locale.clone(),
            name: t.name.clone(),
            description: t.description.clone(),
        })
        .collect();

    // Best match for requested locale
    let effective_trans = if let Some(req_loc) = locale {
        translations
            .iter()
            .find(|t| t.locale.eq_ignore_ascii_case(req_loc))
            .or_else(|| translations.iter().find(|t| t.locale.starts_with("en")))
            .or_else(|| translations.first())
    } else {
        translations
            .iter()
            .find(|t| t.locale.starts_with("en"))
            .or_else(|| translations.first())
    };

    let name = effective_trans
        .map(|t| t.name.clone())
        .unwrap_or_else(|| model.slug.clone());
    let description = effective_trans.and_then(|t| t.description.clone());

    BrandDto {
        id: model.id,
        tenant_id: model.tenant_id,
        slug: model.slug,
        name,
        description,
        logo_media_id: model.logo_media_id,
        banner_media_id: model.banner_media_id,
        website_url: model.website_url,
        is_active: model.is_active,
        metadata: model.metadata,
        translations: translation_dtos,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    }
}

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
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

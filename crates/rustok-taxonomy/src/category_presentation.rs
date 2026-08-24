use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set,
    TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use crate::dto::{
    SetTaxonomyCategoryPresentationInput, TaxonomyCategoryMediaId, TaxonomyCategoryPresentation,
    TaxonomyTermKind,
};
use crate::entities::{taxonomy_category_presentation, taxonomy_term};
use crate::error::{TaxonomyError, TaxonomyResult};
use crate::services::TaxonomyService;

pub const TAXONOMY_CATEGORY_ICON_KEY_MAX_BYTES: usize = 64;

/// Owner-neutral validation boundary for Media-owned Category image references.
///
/// Runtime composition must implement this by delegating to Media's public-image
/// owner contract. The implementation must reject an asset that belongs to a
/// different tenant or is not an active, ready public image. Taxonomy stores
/// only the returned Media identity; it never persists storage paths or delivery
/// URLs.
#[async_trait]
pub trait TaxonomyCategoryMediaReferenceValidator: Send + Sync {
    async fn validate_public_image_reference(
        &self,
        tenant_id: Uuid,
        media_id: TaxonomyCategoryMediaId,
    ) -> TaxonomyResult<()>;
}

impl TaxonomyService {
    /// Read the canonical presentation owned by a shared Category.
    ///
    /// A Category without a persisted presentation has an empty canonical
    /// presentation at revision `0`.
    pub async fn get_category_presentation(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        term_id: Uuid,
    ) -> TaxonomyResult<TaxonomyCategoryPresentation> {
        enforce_taxonomy_scope(&security, Action::Read)?;
        load_category(self.database(), tenant_id, term_id).await?;
        let row = taxonomy_category_presentation::Entity::find_by_id((tenant_id, term_id))
            .one(self.database())
            .await?;

        Ok(row
            .map(presentation_from_model)
            .unwrap_or_else(|| empty_presentation(term_id)))
    }

    /// Replace the canonical presentation for a shared Category.
    ///
    /// Presentation revision is deliberately independent from the Taxonomy term
    /// revision used by Translation CAS. Icon/color/media changes therefore do
    /// not invalidate a text translation proposal. Existing rows are updated
    /// with a storage-level revision compare-and-swap to prevent lost updates.
    pub async fn set_category_presentation(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        term_id: Uuid,
        input: SetTaxonomyCategoryPresentationInput,
        media_validator: Option<&dyn TaxonomyCategoryMediaReferenceValidator>,
    ) -> TaxonomyResult<TaxonomyCategoryPresentation> {
        enforce_taxonomy_scope(&security, Action::Update)?;
        if input.expected_revision.is_some_and(|revision| revision < 0) {
            return Err(TaxonomyError::validation(
                "Category presentation expected revision cannot be negative",
            ));
        }

        let icon_key = input
            .icon_key
            .as_deref()
            .map(normalize_taxonomy_category_icon_key)
            .transpose()?
            .flatten();
        let color = input
            .color
            .as_deref()
            .map(normalize_taxonomy_category_color)
            .transpose()?
            .flatten();

        // Validate owner identity before calling an external capability. The
        // Category check is repeated inside the transaction before persistence
        // so a concurrent delete cannot turn the preflight into write authority.
        load_category(self.database(), tenant_id, term_id).await?;
        validate_media_references(
            media_validator,
            tenant_id,
            input.image_media_id,
            input.cover_media_id,
        )
        .await?;

        let txn = self.database().begin().await?;
        load_category(&txn, tenant_id, term_id).await?;
        let existing = taxonomy_category_presentation::Entity::find_by_id((tenant_id, term_id))
            .one(&txn)
            .await?;

        match existing {
            None => {
                if let Some(expected_revision) = input.expected_revision
                    && expected_revision != 0
                {
                    return Err(TaxonomyError::conflict(format!(
                        "Category presentation revision changed: expected {expected_revision}, current 0",
                    )));
                }

                if icon_key.is_none()
                    && color.is_none()
                    && input.image_media_id.is_none()
                    && input.cover_media_id.is_none()
                {
                    txn.commit().await?;
                    return Ok(empty_presentation(term_id));
                }

                let now = Utc::now().fixed_offset();
                taxonomy_category_presentation::ActiveModel {
                    tenant_id: Set(tenant_id),
                    term_id: Set(term_id),
                    icon_key: Set(icon_key.clone()),
                    color: Set(color.clone()),
                    image_media_id: Set(input
                        .image_media_id
                        .map(TaxonomyCategoryMediaId::into_uuid)),
                    cover_media_id: Set(input
                        .cover_media_id
                        .map(TaxonomyCategoryMediaId::into_uuid)),
                    revision: Set(1),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(&txn)
                .await?;
                txn.commit().await?;

                Ok(TaxonomyCategoryPresentation {
                    term_id,
                    icon_key,
                    color,
                    image_media_id: input.image_media_id,
                    cover_media_id: input.cover_media_id,
                    revision: 1,
                })
            }
            Some(existing) => {
                if let Some(expected_revision) = input.expected_revision
                    && expected_revision != existing.revision
                {
                    return Err(TaxonomyError::conflict(format!(
                        "Category presentation revision changed: expected {expected_revision}, current {}",
                        existing.revision,
                    )));
                }

                let image_media_id = input.image_media_id.map(TaxonomyCategoryMediaId::into_uuid);
                let cover_media_id = input.cover_media_id.map(TaxonomyCategoryMediaId::into_uuid);
                if existing.icon_key == icon_key
                    && existing.color == color
                    && existing.image_media_id == image_media_id
                    && existing.cover_media_id == cover_media_id
                {
                    let result = presentation_from_model(existing);
                    txn.commit().await?;
                    return Ok(result);
                }

                let next_revision = existing.revision.checked_add(1).ok_or_else(|| {
                    TaxonomyError::conflict("Category presentation revision is exhausted")
                })?;
                let now = Utc::now().fixed_offset();
                let updated = taxonomy_category_presentation::Entity::update_many()
                    .col_expr(
                        taxonomy_category_presentation::Column::IconKey,
                        Expr::value(icon_key.clone()),
                    )
                    .col_expr(
                        taxonomy_category_presentation::Column::Color,
                        Expr::value(color.clone()),
                    )
                    .col_expr(
                        taxonomy_category_presentation::Column::ImageMediaId,
                        Expr::value(image_media_id),
                    )
                    .col_expr(
                        taxonomy_category_presentation::Column::CoverMediaId,
                        Expr::value(cover_media_id),
                    )
                    .col_expr(
                        taxonomy_category_presentation::Column::Revision,
                        Expr::value(next_revision),
                    )
                    .col_expr(
                        taxonomy_category_presentation::Column::UpdatedAt,
                        Expr::value(now),
                    )
                    .filter(taxonomy_category_presentation::Column::TenantId.eq(tenant_id))
                    .filter(taxonomy_category_presentation::Column::TermId.eq(term_id))
                    .filter(taxonomy_category_presentation::Column::Revision.eq(existing.revision))
                    .exec(&txn)
                    .await?;
                if updated.rows_affected != 1 {
                    return Err(TaxonomyError::conflict(
                        "Category presentation changed before the update could commit",
                    ));
                }

                txn.commit().await?;
                Ok(TaxonomyCategoryPresentation {
                    term_id,
                    icon_key,
                    color,
                    image_media_id: input.image_media_id,
                    cover_media_id: input.cover_media_id,
                    revision: next_revision,
                })
            }
        }
    }
}

async fn validate_media_references(
    validator: Option<&dyn TaxonomyCategoryMediaReferenceValidator>,
    tenant_id: Uuid,
    image_media_id: Option<TaxonomyCategoryMediaId>,
    cover_media_id: Option<TaxonomyCategoryMediaId>,
) -> TaxonomyResult<()> {
    if image_media_id.is_none() && cover_media_id.is_none() {
        return Ok(());
    }

    let validator = validator.ok_or_else(|| {
        TaxonomyError::validation(
            "Category Media references require the Media public-image validation capability",
        )
    })?;

    if let Some(media_id) = image_media_id {
        validator
            .validate_public_image_reference(tenant_id, media_id)
            .await?;
    }
    if let Some(media_id) = cover_media_id
        && Some(media_id) != image_media_id
    {
        validator
            .validate_public_image_reference(tenant_id, media_id)
            .await?;
    }
    Ok(())
}

async fn load_category<C>(
    db: &C,
    tenant_id: Uuid,
    term_id: Uuid,
) -> TaxonomyResult<taxonomy_term::Model>
where
    C: ConnectionTrait,
{
    let term = taxonomy_term::Entity::find_by_id(term_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(TaxonomyError::TermNotFound(term_id))?;
    if term.kind != TaxonomyTermKind::Category {
        return Err(TaxonomyError::validation(format!(
            "Taxonomy term {term_id} is not a Category",
        )));
    }
    Ok(term)
}

fn presentation_from_model(
    row: taxonomy_category_presentation::Model,
) -> TaxonomyCategoryPresentation {
    TaxonomyCategoryPresentation {
        term_id: row.term_id,
        icon_key: row.icon_key,
        color: row.color,
        image_media_id: row.image_media_id.map(TaxonomyCategoryMediaId::new),
        cover_media_id: row.cover_media_id.map(TaxonomyCategoryMediaId::new),
        revision: row.revision,
    }
}

fn empty_presentation(term_id: Uuid) -> TaxonomyCategoryPresentation {
    TaxonomyCategoryPresentation {
        term_id,
        icon_key: None,
        color: None,
        image_media_id: None,
        cover_media_id: None,
        revision: 0,
    }
}

fn enforce_taxonomy_scope(security: &SecurityContext, action: Action) -> TaxonomyResult<()> {
    if matches!(
        security.get_scope(Resource::Taxonomy, action),
        PermissionScope::None
    ) {
        return Err(TaxonomyError::forbidden("Permission denied"));
    }
    Ok(())
}

/// Normalize a semantic design-system icon key.
///
/// CSS classes, markup, URLs and file paths are deliberately not accepted.
pub fn normalize_taxonomy_category_icon_key(value: &str) -> TaxonomyResult<Option<String>> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(None);
    }
    if normalized.len() > TAXONOMY_CATEGORY_ICON_KEY_MAX_BYTES {
        return Err(TaxonomyError::validation(format!(
            "Category icon key cannot exceed {TAXONOMY_CATEGORY_ICON_KEY_MAX_BYTES} bytes",
        )));
    }

    let mut previous_was_separator = true;
    for character in normalized.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            previous_was_separator = false;
        } else if character == '-' && !previous_was_separator {
            previous_was_separator = true;
        } else {
            return Err(TaxonomyError::validation(
                "Category icon key must be a kebab-case design token",
            ));
        }
    }
    if previous_was_separator {
        return Err(TaxonomyError::validation(
            "Category icon key must be a kebab-case design token",
        ));
    }

    Ok(Some(normalized))
}

/// Normalize canonical Category color to lower-case `#rrggbb` or `#rrggbbaa`.
pub fn normalize_taxonomy_category_color(value: &str) -> TaxonomyResult<Option<String>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let digits = trimmed.strip_prefix('#').ok_or_else(|| {
        TaxonomyError::validation("Category color must use #RGB, #RGBA, #RRGGBB, or #RRGGBBAA")
    })?;
    if !matches!(digits.len(), 3 | 4 | 6 | 8)
        || !digits
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(TaxonomyError::validation(
            "Category color must use #RGB, #RGBA, #RRGGBB, or #RRGGBBAA",
        ));
    }

    let lowercase = digits.to_ascii_lowercase();
    let expanded = if matches!(lowercase.len(), 3 | 4) {
        lowercase
            .chars()
            .flat_map(|character| [character, character])
            .collect::<String>()
    } else {
        lowercase
    };
    Ok(Some(format!("#{expanded}")))
}

#[cfg(test)]
mod tests {
    use super::{normalize_taxonomy_category_color, normalize_taxonomy_category_icon_key};

    #[test]
    fn icon_key_is_bounded_semantic_kebab_case() {
        assert_eq!(
            normalize_taxonomy_category_icon_key("  Message-Square  ")
                .expect("valid token")
                .as_deref(),
            Some("message-square")
        );
        for value in ["message_square", "icon class", "<svg>", "../icon", "a--b"] {
            assert!(normalize_taxonomy_category_icon_key(value).is_err());
        }
    }

    #[test]
    fn color_is_canonical_lowercase_expanded_hex() {
        assert_eq!(
            normalize_taxonomy_category_color(" #F0A ")
                .expect("valid color")
                .as_deref(),
            Some("#ff00aa")
        );
        assert_eq!(
            normalize_taxonomy_category_color("#A1B2C3D4")
                .expect("valid color")
                .as_deref(),
            Some("#a1b2c3d4")
        );
        for value in ["red", "rgb(1 2 3)", "#ggg", "#fff;--owned:1"] {
            assert!(normalize_taxonomy_category_color(value).is_err());
        }
    }
}

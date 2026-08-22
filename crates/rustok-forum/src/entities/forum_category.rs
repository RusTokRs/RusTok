use sea_orm::ActiveValue;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::category_presentation::normalize_category_icon_key;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_categories")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// Transitional CAT-5 binding to the canonical Taxonomy Category identity.
    ///
    /// Legacy Forum category identity/hierarchy/localized copy remain live until
    /// deterministic backfill and read/write cutover evidence are complete.
    pub taxonomy_category_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub moderated: bool,
    pub topic_count: i32,
    pub reply_count: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::forum_category_translation::Entity")]
    Translations,
    #[sea_orm(has_many = "super::forum_topic::Entity")]
    Topics,
}

impl Related<super::forum_category_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl Related<super::forum_topic::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Topics.def()
    }
}

/// Bind one Forum category policy row to an existing canonical Taxonomy Category.
///
/// This is deliberately a narrow migration seam. It does not copy localized
/// category data, hierarchy or presentation into Taxonomy and it does not switch
/// Forum reads/writes away from their legacy owner yet. The caller must perform
/// those staged cutover steps separately.
pub(crate) async fn bind_taxonomy_category<C>(
    db: &C,
    tenant_id: Uuid,
    forum_category_id: Uuid,
    taxonomy_category_id: Uuid,
) -> Result<Model, DbErr>
where
    C: ConnectionTrait,
{
    let taxonomy_exists = rustok_taxonomy::taxonomy_term_identity_exists(
        db,
        tenant_id,
        rustok_taxonomy::TaxonomyTermKind::Category,
        taxonomy_category_id,
    )
    .await
    .map_err(|_| DbErr::Custom("Taxonomy category identity lookup failed".to_string()))?;
    if !taxonomy_exists {
        return Err(DbErr::Custom(
            "Taxonomy category binding must reference a same-tenant Category".to_string(),
        ));
    }

    let category = Entity::find_by_id(forum_category_id)
        .filter(Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("Forum category binding source not found".into()))?;

    if category.taxonomy_category_id == Some(taxonomy_category_id) {
        return Ok(category);
    }

    let duplicate = Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::TaxonomyCategoryId.eq(taxonomy_category_id))
        .filter(Column::Id.ne(forum_category_id))
        .one(db)
        .await?;
    if duplicate.is_some() {
        return Err(DbErr::Custom(
            "Taxonomy category is already bound to another Forum category".to_string(),
        ));
    }

    let mut active: ActiveModel = category.into();
    active.taxonomy_category_id = ActiveValue::Set(Some(taxonomy_category_id));
    active.update(db).await
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if let ActiveValue::Set(Some(icon)) = &mut self.icon {
            let normalized = normalize_category_icon_key(icon).ok_or_else(|| {
                DbErr::Custom(
                    "Forum category icon must be a bounded kebab-case design token".to_string(),
                )
            })?;
            *icon = normalized;
        }

        if let ActiveValue::Set(Some(color)) = &mut self.color {
            let normalized = normalize_category_color(color).ok_or_else(|| {
                DbErr::Custom(
                    "Forum category color must use #RGB, #RGBA, #RRGGBB, or #RRGGBBAA".to_string(),
                )
            })?;
            *color = normalized;
        }

        Ok(self)
    }
}

fn normalize_category_color(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let digits = trimmed.strip_prefix('#')?;
    if !matches!(digits.len(), 3 | 4 | 6 | 8)
        || !digits
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }

    Some(format!("#{digits}"))
}

#[cfg(test)]
mod tests {
    use super::normalize_category_color;

    #[test]
    fn normalizes_supported_category_color_tokens() {
        assert_eq!(
            normalize_category_color(" #0EA5E9 ").as_deref(),
            Some("#0EA5E9")
        );
        assert_eq!(normalize_category_color("#fff").as_deref(), Some("#fff"));
        assert_eq!(normalize_category_color("#abcd").as_deref(), Some("#abcd"));
    }

    #[test]
    fn rejects_css_declaration_injection_before_persistence() {
        for value in [
            "red",
            "rgb(1 2 3)",
            "#ggg",
            "#fff;background:url(https://attacker.invalid/x)",
            "#fff;--owned:1",
        ] {
            assert_eq!(normalize_category_color(value), None, "accepted {value:?}");
        }
    }
}

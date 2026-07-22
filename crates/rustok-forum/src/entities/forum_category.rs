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

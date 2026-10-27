use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(
    rs_type = "String",
    db_type = "Enum",
    enum_name = "product_status_enum"
)]
#[serde(rename_all = "lowercase")]
pub enum ProductStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "archived")]
    Archived,
}

impl std::fmt::Display for ProductStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Active => write!(f, "active"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "products")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<Uuid>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub published_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::product_variant::Entity")]
    Variants,
    #[sea_orm(has_many = "super::product_image::Entity")]
    Images,
}

impl Related<super::product_variant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Variants.def()
    }
}

impl Related<super::product_image::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Images.def()
    }
}

impl ProductStatus {
    /// Validates whether transitioning to target status is valid in the product lifecycle.
    pub fn can_transition_to(&self, target: &ProductStatus) -> bool {
        if self == target {
            return true;
        }
        matches!(
            (self, target),
            (Self::Draft, Self::Active)
                | (Self::Draft, Self::Archived)
                | (Self::Active, Self::Draft)
                | (Self::Active, Self::Archived)
                | (Self::Archived, Self::Draft)
                | (Self::Archived, Self::Active)
        )
    }
}

impl Model {
    /// Returns true if product is active and has a published_at timestamp.
    #[inline]
    pub fn is_published(&self) -> bool {
        self.status == ProductStatus::Active && self.published_at.is_some()
    }

    /// Returns true if product is in draft status.
    #[inline]
    pub fn is_draft(&self) -> bool {
        self.status == ProductStatus::Draft
    }

    /// Returns true if product is archived.
    #[inline]
    pub fn is_archived(&self) -> bool {
        self.status == ProductStatus::Archived
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_product_status_lifecycle_and_helpers() {
        assert!(ProductStatus::Draft.can_transition_to(&ProductStatus::Active));
        assert!(ProductStatus::Active.can_transition_to(&ProductStatus::Archived));
        assert!(ProductStatus::Archived.can_transition_to(&ProductStatus::Draft));
        assert_eq!(ProductStatus::Draft.to_string(), "draft");
        assert_eq!(ProductStatus::Active.to_string(), "active");
        assert_eq!(ProductStatus::Archived.to_string(), "archived");

        let product = Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            status: ProductStatus::Active,
            seller_id: None,
            vendor: None,
            product_type: None,
            shipping_profile_slug: None,
            primary_category_id: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            published_at: Some(Utc::now().into()),
        };

        assert!(product.is_published());
        assert!(!product.is_draft());
        assert!(!product.is_archived());
    }
}

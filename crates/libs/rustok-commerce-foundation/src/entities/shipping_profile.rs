use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "shipping_profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub active: bool,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Model {
    /// Returns true if the shipping profile is active and usable for checkout.
    #[inline]
    pub fn is_usable(&self) -> bool {
        self.active
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_shipping_profile_is_usable() {
        let profile = Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            slug: "default".into(),
            active: true,
            metadata: serde_json::json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        };
        assert!(profile.is_usable());

        let inactive = Model {
            active: false,
            ..profile
        };
        assert!(!inactive.is_usable());
    }
}

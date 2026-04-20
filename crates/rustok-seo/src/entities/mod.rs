use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "meta")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub no_index: bool,
    pub no_follow: bool,
    pub canonical_url: Option<String>,
    pub structured_data: Option<Json>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "meta_translation::Entity")]
    Translations,
}

impl Related<meta_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub mod meta_translation {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "meta_translations")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub meta_id: Uuid,
        pub locale: String,
        pub title: Option<String>,
        pub description: Option<String>,
        pub keywords: Option<String>,
        pub og_title: Option<String>,
        pub og_description: Option<String>,
        pub og_image: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::Entity",
            from = "Column::MetaId",
            to = "super::Column::Id",
            on_delete = "Cascade"
        )]
        Meta,
    }

    impl Related<super::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Meta.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod seo_redirect {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "seo_redirects")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub match_type: String,
        pub source_pattern: String,
        pub target_url: String,
        pub status_code: i32,
        pub expires_at: Option<DateTimeWithTimeZone>,
        pub is_active: bool,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod seo_revision {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "seo_revisions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub target_kind: String,
        pub target_id: Uuid,
        pub revision: i32,
        pub note: Option<String>,
        pub payload: Json,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod seo_sitemap_job {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "seo_sitemap_jobs")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub status: String,
        pub file_count: i32,
        pub started_at: Option<DateTimeWithTimeZone>,
        pub completed_at: Option<DateTimeWithTimeZone>,
        pub last_error: Option<String>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::seo_sitemap_file::Entity")]
        Files,
    }

    impl Related<super::seo_sitemap_file::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Files.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod seo_sitemap_file {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "seo_sitemap_files")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub job_id: Uuid,
        pub path: String,
        pub url_count: i32,
        pub content: String,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::seo_sitemap_job::Entity",
            from = "Column::JobId",
            to = "super::seo_sitemap_job::Column::Id",
            on_delete = "Cascade"
        )]
        Job,
    }

    impl Related<super::seo_sitemap_job::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Job.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

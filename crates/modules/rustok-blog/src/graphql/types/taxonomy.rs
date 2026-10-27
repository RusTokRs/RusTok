use async_graphql::{InputObject, SimpleObject};
use uuid::Uuid;

use crate::BlogError;

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "BlogCategory")]
pub struct GqlBlogCategory {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub settings: String,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "BlogCategoryList")]
pub struct GqlBlogCategoryList {
    pub items: Vec<GqlBlogCategory>,
    pub total: u64,
}

#[derive(InputObject, Clone, Debug, Default)]
pub struct BlogCategoriesFilter {
    pub locale: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "BlogTag")]
pub struct GqlBlogTag {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub use_count: i32,
    pub created_at: String,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "BlogTagList")]
pub struct GqlBlogTagList {
    pub items: Vec<GqlBlogTag>,
    pub total: u64,
}

#[derive(InputObject, Clone, Debug, Default)]
pub struct BlogTagsFilter {
    pub locale: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject, Clone, Debug)]
pub struct CreateBlogCategoryInput {
    pub locale: String,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: Option<i32>,
    pub settings: Option<String>,
}

#[derive(InputObject, Clone, Debug)]
pub struct UpdateBlogCategoryInput {
    pub locale: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub settings: Option<String>,
}

impl From<crate::dto::CategoryListItem> for GqlBlogCategory {
    fn from(item: crate::dto::CategoryListItem) -> Self {
        Self {
            id: item.id,
            locale: item.locale,
            effective_locale: item.effective_locale,
            name: item.name,
            slug: item.slug,
            description: item.description,
            parent_id: item.parent_id,
            position: item.position,
            settings: item.settings.to_string(),
            created_at: item.created_at.to_rfc3339(),
            updated_at: Some(item.updated_at.to_rfc3339()),
        }
    }
}

impl From<crate::dto::CategoryResponse> for GqlBlogCategory {
    fn from(cat: crate::dto::CategoryResponse) -> Self {
        Self {
            id: cat.id,
            locale: cat.locale,
            effective_locale: cat.effective_locale,
            name: cat.name,
            slug: cat.slug,
            description: cat.description,
            parent_id: cat.parent_id,
            position: cat.position,
            settings: cat.settings.to_string(),
            created_at: cat.created_at.to_rfc3339(),
            updated_at: Some(cat.updated_at.to_rfc3339()),
        }
    }
}

impl From<crate::dto::TagListItem> for GqlBlogTag {
    fn from(item: crate::dto::TagListItem) -> Self {
        Self {
            id: item.id,
            locale: item.locale,
            effective_locale: item.effective_locale,
            name: item.name,
            slug: item.slug,
            use_count: item.use_count,
            created_at: item.created_at.to_rfc3339(),
        }
    }
}

fn parse_settings(value: Option<&str>) -> Result<Option<serde_json::Value>, BlogError> {
    value
        .map(|v| {
            serde_json::from_str(v)
                .map_err(|_| BlogError::validation("settings must be a valid JSON document"))
        })
        .transpose()
}

impl TryFrom<CreateBlogCategoryInput> for crate::dto::CreateCategoryInput {
    type Error = BlogError;
    fn try_from(input: CreateBlogCategoryInput) -> Result<Self, Self::Error> {
        let settings =
            parse_settings(input.settings.as_deref())?.unwrap_or_else(|| serde_json::json!({}));
        Ok(Self {
            locale: input.locale,
            name: input.name,
            slug: input.slug,
            description: input.description,
            parent_id: input.parent_id,
            position: input.position,
            settings,
        })
    }
}

impl TryFrom<UpdateBlogCategoryInput> for crate::dto::UpdateCategoryInput {
    type Error = BlogError;
    fn try_from(input: UpdateBlogCategoryInput) -> Result<Self, Self::Error> {
        Ok(Self {
            locale: input.locale,
            name: input.name,
            slug: input.slug,
            description: input.description,
            settings: parse_settings(input.settings.as_deref())?,
        })
    }
}

use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::*;

pub async fn fetch_category_details(
    locale: Option<String>,
) -> Result<Vec<crate::model::BlogCategoryDetail>, ServerFnError> {
    blog_admin_category_details_native(locale).await
}

pub async fn create_category(
    draft: crate::model::BlogCategoryDraft,
) -> Result<String, ServerFnError> {
    blog_admin_create_category_native(draft).await
}

pub async fn update_category(
    id: String,
    update: crate::model::BlogCategoryUpdate,
) -> Result<crate::model::BlogCategoryDetail, ServerFnError> {
    blog_admin_update_category_native(id, update).await
}

pub async fn delete_category(id: String) -> Result<bool, ServerFnError> {
    blog_admin_delete_category_native(id).await
}

/// Maps the owner list item into the admin detail projection.
#[cfg(feature = "ssr")]
fn map_category_detail(item: rustok_blog::CategoryListItem) -> crate::model::BlogCategoryDetail {
    crate::model::BlogCategoryDetail {
        id: item.id.to_string(),
        locale: item.locale,
        effective_locale: item.effective_locale,
        name: item.name,
        slug: item.slug,
        description: item.description,
        parent_id: item.parent_id.map(|id| id.to_string()),
        position: item.position,
        created_at: item.created_at.to_rfc3339(),
        updated_at: Some(item.updated_at.to_rfc3339()),
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/category-details")]
async fn blog_admin_category_details_native(
    locale: Option<String>,
) -> Result<Vec<crate::model::BlogCategoryDetail>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::{CategoryService, ListCategoriesFilter};

        let context = native_context().await?;
        let locale = requested_locale(locale, context.tenant.default_locale.as_str());
        let service = CategoryService::new(context.db, context.event_bus);
        let mut items = Vec::new();
        for page in 1.. {
            let (rows, total) = service
                .list(
                    context.tenant.id,
                    security_context(&context.auth),
                    ListCategoriesFilter {
                        locale: Some(locale.clone()),
                        page,
                        per_page: 100,
                    },
                )
                .await
                .map_err(public_blog_error)?;
            items.extend(rows.into_iter().map(map_category_detail));
            if items.len() as u64 >= total {
                return Ok(items);
            }
            if (items.len() as u64) < page * 100 {
                return Err(ServerFnError::new("Incomplete Blog categories projection"));
            }
        }
        unreachable!("Blog categories are bounded by the Taxonomy owner")
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = locale;
        Err(ServerFnError::new(
            "blog/admin/category-details requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/create-category")]
async fn blog_admin_create_category_native(
    draft: crate::model::BlogCategoryDraft,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::{CategoryService, CreateCategoryInput};

        let context = native_context().await?;
        require_category_permission(&context.auth, rustok_api::Permission::BLOG_CATEGORIES_CREATE)?;
        let parent_id = match draft.parent_id.as_deref() {
            Some(raw) => Some(parse_uuid(raw, "parent_id")?),
            None => None,
        };
        let category_id = CategoryService::new(context.db, context.event_bus)
            .create(
                context.tenant.id,
                security_context(&context.auth),
                CreateCategoryInput {
                    locale: draft.locale,
                    name: draft.name,
                    slug: draft.slug,
                    description: draft.description,
                    parent_id,
                    position: draft.position,
                    settings: serde_json::json!({}),
                },
            )
            .await
            .map_err(public_blog_error)?;
        Ok(category_id.to_string())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = draft;
        Err(ServerFnError::new(
            "blog/admin/create-category requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/update-category")]
async fn blog_admin_update_category_native(
    id: String,
    update: crate::model::BlogCategoryUpdate,
) -> Result<crate::model::BlogCategoryDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::{CategoryService, UpdateCategoryInput};

        let context = native_context().await?;
        require_category_permission(&context.auth, rustok_api::Permission::BLOG_CATEGORIES_UPDATE)?;
        let category_id = parse_uuid(&id, "category_id")?;
        let response = CategoryService::new(context.db, context.event_bus)
            .update(
                context.tenant.id,
                category_id,
                security_context(&context.auth),
                UpdateCategoryInput {
                    locale: update.locale,
                    name: update.name,
                    slug: update.slug,
                    description: update.description,
                    settings: None,
                },
            )
            .await
            .map_err(public_blog_error)?;
        Ok(crate::model::BlogCategoryDetail {
            id: response.id.to_string(),
            locale: response.locale,
            effective_locale: response.effective_locale,
            name: response.name,
            slug: response.slug,
            description: response.description,
            parent_id: response.parent_id.map(|parent| parent.to_string()),
            position: response.position,
            created_at: response.created_at.to_rfc3339(),
            updated_at: Some(response.updated_at.to_rfc3339()),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, update);
        Err(ServerFnError::new(
            "blog/admin/update-category requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/delete-category")]
async fn blog_admin_delete_category_native(id: String) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_blog::CategoryService;

        let context = native_context().await?;
        require_category_permission(&context.auth, rustok_api::Permission::BLOG_CATEGORIES_DELETE)?;
        let category_id = parse_uuid(&id, "category_id")?;
        let mut service = CategoryService::new(context.db, context.event_bus);
        if let Some(cleanup) = context.category_delete_cleanup.clone() {
            service = service.with_category_delete_cleanup(cleanup);
        }
        service
            .delete(
                context.tenant.id,
                category_id,
                security_context(&context.auth),
            )
            .await
            .map_err(public_blog_error)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "blog/admin/delete-category requires the `ssr` feature",
        ))
    }
}

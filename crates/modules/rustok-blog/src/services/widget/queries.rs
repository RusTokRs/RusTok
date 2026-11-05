use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use serde::Deserialize;
use uuid::Uuid;

use super::{BlogWidgetService, contract};
use crate::dto::{
    BlogWidgetInput, BlogWidgetPayload, BlogWidgetPreview, ListCategoriesFilter, PostListQuery,
};
use crate::error::{BlogError, BlogResult};
use crate::services::{CategoryService, PostService, rbac::enforce_scope};

#[derive(Deserialize)]
struct Props {
    locale: Option<String>,
    category_id: Option<Uuid>,
    post_id: Option<Uuid>,
    after: Option<String>,
    per_page: Option<u32>,
    page: Option<u64>,
}

impl BlogWidgetService {
    pub(super) async fn preview_owner(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
        input: BlogWidgetInput,
    ) -> BlogResult<BlogWidgetPreview> {
        if tenant_id.is_nil() || security.user_id.is_none_or(|id| id.is_nil()) {
            return Err(BlogError::forbidden(
                "Authenticated tenant and principal are required",
            ));
        }
        enforce_scope(&security, Resource::BlogPosts, Action::Read)?;
        let validation = Self::validate(input);
        let mut response = BlogWidgetPreview {
            widget_type: validation.widget_type,
            valid: validation.valid,
            normalized_props: validation.normalized_props,
            issues: validation.issues,
            payload: None,
        };
        if !response.valid {
            return Ok(response);
        }
        let props: Props =
            serde_json::from_value(response.normalized_props.clone()).map_err(|_| {
                BlogError::invariant("Validated widget configuration cannot be decoded")
            })?;
        let locale = props.locale.as_deref().unwrap_or(locale);
        let posts = PostService::new(self.db.clone(), self.event_bus.clone());
        response.payload = Some(match response.widget_type.as_str() {
            contract::POST_LIST => BlogWidgetPayload::PostList(
                posts
                    .list_posts_with_locale_fallback_on_channel(
                        tenant_id,
                        security,
                        PostListQuery {
                            locale: Some(locale.into()),
                            category_id: props.category_id,
                            after: props.after,
                            per_page: props.per_page,
                            ..Default::default()
                        },
                        fallback_locale,
                        channel_slug,
                    )
                    .await?,
            ),
            contract::POST_DETAIL => {
                let id = props
                    .post_id
                    .ok_or_else(|| BlogError::invariant("Validated post widget has no post ID"))?;
                BlogWidgetPayload::PostDetail(Box::new(
                    posts
                        .get_post_with_locale_fallback_on_channel(
                            tenant_id,
                            security,
                            id,
                            locale,
                            fallback_locale,
                            channel_slug,
                        )
                        .await?,
                ))
            }
            contract::CATEGORY_LIST => {
                // Catalog discovery never grants access to Category records.
                enforce_scope(&security, Resource::BlogCategories, Action::Read)?;
                let page = props
                    .page
                    .ok_or_else(|| BlogError::invariant("Validated category widget has no page"))?;
                let per_page = u64::from(props.per_page.ok_or_else(|| {
                    BlogError::invariant("Validated category widget has no page size")
                })?);
                let (items, total) = CategoryService::new(self.db.clone(), self.event_bus.clone())
                    .list(
                        tenant_id,
                        security,
                        ListCategoriesFilter {
                            locale: Some(locale.into()),
                            page,
                            per_page,
                        },
                    )
                    .await?;
                BlogWidgetPayload::CategoryList {
                    items,
                    total,
                    page,
                    per_page,
                }
            }
            _ => {
                return Err(BlogError::invariant(
                    "Validated widget is absent from owner catalog",
                ));
            }
        });
        Ok(response)
    }
}

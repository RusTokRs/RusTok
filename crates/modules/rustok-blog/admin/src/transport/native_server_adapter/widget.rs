use crate::model::{
    BlogWidgetPreviewRequest, BlogWidgetPropertyRequest, BlogWidgetPropertySchema,
    BlogWidgetValidation, BlogWidgetValidationRequest,
};
use leptos::prelude::*;
#[cfg(all(test, feature = "ssr"))]
mod tests;

pub(in crate::transport) async fn schema(
    request: BlogWidgetPropertyRequest,
) -> Result<BlogWidgetPropertySchema, ServerFnError> {
    blog_widget_schema_native(request).await
}

pub(in crate::transport) async fn validate(
    request: BlogWidgetValidationRequest,
) -> Result<BlogWidgetValidation, ServerFnError> {
    blog_widget_validate_native(request).await
}

pub(in crate::transport) async fn preview(
    request: BlogWidgetPreviewRequest,
) -> Result<serde_json::Value, ServerFnError> {
    blog_widget_preview_native(request).await
}

#[cfg(feature = "ssr")]
async fn context() -> Result<super::NativeContext, ServerFnError> {
    let context = super::native_context().await?;
    if context.auth.user_id.is_nil()
        || !rustok_api::has_any_effective_permission(
            &context.auth.permissions,
            &[rustok_api::Permission::BLOG_POSTS_READ],
        )
    {
        return Err(ServerFnError::new(
            "Permission denied: blog_posts:read required",
        ));
    }
    Ok(context)
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/widget-schema", input = leptos::server_fn::codec::Json, client = leptos_auth::AuthorizedBrowserClient)]
async fn blog_widget_schema_native(
    request: BlogWidgetPropertyRequest,
) -> Result<BlogWidgetPropertySchema, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        context().await?;
        let reference = crate::core::page_builder::resolve_widget_schema_reference(
            &request.widget_type,
            &request.property_schema,
        )
        .map_err(ServerFnError::new)?;
        let item = rustok_blog::BlogWidgetService::catalog()
            .items
            .into_iter()
            .find(|item| {
                item.widget_type == request.widget_type && item.schema_id == reference.schema_id
            })
            .ok_or_else(super::public_internal_error)?;
        Ok(BlogWidgetPropertySchema {
            schema_id: item.schema_id,
            schema: item.props_schema,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new("Native Blog widget schema requires SSR"))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/widget-validate", input = leptos::server_fn::codec::Json, client = leptos_auth::AuthorizedBrowserClient)]
async fn blog_widget_validate_native(
    request: BlogWidgetValidationRequest,
) -> Result<BlogWidgetValidation, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        context().await?;
        crate::core::page_builder::resolve_widget_schema_reference(
            &request.widget_type,
            &request.property_schema,
        )
        .map_err(ServerFnError::new)?;
        let response = rustok_blog::BlogWidgetService::validate(rustok_blog::BlogWidgetInput {
            widget_type: request.widget_type,
            props: request.props,
        });
        Ok(BlogWidgetValidation {
            widget_type: response.widget_type,
            valid: response.valid,
            normalized_props: response.normalized_props,
            issues: response
                .issues
                .into_iter()
                .map(|issue| crate::model::BlogWidgetIssue {
                    class: issue.class,
                    code: issue.code,
                    message: issue.message,
                    path: issue.path,
                })
                .collect(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "Native Blog widget validation requires SSR",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "blog/admin/widget-preview", input = leptos::server_fn::codec::Json, client = leptos_auth::AuthorizedBrowserClient)]
async fn blog_widget_preview_native(
    request: BlogWidgetPreviewRequest,
) -> Result<serde_json::Value, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let context = context().await?;
        let current = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|_| super::public_internal_error())?;
        if current.tenant_id != context.tenant.id || current.user_id != Some(context.auth.user_id) {
            return Err(ServerFnError::new(
                "Widget preview request context does not match the authenticated principal",
            ));
        }
        let response = rustok_blog::BlogWidgetService::new(context.db, context.event_bus)
            .preview(
                context.tenant.id,
                super::security_context(&context.auth),
                &current.locale,
                Some(&context.tenant.default_locale),
                current.channel_slug.as_deref(),
                rustok_blog::BlogWidgetInput {
                    widget_type: request.widget_type,
                    props: request.props,
                },
            )
            .await
            .map_err(super::public_blog_error)?;
        serde_json::to_value(response).map_err(|_| super::public_internal_error())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "Native Blog widget preview requires SSR",
        ))
    }
}

use super::{ApiError, execute_selected_transport, selected_transport_path};
use crate::model::{
    BlogWidgetPreviewRequest, BlogWidgetPropertyRequest, BlogWidgetPropertySchema,
    BlogWidgetValidation, BlogWidgetValidationRequest,
};

pub async fn load_blog_page_builder_widget_property_schema(
    token: Option<String>,
    tenant_slug: Option<String>,
    request: BlogWidgetPropertyRequest,
) -> Result<BlogWidgetPropertySchema, ApiError> {
    let native = request.clone();
    execute_selected_transport(
        "blog/admin/widget-schema",
        selected_transport_path(),
        move || super::native_server_adapter::widget::schema(native),
        move || super::graphql_adapter::widget::schema(token, tenant_slug, request),
    )
    .await
}

pub async fn validate_blog_page_builder_widget_properties(
    token: Option<String>,
    tenant_slug: Option<String>,
    request: BlogWidgetValidationRequest,
) -> Result<BlogWidgetValidation, ApiError> {
    let native = request.clone();
    execute_selected_transport(
        "blog/admin/widget-validate",
        selected_transport_path(),
        move || super::native_server_adapter::widget::validate(native),
        move || super::graphql_adapter::widget::validate(token, tenant_slug, request),
    )
    .await
}

pub async fn preview_blog_page_builder_widget(
    token: Option<String>,
    tenant_slug: Option<String>,
    request: BlogWidgetPreviewRequest,
) -> Result<serde_json::Value, ApiError> {
    let native = request.clone();
    execute_selected_transport(
        "blog/admin/widget-preview",
        selected_transport_path(),
        move || super::native_server_adapter::widget::preview(native),
        move || super::graphql_adapter::widget::preview(token, tenant_slug, request),
    )
    .await
}

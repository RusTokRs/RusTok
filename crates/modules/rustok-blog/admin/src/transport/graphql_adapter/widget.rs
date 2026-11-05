use serde::Deserialize;
use serde_json::{Value, json};

use super::{ApiError, request};
use crate::model::{
    BlogWidgetPreviewRequest, BlogWidgetPropertyRequest, BlogWidgetPropertySchema,
    BlogWidgetValidation, BlogWidgetValidationRequest, widget::WidgetCatalog,
};

#[derive(Deserialize)]
struct CatalogResponse {
    #[serde(rename = "blogWidgetCatalog")]
    catalog: WidgetCatalog,
}

#[derive(Deserialize)]
struct ValidationResponse {
    #[serde(rename = "validateBlogWidget")]
    validation: BlogWidgetValidation,
}

#[derive(Deserialize)]
struct PreviewResponse {
    #[serde(rename = "previewBlogWidget")]
    preview: Value,
}

pub(in crate::transport) async fn schema(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: BlogWidgetPropertyRequest,
) -> Result<BlogWidgetPropertySchema, ApiError> {
    let reference = crate::core::page_builder::resolve_widget_schema_reference(
        &input.widget_type,
        &input.property_schema,
    )
    .map_err(|error| rustok_graphql::GraphqlHttpError::Graphql(error.to_string()))?;
    let response: CatalogResponse = request(
        "query BlogWidgetCatalog { blogWidgetCatalog }",
        json!({}),
        token,
        tenant_slug,
    )
    .await?;
    let item = response
        .catalog
        .items
        .into_iter()
        .find(|item| item.widget_type == input.widget_type && item.schema_id == reference.schema_id)
        .ok_or_else(|| {
            rustok_graphql::GraphqlHttpError::Graphql("Blog widget schema is unavailable".into())
        })?;
    Ok(BlogWidgetPropertySchema {
        schema_id: item.schema_id,
        schema: item.props_schema,
    })
}

pub(in crate::transport) async fn validate(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: BlogWidgetValidationRequest,
) -> Result<BlogWidgetValidation, ApiError> {
    crate::core::page_builder::resolve_widget_schema_reference(
        &input.widget_type,
        &input.property_schema,
    )
    .map_err(|error| rustok_graphql::GraphqlHttpError::Graphql(error.to_string()))?;
    let response: ValidationResponse = request("query ValidateBlogWidget($widgetType: String!, $props: JSON!) { validateBlogWidget(widgetType: $widgetType, props: $props) }", json!({"widgetType": input.widget_type, "props": input.props}), token, tenant_slug).await?;
    Ok(response.validation)
}

pub(in crate::transport) async fn preview(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: BlogWidgetPreviewRequest,
) -> Result<Value, ApiError> {
    let response: PreviewResponse = request("query PreviewBlogWidget($widgetType: String!, $props: JSON!) { previewBlogWidget(widgetType: $widgetType, props: $props) }", json!({"widgetType": input.widget_type, "props": input.props}), token, tenant_slug).await?;
    Ok(response.preview)
}

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumWidgetPropertySchemaTransportRequest {
    pub widget_type: String,
    pub property_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumWidgetPropertySchemaTransportResponse {
    pub schema_id: String,
    pub schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumWidgetPropertyValidationTransportRequest {
    pub widget_type: String,
    pub property_schema: Value,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumWidgetPropertyValidationIssueTransport {
    pub class: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForumWidgetPropertyValidationTransportResponse {
    pub valid: bool,
    pub normalized_props: Value,
    pub issues: Vec<ForumWidgetPropertyValidationIssueTransport>,
}

#[cfg(feature = "ssr")]
async fn authorize_forum_property_transport() -> Result<(), ServerFnError> {
    use crate::widget_preview_transport::{
        require_forum_module_enabled, require_forum_transport_authorization,
    };
    use leptos::prelude::expect_context;

    let auth = leptos_axum::extract::<rustok_api::AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .map_err(ServerFnError::new)?;
    require_forum_transport_authorization(&auth, &tenant)?;

    let host = expect_context::<rustok_api::HostRuntimeContext>();
    require_forum_module_enabled(&host, tenant.id).await
}

#[cfg(feature = "ssr")]
fn resolve_owner_schema(
    widget_type: &str,
    property_schema: &Value,
) -> Result<crate::ForumWidgetOwnerSchemaRef, ServerFnError> {
    let widget_type = widget_type.trim();
    let contribution = crate::forum_widget_contribution();
    let editor = contribution
        .property_editors
        .iter()
        .find(|editor| editor.component_type == widget_type)
        .ok_or_else(|| {
            ServerFnError::new(format!(
                "Forum widget `{widget_type}` has no registered property editor"
            ))
        })?;
    if editor.property_schema != *property_schema {
        return Err(ServerFnError::new(format!(
            "Forum widget `{widget_type}` property schema reference does not match the registered contribution"
        )));
    }
    serde_json::from_value::<crate::ForumWidgetOwnerSchemaRef>(property_schema.clone()).map_err(
        |error| ServerFnError::new(format!("Forum widget owner schema reference is invalid: {error}")),
    )
}

#[server(prefix = "/api/fn", endpoint = "forum/page-builder-widget-property-schema")]
pub async fn load_forum_page_builder_widget_property_schema(
    request: ForumWidgetPropertySchemaTransportRequest,
) -> Result<ForumWidgetPropertySchemaTransportResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        authorize_forum_property_transport().await?;
        let owner_schema = resolve_owner_schema(&request.widget_type, &request.property_schema)?;
        let item = rustok_forum::ForumWidgetContractService::catalog()
            .items
            .into_iter()
            .find(|item| item.widget_type == request.widget_type.trim())
            .ok_or_else(|| {
                ServerFnError::new(format!(
                    "Forum widget `{}` is missing from the owner catalog",
                    request.widget_type.trim()
                ))
            })?;
        Ok(ForumWidgetPropertySchemaTransportResponse {
            schema_id: owner_schema.schema_id,
            schema: item.props_schema,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::new(
            "forum/page-builder-widget-property-schema requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "forum/page-builder-widget-property-validate")]
pub async fn validate_forum_page_builder_widget_properties(
    request: ForumWidgetPropertyValidationTransportRequest,
) -> Result<ForumWidgetPropertyValidationTransportResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        authorize_forum_property_transport().await?;
        let _owner_schema = resolve_owner_schema(&request.widget_type, &request.property_schema)?;
        let response = rustok_forum::ForumWidgetContractService::validate_props(
            rustok_forum::ValidateForumWidgetPropsInput {
                widget_type: request.widget_type,
                props: request.props,
            },
        );
        Ok(ForumWidgetPropertyValidationTransportResponse {
            valid: response.valid,
            normalized_props: response.normalized_props,
            issues: response
                .issues
                .into_iter()
                .map(|issue| ForumWidgetPropertyValidationIssueTransport {
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
            "forum/page-builder-widget-property-validate requires the `ssr` feature",
        ))
    }
}

use std::sync::Arc;

use rustok_page_builder_admin::{
    PageBuilderContributionHostExtension, PageBuilderContributionPreviewError,
    PageBuilderContributionPreviewFuture, PageBuilderContributionPreviewPort,
    PageBuilderContributionPreviewRequest, PageBuilderContributionPropertyError,
    PageBuilderContributionPropertyIssue, PageBuilderContributionPropertyPort,
    PageBuilderContributionPropertySchema, PageBuilderContributionPropertySchemaFuture,
    PageBuilderContributionPropertySchemaRequest, PageBuilderContributionPropertyValidation,
    PageBuilderContributionPropertyValidationFuture,
    PageBuilderContributionPropertyValidationRequest,
};

/// Host binding only. Blog's transport independently enforces tenant/module/resource authority.
#[derive(Clone)]
struct BlogWidgetPort {
    auth: leptos_auth::AuthContext,
}

pub(super) fn extension(auth: leptos_auth::AuthContext) -> PageBuilderContributionHostExtension {
    let port = Arc::new(BlogWidgetPort { auth });
    PageBuilderContributionHostExtension::new(
        rustok_blog_admin::blog_contribution_manifest(),
        |registries| {
            rustok_blog_admin::register_blog_fly_widgets(registries)
                .map_err(|error| error.to_string())
        },
    )
    .with_preview_port(port.clone())
    .with_property_port(port)
}

fn ensure_provider(provider: &str) -> Result<(), &'static str> {
    if provider == "rustok.blog" {
        Ok(())
    } else {
        Err("Blog widget provider mismatch")
    }
}

impl PageBuilderContributionPreviewPort for BlogWidgetPort {
    fn preview(
        &self,
        request: PageBuilderContributionPreviewRequest,
    ) -> PageBuilderContributionPreviewFuture {
        let token = self.auth.get_token();
        let tenant = self.auth.get_tenant();
        Box::pin(async move {
            ensure_provider(&request.provider).map_err(|message| {
                PageBuilderContributionPreviewError::with_stable_code(
                    message,
                    "BLOG_WIDGET_PROVIDER_MISMATCH",
                )
            })?;
            rustok_blog_admin::preview_blog_page_builder_widget(
                token,
                tenant,
                rustok_blog_admin::BlogWidgetPreviewRequest {
                    widget_type: request.component_type,
                    props: request.props,
                },
            )
            .await
            .map_err(|error| {
                PageBuilderContributionPreviewError::with_stable_code(
                    error.to_string(),
                    "BLOG_WIDGET_PREVIEW_FAILED",
                )
            })
        })
    }
}

impl PageBuilderContributionPropertyPort for BlogWidgetPort {
    fn schema(
        &self,
        request: PageBuilderContributionPropertySchemaRequest,
    ) -> PageBuilderContributionPropertySchemaFuture {
        let token = self.auth.get_token();
        let tenant = self.auth.get_tenant();
        Box::pin(async move {
            ensure_provider(&request.provider).map_err(|message| {
                PageBuilderContributionPropertyError::with_stable_code(
                    message,
                    "BLOG_WIDGET_PROVIDER_MISMATCH",
                )
            })?;
            let schema = rustok_blog_admin::load_blog_page_builder_widget_property_schema(
                token,
                tenant,
                rustok_blog_admin::BlogWidgetPropertyRequest {
                    widget_type: request.component_type,
                    property_schema: request.property_schema,
                },
            )
            .await
            .map_err(|error| {
                PageBuilderContributionPropertyError::with_stable_code(
                    error.to_string(),
                    "BLOG_WIDGET_SCHEMA_FAILED",
                )
            })?;
            Ok(PageBuilderContributionPropertySchema {
                schema_id: schema.schema_id,
                schema: schema.schema,
            })
        })
    }

    fn validate(
        &self,
        request: PageBuilderContributionPropertyValidationRequest,
    ) -> PageBuilderContributionPropertyValidationFuture {
        let token = self.auth.get_token();
        let tenant = self.auth.get_tenant();
        Box::pin(async move {
            ensure_provider(&request.provider).map_err(|message| {
                PageBuilderContributionPropertyError::with_stable_code(
                    message,
                    "BLOG_WIDGET_PROVIDER_MISMATCH",
                )
            })?;
            let result = rustok_blog_admin::validate_blog_page_builder_widget_properties(
                token,
                tenant,
                rustok_blog_admin::BlogWidgetValidationRequest {
                    widget_type: request.component_type,
                    property_schema: request.property_schema,
                    props: request.props,
                },
            )
            .await
            .map_err(|error| {
                PageBuilderContributionPropertyError::with_stable_code(
                    error.to_string(),
                    "BLOG_WIDGET_VALIDATION_FAILED",
                )
            })?;
            Ok(PageBuilderContributionPropertyValidation {
                valid: result.valid,
                normalized_props: result.normalized_props,
                issues: result
                    .issues
                    .into_iter()
                    .map(|issue| PageBuilderContributionPropertyIssue {
                        class: issue.class,
                        code: issue.code,
                        message: issue.message,
                        path: issue.path,
                    })
                    .collect(),
            })
        })
    }
}

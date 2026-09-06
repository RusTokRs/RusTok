use crate::locale::PageBuilderLocaleContext;
use fly::{
    FlyResult, GrapesJsCodec, PageSelection, RenderPolicy, RenderedPage, RuntimeRenderResult,
    render_page, render_page_with_runtime_context,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRenderRequest {
    pub project_data: Value,
    pub selection: PageSelection,
    #[serde(default)]
    pub policy: RenderPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRenderResponse {
    pub page: RenderedPage,
}

impl PageBuilderRenderResponse {
    pub fn document_html(&self) -> String {
        self.page.document_html()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeRenderRequest {
    pub project_data: Value,
    pub selection: PageSelection,
    #[serde(default)]
    pub policy: RenderPolicy,
    #[serde(default)]
    pub context: Value,
}

impl PageBuilderRuntimeRenderRequest {
    pub fn with_locale(mut self, locale: &PageBuilderLocaleContext) -> Self {
        self.context = locale.apply_to_context(&self.context);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderRuntimeRenderResponse {
    pub result: RuntimeRenderResult,
}

impl PageBuilderRuntimeRenderResponse {
    pub fn page(&self) -> &RenderedPage {
        &self.result.page
    }

    pub fn document_html(&self) -> String {
        self.result.document_html()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRenderer;

impl PageBuilderRenderer {
    pub fn render(
        &self,
        request: PageBuilderRenderRequest,
    ) -> FlyResult<PageBuilderRenderResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        let page = render_page(&document, &request.selection, &request.policy)?;
        Ok(PageBuilderRenderResponse { page })
    }

    pub fn render_runtime(
        &self,
        request: PageBuilderRuntimeRenderRequest,
    ) -> FlyResult<PageBuilderRuntimeRenderResponse> {
        let document = GrapesJsCodec::decode_value(request.project_data)?;
        let result = render_page_with_runtime_context(
            &document,
            &request.selection,
            &request.policy,
            &request.context,
        )?;
        Ok(PageBuilderRuntimeRenderResponse { result })
    }

    pub fn render_document_html(
        &self,
        project_data: Value,
        selection: PageSelection,
        policy: RenderPolicy,
    ) -> FlyResult<String> {
        self.render(PageBuilderRenderRequest {
            project_data,
            selection,
            policy,
        })
        .map(|response| response.document_html())
    }

    pub fn render_runtime_document_html(
        &self,
        project_data: Value,
        selection: PageSelection,
        policy: RenderPolicy,
        context: Value,
    ) -> FlyResult<String> {
        self.render_runtime(PageBuilderRuntimeRenderRequest {
            project_data,
            selection,
            policy,
            context,
        })
        .map(|response| response.document_html())
    }

    pub fn render_localized_runtime_document_html(
        &self,
        project_data: Value,
        selection: PageSelection,
        policy: RenderPolicy,
        context: Value,
        locale: &PageBuilderLocaleContext,
    ) -> FlyResult<String> {
        self.render_runtime(
            PageBuilderRuntimeRenderRequest {
                project_data,
                selection,
                policy,
                context,
            }
            .with_locale(locale),
        )
        .map(|response| response.document_html())
    }
}

pub fn render_page_builder_project(
    request: PageBuilderRenderRequest,
) -> FlyResult<PageBuilderRenderResponse> {
    PageBuilderRenderer.render(request)
}

pub fn render_page_builder_runtime(
    request: PageBuilderRuntimeRenderRequest,
) -> FlyResult<PageBuilderRuntimeRenderResponse> {
    PageBuilderRenderer.render_runtime(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn render_api_decodes_selects_and_renders_project_data() {
        let response = render_page_builder_project(PageBuilderRenderRequest {
            project_data: json!({
                "pages": [{
                    "id": "home",
                    "flyPageMeta": { "title": "Home", "slug": "home" },
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{
                            "id": "heading",
                            "type": "heading",
                            "tagName": "h1",
                            "content": "Hello"
                        }]
                    }
                }]
            }),
            selection: PageSelection::Slug("home".to_string()),
            policy: RenderPolicy::default(),
        })
        .expect("render response");
        assert_eq!(response.page.page_id.as_deref(), Some("home"));
        assert!(response.page.html.contains("<h1"));
        assert!(response.document_html().contains("<title>Home</title>"));
    }

    #[test]
    fn runtime_api_materializes_conditions_and_repeaters() {
        let response = render_page_builder_runtime(PageBuilderRuntimeRenderRequest {
            project_data: json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{
                            "id": "banner",
                            "type": "text",
                            "content": "Banner"
                        }, {
                            "id": "row",
                            "type": "text",
                            "content": "{{item.name}}"
                        }]
                    }
                }],
                "flyRuntimeConditions": [{
                    "id": "show-banner",
                    "component_id": "banner",
                    "path": "showBanner",
                    "operator": "truthy"
                }],
                "flyRuntimeRepeaters": [{
                    "id": "rows",
                    "component_id": "row",
                    "path": "items"
                }]
            }),
            selection: PageSelection::First,
            policy: RenderPolicy::default(),
            context: json!({
                "showBanner": false,
                "items": [{ "name": "One" }, { "name": "Two" }]
            }),
        })
        .expect("runtime response");
        assert_eq!(response.result.hidden_components, 1);
        assert_eq!(response.result.repeated_nodes, 2);
        assert!(!response.result.page.html.contains("Banner"));
        assert!(response.result.page.html.contains("One"));
        assert!(response.result.page.html.contains("Two"));
    }

    #[test]
    fn localized_runtime_render_uses_project_translation_catalog() {
        let html = PageBuilderRenderer
            .render_localized_runtime_document_html(
                json!({
                    "pages": [{
                        "id": "home",
                        "component": {
                            "id": "root",
                            "type": "wrapper",
                            "components": [{
                                "id": "heading",
                                "type": "heading",
                                "tagName": "h1",
                                "content": "Static"
                            }]
                        }
                    }],
                    "flyTranslations": [{
                        "id": "hero_title",
                        "values": {
                            "en": "Welcome",
                            "ru": "Добро пожаловать"
                        },
                        "fallback_locale": "en"
                    }],
                    "flyRuntimeBindings": [{
                        "id": "heading-content",
                        "component_id": "heading",
                        "path": "translations.hero_title",
                        "target": "field",
                        "name": "content"
                    }]
                }),
                PageSelection::First,
                RenderPolicy::default(),
                json!({ "customer": { "name": "Ada" } }),
                &PageBuilderLocaleContext::new(Some("ru-RU"), ["en"]),
            )
            .expect("localized render");
        assert!(html.contains("Добро пожаловать"));
        assert!(!html.contains("Welcome"));
    }

    #[test]
    fn render_api_rejects_unknown_page_selection() {
        let error = PageBuilderRenderer
            .render(PageBuilderRenderRequest {
                project_data: json!({ "pages": [] }),
                selection: PageSelection::Id("missing".to_string()),
                policy: RenderPolicy::default(),
            })
            .expect_err("missing page");
        assert!(matches!(error, fly::FlyError::PageNotFound(_)));
    }
}

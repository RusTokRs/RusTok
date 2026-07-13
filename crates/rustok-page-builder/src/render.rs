use fly::{
    render_page, FlyResult, GrapesJsV1Codec, PageSelection, RenderPolicy, RenderedPage,
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

#[derive(Debug, Clone, Copy, Default)]
pub struct PageBuilderRenderer;

impl PageBuilderRenderer {
    pub fn render(
        &self,
        request: PageBuilderRenderRequest,
    ) -> FlyResult<PageBuilderRenderResponse> {
        let document = GrapesJsV1Codec::decode_value(request.project_data)?;
        let page = render_page(&document, &request.selection, &request.policy)?;
        Ok(PageBuilderRenderResponse { page })
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
}

pub fn render_page_builder_project(
    request: PageBuilderRenderRequest,
) -> FlyResult<PageBuilderRenderResponse> {
    PageBuilderRenderer.render(request)
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

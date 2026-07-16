use fly::{
    render_page_with_runtime_context, resolve_localized_page_route, GrapesJsV1Codec,
    LocalizedPageRouteResolution, RenderPolicy, RuntimeRenderResult,
};
use rustok_page_builder::locale::PageBuilderLocaleContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorefrontLocalizedRouteOutput {
    pub route: LocalizedPageRouteResolution,
    pub result: RuntimeRenderResult,
}

impl StorefrontLocalizedRouteOutput {
    pub fn document_html(&self) -> String {
        self.result.document_html()
    }

    pub fn canonical_redirect_needed(&self) -> bool {
        self.route.canonical_redirect_needed()
    }
}

pub fn render_storefront_localized_slug(
    project_data: Value,
    requested_slug: &str,
    mut policy: RenderPolicy,
    context: Value,
) -> fly::FlyResult<StorefrontLocalizedRouteOutput> {
    policy.instrument_components = false;
    let document = GrapesJsV1Codec::decode_value(project_data)?;
    let route = resolve_localized_page_route(&document, requested_slug, &context)?;
    let result = render_page_with_runtime_context(
        &document,
        &route.selection(),
        &policy,
        &route.context,
    )?;
    Ok(StorefrontLocalizedRouteOutput { route, result })
}

pub fn render_storefront_localized_request(
    project_data: Value,
    requested_slug: &str,
    policy: RenderPolicy,
    business_context: Value,
    locale: &PageBuilderLocaleContext,
) -> fly::FlyResult<StorefrontLocalizedRouteOutput> {
    render_storefront_localized_slug(
        project_data,
        requested_slug,
        policy,
        locale.apply_to_context(&business_context),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project() -> Value {
        json!({
            "flyLocales": {
                "default_locale": "en",
                "supported_locales": ["en", "ru"],
                "fallback_locales": ["en"]
            },
            "flyTranslations": [{
                "id": "hero",
                "values": { "en": "Welcome", "ru": "Добро пожаловать" }
            }],
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "slug": { "$localized": { "en": "home", "ru": "glavnaya" } },
                    "title": { "$localized": { "en": "Home", "ru": "Главная" } }
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "heading",
                        "type": "heading",
                        "content": "Static"
                    }]
                }
            }],
            "flyRuntimeBindings": [{
                "id": "heading-content",
                "component_id": "heading",
                "path": "translations.hero",
                "target": "field",
                "name": "content"
            }]
        })
    }

    #[test]
    fn localized_slug_renders_body_and_head_with_matched_locale() {
        let output = render_storefront_localized_slug(
            project(),
            "glavnaya",
            RenderPolicy::default(),
            json!({ "$locale": "ru-RU" }),
        )
        .expect("localized storefront route");
        assert_eq!(output.route.matched_locale.as_deref(), Some("ru"));
        assert_eq!(output.route.canonical_slug, "glavnaya");
        assert_eq!(output.result.page.metadata.title.as_deref(), Some("Главная"));
        assert!(output.result.page.html.contains("Добро пожаловать"));
        assert!(output.document_html().contains("<title>Главная</title>"));
    }

    #[test]
    fn request_locale_context_preserves_business_data() {
        let locale = PageBuilderLocaleContext::new(Some("ru-RU"), ["en"]);
        let output = render_storefront_localized_request(
            project(),
            "glavnaya",
            RenderPolicy::default(),
            json!({ "customer": { "name": "Ada" } }),
            &locale,
        )
        .expect("localized storefront request");
        assert_eq!(output.route.context["customer"]["name"], "Ada");
        assert_eq!(output.route.context["$locale"], "ru");
    }
}

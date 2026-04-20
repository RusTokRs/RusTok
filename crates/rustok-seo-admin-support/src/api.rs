use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use rustok_seo::SeoTargetKind;
use serde::{Deserialize, Serialize};

use crate::model::{SeoMetaMutationInput, SeoMetaView, SeoRevisionView};

pub type ApiError = GraphqlHttpError;

const SEO_META_QUERY: &str = "query SeoEntityPanelMeta($targetKind: SeoTargetKind!, $targetId: UUID!, $locale: String) { seoMeta(targetKind: $targetKind, targetId: $targetId, locale: $locale) { targetKind targetId requestedLocale effectiveLocale availableLocales noindex nofollow canonicalUrl source translation { locale title description keywords ogTitle ogDescription ogImage } structuredData } }";
const UPSERT_SEO_META_MUTATION: &str = "mutation SeoEntityPanelUpsert($input: SeoMetaInput!) { upsertSeoMeta(input: $input) { targetKind targetId requestedLocale effectiveLocale availableLocales noindex nofollow canonicalUrl source translation { locale title description keywords ogTitle ogDescription ogImage } structuredData } }";
const PUBLISH_REVISION_MUTATION: &str = "mutation SeoEntityPanelPublish($targetKind: SeoTargetKind!, $targetId: UUID!, $note: String) { publishSeoRevision(targetKind: $targetKind, targetId: $targetId, note: $note) { revision } }";

#[derive(Debug, Deserialize)]
struct SeoMetaResponse {
    #[serde(rename = "seoMeta")]
    seo_meta: Option<SeoMetaView>,
}

#[derive(Debug, Deserialize)]
struct UpsertSeoMetaResponse {
    #[serde(rename = "upsertSeoMeta")]
    upsert_seo_meta: SeoMetaView,
}

#[derive(Debug, Deserialize)]
struct PublishSeoRevisionResponse {
    #[serde(rename = "publishSeoRevision")]
    publish_seo_revision: SeoRevisionView,
}

#[derive(Debug, Serialize)]
struct SeoMetaVariables {
    #[serde(rename = "targetKind")]
    target_kind: SeoTargetKind,
    #[serde(rename = "targetId")]
    target_id: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpsertSeoMetaVariables {
    input: SeoMetaMutationInput,
}

#[derive(Debug, Serialize)]
struct PublishSeoRevisionVariables {
    #[serde(rename = "targetKind")]
    target_kind: SeoTargetKind,
    #[serde(rename = "targetId")]
    target_id: String,
    note: Option<String>,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
}

fn seo_meta_request(
    target_kind: SeoTargetKind,
    target_id: String,
    locale: Option<String>,
) -> GraphqlRequest<SeoMetaVariables> {
    GraphqlRequest::new(
        SEO_META_QUERY,
        Some(SeoMetaVariables {
            target_kind,
            target_id,
            locale,
        }),
    )
}

fn upsert_seo_meta_request(input: SeoMetaMutationInput) -> GraphqlRequest<UpsertSeoMetaVariables> {
    GraphqlRequest::new(
        UPSERT_SEO_META_MUTATION,
        Some(UpsertSeoMetaVariables { input }),
    )
}

fn publish_seo_revision_request(
    target_kind: SeoTargetKind,
    target_id: String,
    note: Option<String>,
) -> GraphqlRequest<PublishSeoRevisionVariables> {
    GraphqlRequest::new(
        PUBLISH_REVISION_MUTATION,
        Some(PublishSeoRevisionVariables {
            target_kind,
            target_id,
            note,
        }),
    )
}

pub async fn fetch_seo_meta(
    token: Option<String>,
    tenant_slug: Option<String>,
    target_kind: SeoTargetKind,
    target_id: String,
    locale: Option<String>,
) -> Result<Option<SeoMetaView>, ApiError> {
    let graphql_request = seo_meta_request(target_kind, target_id, locale);
    let response: SeoMetaResponse = request(
        graphql_request.query.as_str(),
        graphql_request
            .variables
            .expect("seo meta request variables"),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.seo_meta)
}

pub async fn save_seo_meta(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: SeoMetaMutationInput,
) -> Result<SeoMetaView, ApiError> {
    let graphql_request = upsert_seo_meta_request(input);
    let response: UpsertSeoMetaResponse = request(
        graphql_request.query.as_str(),
        graphql_request.variables.expect("upsert request variables"),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.upsert_seo_meta)
}

pub async fn publish_seo_revision(
    token: Option<String>,
    tenant_slug: Option<String>,
    target_kind: SeoTargetKind,
    target_id: String,
    note: Option<String>,
) -> Result<SeoRevisionView, ApiError> {
    let graphql_request = publish_seo_revision_request(target_kind, target_id, note);
    let response: PublishSeoRevisionResponse = request(
        graphql_request.query.as_str(),
        graphql_request
            .variables
            .expect("publish request variables"),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.publish_seo_revision)
}

#[cfg(test)]
mod tests {
    use super::{
        publish_seo_revision_request, seo_meta_request, upsert_seo_meta_request,
        PUBLISH_REVISION_MUTATION, SEO_META_QUERY, UPSERT_SEO_META_MUTATION,
    };
    use crate::model::{SeoMetaMutationInput, SeoMetaTranslationMutationInput};
    use rustok_seo::SeoTargetKind;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn seo_meta_request_preserves_locale_variable() {
        let request = seo_meta_request(
            SeoTargetKind::Page,
            "target-1".to_string(),
            Some("pt-BR".to_string()),
        );

        assert_eq!(request.query, SEO_META_QUERY);
        assert_eq!(
            serde_json::to_value(&request.variables).expect("request variables should serialize"),
            json!({
                "targetKind": "page",
                "targetId": "target-1",
                "locale": "pt-BR"
            })
        );
    }

    #[test]
    fn upsert_request_embeds_translations_payload() {
        let request = upsert_seo_meta_request(SeoMetaMutationInput {
            target_kind: SeoTargetKind::BlogPost,
            target_id: Uuid::new_v4().to_string(),
            noindex: false,
            nofollow: true,
            canonical_url: Some("https://example.com/post".to_string()),
            structured_data: Some(json!({"@type": "Article"})),
            translations: vec![SeoMetaTranslationMutationInput {
                locale: "ru".to_string(),
                title: Some("Заголовок".to_string()),
                description: None,
                keywords: None,
                og_title: None,
                og_description: None,
                og_image: None,
            }],
        });

        let serialized =
            serde_json::to_value(&request.variables).expect("request variables should serialize");

        assert_eq!(request.query, UPSERT_SEO_META_MUTATION);
        assert_eq!(
            serialized["input"]["translations"][0]["locale"],
            json!("ru")
        );
        assert_eq!(serialized["input"]["nofollow"], json!(true));
    }

    #[test]
    fn publish_request_uses_expected_operation_and_note() {
        let request = publish_seo_revision_request(
            SeoTargetKind::Product,
            "target-2".to_string(),
            Some("ship it".to_string()),
        );

        assert_eq!(request.query, PUBLISH_REVISION_MUTATION);
        assert_eq!(
            serde_json::to_value(&request.variables).expect("request variables should serialize"),
            json!({
                "targetKind": "product",
                "targetId": "target-2",
                "note": "ship it"
            })
        );
    }
}

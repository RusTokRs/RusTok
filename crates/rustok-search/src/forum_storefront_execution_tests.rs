use super::{
    ForumStorefrontSearchRequest, ForumStorefrontSearchExecutionError,
    StorefrontSearchTransport,
    forum_storefront_execution::normalize_request,
};
use uuid::Uuid;

fn request(source_modules: Vec<&str>, category_ids: Vec<String>) -> ForumStorefrontSearchRequest {
    ForumStorefrontSearchRequest {
        tenant_id: Uuid::new_v4(),
        query: "forum".to_string(),
        locale: Some("en".to_string()),
        fallback_locale: "en".to_string(),
        channel_id: None,
        limit: Some(12),
        offset: Some(0),
        ranking_profile: None,
        preset_key: None,
        entity_types: Vec::new(),
        source_modules: source_modules.into_iter().map(str::to_string).collect(),
        statuses: Vec::new(),
        category_ids,
        attribute_filters: Vec::new(),
        sort_attribute_code: None,
        sort_desc: false,
        auth: None,
        request_context: None,
        transport: StorefrontSearchTransport::Graphql,
    }
}

#[test]
fn mixed_source_scope_is_rejected_before_search_execution() {
    let error = normalize_request(request(
        vec!["forum", "product"],
        vec![Uuid::new_v4().to_string()],
    ))
    .expect_err("mixed source scope must not enter Forum storefront Search");

    assert!(matches!(
        error,
        ForumStorefrontSearchExecutionError::Validation(message)
            if message.contains("source_modules: [forum]")
    ));
}

#[test]
fn empty_category_scope_is_rejected_before_search_execution() {
    let error = normalize_request(request(vec!["forum"], Vec::new()))
        .expect_err("Forum storefront Search requires a selected category root");

    assert!(matches!(
        error,
        ForumStorefrontSearchExecutionError::Validation(message)
            if message.contains("at least one category_id")
    ));
}

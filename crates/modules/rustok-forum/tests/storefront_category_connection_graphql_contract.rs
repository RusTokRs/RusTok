#![recursion_limit = "512"]

use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use rustok_forum::ForumQuery;

#[test]
fn storefront_categories_expose_total_count_in_page_info() {
    let schema = Schema::build(ForumQuery::default(), EmptyMutation, EmptySubscription).finish();
    let sdl = schema.sdl();
    assert!(sdl.contains("forumStorefrontCategories("));
    let fields = sdl
        .split("type ForumCategoryConnection {")
        .nth(1)
        .expect("category connection type")
        .split('}')
        .next()
        .expect("connection fields");
    assert!(fields.contains("pageInfo: PageInfo!"));
    assert!(fields.contains("items: [GqlForumCategory!]!"));
    assert!(
        !fields.contains("total:"),
        "there is no connection-level total field"
    );
    assert!(sdl.contains("totalCount:"));
}

/// Validate the actual embedded headless operations, not an independently copied
/// test query. Missing runtime data must be reached only after schema validation.
#[tokio::test]
async fn native_storefront_documents_validate_against_the_live_forum_schema() {
    use async_graphql::{Request, Variables};
    use rustok_forum::ForumMutation;
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .finish();
    let source = include_str!("../storefront/src/transport/graphql_adapter.rs");
    let mut checked = 0;
    for declaration in source.split("const ").skip(1) {
        let Some((name, rest)) = declaration.split_once(": &str = ") else {
            continue;
        };
        let Some(quoted) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        let Some((document, _)) = quoted.split_once("\";") else {
            continue;
        };
        if !document.starts_with("query ") && !document.starts_with("mutation ") {
            continue;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let variables = Variables::from_json(serde_json::json!({
            "id": id, "topicId": id, "categoryId": id, "userIds": [id],
            "locale": "en", "slug": "topic", "shortId": "0123456789abcdef",
            "solvedOnly": false, "unreadOnly": false,
        }));
        let response = schema
            .execute(Request::new(document).variables(variables))
            .await;
        assert!(
            !response.errors.is_empty(),
            "{name}: fixture intentionally has no runtime context"
        );
        for error in response.errors {
            assert!(
                !error.path.is_empty(),
                "{name} failed schema validation: {error:?}"
            );
        }
        checked += 1;
    }
    assert_eq!(
        checked, 13,
        "every embedded read/read-state operation must be validated"
    );

    let invalid = schema
        .execute("{ forumStorefrontCategories { total } }")
        .await;
    assert!(
        invalid.errors.iter().any(|error| error.path.is_empty()),
        "the harness must reject the original invalid field"
    );
}

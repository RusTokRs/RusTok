use std::time::Duration;

use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "rustok-graphql");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_header_constants_and_endpoint_contract() {
    assert_eq!(GRAPHQL_ENDPOINT, "/api/graphql");
    assert_eq!(TENANT_HEADER, "X-Tenant-Slug");
    assert_eq!(AUTH_HEADER, "Authorization");
    assert_eq!(ACCEPT_LANGUAGE_HEADER, "Accept-Language");
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));

    let ep1 = graphql_endpoint_from_base("https://api.example.com");
    assert_eq!(ep1, "https://api.example.com/api/graphql");

    let ep2 = graphql_endpoint_from_base("https://api.example.com/");
    assert_eq!(ep2, "https://api.example.com/api/graphql");

    let ep3 = graphql_endpoint_from_base("/prefix");
    assert_eq!(ep3, "/prefix/api/graphql");
}

#[test]
fn test_graphql_request_extensions_and_operations() {
    let mut req = GraphqlRequest::new(
        "query ListPosts { posts { id title } }",
        Some(serde_json::json!({ "limit": 10 })),
    );
    assert!(req.is_query());
    assert!(!req.is_mutation());

    let apq = persisted_query_extension("dummy_hash_123");
    req = req.with_extensions(apq);

    let serialized = serde_json::to_string(&req).expect("must serialize");
    assert!(serialized.contains("dummy_hash_123"));
    assert!(serialized.contains("persistedQuery"));

    let mutation = GraphqlRequest::<()>::new("  mutation DeleteItem { delete(id: 1) }", None);
    assert!(mutation.is_mutation());
    assert!(!mutation.is_query());
}

#[test]
fn test_graphql_http_error_handling() {
    use std::str::FromStr;

    let net_err = GraphqlHttpError::from_str("Network error").expect("parse network");
    assert_eq!(net_err, GraphqlHttpError::Network);
    assert_eq!(net_err.to_string(), "Network error");

    let auth_err = GraphqlHttpError::from_str("Unauthorized").expect("parse auth");
    assert_eq!(auth_err, GraphqlHttpError::Unauthorized);
    assert_eq!(auth_err.to_string(), "Unauthorized");

    let gql_err = GraphqlHttpError::from_str("GraphQL error: syntax error").expect("parse gql");
    assert_eq!(
        gql_err,
        GraphqlHttpError::Graphql("syntax error".to_string())
    );
    assert_eq!(gql_err.to_string(), "GraphQL error: syntax error");

    let http_err = GraphqlHttpError::from_str("Http error: 404 Not Found").expect("parse http");
    assert_eq!(
        http_err,
        GraphqlHttpError::Http("404 Not Found".to_string())
    );
    assert_eq!(http_err.to_string(), "Http error: 404 Not Found");

    let unknown = GraphqlHttpError::from_str("Something else");
    assert!(unknown.is_err());
}

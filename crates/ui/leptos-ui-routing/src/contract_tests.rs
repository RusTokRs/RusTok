use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rustok_ui_core::{UiRouteContext, UiRouteQueryIntent, UiRouteQueryUpdate, UiRouteQueryWrite};

use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "leptos-ui-routing");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_route_context_query_reader_contract() {
    let mut query = BTreeMap::new();
    query.insert("page".to_string(), "2".to_string());
    query.insert("sort".to_string(), "created_at".to_string());

    let ctx = UiRouteContext {
        route_segment: Some("articles".to_string()),
        subpath: Some("list".to_string()),
        locale: Some("en-US".to_string()),
        query,
    };

    assert_eq!(read_route_query_value(&ctx, "page"), Some("2".to_string()));
    assert_eq!(
        read_route_query_value(&ctx, "sort"),
        Some("created_at".to_string())
    );
    assert_eq!(read_route_query_value(&ctx, "unknown"), None);
}

#[test]
fn test_route_query_policy_sanitization_contract() {
    let policy = RouteQueryPolicy::new(|segment, subpath, query| {
        let mut sanitized = BTreeMap::new();
        if segment == Some("admin") && subpath == Some("dashboard") {
            for (k, v) in query {
                if k != "secret_debug" {
                    sanitized.insert(k.clone(), v.to_uppercase());
                }
            }
        }
        sanitized
    });

    let mut input = BTreeMap::new();
    input.insert("tab".to_string(), "overview".to_string());
    input.insert("secret_debug".to_string(), "1".to_string());

    let output = policy.sanitize(Some("admin"), Some("dashboard"), &input);
    assert_eq!(output.get("tab"), Some(&"OVERVIEW".to_string()));
    assert_eq!(output.get("secret_debug"), None);

    let output_other = policy.sanitize(Some("storefront"), None, &input);
    assert!(output_other.is_empty());

    let passthrough = sanitize_route_query(None, None, None, &input);
    assert_eq!(passthrough.len(), 2);
}

#[test]
fn test_route_query_writer_dispatch_contract() {
    let recorded_updates = Arc::new(Mutex::new(Vec::<(Vec<(String, Option<String>)>, bool)>::new()));
    let recorded_clone = Arc::clone(&recorded_updates);

    let writer = RouteQueryWriter {
        apply: Arc::new(move |updates, replace| {
            recorded_clone.lock().unwrap().push((updates, replace));
        }),
    };

    writer.push_value("filter", "active");
    {
        let records = recorded_updates.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert!(!records[0].1);
        assert_eq!(records[0].0, vec![("filter".to_string(), Some("active".to_string()))]);
    }

    writer.replace_value("page", "3");
    {
        let records = recorded_updates.lock().unwrap();
        assert_eq!(records.len(), 2);
        assert!(records[1].1);
        assert_eq!(records[1].0, vec![("page".to_string(), Some("3".to_string()))]);
    }

    writer.clear_key("filter");
    {
        let records = recorded_updates.lock().unwrap();
        assert_eq!(records.len(), 3);
        assert!(records[2].1);
        assert_eq!(records[2].0, vec![("filter".to_string(), None)]);
    }

    writer.replace_query_update("sort", UiRouteQueryUpdate::Replace("desc".to_string()));
    {
        let records = recorded_updates.lock().unwrap();
        assert_eq!(records.len(), 4);
        assert!(records[3].1);
        assert_eq!(records[3].0, vec![("sort".to_string(), Some("desc".to_string()))]);
    }

    let write = UiRouteQueryWrite {
        updates: vec![("page", Some("1".to_string()))],
        replace: true,
    };
    writer.apply_query_write(write);
    {
        let records = recorded_updates.lock().unwrap();
        assert_eq!(records.len(), 5);
        assert!(records[4].1);
        assert_eq!(records[4].0, vec![("page".to_string(), Some("1".to_string()))]);
    }

    let intent = UiRouteQueryIntent::clear("page");
    writer.apply_query_intent(intent);
    {
        let records = recorded_updates.lock().unwrap();
        assert_eq!(records.len(), 6);
        assert!(records[5].1);
        assert_eq!(records[5].0, vec![("page".to_string(), None)]);
    }
}

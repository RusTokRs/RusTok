use rustok_core::{event_schema, EVENT_SCHEMAS};

#[test]
fn event_schema_lookup_returns_known_schema() {
    let schema = event_schema("node.created");
    assert!(schema.is_some());
    assert_eq!(schema.unwrap().version, 1);
}

#[test]
fn schemas_have_unique_event_types() {
    let mut types = std::collections::HashSet::new();
    for schema in EVENT_SCHEMAS {
        assert!(types.insert(schema.event_type));
    }
}

use super::*;
use crate::{FlyError, GrapesJsCodec, ProjectDocument};
use serde_json::{Map, json};

fn document(content: &str) -> ProjectDocument {
    GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "text",
                    "type": "text",
                    "content": content
                }]
            }
        }]
    }))
    .expect("document")
}

#[test]
fn snapshots_restore_and_verify_hash() {
    let mut catalog = SnapshotCatalog::new(2);
    let snapshot = catalog
        .capture("Initial", &document("Hello"), Map::new())
        .expect("snapshot")
        .clone();
    assert_eq!(
        snapshot.restore().expect("restore").hash().hex(),
        snapshot.project_hash
    );

    let mut tampered = snapshot;
    tampered.project_data["pages"][0]["id"] = json!("changed");
    assert!(matches!(
        tampered.restore(),
        Err(FlyError::SnapshotHashMismatch { .. })
    ));
}

#[test]
fn catalog_evicts_old_snapshots() {
    let mut catalog = SnapshotCatalog::new(2);
    catalog.capture("One", &document("1"), Map::new()).unwrap();
    catalog.capture("Two", &document("2"), Map::new()).unwrap();
    catalog
        .capture("Three", &document("3"), Map::new())
        .unwrap();
    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog.iter().next().unwrap().label, "Two");
}

#[test]
fn structural_diff_tracks_component_changes() {
    let before = document("Hello");
    let mut after = document("Hello world");
    after
        .project
        .assets
        .push(json!({ "id": "hero", "src": "/hero.png" }));
    let diff = compare_projects(&before, &after);
    assert!(diff.changed_components.contains(&"text".to_string()));
    assert!(diff.changed_pages.contains(&"home".to_string()));
    assert!(diff.added_assets.contains(&"hero".to_string()));
    assert!(diff.change_count() >= 3);
}

#[test]
fn catalog_compares_snapshot_with_current() {
    let mut catalog = SnapshotCatalog::default();
    let id = catalog
        .capture("Initial", &document("Hello"), Map::new())
        .unwrap()
        .id
        .clone();
    let diff = catalog
        .compare_with_current(&id, &document("Updated"))
        .expect("diff");
    assert!(!diff.is_empty());
}

#[test]
fn anonymous_components_use_canonical_paths_in_diffs() {
    let before = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{ "type": "text", "content": "Before" }]
            }
        }]
    }))
    .expect("before");
    let after = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{ "type": "text", "content": "After" }]
            }
        }]
    }))
    .expect("after");

    let diff = compare_projects(&before, &after);
    assert!(
        diff.changed_components
            .contains(&"@path:project.pages[0].component.components[0]".to_string())
    );
}

#[test]
fn missing_snapshot_is_explicit() {
    let catalog = SnapshotCatalog::default();
    assert!(matches!(
        catalog.compare_with_current("missing", &document("Current")),
        Err(FlyError::SnapshotNotFound(id)) if id == "missing"
    ));
}

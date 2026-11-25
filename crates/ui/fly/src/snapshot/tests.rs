use super::*;
use crate::{ContentDigest, FlyError, GrapesJsCodec, ProjectDocument};
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

    // Tamper with the declared hash only, leaving payload and digest consistent: this is the
    // one way to reach the hash branch now that the digest is checked first. Tampering with the
    // payload is covered by `tampered_snapshot_payload_is_rejected_by_the_digest`.
    let mut tampered = snapshot;
    tampered.project_hash = "0".repeat(16);
    assert!(matches!(
        tampered.restore(),
        Err(FlyError::SnapshotHashMismatch { .. })
    ));
}

#[test]
fn snapshot_capture_fingerprints_the_actual_payload_even_after_raw_mutation() {
    let mut source = document("Before");
    let old_hash = source.hash();
    // The still-public raw project field can bypass cache invalidation.
    source.project.extensions.insert("custom".to_string(), json!("changed"));
    let mut catalog = SnapshotCatalog::default();
    let snapshot = catalog.capture("Current", &source, Map::new()).expect("capture");
    assert_ne!(snapshot.project_hash, old_hash.hex());
    assert_eq!(snapshot.restore_verified().expect("restore"), source);
}

#[test]
fn snapshot_capture_rejects_exhausted_or_reused_sequence_without_mutation() {
    let mut catalog = SnapshotCatalog::default();
    catalog.capture("First", &document("Original"), Map::new()).expect("first");
    let serialized = serde_json::to_value(&catalog).expect("serialize");
    for sequence in [1_u64, u64::MAX] {
        let mut invalid = serialized.clone();
        invalid["next_sequence"] = json!(sequence);
        let mut restored: SnapshotCatalog = serde_json::from_value(invalid).expect("catalog");
        let before = restored.clone();
        assert!(restored.capture("Duplicate", &document("Original"), Map::new()).is_err());
        assert_eq!(restored, before);
    }
}

#[test]
fn zero_capacity_restored_catalog_cannot_consume_a_sequence() {
    let mut value = serde_json::to_value(SnapshotCatalog::default()).expect("serialize");
    value["maximum_snapshots"] = json!(0);
    let mut catalog: SnapshotCatalog = serde_json::from_value(value).expect("catalog");
    let before = catalog.clone();
    assert!(catalog.capture("Invalid", &document("Original"), Map::new()).is_err());
    assert_eq!(catalog, before);
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

#[test]
fn captured_snapshots_carry_a_verifiable_content_digest() {
    let mut catalog = SnapshotCatalog::default();
    let snapshot = catalog
        .capture("baseline", &document("Original"), Map::new())
        .expect("capture")
        .clone();

    assert!(snapshot.has_content_digest());
    assert_eq!(
        snapshot
            .content_digest
            .as_ref()
            .map(ContentDigest::algorithm),
        Some("sha256")
    );
    snapshot.restore_verified().expect("verified restore");
}

#[test]
fn tampered_snapshot_payload_is_rejected_by_the_digest() {
    let mut catalog = SnapshotCatalog::default();
    let mut snapshot = catalog
        .capture("baseline", &document("Original"), Map::new())
        .expect("capture")
        .clone();

    snapshot.project_data["pages"][0]["id"] = json!("tampered");

    assert!(matches!(
        snapshot.restore(),
        Err(FlyError::SnapshotDigestMismatch { .. })
    ));
}

#[test]
fn digestless_snapshots_remain_readable_but_not_verifiable() {
    let mut catalog = SnapshotCatalog::default();
    let mut snapshot = catalog
        .capture("baseline", &document("Original"), Map::new())
        .expect("capture")
        .clone();

    snapshot.content_digest = None;

    assert!(snapshot.restore().is_ok());
    assert!(matches!(
        snapshot.restore_verified(),
        Err(FlyError::SnapshotDigestMissing(_))
    ));
}

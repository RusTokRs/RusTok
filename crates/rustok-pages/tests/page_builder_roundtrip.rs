#[test]
fn pages_write_api_is_split_by_owner_and_revision() {
    let dto = include_str!("../src/dto/page.rs");
    let service = include_str!("../src/services/page/mod.rs");
    let graphql = include_str!("../src/graphql/types.rs");

    assert!(dto.contains("struct PatchPageMetadataInput"));
    assert!(dto.contains("expected_version: i32"));
    assert!(dto.contains("struct SavePageDocumentInput"));
    assert!(dto.contains("expected_revision: String"));
    assert!(!dto.contains("struct UpdatePageInput"));

    assert!(service.contains("mod metadata;"));
    assert!(service.contains("mod document;"));
    assert!(service.contains("mod lifecycle;"));
    assert!(!service.contains("mod update;"));

    assert!(graphql.contains("PatchGqlPageMetadataInput"));
    assert!(graphql.contains("SaveGqlPageDocumentInput"));
    assert!(!graphql.contains("UpdateGqlPageInput"));
}

#[test]
fn document_and_metadata_services_cannot_cross_write() {
    let metadata = include_str!("../src/services/page/metadata.rs");
    let document = include_str!("../src/services/page/document.rs");

    assert!(!metadata.contains("PageBodyInput"));
    assert!(!metadata.contains("upsert_body_in_tx"));
    assert!(metadata.contains("active.version"));

    assert!(!document.contains("replace_translations_in_tx"));
    assert!(!document.contains("replace_channel_visibility_in_tx"));
    assert!(!document.contains("active.version"));
    assert!(document.contains("PAGE_DOCUMENT_REVISION_CONFLICT"));
    assert!(document.contains("PAGE_PUBLISHED_DOCUMENT_IMMUTABLE"));
}

#[test]
fn publish_binds_only_the_locked_document_revision() {
    let lifecycle = include_str!("../src/services/page/lifecycle.rs");
    let revision_check = lifecycle
        .find("current_revisions != expected_revisions")
        .expect("publish must compare body revisions");
    let compile = lifecycle
        .find("compile_builder_sources(&current_bodies")
        .expect("publish must compile locked bodies");
    let bind = lifecycle
        .find("bind_existing_body_in_tx")
        .expect("publish must bind the artifact");

    assert!(lifecycle.contains("load_bodies_for_publish"));
    assert!(lifecycle.contains("lock_exclusive"));
    assert!(revision_check < compile);
    assert!(compile < bind);
}

#[test]
fn current_fly_tree_remains_the_only_document_authority() {
    let builder = include_str!("../admin/src/builder.rs");
    assert!(builder.contains("save_page_document"));
    assert!(builder.contains("PAGE_PUBLISHED_DOCUMENT_IMMUTABLE"));
    assert!(!builder.contains("update_page("));
    assert!(!builder.contains("PageDraftFormInput"));
    assert!(!builder.contains("frames[0].component"));
}

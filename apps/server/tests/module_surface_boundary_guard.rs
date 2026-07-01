use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("apps/server should live under workspace root")
        .to_path_buf()
}

#[test]
fn optional_module_transport_shims_do_not_live_in_server() {
    let root = repo_root().join("apps/server/src");
    let forbidden_paths = [
        "controllers/blog",
        "controllers/commerce",
        "controllers/forum",
        "controllers/media",
        "controllers/pages.rs",
        "controllers/workflow",
        "graphql/blog",
        "graphql/commerce",
        "graphql/forum",
        "graphql/media",
        "graphql/pages",
        "graphql/workflow",
    ];

    for relative in forbidden_paths {
        let path = root.join(relative);
        assert!(
            !path.exists(),
            "optional module transport surface must be owned by its module crate, not apps/server: {}",
            path.display()
        );
    }
}

#[test]
fn module_route_codegen_prefers_owner_crates_over_server_wrappers() {
    let build_rs = std::fs::read_to_string(repo_root().join("apps/server/build.rs"))
        .expect("server build.rs should be readable");

    assert!(
        !build_rs.contains("crate::controllers::{slug}::routes()"),
        "route codegen must not prefer apps/server controller wrappers for optional modules"
    );
    assert!(
        !build_rs.contains("crate::controllers::{slug}::webhook_routes()"),
        "webhook route codegen must not prefer apps/server controller wrappers for optional modules"
    );
}

#[test]
fn optional_module_openapi_definitions_do_not_live_in_server() {
    let swagger =
        std::fs::read_to_string(repo_root().join("apps/server/src/controllers/swagger.rs"))
            .expect("server swagger controller should be readable");

    for forbidden in [
        "rustok_blog::controllers",
        "rustok_forum::controllers",
        "rustok_pages::controllers",
        "rustok_commerce::controllers",
        "rustok_media::controllers",
        "rustok_workflow::controllers",
        "rustok_product::dto::CreateProductInput",
        "rustok_cart::dto::CartResponse",
    ] {
        assert!(
            !swagger.contains(forbidden),
            "module-owned OpenAPI paths/components must be defined in owner crates, not apps/server: {forbidden}"
        );
    }
}

#[test]
fn content_orchestration_bridge_does_not_live_in_server() {
    let server_impl = repo_root().join("apps/server/src/services/content_orchestration.rs");
    assert!(
        !server_impl.exists(),
        "content orchestration bridge is cross-module domain logic and must live outside apps/server"
    );

    let services_dir = repo_root().join("apps/server/src/services");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&services_dir).expect("server services dir should be readable") {
        let entry = entry.expect("server service entry should be readable");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("server service source should read");
        for forbidden in [
            "rustok_blog::",
            "rustok_forum::",
            "rustok_taxonomy::",
            "rustok_comments::",
        ] {
            if source.contains(forbidden) {
                offenders.push(format!("{} contains {forbidden}", path.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "server services must not own content/blog/forum orchestration internals: {offenders:?}"
    );
}

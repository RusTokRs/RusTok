use super::{
    ManifestError, ManifestManager, ModulesManifest, StorefrontBuildConfig, builtin_module_catalog,
};
use crate::modules::build_registry;
use rustok_build::{BuildRuntimeMode, DeploymentProfile, FrontendArtifactKind, FrontendBuildTool};
use serial_test::serial;
use std::collections::HashMap;
use tempfile::tempdir;

fn manifest_with_modules(slugs: &[&str]) -> ModulesManifest {
    let catalog = builtin_module_catalog();
    let modules = slugs
        .iter()
        .map(|slug| ((*slug).to_string(), catalog.get(slug).unwrap().clone()))
        .collect::<HashMap<_, _>>();

    ModulesManifest {
        schema: 2,
        app: "rustok-server".to_string(),
        modules,
        ..Default::default()
    }
}

fn write_module_manifest(crate_dir: &std::path::Path, contents: &str) {
    std::fs::create_dir_all(crate_dir).unwrap();
    std::fs::write(crate_dir.join("rustok-module.toml"), contents).unwrap();
}

fn write_surface_manifest(crate_dir: &std::path::Path, surface: &str, crate_name: &str) {
    let surface_dir = crate_dir.join(surface);
    std::fs::create_dir_all(&surface_dir).unwrap();
    std::fs::write(
        surface_dir.join("Cargo.toml"),
        format!("[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
    )
    .unwrap();
}

fn write_locale_bundle(dir: &std::path::Path, locale: &str, value: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join(format!("{locale}.json")),
        format!("{{\"title\":\"{value}\"}}"),
    )
    .unwrap();
}

#[test]
fn module_slug_accepts_canonical_underscore_format() {
    assert!(rustok_modules::is_valid_static_module_slug("page_builder"));
    assert!(rustok_modules::is_valid_static_module_slug("ai-alloy"));
    assert!(!rustok_modules::is_valid_static_module_slug("PageBuilder"));
    assert!(!rustok_modules::is_valid_static_module_slug("page.builder"));
}

#[test]
#[serial]
fn derives_deployment_surface_contract_from_build_server_flags() {
    let mut manifest = ModulesManifest::default();

    let headless = ManifestManager::deployment_surface_contract(&manifest);
    assert_eq!(headless.profile, DeploymentProfile::HeadlessApi);
    assert!(!headless.embed_admin);
    assert!(!headless.embed_storefront);

    manifest.build.server.embed_admin = true;
    let server_with_admin = ManifestManager::deployment_surface_contract(&manifest);
    assert_eq!(
        server_with_admin.profile,
        DeploymentProfile::ServerWithAdmin
    );
    assert!(server_with_admin.embed_admin);
    assert!(!server_with_admin.embed_storefront);

    manifest.build.server.embed_storefront = true;
    let monolith = ManifestManager::deployment_surface_contract(&manifest);
    assert_eq!(monolith.profile, DeploymentProfile::Monolith);
    assert!(monolith.embed_admin);
    assert!(monolith.embed_storefront);

    manifest.build.server.embed_admin = false;
    let server_with_storefront = ManifestManager::deployment_surface_contract(&manifest);
    assert_eq!(
        server_with_storefront.profile,
        DeploymentProfile::ServerWithStorefront
    );
    assert!(!server_with_storefront.embed_admin);
    assert!(server_with_storefront.embed_storefront);
}

#[test]
#[serial]
fn derives_build_execution_plan_from_manifest() {
    let mut manifest = ModulesManifest {
        app: "rustok-server".to_string(),
        ..ModulesManifest::default()
    };
    manifest.build.profile = "release".to_string();
    manifest.build.target = "x86_64-unknown-linux-gnu".to_string();
    manifest.build.server.embed_admin = true;
    manifest.build.server.embed_storefront = true;

    let plan = ManifestManager::build_execution_plan(&manifest);

    assert_eq!(plan.cargo_package, "rustok-server");
    assert_eq!(plan.cargo_profile, "release");
    assert_eq!(
        plan.cargo_target.as_deref(),
        Some("x86_64-unknown-linux-gnu")
    );
    assert_eq!(
        plan.cargo_features,
        vec!["embed-admin".to_string(), "embed-storefront".to_string()]
    );
    assert_eq!(
        plan.cargo_command,
        "cargo build -p rustok-server --release --target x86_64-unknown-linux-gnu --features embed-admin,embed-storefront"
    );
    let admin_build = plan.admin_build.expect("expected admin build plan");
    assert_eq!(admin_build.surface, "admin");
    assert_eq!(admin_build.tool, FrontendBuildTool::Trunk);
    assert_eq!(admin_build.workspace_path, "apps/admin");
    assert_eq!(admin_build.artifact_path, "apps/admin/dist");
    assert_eq!(admin_build.artifact_kind, FrontendArtifactKind::Directory);
    assert_eq!(admin_build.command, "trunk build --release");

    let storefront_build = plan
        .storefront_build
        .expect("expected storefront build plan");
    assert_eq!(storefront_build.surface, "storefront");
    assert_eq!(storefront_build.tool, FrontendBuildTool::Cargo);
    assert_eq!(storefront_build.package, "rustok-storefront");
    assert_eq!(storefront_build.workspace_path, ".");
    assert_eq!(
        storefront_build.target.as_deref(),
        Some("x86_64-unknown-linux-gnu")
    );
    assert_eq!(
        storefront_build.command,
        "cargo build -p rustok-storefront --release --target x86_64-unknown-linux-gnu"
    );
}

#[test]
fn derives_single_role_build_plans_without_cross_role_surfaces() {
    let mut manifest = ModulesManifest {
        app: "rustok-server".to_string(),
        ..ModulesManifest::default()
    };
    manifest.build.server.embed_admin = true;
    manifest.build.server.embed_storefront = true;

    let worker = ManifestManager::role_build_plan(&manifest, BuildRuntimeMode::Worker);
    assert_eq!(worker.profile, DeploymentProfile::Worker);
    assert_eq!(worker.execution_plan.runtime_mode, BuildRuntimeMode::Worker);
    assert!(worker.execution_plan.cargo_features.is_empty());
    assert!(worker.execution_plan.admin_build.is_none());
    assert!(worker.execution_plan.storefront_build.is_none());

    let admin = ManifestManager::role_build_plan(&manifest, BuildRuntimeMode::AdminSsr);
    assert_eq!(admin.profile, DeploymentProfile::ServerWithAdmin);
    assert_eq!(
        admin.execution_plan.runtime_mode,
        BuildRuntimeMode::AdminSsr
    );
    assert_eq!(admin.execution_plan.cargo_features, vec!["embed-admin"]);
    assert!(admin.execution_plan.storefront_build.is_none());
}

#[test]
#[serial]
fn rejects_standalone_admin_without_redirect_uris() {
    let mut manifest = ModulesManifest::default();
    manifest.build.server.embed_admin = false;
    manifest.build.admin.stack = "next".to_string();
    manifest.build.admin.public_url = "http://localhost:3001".to_string();

    let result = ManifestManager::validate(&manifest);

    assert!(matches!(
        result,
        Err(ManifestError::InvalidBuildSurface(message))
        if message.contains("build.admin.redirect_uris")
    ));
}

#[test]
#[serial]
fn allows_standalone_surfaces_with_public_oauth_config() {
    let mut manifest = ModulesManifest::default();
    manifest.build.server.embed_admin = false;
    manifest.build.server.embed_storefront = false;
    manifest.build.admin.stack = "next".to_string();
    manifest.build.admin.public_url = "http://localhost:3001".to_string();
    manifest.build.admin.redirect_uris = vec!["http://localhost:3001/auth/callback".to_string()];
    manifest.build.storefront = vec![StorefrontBuildConfig {
        id: "default".to_string(),
        stack: "next".to_string(),
        public_url: "http://localhost:3000".to_string(),
        redirect_uris: vec!["http://localhost:3000/auth/callback".to_string()],
    }];

    let result = ManifestManager::validate(&manifest);

    assert!(result.is_ok(), "expected valid standalone surface config");
}

#[test]
#[serial]
fn allows_registry_superset_when_optional_module_is_removed_from_manifest() {
    let registry = build_registry();
    let manifest = manifest_with_modules(&[
        "index",
        "outbox",
        "content",
        "taxonomy",
        "cart",
        "customer",
        "product",
        "region",
        "pricing",
        "inventory",
        "order",
        "payment",
        "fulfillment",
        "commerce",
        "pages",
        "tenant",
        "rbac",
    ]);

    let result = ManifestManager::validate_with_registry(&manifest, &registry);
    assert!(
        result.is_ok(),
        "optional registry modules may be absent from manifest"
    );
}

#[test]
#[serial]
fn uninstall_removes_default_enabled_entry() {
    let mut manifest = manifest_with_modules(&[
        "index",
        "outbox",
        "content",
        "taxonomy",
        "cart",
        "customer",
        "product",
        "region",
        "pricing",
        "inventory",
        "order",
        "payment",
        "fulfillment",
        "commerce",
        "pages",
        "tenant",
        "rbac",
    ]);
    manifest.settings.default_enabled = vec![
        "content".to_string(),
        "cart".to_string(),
        "customer".to_string(),
        "product".to_string(),
        "region".to_string(),
        "pricing".to_string(),
        "inventory".to_string(),
        "order".to_string(),
        "payment".to_string(),
        "fulfillment".to_string(),
        "commerce".to_string(),
        "pages".to_string(),
    ];

    ManifestManager::uninstall_module(&mut manifest, "pages").unwrap();

    assert!(
        !manifest
            .settings
            .default_enabled
            .iter()
            .any(|slug| slug == "pages")
    );
}

#[test]
#[serial]
fn install_builtin_module_restores_catalog_defaults() {
    let mut manifest = manifest_with_modules(&["index", "outbox", "content", "tenant", "rbac"]);

    ManifestManager::install_builtin_module(&mut manifest, "pages", Some("1.2.0".to_string()))
        .unwrap();

    assert!(manifest.modules.contains_key("pages"));
    assert!(
        manifest
            .settings
            .default_enabled
            .iter()
            .any(|slug| slug == "pages")
    );
}

#[test]
#[serial]
fn catalog_modules_overlay_metadata_from_rustok_module_manifest() {
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("modules.toml");
    let crate_dir = temp.path().join("crates").join("rustok-blog");
    std::fs::create_dir_all(&crate_dir).unwrap();
    std::fs::write(
        crate_dir.join("rustok-module.toml"),
        r#"[module]
ownership = "third_party"
trust_level = "private"
recommended_admin_surfaces = ["custom-admin"]
showcase_admin_surfaces = ["next-admin", "storybook"]

[marketplace]
category = "editorial"
tags = ["editorial", "stories", "news"]
icon = "https://cdn.example.test/modules/blog/icon.svg"
banner = "https://cdn.example.test/modules/blog/banner.png"
screenshots = [
  "https://cdn.example.test/modules/blog/screenshot-1.png",
  "https://cdn.example.test/modules/blog/screenshot-2.png",
]
"#,
    )
    .unwrap();

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::catalog_modules(&manifest).unwrap();

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    let blog = result
        .into_iter()
        .find(|module| module.slug == "blog")
        .unwrap();
    assert_eq!(blog.ownership, "third_party");
    assert_eq!(blog.trust_level, "private");
    assert_eq!(blog.category.as_deref(), Some("editorial"));
    assert_eq!(blog.tags, vec!["editorial", "news", "stories"]);
    assert_eq!(
        blog.icon_url.as_deref(),
        Some("https://cdn.example.test/modules/blog/icon.svg")
    );
    assert_eq!(
        blog.banner_url.as_deref(),
        Some("https://cdn.example.test/modules/blog/banner.png")
    );
    assert_eq!(
        blog.screenshots,
        vec![
            "https://cdn.example.test/modules/blog/screenshot-1.png",
            "https://cdn.example.test/modules/blog/screenshot-2.png",
        ]
    );
    assert_eq!(blog.recommended_admin_surfaces, vec!["custom-admin"]);
    assert_eq!(
        blog.showcase_admin_surfaces,
        vec!["next-admin", "storybook"]
    );
}

fn catalog_modules_error_for_blog_manifest(contents: &str) -> ManifestError {
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("modules.toml");
    let crate_dir = temp.path().join("crates").join("rustok-blog");
    write_module_manifest(&crate_dir, contents);

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::catalog_modules(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    result.expect_err("catalog metadata should fail validation")
}

#[test]
#[serial]
fn catalog_modules_reject_short_marketplace_description() {
    let error = catalog_modules_error_for_blog_manifest(
        r#"[module]
description = "Too short"
ownership = "third_party"
trust_level = "private"
"#,
    );

    assert!(matches!(
        error,
        ManifestError::InvalidModuleMarketplaceMetadata {
            slug,
            field,
            reason,
        } if slug == "blog"
            && field == "description"
            && reason.contains("at least 20 characters")
    ));
}

#[test]
#[serial]
fn catalog_modules_reject_non_svg_marketplace_icon() {
    let error = catalog_modules_error_for_blog_manifest(
        r#"[module]
description = "Blog metadata description is long enough."
ownership = "third_party"
trust_level = "private"

[marketplace]
icon = "https://cdn.example.test/modules/blog/icon.png"
"#,
    );

    assert!(matches!(
        error,
        ManifestError::InvalidModuleMarketplaceMetadata {
            slug,
            field,
            reason,
        } if slug == "blog"
            && field == "icon"
            && reason.contains(".svg")
    ));
}

#[test]
#[serial]
fn catalog_modules_reject_invalid_marketplace_screenshot_url() {
    let error = catalog_modules_error_for_blog_manifest(
        r#"[module]
description = "Blog metadata description is long enough."
ownership = "third_party"
trust_level = "private"

[marketplace]
screenshots = ["not-a-url"]
"#,
    );

    assert!(matches!(
        error,
        ManifestError::InvalidModuleMarketplaceMetadata {
            slug,
            field,
            reason,
        } if slug == "blog"
            && field == "screenshots[0]"
            && reason.contains("valid absolute URL")
    ));
}

#[test]
#[serial]
fn validate_overlays_server_entrypoints_from_rustok_module_manifest() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let content_dir = temp.path().join("crates").join("rustok-content");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[crate]
entry_type = "BlogModule"

[provides.graphql]
query = "graphql::BlogQuery"
mutation = "graphql::BlogMutation"

[provides.http]
routes = "controllers::routes"
webhook_routes = "controllers::webhook_routes"
"#,
    );
    write_module_manifest(
        &content_dir,
        r#"[module]
slug = "content"
name = "Content"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    manifest.modules.get_mut("content").unwrap().path = Some("crates/rustok-content".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::load()
        .and_then(|loaded| {
            ManifestManager::validate(&loaded)?;
            super::resolve_module_specs(&loaded)
        })
        .map(|specs| specs.get("blog").cloned());

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    let blog = result
        .expect("manifest should validate with explicit server entry points")
        .expect("blog spec");
    assert_eq!(blog.entry_type.as_deref(), Some("rustok_blog::BlogModule"));
    assert_eq!(
        blog.graphql_query_type.as_deref(),
        Some("rustok_blog::graphql::BlogQuery")
    );
    assert_eq!(
        blog.graphql_mutation_type.as_deref(),
        Some("rustok_blog::graphql::BlogMutation")
    );
    assert_eq!(
        blog.http_routes_fn.as_deref(),
        Some("rustok_blog::controllers::routes")
    );
    assert_eq!(
        blog.http_webhook_routes_fn.as_deref(),
        Some("rustok_blog::controllers::webhook_routes")
    );
}

#[test]
#[serial]
fn catalog_modules_require_rustok_module_manifest_for_path_modules() {
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("modules.toml");
    let crate_dir = temp.path().join("crates").join("rustok-blog");
    std::fs::create_dir_all(&crate_dir).unwrap();

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::catalog_modules(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::MissingModulePackageManifest { slug, .. }) if slug == "blog"
    ));
}

#[test]
#[serial]
fn catalog_modules_reject_conflicting_admin_surface_metadata() {
    let temp = tempdir().unwrap();
    let manifest_path = temp.path().join("modules.toml");
    let crate_dir = temp.path().join("crates").join("rustok-blog");
    std::fs::create_dir_all(&crate_dir).unwrap();
    std::fs::write(
        crate_dir.join("rustok-module.toml"),
        r#"[module]
ownership = "first_party"
trust_level = "verified"
recommended_admin_surfaces = ["leptos-admin", "next-admin"]
showcase_admin_surfaces = ["next-admin"]
"#,
    )
    .unwrap();

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::catalog_modules(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::ConflictingModuleAdminSurface { slug, surface })
            if slug == "blog" && surface == "next-admin"
    ));
}

#[test]
#[serial]
fn validate_module_settings_applies_defaults_from_module_manifest() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
postsPerPage = { type = "integer", default = 20, min = 1, max = 100 }
showAuthor = { type = "boolean", default = true }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::validate_module_settings("blog", serde_json::json!({}));

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    let settings = result.expect("settings should be normalized from defaults");
    assert_eq!(settings["postsPerPage"], serde_json::json!(20));
    assert_eq!(settings["showAuthor"], serde_json::json!(true));
}

#[test]
#[serial]
fn validate_module_settings_rejects_unknown_keys() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
postsPerPage = { type = "integer", default = 20, min = 1, max = 100 }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result =
        ManifestManager::validate_module_settings("blog", serde_json::json!({ "unknown": true }));

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleSettingValue { slug, key, .. })
            if slug == "blog" && key == "unknown"
    ));
}

#[test]
#[serial]
fn validate_module_settings_rejects_out_of_range_values() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
postsPerPage = { type = "integer", default = 20, min = 1, max = 100 }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::validate_module_settings(
        "blog",
        serde_json::json!({ "postsPerPage": 1000 }),
    );

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleSettingValue { slug, key, .. })
            if slug == "blog" && key == "postsPerPage"
    ));
}

#[test]
#[serial]
fn validate_module_settings_rejects_values_outside_declared_options() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
layout = { type = "string", default = "grid", options = ["grid", "list"] }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result =
        ManifestManager::validate_module_settings("blog", serde_json::json!({ "layout": "hero" }));

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
            if slug == "blog"
                && key == "layout"
                && reason.contains("must be one of")
    ));
}

#[test]
#[serial]
fn validate_rejects_setting_schema_with_default_outside_declared_options() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
layout = { type = "string", default = "hero", options = ["grid", "list"] }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::validate_module_settings("blog", serde_json::json!({}));

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleSettingSchema { slug, key, reason })
            if slug == "blog"
                && key == "layout"
                && reason.contains("default must be one of the declared options")
    ));
}

#[test]
#[serial]
fn validate_module_settings_rejects_unknown_object_keys_outside_declared_shape() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
seo = { type = "object", object_keys = ["metaTitle", "metaDescription", "indexable"] }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::validate_module_settings(
        "blog",
        serde_json::json!({
            "seo": {
                "metaTitle": "Welcome",
                "unknown": true
            }
        }),
    );

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
            if slug == "blog"
                && key == "seo"
                && reason.contains("unknown object key 'unknown'")
    ));
}

#[test]
#[serial]
fn validate_module_settings_rejects_array_items_that_do_not_match_declared_item_type() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
featuredPostIds = { type = "array", item_type = "string", default = [] }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::validate_module_settings(
        "blog",
        serde_json::json!({ "featuredPostIds": ["post-1", 2] }),
    );

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
            if slug == "blog"
                && key == "featuredPostIds"
                && reason.contains("array item at index 1 must be string")
    ));
}

#[test]
#[serial]
fn validate_rejects_setting_schema_with_item_type_on_non_array_setting() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
seo = { type = "object", item_type = "string" }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::validate_module_settings("blog", serde_json::json!({}));

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleSettingSchema { slug, key, reason })
            if slug == "blog"
                && key == "seo"
                && reason.contains("item_type is only supported for array settings")
    ));
}

#[test]
#[serial]
fn validate_module_settings_rejects_nested_object_property_type_mismatch() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
seo = { type = "object", properties = { metaTitle = { type = "string" }, indexable = { type = "boolean", default = true } } }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::validate_module_settings(
        "blog",
        serde_json::json!({
            "seo": {
                "metaTitle": 42
            }
        }),
    );

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
            if slug == "blog"
                && key == "seo.metaTitle"
                && reason.contains("expected") && reason.contains("string")
    ));
}

#[test]
#[serial]
fn validate_module_settings_rejects_nested_array_item_schema_mismatch() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let manifest_path = temp.path().join("modules.toml");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[settings]
contentBlocks = { type = "array", items = { type = "object", properties = { kind = { type = "string" }, enabled = { type = "boolean" } } } }
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    ManifestManager::save_to_path(&manifest_path, &manifest).unwrap();

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", &manifest_path);
    }

    let result = ManifestManager::validate_module_settings(
        "blog",
        serde_json::json!({
            "contentBlocks": [
                { "kind": "hero", "enabled": true },
                { "kind": "gallery", "enabled": "yes" }
            ]
        }),
    );

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleSettingValue { slug, key, reason })
            if slug == "blog"
                && key == "contentBlocks[1].enabled"
                && reason.contains("expected") && reason.contains("boolean")
    ));
}

#[test]
#[serial]
fn validate_rejects_dependency_version_mismatch_from_module_package_manifest() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let content_dir = temp.path().join("crates").join("rustok-content");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[dependencies]
content = { version_req = ">=0.2.0" }
"#,
    );
    write_module_manifest(
        &content_dir,
        r#"[module]
slug = "content"
name = "Content"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    manifest.modules.get_mut("content").unwrap().path = Some("crates/rustok-content".to_string());

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
    }

    let result = ManifestManager::validate(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::IncompatibleDependencyVersion {
            slug,
            dependency,
            ..
        }) if slug == "blog" && dependency == "content"
    ));
}

#[test]
#[serial]
fn validate_rejects_conflicting_module_from_module_package_manifest() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let forum_dir = temp.path().join("crates").join("rustok-forum");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[conflicts]
modules = ["forum"]
"#,
    );
    write_module_manifest(
        &forum_dir,
        r#"[module]
slug = "forum"
name = "Forum"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "content", "comments", "blog", "forum", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    manifest.modules.get_mut("forum").unwrap().path = Some("crates/rustok-forum".to_string());

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
    }

    let result = ManifestManager::validate(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::ConflictingModule {
            slug,
            conflicts_with,
        }) if slug == "blog" && conflicts_with == "forum"
    ));
}

#[test]
#[serial]
fn validate_uses_module_package_version_for_dependency_checks() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let content_dir = temp.path().join("crates").join("rustok-content");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[dependencies]
content = { version_req = ">=0.1.0" }
"#,
    );
    write_module_manifest(
        &content_dir,
        r#"[module]
slug = "content"
name = "Content"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
    );

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());
    manifest.modules.get_mut("content").unwrap().path = Some("crates/rustok-content".to_string());

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
    }

    let result = ManifestManager::validate(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(
        result.is_ok(),
        "expected dependency version to be resolved from rustok-module.toml"
    );
}

#[test]
#[serial]
fn validate_rejects_admin_subcrate_without_manifest_wiring() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"
"#,
    );
    write_surface_manifest(&blog_dir, "admin", "rustok-blog-admin");

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
    }

    let result = ManifestManager::validate(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleUiWiring { slug, surface, reason })
            if slug == "blog"
                && surface == "admin"
                && reason.contains("[provides.admin_ui].leptos_crate")
    ));
}

#[test]
#[serial]
fn validate_rejects_storefront_wiring_without_subcrate() {
    let temp = tempdir().unwrap();
    let pages_dir = temp.path().join("crates").join("rustok-pages");
    write_module_manifest(
        &pages_dir,
        r#"[module]
slug = "pages"
name = "Pages"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.storefront_ui]
leptos_crate = "rustok-pages-storefront"
"#,
    );

    let mut manifest =
        manifest_with_modules(&["index", "outbox", "pages", "content", "tenant", "rbac"]);
    manifest.modules.get_mut("pages").unwrap().path = Some("crates/rustok-pages".to_string());

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
    }

    let result = ManifestManager::validate(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleUiWiring { slug, surface, reason })
            if slug == "pages"
                && surface == "storefront"
                && reason.contains("declares [provides.storefront_ui].leptos_crate")
    ));
}

#[test]
#[serial]
fn validate_accepts_manifest_declared_admin_i18n_bundles() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");
    let next_messages_dir = temp
        .path()
        .join("apps")
        .join("next-admin")
        .join("packages")
        .join("blog")
        .join("messages");

    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"
next_package = "@rustok/blog-admin"

[provides.admin_ui.i18n]
default_locale = "en"
supported_locales = ["en", "ru"]
leptos_locales_path = "admin/locales"
next_messages_path = "../../apps/next-admin/packages/blog/messages"
"#,
    );
    write_surface_manifest(&blog_dir, "admin", "rustok-blog-admin");
    write_locale_bundle(&blog_dir.join("admin").join("locales"), "en", "Blog");
    write_locale_bundle(&blog_dir.join("admin").join("locales"), "ru", "Блог");
    write_locale_bundle(&next_messages_dir, "en", "Blog");
    write_locale_bundle(&next_messages_dir, "ru", "Блог");

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
    }

    let result = ManifestManager::validate(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(result.is_ok(), "expected i18n UI wiring to validate");
}

#[test]
#[serial]
fn validate_rejects_ui_i18n_default_locale_outside_supported_locales() {
    let temp = tempdir().unwrap();
    let blog_dir = temp.path().join("crates").join("rustok-blog");

    write_module_manifest(
        &blog_dir,
        r#"[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"

[provides.admin_ui.i18n]
default_locale = "ru"
supported_locales = ["en"]
leptos_locales_path = "admin/locales"
"#,
    );
    write_surface_manifest(&blog_dir, "admin", "rustok-blog-admin");
    write_locale_bundle(&blog_dir.join("admin").join("locales"), "en", "Blog");

    let mut manifest = manifest_with_modules(&[
        "index", "outbox", "blog", "content", "comments", "tenant", "rbac",
    ]);
    manifest.modules.get_mut("blog").unwrap().path = Some("crates/rustok-blog".to_string());

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
    }

    let result = ManifestManager::validate(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleUiWiring { slug, surface, reason })
            if slug == "blog"
                && surface == "admin"
                && reason.contains("default_locale 'ru'")
    ));
}

#[test]
#[serial]
fn validate_accepts_manifest_declared_i18n_with_script_and_numeric_region_locales() {
    let temp = tempdir().unwrap();
    let pages_dir = temp.path().join("crates").join("rustok-pages");

    write_module_manifest(
        &pages_dir,
        r#"[module]
slug = "pages"
name = "Pages"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.storefront_ui]
leptos_crate = "rustok-pages-storefront"

[provides.storefront_ui.i18n]
default_locale = "zh-hant"
supported_locales = ["zh_hant", "es-419"]
leptos_locales_path = "storefront/locales"
"#,
    );
    write_surface_manifest(&pages_dir, "storefront", "rustok-pages-storefront");
    write_locale_bundle(
        &pages_dir.join("storefront").join("locales"),
        "zh-Hant",
        "Pages",
    );
    write_locale_bundle(
        &pages_dir.join("storefront").join("locales"),
        "es-419",
        "Pages",
    );

    let mut manifest =
        manifest_with_modules(&["index", "outbox", "pages", "content", "tenant", "rbac"]);
    manifest.modules.get_mut("pages").unwrap().path = Some("crates/rustok-pages".to_string());

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
    }

    let result = ManifestManager::validate(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(
        result.is_ok(),
        "expected script/numeric locale UI wiring to validate"
    );
}

#[test]
#[serial]
fn validate_rejects_manifest_declared_i18n_bundle_missing_locale_file() {
    let temp = tempdir().unwrap();
    let forum_dir = temp.path().join("crates").join("rustok-forum");

    write_module_manifest(
        &forum_dir,
        r#"[module]
slug = "forum"
name = "Forum"
version = "0.1.0"
ownership = "first_party"
trust_level = "verified"

[provides.storefront_ui]
leptos_crate = "rustok-forum-storefront"

[provides.storefront_ui.i18n]
default_locale = "en"
supported_locales = ["en", "ru"]
leptos_locales_path = "storefront/locales"
"#,
    );
    write_surface_manifest(&forum_dir, "storefront", "rustok-forum-storefront");
    write_locale_bundle(&forum_dir.join("storefront").join("locales"), "en", "Forum");

    let mut manifest =
        manifest_with_modules(&["index", "outbox", "forum", "content", "tenant", "rbac"]);
    manifest.modules.get_mut("forum").unwrap().path = Some("crates/rustok-forum".to_string());

    let previous = std::env::var("RUSTOK_MODULES_MANIFEST").ok();
    unsafe {
        std::env::set_var("RUSTOK_MODULES_MANIFEST", temp.path().join("modules.toml"));
    }

    let result = ManifestManager::validate(&manifest);

    match previous {
        Some(value) => unsafe {
            std::env::set_var("RUSTOK_MODULES_MANIFEST", value);
        },
        None => unsafe {
            std::env::remove_var("RUSTOK_MODULES_MANIFEST");
        },
    }

    assert!(matches!(
        result,
        Err(ManifestError::InvalidModuleUiWiring { slug, surface, reason })
            if slug == "forum"
                && surface == "storefront"
                && reason.contains("missing locale bundle")
                && reason.contains("ru.json")
    ));
}

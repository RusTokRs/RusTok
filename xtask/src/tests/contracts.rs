use super::*;

#[test]
fn resolve_workspace_inherited_string_uses_workspace_package_value() {
    let package_manifest: toml::Value = toml::from_str(
        r#"
                [package]
                version.workspace = true
            "#,
    )
    .expect("package manifest should parse");
    let workspace_manifest: toml::Value = toml::from_str(
        r#"
                [workspace.package]
                version = "1.2.3"
            "#,
    )
    .expect("workspace manifest should parse");

    let resolved = resolve_workspace_inherited_string(
        package_manifest
            .get("package")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("version")),
        &workspace_manifest,
        "version",
    )
    .expect("version should resolve");

    assert_eq!(resolved.as_deref(), Some("1.2.3"));
}

#[test]
fn validate_module_publish_contract_rejects_short_description() {
    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "blog"
                name = "Blog"
                version = "1.2.3"
                description = "Too short"
                ownership = "first_party"
                trust_level = "verified"
            "#,
    )
    .expect("module manifest should parse");

    let error = validate_module_publish_contract("blog", &manifest)
        .expect_err("short descriptions must fail");
    assert!(error.to_string().contains("at least 20 characters"));
}

#[test]
fn validate_module_local_docs_file_rejects_missing_heading() {
    let path = env::temp_dir().join(format!("xtask-local-docs-{}-README.md", std::process::id()));
    std::fs::write(&path, "# Heading\n\n## Назначение\n")
        .expect("temporary docs file should be writable");

    let error = validate_module_local_docs_file(
        "blog",
        &path,
        &["## Назначение", "## Зона ответственности"],
    )
    .expect_err("missing heading must fail");

    assert!(error.to_string().contains("must contain heading"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn normalize_module_ui_classification_accepts_supported_values() {
    assert_eq!(
        normalize_module_ui_classification("admin-only").expect("admin-only should normalize"),
        "admin_only"
    );
    assert_eq!(
        normalize_module_ui_classification("dual_surface").expect("dual_surface should normalize"),
        "dual_surface"
    );
}

#[test]
fn validate_module_ui_metadata_contract_rejects_missing_admin_metadata() {
    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "admin_only"
                recommended_admin_surfaces = ["leptos-admin"]

                [provides.admin_ui]
                leptos_crate = "rustok-demo-admin"
            "#,
    )
    .expect("module manifest should parse");

    let error = validate_module_ui_metadata_contract("demo", &manifest)
        .expect_err("missing admin metadata must fail");
    assert!(error
        .to_string()
        .contains("provides.admin_ui.route_segment"));
}

#[test]
fn validate_module_ui_metadata_contract_accepts_dual_surface_manifest() {
    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "dual_surface"
                recommended_admin_surfaces = ["leptos-admin"]

                [provides.admin_ui]
                leptos_crate = "rustok-demo-admin"
                route_segment = "demo"
                nav_label = "Demo"

                [provides.admin_ui.i18n]
                default_locale = "en"
                supported_locales = ["en", "ru"]
                leptos_locales_path = "admin/locales"

                [provides.storefront_ui]
                leptos_crate = "rustok-demo-storefront"
                slot = "home_after_catalog"
                route_segment = "demo"
                page_title = "Demo"

                [provides.storefront_ui.i18n]
                default_locale = "en"
                supported_locales = ["en", "ru"]
                leptos_locales_path = "storefront/locales"
            "#,
    )
    .expect("module manifest should parse");

    validate_module_ui_metadata_contract("demo", &manifest).expect("valid UI metadata should pass");
}

#[test]
fn validate_module_admin_surface_contract_rejects_surfaces_without_admin_ui() {
    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "no_ui"
                recommended_admin_surfaces = ["leptos-admin"]
            "#,
    )
    .expect("module manifest should parse");

    let error = validate_module_admin_surface_contract("demo", &manifest)
        .expect_err("admin surfaces without provides.admin_ui must fail");
    assert!(error
        .to_string()
        .contains("does not declare [provides.admin_ui]"));
}

#[test]
fn validate_module_admin_surface_contract_rejects_missing_recommended_surface() {
    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "admin_only"

                [provides.admin_ui]
                leptos_crate = "rustok-demo-admin"
            "#,
    )
    .expect("module manifest should parse");

    let error = validate_module_admin_surface_contract("demo", &manifest)
        .expect_err("admin ui without recommended surface must fail");
    assert!(error
        .to_string()
        .contains("must declare at least one recommended_admin_surface"));
}

#[test]
fn validate_module_admin_surface_contract_rejects_missing_leptos_admin_recommendation() {
    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "admin_only"
                recommended_admin_surfaces = ["next-admin"]

                [provides.admin_ui]
                leptos_crate = "rustok-demo-admin"
            "#,
    )
    .expect("module manifest should parse");

    let error = validate_module_admin_surface_contract("demo", &manifest)
        .expect_err("leptos admin ui without leptos-admin recommendation must fail");
    assert!(error.to_string().contains("must include 'leptos-admin'"));
}

#[test]
fn validate_module_admin_surface_contract_accepts_leptos_admin_manifest() {
    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "admin_only"
                recommended_admin_surfaces = ["leptos-admin"]
                showcase_admin_surfaces = ["next-admin"]

                [provides.admin_ui]
                leptos_crate = "rustok-demo-admin"
            "#,
    )
    .expect("module manifest should parse");

    validate_module_admin_surface_contract("demo", &manifest)
        .expect("valid admin surface manifest should pass");
}

#[test]
fn validate_module_ui_classification_contract_rejects_surface_drift() {
    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "cart"
                name = "Cart"
                version = "1.2.3"
                description = "Default cart submodule in the ecommerce family"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "capability_only"

                [provides.storefront_ui]
                leptos_crate = "rustok-cart-storefront"
            "#,
    )
    .expect("module manifest should parse");

    let error = validate_module_ui_classification_contract("cart", &manifest)
        .expect_err("ui classification drift must fail");
    assert!(error
        .to_string()
        .contains("manifest UI surfaces resolve to 'storefront_only'"));
}

#[test]
fn extract_runtime_module_dependencies_reads_dependency_array() {
    let base = env::temp_dir().join(format!("xtask-runtime-deps-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    let lib_path = src_dir.join("lib.rs");
    std::fs::write(
        &lib_path,
        r#"
                impl RusToKModule for DemoModule {
                    fn dependencies(&self) -> &[&'static str] {
                        &["content", "taxonomy"]
                    }
                }
            "#,
    )
    .expect("temporary lib.rs should be writable");

    let dependencies = extract_runtime_module_dependencies(&base)
        .expect("dependencies should parse")
        .expect("runtime implementation should be detected");
    assert!(dependencies.contains("content"));
    assert!(dependencies.contains("taxonomy"));
    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_entry_type_contract_rejects_missing_entry_type_for_runtime_module() {
    let base = env::temp_dir().join(format!("xtask-entry-type-missing-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    let lib_path = src_dir.join("lib.rs");
    std::fs::write(
        &lib_path,
        "pub struct DemoModule;\nimpl RusToKModule for DemoModule {}\n",
    )
    .expect("temporary lib.rs should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "no_ui"
            "#,
    )
    .expect("module manifest should parse");

    let error = validate_module_entry_type_contract("demo", &manifest, &base)
        .expect_err("missing entry_type must fail");
    assert!(error
        .to_string()
        .contains("must declare [crate].entry_type"));
    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_entry_type_contract_accepts_non_runtime_module_without_entry_type() {
    let base = env::temp_dir().join(format!(
        "xtask-entry-type-capability-{}",
        std::process::id()
    ));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    let lib_path = src_dir.join("lib.rs");
    std::fs::write(&lib_path, "pub fn helper() {}\n").expect("temporary lib.rs should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "capability_only"
            "#,
    )
    .expect("module manifest should parse");

    validate_module_entry_type_contract("demo", &manifest, &base)
        .expect("capability-style module without RusToKModule impl should pass");
    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_runtime_metadata_contract_rejects_description_drift() {
    let base = env::temp_dir().join(format!("xtask-runtime-meta-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    let lib_path = src_dir.join("lib.rs");
    std::fs::write(
        &lib_path,
        r#"
                pub struct DemoModule;
                impl RusToKModule for DemoModule {
                    fn slug(&self) -> &'static str { "demo" }
                    fn name(&self) -> &'static str { "Demo" }
                    fn description(&self) -> &'static str { "Runtime description" }
                }
            "#,
    )
    .expect("temporary lib.rs should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "Manifest description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "no_ui"

                [crate]
                entry_type = "DemoModule"
            "#,
    )
    .expect("module manifest should parse");

    let error = validate_module_runtime_metadata_contract("demo", &manifest, &base)
        .expect_err("description drift must fail");
    assert!(error.to_string().contains("description mismatch"));
    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_semantics_contract_rejects_path_module_with_non_first_party_ownership() {
    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };
    let metadata = ModulePackageMetadata {
        slug: "demo".to_string(),
        name: "Demo".to_string(),
        version: "0.1.0".to_string(),
        description: "A sufficiently long demo module description".to_string(),
        ownership: "third_party".to_string(),
        trust_level: "verified".to_string(),
        ui_classification: "no_ui".to_string(),
        recommended_admin_surfaces: Vec::new(),
        showcase_admin_surfaces: Vec::new(),
    };

    let error = validate_module_semantics_contract("demo", &spec, &metadata)
        .expect_err("path module with non-first-party ownership must fail");
    assert!(error
        .to_string()
        .contains("must declare ownership='first_party'"));
}

#[test]
fn validate_module_semantics_contract_rejects_required_module_without_core_trust_level() {
    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: true,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };
    let metadata = ModulePackageMetadata {
        slug: "demo".to_string(),
        name: "Demo".to_string(),
        version: "0.1.0".to_string(),
        description: "A sufficiently long demo module description".to_string(),
        ownership: "first_party".to_string(),
        trust_level: "verified".to_string(),
        ui_classification: "no_ui".to_string(),
        recommended_admin_surfaces: Vec::new(),
        showcase_admin_surfaces: Vec::new(),
    };

    let error = validate_module_semantics_contract("demo", &spec, &metadata)
        .expect_err("required module without core trust level must fail");
    assert!(error
        .to_string()
        .contains("must declare trust_level='core'"));
}

#[test]
fn validate_module_semantics_contract_rejects_optional_module_with_core_trust_level() {
    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };
    let metadata = ModulePackageMetadata {
        slug: "demo".to_string(),
        name: "Demo".to_string(),
        version: "0.1.0".to_string(),
        description: "A sufficiently long demo module description".to_string(),
        ownership: "first_party".to_string(),
        trust_level: "core".to_string(),
        ui_classification: "no_ui".to_string(),
        recommended_admin_surfaces: Vec::new(),
        showcase_admin_surfaces: Vec::new(),
    };

    let error = validate_module_semantics_contract("demo", &spec, &metadata)
        .expect_err("optional module with core trust level must fail");
    assert!(error
        .to_string()
        .contains("must not declare trust_level='core'"));
}

#[test]
fn validate_module_kind_contract_rejects_required_module_without_core_kind() {
    let base = env::temp_dir().join(format!("xtask-kind-required-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    let lib_path = src_dir.join("lib.rs");
    std::fs::write(
        &lib_path,
        r#"
                pub struct DemoModule;
                impl RusToKModule for DemoModule {
                    fn kind(&self) -> ModuleKind { ModuleKind::Optional }
                }
            "#,
    )
    .expect("temporary lib.rs should be writable");

    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: true,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_kind_contract("demo", &spec, &base)
        .expect_err("required module without ModuleKind::Core must fail");
    assert!(error
        .to_string()
        .contains("must declare fn kind(&self) -> ModuleKind { ModuleKind::Core }"));

    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_kind_contract_rejects_optional_module_declaring_core_kind() {
    let base = env::temp_dir().join(format!("xtask-kind-optional-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    let lib_path = src_dir.join("lib.rs");
    std::fs::write(
        &lib_path,
        r#"
                pub struct DemoModule;
                impl RusToKModule for DemoModule {
                    fn kind(&self) -> ModuleKind { ModuleKind::Core }
                }
            "#,
    )
    .expect("temporary lib.rs should be writable");

    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_kind_contract("demo", &spec, &base)
        .expect_err("optional module declaring ModuleKind::Core must fail");
    assert!(error
        .to_string()
        .contains("must not declare ModuleKind::Core"));

    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_transport_surface_contract_rejects_missing_declared_symbol() {
    let base = env::temp_dir().join(format!("xtask-surface-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    let lib_path = src_dir.join("lib.rs");
    std::fs::write(&lib_path, "pub struct ExistingQuery;\n")
        .expect("temporary lib.rs should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "no_ui"

                [crate]
                entry_type = "DemoModule"

                [provides.graphql]
                query = "graphql::MissingQuery"
            "#,
    )
    .expect("module manifest should parse");

    let error = validate_module_transport_surface_contract("demo", &manifest, &base)
        .expect_err("missing declared symbol must fail");
    assert!(error.to_string().contains("MissingQuery"));
    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_entry_type_contract_accepts_runtime_struct_with_fields() {
    let base = env::temp_dir().join(format!("xtask-entry-type-fields-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    let lib_path = src_dir.join("lib.rs");
    std::fs::write(
        &lib_path,
        r#"
                pub struct DemoModule {
                    service: usize,
                }

                impl RusToKModule for DemoModule {}
            "#,
    )
    .expect("temporary lib.rs should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "no_ui"

                [crate]
                entry_type = "DemoModule"
            "#,
    )
    .expect("module manifest should parse");

    validate_module_entry_type_contract("demo", &manifest, &base)
        .expect("runtime struct with fields should satisfy entry_type contract");

    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_server_http_surface_contract_rejects_missing_server_routes_export() {
    let base = env::temp_dir().join(format!("xtask-server-http-routes-{}", std::process::id()));
    let controller_dir = base
        .join("apps")
        .join("server")
        .join("src")
        .join("controllers")
        .join("demo");
    std::fs::create_dir_all(&controller_dir).expect("temporary server controller dir should exist");
    std::fs::write(controller_dir.join("mod.rs"), "pub mod api;\n")
        .expect("temporary controller mod.rs should be writable");
    std::fs::write(
        base.join("modules.toml"),
        "app = \"rustok-server\"\nschema = 2\n",
    )
    .expect("temporary modules.toml should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "no_ui"

                [crate]
                entry_type = "DemoModule"

                [provides.http]
                routes = "controllers::routes"
            "#,
    )
    .expect("module manifest should parse");

    let error =
        validate_module_server_http_surface_contract(&base.join("modules.toml"), "demo", &manifest)
            .expect_err("missing server routes export must fail");
    assert!(error.to_string().contains("does not export pub routes()"));

    let _ = std::fs::remove_file(controller_dir.join("mod.rs"));
    let _ = std::fs::remove_file(base.join("modules.toml"));
    let _ = std::fs::remove_dir(&controller_dir);
    let _ = std::fs::remove_dir(
        base.join("apps")
            .join("server")
            .join("src")
            .join("controllers"),
    );
    let _ = std::fs::remove_dir(base.join("apps").join("server").join("src"));
    let _ = std::fs::remove_dir(base.join("apps").join("server"));
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_server_http_surface_contract_accepts_reexported_routes_and_webhooks() {
    let base = env::temp_dir().join(format!("xtask-server-http-reexport-{}", std::process::id()));
    let controllers_dir = base
        .join("apps")
        .join("server")
        .join("src")
        .join("controllers");
    std::fs::create_dir_all(&controllers_dir)
        .expect("temporary server controllers dir should exist");
    std::fs::write(
        controllers_dir.join("demo.rs"),
        "pub use rustok_demo::controllers::*;\n",
    )
    .expect("temporary controller re-export should be writable");
    std::fs::write(
        base.join("modules.toml"),
        "app = \"rustok-server\"\nschema = 2\n",
    )
    .expect("temporary modules.toml should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "no_ui"

                [crate]
                entry_type = "DemoModule"

                [provides.http]
                routes = "controllers::routes"
                webhook_routes = "controllers::webhook_routes"
            "#,
    )
    .expect("module manifest should parse");

    validate_module_server_http_surface_contract(&base.join("modules.toml"), "demo", &manifest)
        .expect("re-exported controller shim should satisfy host HTTP contract");

    let _ = std::fs::remove_file(controllers_dir.join("demo.rs"));
    let _ = std::fs::remove_file(base.join("modules.toml"));
    let _ = std::fs::remove_dir(&controllers_dir);
    let _ = std::fs::remove_dir(base.join("apps").join("server").join("src"));
    let _ = std::fs::remove_dir(base.join("apps").join("server"));
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_server_registry_contract_rejects_missing_server_feature() {
    let base = env::temp_dir().join(format!(
        "xtask-server-registry-missing-{}",
        std::process::id()
    ));
    let module_root = base.join("crates").join("demo-module");
    let src_dir = module_root.join("src");
    let server_dir = base.join("apps").join("server");
    let server_modules_dir = server_dir.join("src").join("modules");
    std::fs::create_dir_all(&src_dir).expect("temporary module src dir should exist");
    std::fs::create_dir_all(&server_modules_dir)
        .expect("temporary server modules dir should exist");

    std::fs::write(
        src_dir.join("lib.rs"),
        "pub struct DemoModule;\nimpl RusToKModule for DemoModule {}\n",
    )
    .expect("temporary lib.rs should be writable");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = []
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    std::fs::write(
        server_modules_dir.join("mod.rs"),
        "pub fn build_registry() {}\n",
    )
    .expect("temporary server modules mod.rs should be writable");
    std::fs::write(
        base.join("modules.toml"),
        "app = \"rustok-server\"\nschema = 2\n",
    )
    .expect("temporary modules.toml should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "capability_only"

                [crate]
                entry_type = "DemoModule"
            "#,
    )
    .expect("module manifest should parse");
    let spec = ModuleSpec {
        crate_name: "demo-module".to_string(),
        source: "path".to_string(),
        path: Some("crates/demo-module".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_server_registry_contract(
        &base.join("modules.toml"),
        "demo",
        &spec,
        &manifest,
        &module_root,
    )
    .expect_err("missing server feature must fail");
    assert!(error.to_string().contains("must expose feature 'mod-demo'"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(server_modules_dir.join("mod.rs"));
    let _ = std::fs::remove_file(base.join("modules.toml"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&module_root);
    let _ = std::fs::remove_dir(&server_modules_dir);
    let _ = std::fs::remove_dir(server_dir.join("src"));
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_server_registry_contract_accepts_optional_runtime_module() {
    let base = env::temp_dir().join(format!("xtask-server-registry-ok-{}", std::process::id()));
    let module_root = base.join("crates").join("alloy");
    let src_dir = module_root.join("src");
    let server_dir = base.join("apps").join("server");
    let server_modules_dir = server_dir.join("src").join("modules");
    std::fs::create_dir_all(&src_dir).expect("temporary module src dir should exist");
    std::fs::create_dir_all(&server_modules_dir)
        .expect("temporary server modules dir should exist");

    std::fs::write(
        src_dir.join("lib.rs"),
        "pub struct AlloyModule;\nimpl RusToKModule for AlloyModule {}\n",
    )
    .expect("temporary lib.rs should be writable");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = []
                mod-alloy = ["dep:alloy"]
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    std::fs::write(
        server_modules_dir.join("mod.rs"),
        "pub fn build_registry() {}\n",
    )
    .expect("temporary server modules mod.rs should be writable");
    std::fs::write(
        base.join("modules.toml"),
        "app = \"rustok-server\"\nschema = 2\n",
    )
    .expect("temporary modules.toml should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "alloy"
                name = "Alloy"
                version = "0.1.0"
                description = "A sufficiently long alloy module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "capability_only"

                [crate]
                entry_type = "AlloyModule"
            "#,
    )
    .expect("module manifest should parse");
    let spec = ModuleSpec {
        crate_name: "alloy".to_string(),
        source: "path".to_string(),
        path: Some("crates/alloy".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    validate_module_server_registry_contract(
        &base.join("modules.toml"),
        "alloy",
        &spec,
        &manifest,
        &module_root,
    )
    .expect("optional runtime module should map into server registry");

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(server_modules_dir.join("mod.rs"));
    let _ = std::fs::remove_file(base.join("modules.toml"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&module_root);
    let _ = std::fs::remove_dir(&server_modules_dir);
    let _ = std::fs::remove_dir(server_dir.join("src"));
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_server_registry_contract_accepts_capability_only_always_linked_module() {
    let base = env::temp_dir().join(format!("xtask-server-registry-flex-{}", std::process::id()));
    let module_root = base.join("crates").join("flex");
    let src_dir = module_root.join("src");
    let server_dir = base.join("apps").join("server");
    let server_modules_dir = server_dir.join("src").join("modules");
    std::fs::create_dir_all(&src_dir).expect("temporary module src dir should exist");
    std::fs::create_dir_all(&server_modules_dir)
        .expect("temporary server modules dir should exist");

    std::fs::write(
        src_dir.join("lib.rs"),
        "pub struct FlexModule;\nimpl RusToKModule for FlexModule {}\n",
    )
    .expect("temporary lib.rs should be writable");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = []
                mod-flex = []
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    std::fs::write(
        server_modules_dir.join("mod.rs"),
        "pub fn build_registry() {}\n",
    )
    .expect("temporary server modules mod.rs should be writable");
    std::fs::write(
        base.join("modules.toml"),
        "app = \"rustok-server\"\nschema = 2\n",
    )
    .expect("temporary modules.toml should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "flex"
                name = "Flex"
                version = "0.1.0"
                description = "A sufficiently long flex capability module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "capability_only"

                [crate]
                entry_type = "FlexModule"
            "#,
    )
    .expect("module manifest should parse");
    let spec = ModuleSpec {
        crate_name: "flex".to_string(),
        source: "path".to_string(),
        path: Some("crates/flex".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    validate_module_server_registry_contract(
        &base.join("modules.toml"),
        "flex",
        &spec,
        &manifest,
        &module_root,
    )
    .expect("capability-only always-linked module should satisfy server registry contract");

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(server_modules_dir.join("mod.rs"));
    let _ = std::fs::remove_file(base.join("modules.toml"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&module_root);
    let _ = std::fs::remove_dir(&server_modules_dir);
    let _ = std::fs::remove_dir(server_dir.join("src"));
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_server_registry_contract_rejects_required_module_missing_direct_registration() {
    let base = env::temp_dir().join(format!(
        "xtask-server-registry-required-{}",
        std::process::id()
    ));
    let module_root = base.join("crates").join("rustok-auth");
    let src_dir = module_root.join("src");
    let server_dir = base.join("apps").join("server");
    let server_modules_dir = server_dir.join("src").join("modules");
    std::fs::create_dir_all(&src_dir).expect("temporary module src dir should exist");
    std::fs::create_dir_all(&server_modules_dir)
        .expect("temporary server modules dir should exist");

    std::fs::write(
            src_dir.join("lib.rs"),
            "pub struct AuthModule;\nimpl RusToKModule for AuthModule { fn kind(&self) -> ModuleKind { ModuleKind::Core } }\n",
        )
        .expect("temporary lib.rs should be writable");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = []
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    std::fs::write(
        server_modules_dir.join("mod.rs"),
        "pub fn build_registry() {}\n",
    )
    .expect("temporary server modules mod.rs should be writable");
    std::fs::write(
        base.join("modules.toml"),
        "app = \"rustok-server\"\nschema = 2\n",
    )
    .expect("temporary modules.toml should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "auth"
                name = "Auth"
                version = "0.1.0"
                description = "A sufficiently long auth module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "no_ui"

                [crate]
                entry_type = "AuthModule"
            "#,
    )
    .expect("module manifest should parse");
    let spec = ModuleSpec {
        crate_name: "rustok-auth".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-auth".to_string()),
        required: true,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_server_registry_contract(
        &base.join("modules.toml"),
        "auth",
        &spec,
        &manifest,
        &module_root,
    )
    .expect_err("required module missing direct registration must fail");
    assert!(error
        .to_string()
        .contains("must be registered directly in apps/server/src/modules/mod.rs"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(server_modules_dir.join("mod.rs"));
    let _ = std::fs::remove_file(base.join("modules.toml"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&module_root);
    let _ = std::fs::remove_dir(&server_modules_dir);
    let _ = std::fs::remove_dir(server_dir.join("src"));
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_server_registry_contract_rejects_optional_module_direct_registration() {
    let base = env::temp_dir().join(format!(
        "xtask-server-registry-direct-optional-{}",
        std::process::id()
    ));
    let module_root = base.join("crates").join("alloy");
    let src_dir = module_root.join("src");
    let server_dir = base.join("apps").join("server");
    let server_modules_dir = server_dir.join("src").join("modules");
    std::fs::create_dir_all(&src_dir).expect("temporary module src dir should exist");
    std::fs::create_dir_all(&server_modules_dir)
        .expect("temporary server modules dir should exist");

    std::fs::write(
        src_dir.join("lib.rs"),
        "pub struct AlloyModule;\nimpl RusToKModule for AlloyModule {}\n",
    )
    .expect("temporary lib.rs should be writable");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = []
                mod-alloy = ["dep:alloy"]
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    std::fs::write(
        server_modules_dir.join("mod.rs"),
        "pub fn build_registry() { let _ = ModuleRegistry::new().register(AlloyModule); }\n",
    )
    .expect("temporary server modules mod.rs should be writable");
    std::fs::write(
        base.join("modules.toml"),
        "app = \"rustok-server\"\nschema = 2\n",
    )
    .expect("temporary modules.toml should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "alloy"
                name = "Alloy"
                version = "0.1.0"
                description = "A sufficiently long alloy module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "capability_only"

                [crate]
                entry_type = "AlloyModule"
            "#,
    )
    .expect("module manifest should parse");
    let spec = ModuleSpec {
        crate_name: "alloy".to_string(),
        source: "path".to_string(),
        path: Some("crates/alloy".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_server_registry_contract(
        &base.join("modules.toml"),
        "alloy",
        &spec,
        &manifest,
        &module_root,
    )
    .expect_err("optional module direct registration must fail");
    assert!(error
        .to_string()
        .contains("must not be registered directly"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(server_modules_dir.join("mod.rs"));
    let _ = std::fs::remove_file(base.join("modules.toml"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&module_root);
    let _ = std::fs::remove_dir(&server_modules_dir);
    let _ = std::fs::remove_dir(server_dir.join("src"));
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_server_registry_contract_rejects_server_feature_dependency_drift() {
    let base = env::temp_dir().join(format!(
        "xtask-server-registry-drift-{}",
        std::process::id()
    ));
    let module_root = base.join("crates").join("blog");
    let src_dir = module_root.join("src");
    let server_dir = base.join("apps").join("server");
    let server_modules_dir = server_dir.join("src").join("modules");
    std::fs::create_dir_all(&src_dir).expect("temporary module src dir should exist");
    std::fs::create_dir_all(&server_modules_dir)
        .expect("temporary server modules dir should exist");

    std::fs::write(
            src_dir.join("lib.rs"),
            "pub struct BlogModule;\nimpl RusToKModule for BlogModule { fn dependencies(&self) -> &[&'static str] { &[\"content\", \"taxonomy\"] } }\n",
        )
        .expect("temporary lib.rs should be writable");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = []
                mod-blog = ["dep:rustok-blog", "mod-taxonomy"]
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    std::fs::write(
        server_modules_dir.join("mod.rs"),
        "pub fn build_registry() {}\n",
    )
    .expect("temporary server modules mod.rs should be writable");
    std::fs::write(
        base.join("modules.toml"),
        "app = \"rustok-server\"\nschema = 2\n",
    )
    .expect("temporary modules.toml should be writable");

    let manifest: ModulePackageManifest = toml::from_str(
        r#"
                [module]
                slug = "blog"
                name = "Blog"
                version = "0.1.0"
                description = "A sufficiently long blog module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "capability_only"

                [crate]
                entry_type = "BlogModule"

                [dependencies]
                content = {}
                taxonomy = {}
            "#,
    )
    .expect("module manifest should parse");
    let spec = ModuleSpec {
        crate_name: "rustok-blog".to_string(),
        source: "path".to_string(),
        path: Some("crates/blog".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: Some(vec!["content".to_string(), "taxonomy".to_string()]),
        features: None,
    };

    let error = validate_module_server_registry_contract(
        &base.join("modules.toml"),
        "blog",
        &spec,
        &manifest,
        &module_root,
    )
    .expect_err("missing mod-content must fail");
    assert!(error.to_string().contains("server feature graph drift"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(server_modules_dir.join("mod.rs"));
    let _ = std::fs::remove_file(base.join("modules.toml"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&module_root);
    let _ = std::fs::remove_dir(&server_modules_dir);
    let _ = std::fs::remove_dir(server_dir.join("src"));
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_default_enabled_server_contract_rejects_missing_server_default_feature() {
    let base = env::temp_dir().join(format!(
        "xtask-default-enabled-missing-{}",
        std::process::id()
    ));
    let server_dir = base.join("apps").join("server");
    std::fs::create_dir_all(&server_dir).expect("temporary server dir should exist");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = ["mod-content"]
                mod-content = ["dep:rustok-content"]
                mod-pages = ["dep:rustok-pages", "mod-content"]
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let manifest = Manifest {
        schema: 2,
        app: "rustok-server".to_string(),
        build: None,
        modules: HashMap::from([
            (
                "content".to_string(),
                ModuleSpec {
                    crate_name: "rustok-content".to_string(),
                    source: "path".to_string(),
                    path: Some("crates/rustok-content".to_string()),
                    required: false,
                    version: None,
                    git: None,
                    rev: None,
                    depends_on: None,
                    features: None,
                },
            ),
            (
                "pages".to_string(),
                ModuleSpec {
                    crate_name: "rustok-pages".to_string(),
                    source: "path".to_string(),
                    path: Some("crates/rustok-pages".to_string()),
                    required: false,
                    version: None,
                    git: None,
                    rev: None,
                    depends_on: Some(vec!["content".to_string()]),
                    features: None,
                },
            ),
        ]),
        settings: Some(super::Settings {
            default_enabled: Some(vec!["content".to_string(), "pages".to_string()]),
        }),
    };

    let error = validate_default_enabled_server_contract(&manifest_path, &manifest)
        .expect_err("missing mod-pages in server defaults must fail");
    assert!(error
        .to_string()
        .contains("default_enabled modules must be present"));

    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_default_enabled_server_contract_rejects_required_module_in_default_enabled() {
    let base = env::temp_dir().join(format!(
        "xtask-default-enabled-required-{}",
        std::process::id()
    ));
    let server_dir = base.join("apps").join("server");
    std::fs::create_dir_all(&server_dir).expect("temporary server dir should exist");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = ["mod-content"]
                mod-content = ["dep:rustok-content"]
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let manifest = Manifest {
        schema: 2,
        app: "rustok-server".to_string(),
        build: None,
        modules: HashMap::from([(
            "channel".to_string(),
            ModuleSpec {
                crate_name: "rustok-channel".to_string(),
                source: "path".to_string(),
                path: Some("crates/rustok-channel".to_string()),
                required: true,
                version: None,
                git: None,
                rev: None,
                depends_on: None,
                features: None,
            },
        )]),
        settings: Some(super::Settings {
            default_enabled: Some(vec!["channel".to_string()]),
        }),
    };

    let error = validate_default_enabled_server_contract(&manifest_path, &manifest)
        .expect_err("required modules must not appear in default_enabled");
    assert!(error
        .to_string()
        .contains("default_enabled must list only optional modules"));

    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_default_enabled_server_contract_accepts_present_server_default_features() {
    let base = env::temp_dir().join(format!("xtask-default-enabled-ok-{}", std::process::id()));
    let server_dir = base.join("apps").join("server");
    std::fs::create_dir_all(&server_dir).expect("temporary server dir should exist");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = ["mod-content", "mod-pages"]
                mod-content = ["dep:rustok-content"]
                mod-pages = ["dep:rustok-pages", "mod-content"]
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let manifest = Manifest {
        schema: 2,
        app: "rustok-server".to_string(),
        build: None,
        modules: HashMap::from([
            (
                "content".to_string(),
                ModuleSpec {
                    crate_name: "rustok-content".to_string(),
                    source: "path".to_string(),
                    path: Some("crates/rustok-content".to_string()),
                    required: false,
                    version: None,
                    git: None,
                    rev: None,
                    depends_on: None,
                    features: None,
                },
            ),
            (
                "pages".to_string(),
                ModuleSpec {
                    crate_name: "rustok-pages".to_string(),
                    source: "path".to_string(),
                    path: Some("crates/rustok-pages".to_string()),
                    required: false,
                    version: None,
                    git: None,
                    rev: None,
                    depends_on: Some(vec!["content".to_string()]),
                    features: None,
                },
            ),
        ]),
        settings: Some(super::Settings {
            default_enabled: Some(vec!["content".to_string(), "pages".to_string()]),
        }),
    };

    validate_default_enabled_server_contract(&manifest_path, &manifest)
        .expect("default_enabled slugs present in server defaults should pass");

    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_default_enabled_server_contract_rejects_missing_optional_dependency_closure() {
    let base = env::temp_dir().join(format!(
        "xtask-default-enabled-closure-{}",
        std::process::id()
    ));
    let server_dir = base.join("apps").join("server");
    std::fs::create_dir_all(&server_dir).expect("temporary server dir should exist");
    std::fs::write(
        server_dir.join("Cargo.toml"),
        r#"
                [features]
                default = ["mod-blog"]
                mod-content = ["dep:rustok-content"]
                mod-blog = ["dep:rustok-blog", "mod-content"]
            "#,
    )
    .expect("temporary server Cargo.toml should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let manifest = Manifest {
        schema: 2,
        app: "rustok-server".to_string(),
        build: None,
        modules: HashMap::from([
            (
                "content".to_string(),
                ModuleSpec {
                    crate_name: "rustok-content".to_string(),
                    source: "path".to_string(),
                    path: Some("crates/rustok-content".to_string()),
                    required: false,
                    version: None,
                    git: None,
                    rev: None,
                    depends_on: None,
                    features: None,
                },
            ),
            (
                "blog".to_string(),
                ModuleSpec {
                    crate_name: "rustok-blog".to_string(),
                    source: "path".to_string(),
                    path: Some("crates/rustok-blog".to_string()),
                    required: false,
                    version: None,
                    git: None,
                    rev: None,
                    depends_on: Some(vec!["content".to_string()]),
                    features: None,
                },
            ),
        ]),
        settings: Some(super::Settings {
            default_enabled: Some(vec!["blog".to_string()]),
        }),
    };

    let error = validate_default_enabled_server_contract(&manifest_path, &manifest)
        .expect_err("missing optional dependency closure must fail");
    assert!(error
        .to_string()
        .contains("default_enabled must include optional dependency closure"));

    let _ = std::fs::remove_file(server_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&server_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_host_ui_inventory_contract_rejects_orphan_module_ui_dependency() {
    let base = env::temp_dir().join(format!("xtask-host-inventory-{}", std::process::id()));
    let admin_dir = base.join("apps").join("admin");
    let storefront_dir = base.join("apps").join("storefront");
    let demo_admin_dir = base.join("crates").join("rustok-demo").join("admin");
    let demo_root = base.join("crates").join("rustok-demo");
    std::fs::create_dir_all(&admin_dir).expect("temporary admin dir should exist");
    std::fs::create_dir_all(&storefront_dir).expect("temporary storefront dir should exist");
    std::fs::create_dir_all(&demo_admin_dir).expect("temporary demo admin dir should exist");

    std::fs::write(
            admin_dir.join("Cargo.toml"),
            r#"
                [features]
                hydrate = ["rustok-demo-admin/hydrate"]
                ssr = ["rustok-demo-admin/ssr"]

                [dependencies]
                rustok-demo-admin = { path = "../../crates/rustok-demo/admin", default-features = false }
            "#,
        )
        .expect("temporary admin Cargo.toml should be writable");
    std::fs::write(storefront_dir.join("Cargo.toml"), "[dependencies]\n")
        .expect("temporary storefront Cargo.toml should be writable");
    std::fs::write(
        demo_admin_dir.join("Cargo.toml"),
        r#"
                [package]
                name = "rustok-demo-admin"
                version = "0.1.0"
            "#,
    )
    .expect("temporary demo admin Cargo.toml should be writable");
    std::fs::write(
        demo_root.join("rustok-module.toml"),
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "no_ui"
            "#,
    )
    .expect("temporary rustok-module.toml should be writable");

    let manifest_path = base.join("modules.toml");
    std::fs::write(
            &manifest_path,
            "app = \"rustok-server\"\nschema = 2\n[modules]\ndemo = { crate = \"rustok-demo\", source = \"path\", path = \"crates/rustok-demo\" }\n",
        )
        .expect("temporary modules.toml should be writable");
    let manifest = load_manifest_from(&manifest_path).expect("manifest should parse");

    let error = validate_host_ui_inventory_contract(&manifest_path, &manifest)
        .expect_err("orphan module ui dependency must fail");
    assert!(error
        .to_string()
        .contains("but no module manifest declares it as admin UI"));

    let _ = std::fs::remove_file(admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(storefront_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(demo_admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(demo_root.join("rustok-module.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&demo_admin_dir);
    let _ = std::fs::remove_dir(&demo_root);
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&admin_dir);
    let _ = std::fs::remove_dir(&storefront_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_host_ui_inventory_contract_ignores_support_ui_dependency_without_module_manifest() {
    let base = env::temp_dir().join(format!(
        "xtask-host-inventory-support-{}",
        std::process::id()
    ));
    let admin_dir = base.join("apps").join("admin");
    let storefront_dir = base.join("apps").join("storefront");
    let support_admin_dir = base.join("crates").join("rustok-ai").join("admin");
    std::fs::create_dir_all(&admin_dir).expect("temporary admin dir should exist");
    std::fs::create_dir_all(&storefront_dir).expect("temporary storefront dir should exist");
    std::fs::create_dir_all(&support_admin_dir).expect("temporary support admin dir should exist");

    std::fs::write(
            admin_dir.join("Cargo.toml"),
            r#"
                [features]
                hydrate = ["rustok-ai-admin/hydrate"]
                ssr = ["rustok-ai-admin/ssr"]

                [dependencies]
                rustok-ai-admin = { path = "../../crates/rustok-ai/admin", default-features = false }
            "#,
        )
        .expect("temporary admin Cargo.toml should be writable");
    std::fs::write(storefront_dir.join("Cargo.toml"), "[dependencies]\n")
        .expect("temporary storefront Cargo.toml should be writable");
    std::fs::write(
        support_admin_dir.join("Cargo.toml"),
        r#"
                [package]
                name = "rustok-ai-admin"
                version = "0.1.0"
            "#,
    )
    .expect("temporary support admin Cargo.toml should be writable");

    let manifest_path = base.join("modules.toml");
    std::fs::write(
        &manifest_path,
        "app = \"rustok-server\"\nschema = 2\n[modules]\n",
    )
    .expect("temporary modules.toml should be writable");
    let manifest = load_manifest_from(&manifest_path).expect("manifest should parse");

    validate_host_ui_inventory_contract(&manifest_path, &manifest)
        .expect("support ui dependency without rustok-module.toml should be ignored");

    let _ = std::fs::remove_file(admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(storefront_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(support_admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&support_admin_dir);
    let _ = std::fs::remove_dir(base.join("crates").join("rustok-ai"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&admin_dir);
    let _ = std::fs::remove_dir(&storefront_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_host_ui_inventory_contract_rejects_orphan_feature_entry_for_declared_module_ui() {
    let base = env::temp_dir().join(format!(
        "xtask-host-inventory-feature-{}",
        std::process::id()
    ));
    let admin_dir = base.join("apps").join("admin");
    let storefront_dir = base.join("apps").join("storefront");
    let demo_root = base.join("crates").join("rustok-demo");
    std::fs::create_dir_all(&admin_dir).expect("temporary admin dir should exist");
    std::fs::create_dir_all(&storefront_dir).expect("temporary storefront dir should exist");
    std::fs::create_dir_all(&demo_root).expect("temporary demo root should exist");

    std::fs::write(
        admin_dir.join("Cargo.toml"),
        r#"
                [features]
                hydrate = []
                ssr = ["rustok-demo-admin/ssr"]

                [dependencies]
            "#,
    )
    .expect("temporary admin Cargo.toml should be writable");
    std::fs::write(storefront_dir.join("Cargo.toml"), "[dependencies]\n")
        .expect("temporary storefront Cargo.toml should be writable");
    std::fs::write(
        demo_root.join("rustok-module.toml"),
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "admin_only"

                [provides.admin_ui]
                leptos_crate = "rustok-demo-admin"
            "#,
    )
    .expect("temporary rustok-module.toml should be writable");

    let manifest_path = base.join("modules.toml");
    std::fs::write(
            &manifest_path,
            "app = \"rustok-server\"\nschema = 2\n[modules]\ndemo = { crate = \"rustok-demo\", source = \"path\", path = \"crates/rustok-demo\" }\n",
        )
        .expect("temporary modules.toml should be writable");
    let manifest = load_manifest_from(&manifest_path).expect("manifest should parse");

    let error = validate_host_ui_inventory_contract(&manifest_path, &manifest)
        .expect_err("orphan feature entry must fail");
    assert!(error
        .to_string()
        .contains("feature 'ssr' references 'rustok-demo-admin/ssr'"));

    let _ = std::fs::remove_file(admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(storefront_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(demo_root.join("rustok-module.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&demo_root);
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&admin_dir);
    let _ = std::fs::remove_dir(&storefront_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_server_event_runtime_contract_rejects_legacy_dispatcher_references() {
    let base = env::temp_dir().join(format!(
        "xtask-server-event-runtime-legacy-{}",
        std::process::id()
    ));
    let services_dir = base
        .join("apps")
        .join("server")
        .join("src")
        .join("services");
    std::fs::create_dir_all(&services_dir).expect("temporary services dir should exist");
    std::fs::write(
        services_dir.join("app_runtime.rs"),
        "fn bootstrap() { spawn_index_dispatcher(); let _ = WorkflowCronScheduler::new(todo!()); }",
    )
    .expect("temporary app_runtime.rs should be writable");
    std::fs::write(
        services_dir.join("mod.rs"),
        "pub mod module_event_dispatcher;\n",
    )
    .expect("temporary services mod.rs should be writable");
    std::fs::write(
        services_dir.join("module_event_dispatcher.rs"),
        "pub fn spawn_module_event_dispatcher() {}\n",
    )
    .expect("temporary module_event_dispatcher.rs should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let error = validate_server_event_runtime_contract(&manifest_path)
        .expect_err("legacy dispatcher references must fail");
    assert!(error
        .to_string()
        .contains("legacy index/search dispatchers"));

    let _ = std::fs::remove_file(services_dir.join("app_runtime.rs"));
    let _ = std::fs::remove_file(services_dir.join("mod.rs"));
    let _ = std::fs::remove_file(services_dir.join("module_event_dispatcher.rs"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&services_dir);
    let _ = std::fs::remove_dir(base.join("apps").join("server").join("src"));
    let _ = std::fs::remove_dir(base.join("apps").join("server"));
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_server_event_runtime_contract_accepts_module_owned_dispatcher_path() {
    let base = env::temp_dir().join(format!(
        "xtask-server-event-runtime-ok-{}",
        std::process::id()
    ));
    let services_dir = base
        .join("apps")
        .join("server")
        .join("src")
        .join("services");
    std::fs::create_dir_all(&services_dir).expect("temporary services dir should exist");
    std::fs::write(
            services_dir.join("app_runtime.rs"),
            "use crate::services::module_event_dispatcher::spawn_module_event_dispatcher;\nfn bootstrap() { spawn_module_event_dispatcher(); }\nfn workflow() { let _ = WorkflowCronScheduler::new(todo!()); }",
        )
        .expect("temporary app_runtime.rs should be writable");
    std::fs::write(
        services_dir.join("mod.rs"),
        "pub mod module_event_dispatcher;\n",
    )
    .expect("temporary services mod.rs should be writable");
    std::fs::write(
        services_dir.join("module_event_dispatcher.rs"),
        "pub fn spawn_module_event_dispatcher() {}\n",
    )
    .expect("temporary module_event_dispatcher.rs should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    validate_server_event_runtime_contract(&manifest_path)
        .expect("module-owned dispatcher path should validate");

    let _ = std::fs::remove_file(services_dir.join("app_runtime.rs"));
    let _ = std::fs::remove_file(services_dir.join("mod.rs"));
    let _ = std::fs::remove_file(services_dir.join("module_event_dispatcher.rs"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&services_dir);
    let _ = std::fs::remove_dir(base.join("apps").join("server").join("src"));
    let _ = std::fs::remove_dir(base.join("apps").join("server"));
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_host_ui_contract_rejects_missing_admin_feature_wiring() {
    let base = env::temp_dir().join(format!("xtask-host-ui-missing-{}", std::process::id()));
    let admin_dir = base.join("apps").join("admin");
    let ui_crate_dir = base.join("crates").join("rustok-demo").join("admin");
    std::fs::create_dir_all(&admin_dir).expect("temporary admin dir should exist");
    std::fs::create_dir_all(&ui_crate_dir).expect("temporary ui crate dir should exist");

    std::fs::write(
            admin_dir.join("Cargo.toml"),
            r#"
                [features]
                hydrate = []
                ssr = []

                [dependencies]
                rustok-demo-admin = { path = "../../crates/rustok-demo/admin", default-features = false }
            "#,
        )
        .expect("temporary admin Cargo.toml should be writable");
    std::fs::write(
        ui_crate_dir.join("Cargo.toml"),
        r#"
                [package]
                name = "rustok-demo-admin"
                version = "0.1.0"

                [features]
                hydrate = []
                ssr = []
            "#,
    )
    .expect("temporary ui crate Cargo.toml should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(
            &manifest_path,
            "app = \"rustok-server\"\nschema = 2\n[modules]\ndemo = { crate = \"rustok-demo\", source = \"path\", path = \"crates/rustok-demo\" }\n",
        )
            .expect("temporary modules.toml should be writable");
    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_host_ui_contract(
        &manifest_path,
        "demo",
        &spec,
        Some("rustok-demo-admin"),
        None,
    )
    .expect_err("missing host ui feature wiring must fail");
    assert!(error
        .to_string()
        .contains("feature 'hydrate' is missing 'rustok-demo-admin/hydrate'"));

    let _ = std::fs::remove_file(admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(ui_crate_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&ui_crate_dir);
    let _ = std::fs::remove_dir(base.join("crates").join("rustok-demo"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&admin_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_host_ui_contract_accepts_storefront_ssr_wiring() {
    let base = env::temp_dir().join(format!("xtask-host-ui-ok-{}", std::process::id()));
    let storefront_dir = base.join("apps").join("storefront");
    let ui_crate_dir = base.join("crates").join("rustok-demo").join("storefront");
    std::fs::create_dir_all(&storefront_dir).expect("temporary storefront dir should exist");
    std::fs::create_dir_all(&ui_crate_dir).expect("temporary ui crate dir should exist");

    std::fs::write(
            storefront_dir.join("Cargo.toml"),
            r#"
                [features]
                hydrate = []
                ssr = ["rustok-demo-storefront/ssr"]

                [dependencies]
                rustok-demo-storefront = { path = "../../crates/rustok-demo/storefront", default-features = false }
            "#,
        )
        .expect("temporary storefront Cargo.toml should be writable");
    std::fs::write(
        ui_crate_dir.join("Cargo.toml"),
        r#"
                [package]
                name = "rustok-demo-storefront"
                version = "0.1.0"

                [features]
                hydrate = []
                ssr = []
            "#,
    )
    .expect("temporary ui crate Cargo.toml should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(
            &manifest_path,
            "app = \"rustok-server\"\nschema = 2\n[modules]\ndemo = { crate = \"rustok-demo\", source = \"path\", path = \"crates/rustok-demo\" }\n",
        )
            .expect("temporary modules.toml should be writable");
    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    validate_module_host_ui_contract(
        &manifest_path,
        "demo",
        &spec,
        None,
        Some("rustok-demo-storefront"),
    )
    .expect("storefront ssr wiring should validate");

    let _ = std::fs::remove_file(storefront_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(ui_crate_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&ui_crate_dir);
    let _ = std::fs::remove_dir(base.join("crates").join("rustok-demo"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&storefront_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_host_ui_contract_rejects_missing_dependency_admin_ui_wiring() {
    let base = env::temp_dir().join(format!("xtask-host-ui-dependency-{}", std::process::id()));
    let admin_dir = base.join("apps").join("admin");
    let demo_admin_dir = base.join("crates").join("rustok-demo").join("admin");
    let comments_admin_dir = base.join("crates").join("rustok-comments").join("admin");
    let demo_root = base.join("crates").join("rustok-demo");
    let comments_root = base.join("crates").join("rustok-comments");
    std::fs::create_dir_all(&admin_dir).expect("temporary admin dir should exist");
    std::fs::create_dir_all(&demo_admin_dir).expect("temporary demo admin dir should exist");
    std::fs::create_dir_all(&comments_admin_dir)
        .expect("temporary comments admin dir should exist");

    std::fs::write(
            admin_dir.join("Cargo.toml"),
            r#"
                [features]
                hydrate = ["rustok-demo-admin/hydrate"]
                ssr = ["rustok-demo-admin/ssr"]

                [dependencies]
                rustok-demo-admin = { path = "../../crates/rustok-demo/admin", default-features = false }
            "#,
        )
        .expect("temporary admin Cargo.toml should be writable");
    std::fs::write(
        demo_admin_dir.join("Cargo.toml"),
        r#"
                [package]
                name = "rustok-demo-admin"
                version = "0.1.0"

                [features]
                hydrate = []
                ssr = []
            "#,
    )
    .expect("temporary demo admin Cargo.toml should be writable");
    std::fs::write(
        comments_admin_dir.join("Cargo.toml"),
        r#"
                [package]
                name = "rustok-comments-admin"
                version = "0.1.0"

                [features]
                hydrate = []
                ssr = []
            "#,
    )
    .expect("temporary comments admin Cargo.toml should be writable");
    std::fs::write(
        demo_root.join("rustok-module.toml"),
        r#"
                [module]
                slug = "demo"
                name = "Demo"
                version = "0.1.0"
                description = "A sufficiently long demo module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "admin_only"

                [provides.admin_ui]
                leptos_crate = "rustok-demo-admin"
            "#,
    )
    .expect("temporary demo rustok-module.toml should be writable");
    std::fs::write(
        comments_root.join("rustok-module.toml"),
        r#"
                [module]
                slug = "comments"
                name = "Comments"
                version = "0.1.0"
                description = "A sufficiently long comments module description"
                ownership = "first_party"
                trust_level = "verified"
                ui_classification = "admin_only"

                [provides.admin_ui]
                leptos_crate = "rustok-comments-admin"
            "#,
    )
    .expect("temporary comments rustok-module.toml should be writable");

    let manifest_path = base.join("modules.toml");
    std::fs::write(
            &manifest_path,
            "app = \"rustok-server\"\nschema = 2\n[modules]\ndemo = { crate = \"rustok-demo\", source = \"path\", path = \"crates/rustok-demo\", depends_on = [\"comments\"] }\ncomments = { crate = \"rustok-comments\", source = \"path\", path = \"crates/rustok-comments\" }\n",
        )
        .expect("temporary modules.toml should be writable");
    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: Some(vec!["comments".to_string()]),
        features: None,
    };

    let error = validate_module_host_ui_contract(
        &manifest_path,
        "demo",
        &spec,
        Some("rustok-demo-admin"),
        None,
    )
    .expect_err("missing dependency admin UI wiring must fail");
    assert!(error
        .to_string()
        .contains("missing UI dependency from module 'comments'"));

    let _ = std::fs::remove_file(admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(demo_admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(comments_admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(demo_root.join("rustok-module.toml"));
    let _ = std::fs::remove_file(comments_root.join("rustok-module.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&demo_admin_dir);
    let _ = std::fs::remove_dir(&comments_admin_dir);
    let _ = std::fs::remove_dir(&demo_root);
    let _ = std::fs::remove_dir(&comments_root);
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&admin_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_host_ui_contract_rejects_non_canonical_ui_dependency_path() {
    let base = env::temp_dir().join(format!("xtask-host-ui-canonical-{}", std::process::id()));
    let admin_dir = base.join("apps").join("admin");
    let demo_admin_dir = base.join("crates").join("rustok-demo").join("admin");
    let wrong_admin_dir = base.join("crates").join("wrong-demo").join("admin");
    std::fs::create_dir_all(&admin_dir).expect("temporary admin dir should exist");
    std::fs::create_dir_all(&demo_admin_dir).expect("temporary demo admin dir should exist");
    std::fs::create_dir_all(&wrong_admin_dir).expect("temporary wrong admin dir should exist");

    std::fs::write(
            admin_dir.join("Cargo.toml"),
            r#"
                [features]
                hydrate = ["rustok-demo-admin/hydrate"]
                ssr = ["rustok-demo-admin/ssr"]

                [dependencies]
                rustok-demo-admin = { path = "../../crates/wrong-demo/admin", default-features = false }
            "#,
        )
        .expect("temporary admin Cargo.toml should be writable");
    std::fs::write(
        demo_admin_dir.join("Cargo.toml"),
        r#"
                [package]
                name = "rustok-demo-admin"
                version = "0.1.0"

                [features]
                hydrate = []
                ssr = []
            "#,
    )
    .expect("temporary demo admin Cargo.toml should be writable");
    std::fs::write(
        wrong_admin_dir.join("Cargo.toml"),
        r#"
                [package]
                name = "rustok-demo-admin"
                version = "0.1.0"

                [features]
                hydrate = []
                ssr = []
            "#,
    )
    .expect("temporary wrong admin Cargo.toml should be writable");

    let manifest_path = base.join("modules.toml");
    std::fs::write(
            &manifest_path,
            "app = \"rustok-server\"\nschema = 2\n[modules]\ndemo = { crate = \"rustok-demo\", source = \"path\", path = \"crates/rustok-demo\" }\n",
        )
        .expect("temporary modules.toml should be writable");
    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_host_ui_contract(
        &manifest_path,
        "demo",
        &spec,
        Some("rustok-demo-admin"),
        None,
    )
    .expect_err("non-canonical ui dependency path must fail");
    assert!(error.to_string().contains("instead of canonical"));

    let _ = std::fs::remove_file(admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(demo_admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(wrong_admin_dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&demo_admin_dir);
    let _ = std::fs::remove_dir(base.join("crates").join("rustok-demo"));
    let _ = std::fs::remove_dir(&wrong_admin_dir);
    let _ = std::fs::remove_dir(base.join("crates").join("wrong-demo"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&admin_dir);
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_permission_contract_rejects_unknown_permission_constant() {
    let base = env::temp_dir().join(format!("xtask-permissions-unknown-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    std::fs::write(
        src_dir.join("lib.rs"),
        r#"
                pub struct DemoModule;
                impl RusToKModule for DemoModule {
                    fn permissions(&self) -> Vec<Permission> {
                        vec![Permission::DOES_NOT_EXIST]
                    }
                }
            "#,
    )
    .expect("temporary lib.rs should be writable");

    let error = validate_module_permission_contract("demo", &base)
        .expect_err("unknown permission constant must fail");
    assert!(error
        .to_string()
        .contains("unknown Permission::DOES_NOT_EXIST"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_permission_contract_rejects_duplicate_permission_semantics() {
    let base = env::temp_dir().join(format!(
        "xtask-permissions-duplicate-{}",
        std::process::id()
    ));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    std::fs::write(
        src_dir.join("lib.rs"),
        r#"
                pub struct DemoModule;
                impl RusToKModule for DemoModule {
                    fn permissions(&self) -> Vec<Permission> {
                        vec![
                            Permission::USERS_READ,
                            Permission::new(Resource::Users, Action::Read),
                        ]
                    }
                }
            "#,
    )
    .expect("temporary lib.rs should be writable");

    let error = validate_module_permission_contract("demo", &base)
        .expect_err("duplicate permission semantics must fail");
    assert!(error
        .to_string()
        .contains("duplicate permission 'users:read'"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_permission_contract_rejects_missing_minimum_runtime_permission() {
    let base = env::temp_dir().join(format!("xtask-permissions-minimum-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    std::fs::write(
        src_dir.join("lib.rs"),
        r#"
                pub struct BlogModule;
                impl RusToKModule for BlogModule {
                    fn permissions(&self) -> Vec<Permission> {
                        vec![Permission::BLOG_POSTS_READ]
                    }
                }
            "#,
    )
    .expect("temporary lib.rs should be writable");

    let error = validate_module_permission_contract("blog", &base)
        .expect_err("missing minimum runtime permission must fail");
    assert!(error
        .to_string()
        .contains("must declare minimum runtime permission 'blog_posts:manage'"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_event_listener_contract_rejects_missing_registration_hook() {
    let base = env::temp_dir().join(format!(
        "xtask-event-listener-missing-{}",
        std::process::id()
    ));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    std::fs::write(
        src_dir.join("lib.rs"),
        r#"
                pub struct SearchModule;

                impl SearchModule {
                    pub fn new() -> Self {
                        Self
                    }
                }
            "#,
    )
    .expect("temporary lib.rs should be writable");

    let error = validate_module_event_listener_contract("search", &base)
        .expect_err("missing register_event_listeners hook must fail");
    assert!(error.to_string().contains("register_event_listeners"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_event_listener_contract_accepts_index_runtime_registration() {
    let base = env::temp_dir().join(format!("xtask-event-listener-ok-{}", std::process::id()));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    std::fs::write(
        src_dir.join("lib.rs"),
        r#"
                pub struct IndexModule;

                impl IndexModule {
                    pub fn register_event_listeners(&self) {
                        let runtime = IndexerRuntimeConfig::new(2, 100, 10);
                        let _content = ContentIndexer::with_runtime(todo!(), runtime.clone());
                        let _product = ProductIndexer::with_runtime(todo!(), runtime);
                    }
                }
            "#,
    )
    .expect("temporary lib.rs should be writable");

    validate_module_event_listener_contract("index", &base)
        .expect("index event listener fragments should validate");

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_event_ingress_contract_rejects_workflow_without_webhook_routes() {
    let base = env::temp_dir().join(format!(
        "xtask-event-ingress-missing-{}",
        std::process::id()
    ));
    let workflow_controllers_dir = base
        .join("crates")
        .join("rustok-workflow")
        .join("src")
        .join("controllers");
    let workflow_services_dir = base
        .join("crates")
        .join("rustok-workflow")
        .join("src")
        .join("services");
    let server_workflow_dir = base
        .join("apps")
        .join("server")
        .join("src")
        .join("controllers")
        .join("workflow");
    std::fs::create_dir_all(&workflow_controllers_dir)
        .expect("temporary workflow controllers dir should exist");
    std::fs::create_dir_all(&workflow_services_dir)
        .expect("temporary workflow services dir should exist");
    std::fs::create_dir_all(&server_workflow_dir)
        .expect("temporary server workflow dir should exist");
    std::fs::write(
        workflow_controllers_dir.join("mod.rs"),
        "pub fn routes() {}\n",
    )
    .expect("temporary workflow controllers mod.rs should be writable");
    std::fs::write(
        workflow_services_dir.join("trigger_handler.rs"),
        "impl EventHandler for WorkflowTriggerHandler {}\n",
    )
    .expect("temporary trigger_handler.rs should be writable");
    std::fs::write(
        server_workflow_dir.join("mod.rs"),
        "pub fn webhook_routes() -> Routes { rustok_workflow::controllers::webhook_routes() }\n",
    )
    .expect("temporary server workflow shim should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");
    let module_root = base.join("crates").join("rustok-workflow");

    let error = validate_module_event_ingress_contract(&manifest_path, "workflow", &module_root)
        .expect_err("missing workflow webhook routes must fail");
    assert!(error.to_string().contains("webhook_routes"));

    let _ = std::fs::remove_file(workflow_controllers_dir.join("mod.rs"));
    let _ = std::fs::remove_file(workflow_services_dir.join("trigger_handler.rs"));
    let _ = std::fs::remove_file(server_workflow_dir.join("mod.rs"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&workflow_controllers_dir);
    let _ = std::fs::remove_dir(&workflow_services_dir);
    let _ = std::fs::remove_dir(base.join("crates").join("rustok-workflow").join("src"));
    let _ = std::fs::remove_dir(base.join("crates").join("rustok-workflow"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&server_workflow_dir);
    let _ = std::fs::remove_dir(
        base.join("apps")
            .join("server")
            .join("src")
            .join("controllers"),
    );
    let _ = std::fs::remove_dir(base.join("apps").join("server").join("src"));
    let _ = std::fs::remove_dir(base.join("apps").join("server"));
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_event_ingress_contract_accepts_workflow_webhook_shim() {
    let base = env::temp_dir().join(format!("xtask-event-ingress-ok-{}", std::process::id()));
    let workflow_controllers_dir = base
        .join("crates")
        .join("rustok-workflow")
        .join("src")
        .join("controllers");
    let workflow_services_dir = base
        .join("crates")
        .join("rustok-workflow")
        .join("src")
        .join("services");
    let server_workflow_dir = base
        .join("apps")
        .join("server")
        .join("src")
        .join("controllers")
        .join("workflow");
    std::fs::create_dir_all(&workflow_controllers_dir)
        .expect("temporary workflow controllers dir should exist");
    std::fs::create_dir_all(&workflow_services_dir)
        .expect("temporary workflow services dir should exist");
    std::fs::create_dir_all(&server_workflow_dir)
        .expect("temporary server workflow dir should exist");
    std::fs::write(
        workflow_controllers_dir.join("mod.rs"),
        "pub fn routes() {}\npub fn webhook_routes() { Routes::new().prefix(\"webhooks\"); }\n",
    )
    .expect("temporary workflow controllers mod.rs should be writable");
    std::fs::write(
        workflow_services_dir.join("trigger_handler.rs"),
        "impl EventHandler for WorkflowTriggerHandler {}\n",
    )
    .expect("temporary trigger_handler.rs should be writable");
    std::fs::write(
        server_workflow_dir.join("mod.rs"),
        "pub fn webhook_routes() -> Routes { rustok_workflow::controllers::webhook_routes() }\n",
    )
    .expect("temporary server workflow shim should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");
    let module_root = base.join("crates").join("rustok-workflow");

    validate_module_event_ingress_contract(&manifest_path, "workflow", &module_root)
        .expect("workflow event ingress contract should validate");

    let _ = std::fs::remove_file(workflow_controllers_dir.join("mod.rs"));
    let _ = std::fs::remove_file(workflow_services_dir.join("trigger_handler.rs"));
    let _ = std::fs::remove_file(server_workflow_dir.join("mod.rs"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&workflow_controllers_dir);
    let _ = std::fs::remove_dir(&workflow_services_dir);
    let _ = std::fs::remove_dir(base.join("crates").join("rustok-workflow").join("src"));
    let _ = std::fs::remove_dir(base.join("crates").join("rustok-workflow"));
    let _ = std::fs::remove_dir(base.join("crates"));
    let _ = std::fs::remove_dir(&server_workflow_dir);
    let _ = std::fs::remove_dir(
        base.join("apps")
            .join("server")
            .join("src")
            .join("controllers"),
    );
    let _ = std::fs::remove_dir(base.join("apps").join("server").join("src"));
    let _ = std::fs::remove_dir(base.join("apps").join("server"));
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_index_search_boundary_contract_rejects_index_exposing_search_engine() {
    let base = env::temp_dir().join(format!(
        "xtask-index-search-boundary-bad-{}",
        std::process::id()
    ));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    std::fs::write(
            src_dir.join("lib.rs"),
            "pub use crate::search::PgSearchEngine;\npub struct IndexModule; pub struct ContentIndexer; pub struct ProductIndexer; pub struct IndexerRuntimeConfig;",
        )
        .expect("temporary lib.rs should be writable");
    std::fs::write(
            base.join("README.md"),
            "read-model substrate\nContentIndexer::with_runtime\nProductIndexer::with_runtime\nIndexerRuntimeConfig\n",
        )
        .expect("temporary README.md should be writable");

    let error = validate_module_index_search_boundary_contract("index", &base)
        .expect_err("index must not expose search-owned symbols");
    assert!(error.to_string().contains("PgSearchEngine"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(base.join("README.md"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_index_search_boundary_contract_accepts_search_surface() {
    let base = env::temp_dir().join(format!(
        "xtask-index-search-boundary-ok-{}",
        std::process::id()
    ));
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    std::fs::write(
            src_dir.join("lib.rs"),
            "pub use crate::engine::SearchEngineKind;\npub use crate::pg_engine::PgSearchEngine;\npub use crate::ingestion::SearchIngestionHandler;\n",
        )
        .expect("temporary lib.rs should be writable");
    std::fs::write(
        base.join("README.md"),
        "search_documents\nproduct-facing search contracts\n",
    )
    .expect("temporary README.md should be writable");

    validate_module_index_search_boundary_contract("search", &base)
        .expect("search boundary surface should validate");

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(base.join("README.md"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_search_operator_surface_contract_rejects_missing_readme_marker() {
    let base = env::temp_dir().join(format!(
        "xtask-search-operator-surface-missing-{}",
        std::process::id()
    ));
    let src_dir = base.join("src");
    let docs_dir = base.join("docs");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    std::fs::create_dir_all(&docs_dir).expect("temporary docs dir should exist");
    std::fs::write(
        src_dir.join("lib.rs"),
        r#"
                pub struct SearchDiagnosticsService;
                pub struct SearchAnalyticsService;
                pub struct SearchSettingsService;
                pub struct SearchDictionaryService;
            "#,
    )
    .expect("temporary lib.rs should be writable");
    std::fs::write(
        base.join("README.md"),
        "searchDiagnostics\nsearchAnalytics\nsearchSettingsPreview\n",
    )
    .expect("temporary README.md should be writable");
    std::fs::write(docs_dir.join("observability-runbook.md"), "# runbook\n")
        .expect("temporary observability-runbook.md should be writable");

    let error = validate_module_search_operator_surface_contract("search", &base)
        .expect_err("missing operator readme marker must fail");
    assert!(error.to_string().contains("triggerSearchRebuild"));

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(base.join("README.md"));
    let _ = std::fs::remove_file(docs_dir.join("observability-runbook.md"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&docs_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_search_operator_surface_contract_accepts_operator_plane() {
    let base = env::temp_dir().join(format!(
        "xtask-search-operator-surface-ok-{}",
        std::process::id()
    ));
    let src_dir = base.join("src");
    let docs_dir = base.join("docs");
    std::fs::create_dir_all(&src_dir).expect("temporary src dir should exist");
    std::fs::create_dir_all(&docs_dir).expect("temporary docs dir should exist");
    std::fs::write(
        src_dir.join("lib.rs"),
        r#"
                pub struct SearchDiagnosticsService;
                pub struct SearchAnalyticsService;
                pub struct SearchSettingsService;
                pub struct SearchDictionaryService;
            "#,
    )
    .expect("temporary lib.rs should be writable");
    std::fs::write(
        base.join("README.md"),
        "searchDiagnostics\nsearchAnalytics\nsearchSettingsPreview\ntriggerSearchRebuild\n",
    )
    .expect("temporary README.md should be writable");
    std::fs::write(docs_dir.join("observability-runbook.md"), "# runbook\n")
        .expect("temporary observability-runbook.md should be writable");

    validate_module_search_operator_surface_contract("search", &base)
        .expect("search operator-plane contract should validate");

    let _ = std::fs::remove_file(src_dir.join("lib.rs"));
    let _ = std::fs::remove_file(base.join("README.md"));
    let _ = std::fs::remove_file(docs_dir.join("observability-runbook.md"));
    let _ = std::fs::remove_dir(&src_dir);
    let _ = std::fs::remove_dir(&docs_dir);
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_docs_navigation_contract_rejects_missing_ui_navigation_link() {
    let base = env::temp_dir().join(format!("xtask-docs-nav-missing-{}", std::process::id()));
    let docs_modules_dir = base.join("docs").join("modules");
    std::fs::create_dir_all(&docs_modules_dir).expect("temporary docs/modules dir should exist");
    std::fs::write(
            docs_modules_dir.join("_index.md"),
            "| `rustok-demo` | [docs](../../crates/rustok-demo/docs/README.md) | [plan](../../crates/rustok-demo/docs/implementation-plan.md) |\n",
        )
        .expect("temporary _index.md should be writable");
    std::fs::write(docs_modules_dir.join("UI_PACKAGES_INDEX.md"), "# ui\n")
        .expect("temporary UI index should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_docs_navigation_contract(
        &manifest_path,
        "demo",
        &spec,
        Some("rustok-demo-admin"),
        None,
        &[],
    )
    .expect_err("missing admin ui link must fail");
    assert!(error.to_string().contains("declares admin UI"));

    let _ = std::fs::remove_file(docs_modules_dir.join("_index.md"));
    let _ = std::fs::remove_file(docs_modules_dir.join("UI_PACKAGES_INDEX.md"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&docs_modules_dir);
    let _ = std::fs::remove_dir(base.join("docs"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_docs_navigation_contract_accepts_documented_storefront_module() {
    let base = env::temp_dir().join(format!("xtask-docs-nav-ok-{}", std::process::id()));
    let docs_modules_dir = base.join("docs").join("modules");
    std::fs::create_dir_all(&docs_modules_dir).expect("temporary docs/modules dir should exist");
    std::fs::write(
            docs_modules_dir.join("_index.md"),
            "| `rustok-demo` | [docs](../../crates/rustok-demo/docs/README.md) | [plan](../../crates/rustok-demo/docs/implementation-plan.md) |\n",
        )
        .expect("temporary _index.md should be writable");
    std::fs::write(
        docs_modules_dir.join("UI_PACKAGES_INDEX.md"),
        "- `rustok-demo` storefront UI: [README](../../crates/rustok-demo/storefront/README.md)\n",
    )
    .expect("temporary UI index should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    validate_module_docs_navigation_contract(
        &manifest_path,
        "demo",
        &spec,
        None,
        Some("rustok-demo-storefront"),
        &[],
    )
    .expect("documented storefront UI should validate");

    let _ = std::fs::remove_file(docs_modules_dir.join("_index.md"));
    let _ = std::fs::remove_file(docs_modules_dir.join("UI_PACKAGES_INDEX.md"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&docs_modules_dir);
    let _ = std::fs::remove_dir(base.join("docs"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_docs_navigation_contract_rejects_missing_next_admin_showcase_entry() {
    let base = env::temp_dir().join(format!("xtask-docs-nav-next-admin-{}", std::process::id()));
    let docs_modules_dir = base.join("docs").join("modules");
    let next_admin_package_dir = base
        .join("apps")
        .join("next-admin")
        .join("packages")
        .join("blog");
    std::fs::create_dir_all(&docs_modules_dir).expect("temporary docs/modules dir should exist");
    std::fs::create_dir_all(&next_admin_package_dir)
        .expect("temporary next-admin package dir should exist");
    std::fs::write(
            docs_modules_dir.join("_index.md"),
            "| `rustok-blog` | [docs](../../crates/rustok-blog/docs/README.md) | [plan](../../crates/rustok-blog/docs/implementation-plan.md) |\n",
        )
        .expect("temporary _index.md should be writable");
    std::fs::write(
        docs_modules_dir.join("UI_PACKAGES_INDEX.md"),
        "- `rustok-blog` admin UI: [README](../../crates/rustok-blog/admin/README.md)\n",
    )
    .expect("temporary UI index should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let spec = ModuleSpec {
        crate_name: "rustok-blog".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-blog".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_docs_navigation_contract(
        &manifest_path,
        "blog",
        &spec,
        Some("rustok-blog-admin"),
        None,
        &["next-admin".to_string()],
    )
    .expect_err("missing next-admin showcase entry must fail");
    assert!(error
        .to_string()
        .contains("showcase_admin_surfaces=['next-admin']"));

    let _ = std::fs::remove_file(docs_modules_dir.join("_index.md"));
    let _ = std::fs::remove_file(docs_modules_dir.join("UI_PACKAGES_INDEX.md"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&docs_modules_dir);
    let _ = std::fs::remove_dir(base.join("docs"));
    let _ = std::fs::remove_dir(&next_admin_package_dir);
    let _ = std::fs::remove_dir(base.join("apps").join("next-admin").join("packages"));
    let _ = std::fs::remove_dir(base.join("apps").join("next-admin"));
    let _ = std::fs::remove_dir(base.join("apps"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_registry_docs_contract_rejects_missing_row() {
    let base = env::temp_dir().join(format!(
        "xtask-registry-docs-missing-{}",
        std::process::id()
    ));
    let docs_modules_dir = base.join("docs").join("modules");
    std::fs::create_dir_all(&docs_modules_dir).expect("temporary docs/modules dir should exist");
    std::fs::write(
        docs_modules_dir.join("registry.md"),
        "### Core-модули\n\n| Slug | Crate | Role |\n|---|---|---|\n| `auth` | `rustok-auth` | auth |\n\n### Optional-модули\n\n| Slug | Crate | Dependencies | Role |\n|---|---|---|---|\n",
    )
    .expect("temporary registry.md should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: None,
        features: None,
    };

    let error = validate_module_registry_docs_contract(&manifest_path, "demo", &spec)
        .expect_err("missing registry row must fail");
    assert!(error.to_string().contains("missing central registry row"));

    let _ = std::fs::remove_file(docs_modules_dir.join("registry.md"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&docs_modules_dir);
    let _ = std::fs::remove_dir(base.join("docs"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_registry_docs_contract_rejects_dependency_drift() {
    let base = env::temp_dir().join(format!("xtask-registry-docs-deps-{}", std::process::id()));
    let docs_modules_dir = base.join("docs").join("modules");
    std::fs::create_dir_all(&docs_modules_dir).expect("temporary docs/modules dir should exist");
    std::fs::write(
        docs_modules_dir.join("registry.md"),
        "### Core-модули\n\n| Slug | Crate | Role |\n|---|---|---|\n| `auth` | `rustok-auth` | auth |\n\n### Optional-модули\n\n| Slug | Crate | Dependencies | Role |\n|---|---|---|---|\n| `demo` | `rustok-demo` | `content` | demo |\n",
    )
    .expect("temporary registry.md should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let spec = ModuleSpec {
        crate_name: "rustok-demo".to_string(),
        source: "path".to_string(),
        path: Some("crates/rustok-demo".to_string()),
        required: false,
        version: None,
        git: None,
        rev: None,
        depends_on: Some(vec!["taxonomy".to_string()]),
        features: None,
    };

    let error = validate_module_registry_docs_contract(&manifest_path, "demo", &spec)
        .expect_err("dependency drift must fail");
    assert!(error.to_string().contains("dependency mismatch"));

    let _ = std::fs::remove_file(docs_modules_dir.join("registry.md"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&docs_modules_dir);
    let _ = std::fs::remove_dir(base.join("docs"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_central_module_registry_inventory_contract_rejects_undocumented_module_rows() {
    let base = env::temp_dir().join(format!(
        "xtask-registry-docs-undocumented-{}",
        std::process::id()
    ));
    let docs_modules_dir = base.join("docs").join("modules");
    std::fs::create_dir_all(&docs_modules_dir).expect("temporary docs/modules dir should exist");
    std::fs::write(
        docs_modules_dir.join("registry.md"),
        "### Core-модули\n\n| Slug | Crate | Role |\n|---|---|---|\n| `auth` | `rustok-auth` | auth |\n\n### Optional-модули\n\n| Slug | Crate | Dependencies | Role |\n|---|---|---|---|\n| `demo` | `rustok-demo` | — | demo |\n| `ghost` | `rustok-ghost` | — | ghost |\n",
    )
    .expect("temporary registry.md should be writable");
    let manifest_path = base.join("modules.toml");
    std::fs::write(&manifest_path, "app = \"rustok-server\"\nschema = 2\n")
        .expect("temporary modules.toml should be writable");

    let manifest = Manifest {
        schema: 2,
        app: "rustok-server".to_string(),
        build: None,
        modules: HashMap::from([(
            "demo".to_string(),
            ModuleSpec {
                crate_name: "rustok-demo".to_string(),
                source: "path".to_string(),
                path: Some("crates/rustok-demo".to_string()),
                required: false,
                version: None,
                git: None,
                rev: None,
                depends_on: None,
                features: None,
            },
        )]),
        settings: None,
    };

    let error = validate_central_module_registry_inventory_contract(&manifest_path, &manifest)
        .expect_err("extra registry rows must fail");
    assert!(error
        .to_string()
        .contains("contains module rows not present in modules.toml"));

    let _ = std::fs::remove_file(docs_modules_dir.join("registry.md"));
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&docs_modules_dir);
    let _ = std::fs::remove_dir(base.join("docs"));
    let _ = std::fs::remove_dir(&base);
}

#[test]
fn validate_module_ui_surface_contract_rejects_declared_missing_subcrate() {
    let error = validate_module_ui_surface_contract(
        "blog",
        Path::new("crates/rustok-blog"),
        "preview",
        Some("rustok-blog-preview"),
    )
    .expect_err("declared surface without subcrate must fail");

    assert!(error
        .to_string()
        .contains("declares [provides.preview_ui].leptos_crate"));
}

#[test]
fn validate_module_ui_surface_contract_accepts_wired_existing_admin_subcrate() {
    validate_module_ui_surface_contract(
        "blog",
        &super::workspace_root().join("crates").join("rustok-blog"),
        "admin",
        Some("rustok-blog-admin"),
    )
    .expect("wired admin subcrate should validate");
}

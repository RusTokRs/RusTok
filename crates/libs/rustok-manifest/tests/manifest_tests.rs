use rustok_manifest::*;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn test_parse_real_modules_manifest() {
    let root = workspace_root();
    let manifest_path = root.join("modules.toml");
    assert!(manifest_path.exists(), "modules.toml should exist at workspace root");

    let manifest = load_modules_manifest(&manifest_path).expect("modules.toml should parse cleanly");
    assert_eq!(manifest.schema, 2);
    assert_eq!(manifest.app, "rustok-server");
    assert!(!manifest.modules.is_empty());

    // Validate the real manifest
    validate_modules_manifest(&manifest).expect("real modules.toml must pass validation");
}

#[test]
fn test_parse_real_blog_module_manifest() {
    let root = workspace_root();
    let manifest_path = root.join("crates/modules/rustok-blog/rustok-module.toml");
    assert!(manifest_path.exists(), "blog rustok-module.toml should exist");

    let manifest = load_module_package_manifest(&manifest_path).expect("blog manifest should parse");
    assert_eq!(manifest.module.slug, "blog");
    assert_eq!(manifest.module.ui_classification, "dual_surface");

    // Validate the real blog manifest
    validate_module_package_manifest(&manifest).expect("real blog manifest must pass validation");
}

#[test]
fn test_invalid_schema_version() {
    let toml_str = r#"
        schema = 1
        app = "rustok-server"
        [modules]
        blog = { crate = "rustok-blog", source = "path" }
    "#;
    let manifest = parse_modules_manifest(toml_str).unwrap();
    let err = validate_modules_manifest(&manifest).unwrap_err();
    match err {
        ManifestError::UnsupportedSchema(v) => assert_eq!(v, 1),
        other => panic!("expected UnsupportedSchema, got: {other:?}"),
    }
}

#[test]
fn test_invalid_slug() {
    let toml_str = r#"
        schema = 2
        app = "rustok-server"
        [modules]
        _bad_slug = { crate = "rustok-bad", source = "path" }
    "#;
    let manifest = parse_modules_manifest(toml_str).unwrap();
    let err = validate_modules_manifest(&manifest).unwrap_err();
    match err {
        ManifestError::InvalidSlug(slug) => assert_eq!(slug, "_bad_slug"),
        other => panic!("expected InvalidSlug, got: {other:?}"),
    }
}

#[test]
fn test_missing_dependency() {
    let toml_str = r#"
        schema = 2
        app = "rustok-server"
        [modules]
        blog = { crate = "rustok-blog", source = "path", depends_on = ["non_existent"] }
    "#;
    let manifest = parse_modules_manifest(toml_str).unwrap();
    let err = validate_modules_manifest(&manifest).unwrap_err();
    match err {
        ManifestError::MissingDependency(mod_name, dep) => {
            assert_eq!(mod_name, "blog");
            assert_eq!(dep, "non_existent");
        }
        other => panic!("expected MissingDependency, got: {other:?}"),
    }
}

#[test]
fn test_cyclic_dependency() {
    let toml_str = r#"
        schema = 2
        app = "rustok-server"
        [modules]
        a = { crate = "rustok-a", source = "path", depends_on = ["b"] }
        b = { crate = "rustok-b", source = "path", depends_on = ["a"] }
    "#;
    let manifest = parse_modules_manifest(toml_str).unwrap();
    let err = validate_modules_manifest(&manifest).unwrap_err();
    match err {
        ManifestError::CyclicDependency(nodes) => {
            assert!(nodes.contains('a') && nodes.contains('b'));
        }
        other => panic!("expected CyclicDependency, got: {other:?}"),
    }
}

#[test]
fn test_required_module_in_default_enabled_rejected() {
    let toml_str = r#"
        schema = 2
        app = "rustok-server"
        [modules]
        auth = { crate = "rustok-auth", source = "path", required = true }
        [settings]
        default_enabled = ["auth"]
    "#;
    let manifest = parse_modules_manifest(toml_str).unwrap();
    let err = validate_modules_manifest(&manifest).unwrap_err();
    match err {
        ManifestError::Validation(msg) => {
            assert!(msg.contains("marked as required=true"));
        }
        other => panic!("expected Validation error, got: {other:?}"),
    }
}

#[test]
fn test_ui_classification_mismatch() {
    let toml_str = r#"
        [module]
        slug = "blog"
        name = "Blog"
        version = "0.1.0"
        description = "Blog"
        ui_classification = "no_ui"

        [provides.admin_ui]
        leptos_crate = "rustok-blog-admin"
    "#;
    let manifest = parse_module_package_manifest(toml_str).unwrap();
    let err = validate_module_package_manifest(&manifest).unwrap_err();
    match err {
        ManifestError::Validation(msg) => {
            assert!(msg.contains("ui_classification is 'no_ui'"));
        }
        other => panic!("expected Validation error, got: {other:?}"),
    }
}

#[test]
fn test_hash_determinism() {
    let toml_str = r#"
        schema = 2
        app = "rustok-server"
        [modules]
        b = { crate = "rustok-b", source = "path" }
        a = { crate = "rustok-a", source = "path" }
    "#;
    let manifest1 = parse_modules_manifest(toml_str).unwrap();
    let hash1 = compute_modules_manifest_hash(&manifest1).unwrap();

    let toml_str2 = r#"
        schema = 2
        app = "rustok-server"
        [modules]
        a = { crate = "rustok-a", source = "path" }
        b = { crate = "rustok-b", source = "path" }
    "#;
    let manifest2 = parse_modules_manifest(toml_str2).unwrap();
    let hash2 = compute_modules_manifest_hash(&manifest2).unwrap();

    assert_eq!(hash1, hash2, "BTreeMap sorting ensures deterministic hash");
}

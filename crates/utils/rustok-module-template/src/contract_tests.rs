use rustok_modules::MODULE_ARTIFACT_SOURCE_MANIFEST_FILE;

use crate::{
    ModuleTemplateError, ModuleTemplateInput, RUST_TOOLCHAIN, TEMPLATE_VERSION, render,
};

fn sample_input() -> ModuleTemplateInput {
    ModuleTemplateInput {
        slug: "contract_module".to_string(),
        version: "0.1.0".to_string(),
        display_name: "Contract Verification Module".to_string(),
    }
}

#[test]
fn display_name_validation_matrix() {
    let mut input = sample_input();

    // 1. Empty or whitespace
    input.display_name = "".to_string();
    assert!(matches!(render(&input), Err(ModuleTemplateError::InvalidDisplayName)));
    input.display_name = "   ".to_string();
    assert!(matches!(render(&input), Err(ModuleTemplateError::InvalidDisplayName)));

    // 2. Control character
    input.display_name = "Contract\nModule".to_string();
    assert!(matches!(render(&input), Err(ModuleTemplateError::InvalidDisplayName)));

    // 3. Exceeds 80 chars
    input.display_name = "a".repeat(81);
    assert!(matches!(render(&input), Err(ModuleTemplateError::InvalidDisplayName)));

    // 4. Exactly 80 chars
    input.display_name = "a".repeat(80);
    assert!(render(&input).is_ok());
}

#[test]
fn rendered_module_completeness_and_placeholder_freedom() {
    let input = sample_input();
    let rendered = render(&input).expect("render template must succeed");

    let expected_files = [
        "Cargo.toml",
        "README.md",
        "docs/index-integration.md",
        "src/lib.rs",
        "rust-toolchain.toml",
        "module-build-policy.toml",
        "tests/contract.rs",
        "tests/sandbox-scenario.json",
        "locales/en.json",
        MODULE_ARTIFACT_SOURCE_MANIFEST_FILE,
    ];

    assert_eq!(rendered.files().len(), expected_files.len());

    for expected in expected_files {
        let file_bytes = rendered
            .file(expected)
            .unwrap_or_else(|| panic!("missing expected file {expected}"));
        assert!(!file_bytes.is_empty(), "file {expected} must not be empty");

        // Verify no unresolved placeholders escape into rendered files
        let text = String::from_utf8_lossy(file_bytes);
        assert!(
            !text.contains("{{") && !text.contains("}}"),
            "file {expected} contains unresolved placeholder: {text}"
        );
    }

    // Verify non-existent file query returns None
    assert!(rendered.file("non-existent-file.txt").is_none());
}

#[test]
fn rendered_cargo_metadata_and_constants_contract() {
    let input = sample_input();
    let rendered = render(&input).expect("render template");

    let cargo = std::str::from_utf8(rendered.file("Cargo.toml").unwrap()).unwrap();
    assert!(cargo.contains("name = \"contract-module\""));
    assert!(cargo.contains("version = \"0.1.0\""));
    assert!(cargo.contains(&format!("rust-version = \"{RUST_TOOLCHAIN}\"")));
    assert!(cargo.contains(&format!("template_version = \"{TEMPLATE_VERSION}\"")));

    let toolchain = std::str::from_utf8(rendered.file("rust-toolchain.toml").unwrap()).unwrap();
    assert!(toolchain.contains(&format!("channel = \"{RUST_TOOLCHAIN}\"")));
    assert!(toolchain.contains("targets = [\"wasm32-wasip2\"]"));
}


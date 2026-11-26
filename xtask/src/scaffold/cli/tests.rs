use super::options::Options;
use super::{registration, scaffold};
use crate::scaffold::registration::TempWorkspace;
use std::fs;
use std::path::Path;
use std::process::Command;

const OWNER: &str = "crates/modules/rustok-example";
const REGISTRY: &str = "crates/utils/rustok-cli-registry";
const GENERATOR: &str = "scripts/generate/generate-cli-registry.mjs";
const TARGETS: &[&str] = &[
    "Cargo.toml",
    "crates/modules/rustok-example/rustok-module.toml",
    "crates/utils/rustok-cli-registry/Cargo.toml",
    "crates/utils/rustok-cli-registry/src/generated.rs",
];

fn fixture() -> (TempWorkspace, Options) {
    let temporary = TempWorkspace::new().unwrap();
    let root = &temporary.root;
    let workspace = crate::manifest_io::workspace_root();
    crate::scaffold::tests::copy_scaffold_inputs(root);
    let owner_options = crate::scaffold::tests::options();
    let plan =
        crate::scaffold::build_scaffold_plan(root, &root.join(OWNER), &owner_options, "Example")
            .unwrap();
    for file in plan {
        fs::create_dir_all(file.path.parent().unwrap()).unwrap();
        fs::write(file.path, file.content).unwrap();
    }
    for path in [
        "cli-registry.toml",
        "crates/utils/rustok-cli-registry/Cargo.toml",
        GENERATOR,
        "scripts/verify/verify-module-source-layout.mjs",
        "scripts/verify/native-module-layout.mjs",
    ] {
        let target = root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(workspace.join(path), target).unwrap();
    }
    let baseline = Command::new("node")
        .arg(root.join(GENERATOR))
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        baseline.status.success(),
        "{}",
        String::from_utf8_lossy(&baseline.stderr)
    );

    let adapter = root.join("inspect.rs");
    fs::write(&adapter, concat!(
        "use rustok_cli_core::{CliCoreResult, CommandOutcome, CommandRequest};\n",
        "use rustok_runtime::RuntimeComposition;\n",
        "pub(crate) async fn execute(_: &RuntimeComposition, _: CommandRequest) -> CliCoreResult<CommandOutcome> {\n",
        "    Ok(CommandOutcome::success(std::any::type_name::<rustok_example::ExampleModule>()))\n",
        "}\n",
    )).unwrap();
    let tests = root.join("contract.rs");
    fs::write(&tests, concat!(
        "#[test]\nfn discovers_owner_command() {\n",
        "    let runtime = rustok_runtime::RuntimeComposition::without_database(serde_json::Value::Null);\n",
        "    assert_eq!(crate::command_provider(&runtime).commands()[0].name, \"inspect-module\");\n",
        "}\n",
    )).unwrap();
    let args = [
        "example",
        "--name",
        "inspect-module",
        "--summary",
        "Inspect module identity",
        "--adapter",
        adapter.to_str().unwrap(),
        "--tests",
        tests.to_str().unwrap(),
        "--dependency",
        "serde_json",
    ]
    .map(str::to_owned);
    (temporary, Options::parse(&args).unwrap())
}

fn originals(root: &Path) -> Vec<Vec<u8>> {
    TARGETS
        .iter()
        .map(|path| fs::read(root.join(path)).unwrap())
        .collect()
}

#[test]
fn cli_scaffold_previews_and_applies_the_owner_and_registry_as_one_plan() {
    let (temporary, mut options) = fixture();
    let root = &temporary.root;
    let before = originals(root);
    let mut output = Vec::new();
    options.dry_run = true;
    options.supports_dry_run = true;
    scaffold(root, &options, &mut output, |_| {
        panic!("preview must not validate an applied tree")
    })
    .unwrap();
    let output = String::from_utf8(output).unwrap();
    for marker in [
        "cli/src/commands/inspect_module.rs",
        "[provides.cli]",
        "rustok_example_cli::command_provider",
        ".with_dry_run()",
    ] {
        assert!(output.contains(marker), "missing {marker} in {output}");
    }
    assert!(!root.join(OWNER).join("cli").exists());
    assert!(!root.join(".rustok").exists());
    assert_eq!(originals(root), before);

    options.dry_run = false;
    scaffold(root, &options, std::io::sink(), |slug| {
        crate::module_commands::lint_module_source(root, slug, "--cli")?;
        registration::check_registry(root)
    })
    .unwrap();
    assert_eq!(
        fs::read(root.join(OWNER).join("cli/src/commands/inspect_module.rs")).unwrap(),
        fs::read(&options.adapter).unwrap()
    );
    let readme = fs::read_to_string(root.join(OWNER).join("cli/README.md")).unwrap();
    assert!(readme
        .lines()
        .any(|line| line.starts_with("- `src/provider.rs`")));
    let api = fs::read_to_string(root.join(OWNER).join("cli/CRATE_API.md")).unwrap();
    assert!(api.lines().any(|line| line == "Inspect module identity"));
    let generated = fs::read_to_string(root.join(REGISTRY).join("src/generated.rs")).unwrap();
    assert!(generated.contains("rustok_cli_platform::command_provider"));
    assert!(generated.contains("rustok_example_cli::command_provider"));

    let after = originals(root);
    let error = scaffold(root, &options, std::io::sink(), |_| unreachable!()).unwrap_err();
    assert!(error.to_string().contains("CLI package already exists"));
    assert_eq!(originals(root), after);
}

#[test]
fn cli_scaffold_restores_the_existing_owner_after_validation_failure() {
    let (temporary, options) = fixture();
    let root = &temporary.root;
    let before = originals(root);
    let error = scaffold(root, &options, std::io::sink(), |slug| {
        assert_eq!(slug, "example");
        assert!(root.join(OWNER).join("cli/Cargo.toml").is_file());
        registration::check_registry(root)?;
        anyhow::bail!("deliberate validation failure")
    })
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("deliberate validation failure"),
        "{error:#}"
    );
    assert_eq!(originals(root), before);
    assert!(root.join(OWNER).join("src/module.rs").is_file());
    assert!(!root.join(OWNER).join("cli").exists());
}

#[test]
fn cli_scaffold_rejects_a_placeholder_before_touching_registrations() {
    let (temporary, options) = fixture();
    let root = &temporary.root;
    let before = originals(root);
    fs::write(
        &options.adapter,
        "pub async fn execute(_: (), _: ()) { todo!() }",
    )
    .unwrap();
    let result = scaffold(root, &options, std::io::sink(), |_| unreachable!()).unwrap_err();
    assert!(format!("{result:#}").contains("Unimplemented CLI source"));
    assert_eq!(originals(root), before);
    assert!(!root.join(OWNER).join("cli").exists());
}

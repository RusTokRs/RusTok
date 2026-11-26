pub(super) const CLI_ADAPTER: &str = r#"use rustok_cli_core::{CliCoreResult, CommandOutcome, CommandRequest};
use rustok_core::module::RusToKModule;
use rustok_runtime::RuntimeComposition;
use rustok_scaffold_matrix_headless::ScaffoldMatrixHeadlessModule;

pub(crate) async fn execute(
    _: &RuntimeComposition,
    _: CommandRequest,
) -> CliCoreResult<CommandOutcome> {
    Ok(CommandOutcome::success(ScaffoldMatrixHeadlessModule.slug()))
}
"#;

pub(super) const CLI_CONTRACT: &str = r#"use rustok_cli_core::CommandRequest;
use rustok_runtime::RuntimeComposition;

#[tokio::test]
async fn reads_the_owner_identity() {
    let provider = crate::command_provider(&RuntimeComposition::without_database(Default::default()));
    let commands = provider.commands();
    assert_eq!(commands.len(), 1);
    assert!(commands[0].matches("scaffold_matrix_headless", "module-identity"));
    let request = CommandRequest::new(
        "scaffold_matrix_headless", "module-identity", Default::default(), false,
    );
    assert_eq!(provider.execute(request).await.unwrap().message, "scaffold_matrix_headless");
}
"#;

pub(super) const DISTRIBUTION_CONTRACT: &str = concat!(
    r#"#[test]
fn registers_each_selected_scaffold_once() {
    let registry = rustok_distribution::build_registry();
    for slug in [
        "scaffold_matrix_headless",
        "scaffold_matrix_admin",
        "scaffold_matrix_storefront",
        "scaffold_matrix_dual",
        "scaffold_matrix_localized",
    ] {
        assert_eq!(registry.modules().filter(|module| module.slug() == slug).count(), 1);
    }
}
"#,
    include_str!("index_fixture.rs.in")
);

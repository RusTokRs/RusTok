#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn machine_translation_constants_and_descriptors() {
    use crate::{
        machine_translation_policy_digest, machine_translation_task_descriptor,
        MACHINE_TRANSLATION_PROVIDER_SLUG, MACHINE_TRANSLATION_TASK_SLUG,
    };

    assert_eq!(MACHINE_TRANSLATION_TASK_SLUG, "machine_translation");
    assert_eq!(MACHINE_TRANSLATION_PROVIDER_SLUG, "rustok_ai");

    let descriptor = machine_translation_task_descriptor();
    assert_eq!(descriptor.task_slug, "machine_translation");
    assert_eq!(descriptor.owner, "translation");
    assert_eq!(
        descriptor.prompt_policy_digest,
        machine_translation_policy_digest()
    );
}

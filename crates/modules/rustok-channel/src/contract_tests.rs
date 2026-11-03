use rustok_core::module::{ModuleKind, RusToKModule};
use uuid::Uuid;

use crate::{
    CHANNEL_RESOLUTION_INVALIDATION_SCOPE, CHANNEL_RESOLUTION_POLICY_SCHEMA_VERSION, ChannelError,
    ChannelModule, ChannelReadSelector, ChannelResolver, ChannelRuntimeSelected, ChannelService,
    ChannelTargetType, TargetSurface,
};

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
fn module_metadata_matches_contract() {
    let module = ChannelModule;
    assert_eq!(module.slug(), "channel");
    assert_eq!(module.name(), "Channel");
    assert_eq!(module.kind(), ModuleKind::Core);
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn debug_representations_are_informative() {
    assert_eq!(format!("{:?}", ChannelModule), "ChannelModule");
    assert_eq!(
        format!("{:?}", ChannelRuntimeSelected),
        "ChannelRuntimeSelected"
    );

    let dummy_db = rustok_test_utils::setup_test_db().await;
    let service = ChannelService::new(dummy_db.clone());
    let service_debug = format!("{service:?}");
    assert!(service_debug.contains("ChannelService"));

    let resolver = ChannelResolver::new(dummy_db);
    let resolver_debug = format!("{resolver:?}");
    assert!(resolver_debug.contains("ChannelResolver"));
}

#[test]
fn target_type_contracts() {
    for (raw, expected) in [
        ("web_domain", ChannelTargetType::WebDomain),
        ("mobile_app", ChannelTargetType::MobileApp),
        ("api_client", ChannelTargetType::ApiClient),
        ("embedded", ChannelTargetType::Embedded),
        ("external", ChannelTargetType::External),
    ] {
        assert_eq!(ChannelTargetType::parse(raw), Some(expected));
        assert_eq!(expected.as_str(), raw);
    }
    assert_eq!(ChannelTargetType::parse("unknown_type"), None);

    assert!(ChannelTargetType::WebDomain.supports_host_resolution());
    assert!(!ChannelTargetType::MobileApp.supports_host_resolution());
    assert!(!ChannelTargetType::ApiClient.supports_host_resolution());
    assert!(!ChannelTargetType::Embedded.supports_host_resolution());
    assert!(!ChannelTargetType::External.supports_host_resolution());
}

#[test]
fn policy_constants_match_contract() {
    assert_eq!(CHANNEL_RESOLUTION_POLICY_SCHEMA_VERSION, 1);
    assert_eq!(CHANNEL_RESOLUTION_INVALIDATION_SCOPE, "resolution");
    assert_eq!(TargetSurface::default(), TargetSurface::Http);
}

#[test]
fn channel_read_selector_serde_contract() {
    let id = Uuid::new_v4();
    let selector = ChannelReadSelector::Id(id);
    let json = serde_json::to_string(&selector).unwrap();
    assert_eq!(
        serde_json::from_str::<ChannelReadSelector>(&json).unwrap(),
        selector
    );

    let default_selector = ChannelReadSelector::Default;
    let json = serde_json::to_string(&default_selector).unwrap();
    assert_eq!(
        serde_json::from_str::<ChannelReadSelector>(&json).unwrap(),
        default_selector
    );
}

#[test]
fn channel_error_display_contracts() {
    let id = Uuid::new_v4();
    assert_eq!(
        ChannelError::NotFound(id).to_string(),
        format!("channel {id} not found")
    );
    assert_eq!(
        ChannelError::InactiveChannel(id).to_string(),
        format!("channel {id} is not active")
    );
    assert_eq!(
        ChannelError::SlugAlreadyExists("web".to_string()).to_string(),
        "channel `web` already exists for this tenant"
    );
    assert_eq!(
        ChannelError::Validation("invalid".to_string()).to_string(),
        "channel validation error: invalid"
    );
}

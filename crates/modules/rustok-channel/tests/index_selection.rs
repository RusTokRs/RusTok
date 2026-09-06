use rustok_channel::{ChannelModule, ChannelRuntimeSelected};
use rustok_core::{ModuleRuntimeExtensions, RusToKModule};

#[test]
fn channel_module_publishes_only_a_typed_selection_marker_for_index_bridges() {
    let mut extensions = ModuleRuntimeExtensions::default();
    ChannelModule
        .register_runtime_extensions(&mut extensions)
        .expect("Channel runtime marker registration should succeed");

    assert!(extensions.contains::<ChannelRuntimeSelected>());
    let cargo = include_str!("../Cargo.toml");
    assert!(!cargo.contains("rustok-index"));
    assert!(!cargo.contains("register_index_schema_source"));
}

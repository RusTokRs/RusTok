use super::EventsModule;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};

#[tokio::test]
async fn events_module_defers_runtime_health_to_host() {
    assert_eq!(EventsModule.health().await, HealthStatus::Degraded);
}

#[test]
fn events_module_metadata_and_dependencies() {
    let module = EventsModule;
    assert_eq!(module.slug(), "events");
    assert_eq!(module.name(), "Events");
    assert_eq!(module.kind(), ModuleKind::Core);
    assert_eq!(module.dependencies(), &["outbox"]);
    assert!(!module.description().is_empty());
    assert!(!module.version().is_empty());
    assert!(module.migrations().is_empty());
}

#[test]
fn events_module_implements_standard_traits() {
    let module: EventsModule = Default::default();
    let cloned = module;
    assert_eq!(module, cloned);
    assert_eq!(format!("{module:?}"), "EventsModule");
}


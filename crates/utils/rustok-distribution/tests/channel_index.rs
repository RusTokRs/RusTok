use rustok_distribution::{build_registry, build_runtime_extensions};
use rustok_index::{
    EntityName, ModuleName, PostgresIndexSourceFactoryCatalog, SchemaRef, SchemaVersion,
    SharedIndexSchemaRegistry,
};

#[test]
fn selected_channel_bridge_publishes_schema_and_source_factory() {
    let registry = build_registry();
    let extensions = build_runtime_extensions(&registry)
        .expect("selected SalesChannel Index bridge should compose");
    let schema = SchemaRef {
        module: ModuleName::new("rustok-channel").unwrap(),
        entity: EntityName::new("sales_channel").unwrap(),
        version: SchemaVersion::INITIAL,
    };

    let shared = extensions
        .get::<SharedIndexSchemaRegistry>()
        .expect("selected SalesChannel schema should materialize");
    assert!(shared.registry().get(&schema).is_some());

    let factories = extensions
        .get::<PostgresIndexSourceFactoryCatalog>()
        .expect("selected SalesChannel source factory should remain available to the host");
    assert!(factories.iter().any(|factory| {
        factory.owner_module() == "channel"
            && factory.factory_name() == "sales-channel-postgres-primary"
    }));
}

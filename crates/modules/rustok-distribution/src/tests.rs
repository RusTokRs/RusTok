use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleRegistry, ModuleRuntimeExtensions, RusToKModule};
use rustok_index::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexSchema,
    IndexSchemaSourceCatalog, IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
    SharedIndexSchemaRegistry, register_index_schema_source,
};
use sea_orm_migration::MigrationTrait;

use super::{build_runtime_extensions, composition_identity};

struct DemoIndexSourceModule;

impl MigrationSource for DemoIndexSourceModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for DemoIndexSourceModule {
    fn slug(&self) -> &'static str {
        "demo_source"
    }

    fn name(&self) -> &'static str {
        "Demo source"
    }

    fn description(&self) -> &'static str {
        "Test source-owned Index schema publisher"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        register_index_schema_source(extensions, self.slug(), demo_schema()).map_err(|error| {
            rustok_core::Error::Validation(format!("demo source registration failed: {error}"))
        })
    }
}

fn demo_schema() -> IndexSchema {
    IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new("demo-source").unwrap(),
            entity: EntityName::new("item").unwrap(),
            version: SchemaVersion::INITIAL,
        },
        locale_mode: LocaleMode::None,
        fields: vec![IndexField {
            name: FieldName::new("id").unwrap(),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: true,
        }],
        links: Vec::new(),
    }
}

#[test]
fn selected_composition_identity_is_stable_and_contains_modules() {
    let first = composition_identity();
    let second = composition_identity();

    assert_eq!(first, second);
    assert_eq!(first.hash.len(), 64);
    assert!(
        first
            .hash
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
    assert!(first.modules.iter().any(|module| module.slug == "tenant"));
    assert!(
        first
            .modules
            .iter()
            .any(|module| module.slug == "social_graph")
    );
    #[cfg(feature = "mod-reactions")]
    assert!(
        first
            .modules
            .iter()
            .any(|module| module.slug == "reactions")
    );
    #[cfg(feature = "mod-ai")]
    assert!(first.modules.iter().any(|module| module.slug == "ai"));
    #[cfg(feature = "mod-security")]
    assert!(first.modules.iter().any(|module| module.slug == "security"));
}

#[test]
fn source_schema_catalog_materializes_after_all_modules_register() {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(DemoIndexSourceModule);
    let extensions = build_runtime_extensions(&registry)
        .expect("source-owned schema registry should materialize");

    let catalog = extensions
        .get::<IndexSchemaSourceCatalog>()
        .expect("Index module should seed the source catalog");
    assert_eq!(catalog.len(), 1);
    let shared = extensions
        .get::<SharedIndexSchemaRegistry>()
        .expect("non-empty source catalog should publish a shared registry");
    assert!(shared.registry().get(&demo_schema().reference).is_some());
}

#[test]
fn empty_source_catalog_does_not_publish_false_query_registry() {
    let registry = ModuleRegistry::new().register(IndexModule);
    let extensions = build_runtime_extensions(&registry)
        .expect("empty source catalog should remain a valid module composition");
    assert!(extensions.contains::<IndexSchemaSourceCatalog>());
    assert!(!extensions.contains::<SharedIndexSchemaRegistry>());
}

#[cfg(feature = "mod-product")]
#[test]
fn selected_product_bridge_publishes_schema_and_source_factory() {
    let registry = super::build_registry();
    let extensions = super::build_runtime_extensions(&registry)
        .expect("selected Product Index bridge should compose");
    let schema = SchemaRef {
        module: ModuleName::new("rustok-product").unwrap(),
        entity: EntityName::new("product").unwrap(),
        version: SchemaVersion::new(super::product_index::PRODUCT_SCHEMA_ROUTING_KEY),
    };
    let shared = extensions
        .get::<SharedIndexSchemaRegistry>()
        .expect("selected Product schema should materialize");
    assert!(shared.registry().get(&schema).is_some());
    let factories = extensions
        .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
        .expect("selected Product source factory should remain available to the host");
    assert!(factories.iter().any(|factory| {
        factory.owner_module() == "product"
            && factory.factory_name() == super::product_index::PRODUCT_INDEX_SOURCE
    }));
}

#[cfg(feature = "ai-translation")]
#[tokio::test]
async fn selected_ai_translation_bridge_publishes_factory_and_stays_optional_without_keyring() {
    let registry = super::build_registry();
    let extensions =
        super::build_runtime_extensions(&registry).expect("distribution runtime extensions");
    let factory = extensions
        .get::<rustok_translation::SharedMachineTranslationPortFactory>()
        .cloned()
        .expect("selected AI Translation bridge must publish its owner-neutral factory");
    let database = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("isolated runtime database");
    let context = rustok_api::HostRuntimeContext::new(database).with_shared_value(factory);

    assert!(
        rustok_translation::machine_translation_port_from_context(&context)
            .expect("missing deployment keyring is an optional provider state")
            .is_none(),
        "manual Translation workflows must remain available when AI keyring provisioning is absent"
    );
}


use std::{collections::BTreeMap, fmt, sync::Arc};

use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use thiserror::Error;

use crate::{
    IndexMutationEventError, IndexSourceCatalog, SharedIndexMutationEventRegistry,
    materialize_index_mutation_event_registry,
};

const MAX_FACTORY_ID_BYTES: usize = 128;

/// Host-database-aware constructor for one owner-published replay source.
///
/// Factories are registered during module composition, before a database connection is available.
/// The executable host invokes them once with its selected PostgreSQL connection immediately before
/// the immutable source registry is materialized. Implementations must only construct and register
/// source adapters; they must not execute SQL or start background tasks.
pub trait PostgresIndexSourceFactory: Send + Sync {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), String>;
}

#[derive(Clone)]
pub struct PostgresIndexSourceFactoryDescriptor {
    owner_module: String,
    factory_name: String,
    factory: Arc<dyn PostgresIndexSourceFactory>,
}

impl PostgresIndexSourceFactoryDescriptor {
    pub fn owner_module(&self) -> &str {
        &self.owner_module
    }

    pub fn factory_name(&self) -> &str {
        &self.factory_name
    }
}

impl fmt::Debug for PostgresIndexSourceFactoryDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexSourceFactoryDescriptor")
            .field("owner_module", &self.owner_module)
            .field("factory_name", &self.factory_name)
            .finish_non_exhaustive()
    }
}

/// Module-registration catalog for PostgreSQL-backed Index source constructors.
#[derive(Clone, Default)]
pub struct PostgresIndexSourceFactoryCatalog {
    factories: BTreeMap<(String, String), PostgresIndexSourceFactoryDescriptor>,
}

impl PostgresIndexSourceFactoryCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.factories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PostgresIndexSourceFactoryDescriptor> {
        self.factories.values()
    }

    fn register<F>(
        &mut self,
        owner_module: String,
        factory_name: String,
        factory: F,
    ) -> Result<(), PostgresIndexSourceFactoryError>
    where
        F: PostgresIndexSourceFactory + 'static,
    {
        validate_factory_id("owner module", &owner_module)?;
        validate_factory_id("factory name", &factory_name)?;
        let key = (owner_module.clone(), factory_name.clone());
        if self.factories.contains_key(&key) {
            return Err(PostgresIndexSourceFactoryError::DuplicateFactory {
                owner_module,
                factory_name,
            });
        }
        self.factories.insert(
            key,
            PostgresIndexSourceFactoryDescriptor {
                owner_module,
                factory_name,
                factory: Arc::new(factory),
            },
        );
        Ok(())
    }
}

impl fmt::Debug for PostgresIndexSourceFactoryCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexSourceFactoryCatalog")
            .field("factories", &self.factories)
            .finish()
    }
}

#[derive(Clone)]
struct PostgresIndexSourcesMaterialized;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PostgresIndexSourceFactoryError {
    #[error("PostgreSQL Index source factory {kind} is invalid: {value}")]
    InvalidFactoryId { kind: &'static str, value: String },
    #[error("PostgreSQL Index source factory is already registered: {owner_module}/{factory_name}")]
    DuplicateFactory {
        owner_module: String,
        factory_name: String,
    },
    #[error("PostgreSQL Index sources are already materialized")]
    AlreadyMaterialized,
    #[error("PostgreSQL Index source factory failed: {owner_module}/{factory_name}")]
    FactoryFailed {
        owner_module: String,
        factory_name: String,
    },
    #[error("PostgreSQL Index mutation event registry materialization failed")]
    MutationEventRegistry(#[source] IndexMutationEventError),
}

/// Publishes one host-database-aware source constructor during module registration.
pub fn register_postgres_index_source_factory<F>(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    factory_name: impl Into<String>,
    factory: F,
) -> Result<(), PostgresIndexSourceFactoryError>
where
    F: PostgresIndexSourceFactory + 'static,
{
    extensions
        .get_or_insert_with::<PostgresIndexSourceFactoryCatalog, _>(
            PostgresIndexSourceFactoryCatalog::new,
        )
        .register(owner_module.into(), factory_name.into(), factory)
}

/// Constructs every selected PostgreSQL replay source and commits the source/event catalogs
/// atomically.
///
/// The function performs no SQL and starts no task. A cloned source catalog is staged separately so
/// one failing owner factory or mutation-route validation cannot leave earlier sources partially
/// registered in the live runtime extensions. Exact mutation routes are frozen only after every
/// source exists, allowing the event registry to verify owner, source, and schema identity against
/// the same staged catalog. The returned count is the number of factories invoked successfully.
pub fn materialize_postgres_index_sources(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<usize, PostgresIndexSourceFactoryError> {
    if extensions.contains::<PostgresIndexSourcesMaterialized>()
        || extensions.contains::<SharedIndexMutationEventRegistry>()
    {
        return Err(PostgresIndexSourceFactoryError::AlreadyMaterialized);
    }

    let factories = extensions
        .get::<PostgresIndexSourceFactoryCatalog>()
        .cloned()
        .unwrap_or_default();
    let mut staged = extensions.clone();
    staged.insert(
        extensions
            .get::<IndexSourceCatalog>()
            .cloned()
            .unwrap_or_default(),
    );

    for descriptor in factories.iter() {
        if let Err(error) = descriptor.factory.register_source(&mut staged, db.clone()) {
            tracing::error!(
                error = %error,
                owner_module = descriptor.owner_module(),
                factory_name = descriptor.factory_name(),
                "PostgreSQL Index source factory failed during host composition"
            );
            return Err(PostgresIndexSourceFactoryError::FactoryFailed {
                owner_module: descriptor.owner_module.clone(),
                factory_name: descriptor.factory_name.clone(),
            });
        }
    }

    let event_registry = materialize_index_mutation_event_registry(&staged)
        .map_err(PostgresIndexSourceFactoryError::MutationEventRegistry)?;
    if let Some(event_registry) = event_registry {
        staged.insert(event_registry);
    }

    let count = factories.len();
    staged.insert(PostgresIndexSourcesMaterialized);
    *extensions = staged;
    Ok(count)
}

fn validate_factory_id(
    kind: &'static str,
    value: &str,
) -> Result<(), PostgresIndexSourceFactoryError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_FACTORY_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        });
    if valid {
        Ok(())
    } else {
        Err(PostgresIndexSourceFactoryError::InvalidFactoryId {
            kind,
            value: value.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use rustok_core::ModuleRuntimeExtensions;
    use sea_orm::Database;

    use crate::{
        IndexMutationEventCatalog, IndexSource, IndexSourceCatalog, IndexSourceFailure,
        IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
        SharedIndexMutationEventRegistry, register_index_mutation_event, register_index_source,
    };

    use super::*;

    struct NoopSource;

    #[async_trait]
    impl IndexSource for NoopSource {
        async fn scan(
            &self,
            request: IndexSourceScanRequest,
        ) -> Result<IndexSourcePage, IndexSourceFailure> {
            Ok(IndexSourcePage::new(&request, Vec::new(), None).expect("valid final page"))
        }

        async fn load(
            &self,
            request: IndexSourceLoadRequest,
        ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
            Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("valid empty load"))
        }
    }

    fn factory_schema() -> crate::SchemaRef {
        crate::SchemaRef {
            module: crate::ModuleName::new("factory-owner").unwrap(),
            entity: crate::EntityName::new("item").unwrap(),
            version: crate::SchemaVersion::INITIAL,
        }
    }

    struct NoopFactory;

    impl PostgresIndexSourceFactory for NoopFactory {
        fn register_source(
            &self,
            extensions: &mut ModuleRuntimeExtensions,
            _db: DatabaseConnection,
        ) -> Result<(), String> {
            register_index_source(
                extensions,
                "factory_owner",
                "factory-owner-primary",
                [factory_schema()],
                NoopSource,
            )
            .map_err(|error| error.to_string())
        }
    }

    struct FailingFactory;

    impl PostgresIndexSourceFactory for FailingFactory {
        fn register_source(
            &self,
            _extensions: &mut ModuleRuntimeExtensions,
            _db: DatabaseConnection,
        ) -> Result<(), String> {
            Err("owner details stay in startup logs".to_string())
        }
    }

    async fn connection() -> DatabaseConnection {
        Database::connect("sqlite::memory:")
            .await
            .expect("test database")
    }

    #[tokio::test]
    async fn empty_factory_catalog_materializes_without_false_sources() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(IndexSourceCatalog::new());

        assert_eq!(
            materialize_postgres_index_sources(&mut extensions, connection().await).unwrap(),
            0
        );
        assert!(
            extensions
                .get::<IndexSourceCatalog>()
                .expect("source catalog")
                .is_empty()
        );
        assert!(!extensions.contains::<SharedIndexMutationEventRegistry>());
    }

    #[tokio::test]
    async fn factories_register_into_one_staged_source_catalog() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(IndexSourceCatalog::new());
        register_postgres_index_source_factory(
            &mut extensions,
            "factory_owner",
            "primary",
            NoopFactory,
        )
        .unwrap();

        assert_eq!(
            materialize_postgres_index_sources(&mut extensions, connection().await).unwrap(),
            1
        );
        assert_eq!(
            extensions
                .get::<IndexSourceCatalog>()
                .expect("source catalog")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn valid_event_routes_freeze_with_the_staged_source_catalog() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(IndexSourceCatalog::new());
        extensions.insert(IndexMutationEventCatalog::new());
        register_postgres_index_source_factory(
            &mut extensions,
            "factory_owner",
            "primary",
            NoopFactory,
        )
        .unwrap();
        register_index_mutation_event(
            &mut extensions,
            "factory_owner",
            "factory_owner.item.changed.v1",
            "factory-owner-primary",
            factory_schema(),
        )
        .unwrap();

        materialize_postgres_index_sources(&mut extensions, connection().await).unwrap();

        let registry = extensions
            .get::<SharedIndexMutationEventRegistry>()
            .expect("event registry should be materialized with sources");
        let route = registry
            .get("factory_owner.item.changed.v1")
            .expect("exact event route");
        assert_eq!(route.owner_module(), "factory_owner");
        assert_eq!(route.source_name(), "factory-owner-primary");
        assert_eq!(route.schema(), &factory_schema());
    }

    #[tokio::test]
    async fn invalid_event_route_aborts_the_staged_source_catalog() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(IndexSourceCatalog::new());
        extensions.insert(IndexMutationEventCatalog::new());
        register_postgres_index_source_factory(
            &mut extensions,
            "factory_owner",
            "primary",
            NoopFactory,
        )
        .unwrap();
        register_index_mutation_event(
            &mut extensions,
            "another_owner",
            "factory_owner.item.changed.v1",
            "factory-owner-primary",
            factory_schema(),
        )
        .unwrap();

        let error = materialize_postgres_index_sources(&mut extensions, connection().await)
            .expect_err("route owner mismatch must abort composition");
        assert!(matches!(
            error,
            PostgresIndexSourceFactoryError::MutationEventRegistry(_)
        ));
        assert!(
            extensions
                .get::<IndexSourceCatalog>()
                .expect("source catalog")
                .is_empty()
        );
        assert!(!extensions.contains::<SharedIndexMutationEventRegistry>());
    }

    #[tokio::test]
    async fn failing_factory_does_not_commit_partial_source_catalog() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(IndexSourceCatalog::new());
        register_postgres_index_source_factory(
            &mut extensions,
            "factory_owner",
            "a_ok",
            NoopFactory,
        )
        .unwrap();
        register_postgres_index_source_factory(
            &mut extensions,
            "factory_owner",
            "b_fail",
            FailingFactory,
        )
        .unwrap();

        let error = materialize_postgres_index_sources(&mut extensions, connection().await)
            .expect_err("factory failure must abort composition");
        assert!(matches!(
            error,
            PostgresIndexSourceFactoryError::FactoryFailed { .. }
        ));
        assert!(
            extensions
                .get::<IndexSourceCatalog>()
                .expect("source catalog")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn duplicate_materialization_fails_closed() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(IndexSourceCatalog::new());
        materialize_postgres_index_sources(&mut extensions, connection().await).unwrap();

        assert_eq!(
            materialize_postgres_index_sources(&mut extensions, connection().await).unwrap_err(),
            PostgresIndexSourceFactoryError::AlreadyMaterialized
        );
    }
}

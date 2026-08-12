use std::sync::Arc;

use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use thiserror::Error;

use crate::application::{SharedIndexQueryRuntime, SharedIndexSchemaRegistry};
use crate::domain::SchemaRef;

use super::{
    PostgresIndexQueryAdmissionCatalog, PostgresIndexQueryAdmissionError, PostgresIndexQueryPort,
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexQueryRuntimeCompositionError {
    #[error("shared Index query runtime is already materialized")]
    AlreadyMaterialized,
    #[error(
        "PostgreSQL Index query admission owner {owner_module} targets unregistered schema {schema}"
    )]
    AdmissionSchemaMissing {
        owner_module: String,
        schema: SchemaRef,
    },
    #[error(
        "PostgreSQL Index link-target availability owner {owner_module} targets unregistered schema {schema}"
    )]
    LinkAvailabilitySchemaMissing {
        owner_module: String,
        schema: SchemaRef,
    },
    #[error(transparent)]
    AdmissionCatalog(#[from] PostgresIndexQueryAdmissionError),
}

/// Materializes the canonical PostgreSQL-backed query runtime from the complete source registry.
///
/// Absence of a source registry is represented as `Ok(None)` and never produces an empty or
/// partially useful runtime. Trusted schema-scoped entity admission rules and generic link-target
/// availability policies are snapshotted into the immutable runtime at materialization time. Every
/// owner rule/policy must target a schema present in the same complete immutable registry; dangling
/// registrations fail composition rather than silently never applying. When any admission or
/// availability policy exists, runtime-local pass-through descriptors are added for every otherwise
/// ungoverned registered root schema so the same composite still fences governed linked targets.
///
/// The function performs no database I/O and makes no tenant schema-readiness or owner-freshness
/// claim; those checks remain inside the query port when a request executes.
pub fn materialize_postgres_index_query_runtime(
    extensions: &mut ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<Option<SharedIndexQueryRuntime>, IndexQueryRuntimeCompositionError> {
    if extensions.contains::<SharedIndexQueryRuntime>() {
        return Err(IndexQueryRuntimeCompositionError::AlreadyMaterialized);
    }

    let Some(registry) = extensions.get::<SharedIndexSchemaRegistry>().cloned() else {
        return Ok(None);
    };
    let mut admissions = extensions
        .get::<PostgresIndexQueryAdmissionCatalog>()
        .cloned()
        .unwrap_or_default();
    for descriptor in admissions.iter() {
        if registry.registry().get(descriptor.schema()).is_none() {
            return Err(IndexQueryRuntimeCompositionError::AdmissionSchemaMissing {
                owner_module: descriptor.owner_module().to_owned(),
                schema: descriptor.schema().clone(),
            });
        }
    }
    for (schema, owner_module) in admissions.link_availability_iter() {
        if registry.registry().get(schema).is_none() {
            return Err(
                IndexQueryRuntimeCompositionError::LinkAvailabilitySchemaMissing {
                    owner_module: owner_module.to_owned(),
                    schema: schema.clone(),
                },
            );
        }
    }
    if !admissions.is_empty() {
        for registered in registry.registry().iter() {
            admissions.ensure_runtime_schema(registered.schema.reference.clone())?;
        }
    }

    let runtime = SharedIndexQueryRuntime::new(Arc::new(PostgresIndexQueryPort::with_admissions(
        db,
        registry.shared(),
        admissions,
    )));
    extensions.insert(runtime.clone());
    Ok(Some(runtime))
}

#[cfg(test)]
mod tests {
    use rustok_core::ModuleRuntimeExtensions;
    use sea_orm::Database;

    use crate::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexValueType,
        LocaleMode, ModuleName, PostgresIndexQueryAdmissionCatalog, PostgresQueryEntityAdmission,
        SchemaRef, SchemaVersion, SharedIndexQueryRuntime, materialize_index_schema_registry,
        register_index_schema_source, register_postgres_index_query_admission,
        register_postgres_index_query_link_target_availability,
    };

    use super::{IndexQueryRuntimeCompositionError, materialize_postgres_index_query_runtime};

    fn schema() -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("runtime-owner").unwrap(),
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

    async fn connection() -> sea_orm::DatabaseConnection {
        Database::connect("sqlite::memory:")
            .await
            .expect("test connection should initialize")
    }

    #[tokio::test]
    async fn missing_source_registry_does_not_publish_false_runtime() {
        let mut extensions = ModuleRuntimeExtensions::default();
        let runtime = materialize_postgres_index_query_runtime(&mut extensions, connection().await)
            .expect("missing registry should be accepted");

        assert!(runtime.is_none());
        assert!(!extensions.contains::<SharedIndexQueryRuntime>());
    }

    #[tokio::test]
    async fn complete_source_registry_materializes_one_shared_runtime() {
        let mut extensions = ModuleRuntimeExtensions::default();
        register_index_schema_source(&mut extensions, "runtime_owner", schema())
            .expect("source schema should register");
        let registry = materialize_index_schema_registry(&extensions)
            .expect("source registry should validate")
            .expect("non-empty source registry should materialize");
        extensions.insert(registry);

        let runtime = materialize_postgres_index_query_runtime(&mut extensions, connection().await)
            .expect("query runtime should materialize")
            .expect("registry should produce a runtime");

        assert!(extensions.contains::<SharedIndexQueryRuntime>());
        assert!(std::sync::Arc::ptr_eq(
            &runtime.shared_port(),
            &extensions
                .get::<SharedIndexQueryRuntime>()
                .expect("runtime should be published")
                .shared_port(),
        ));
    }

    #[tokio::test]
    async fn query_admission_catalog_is_snapshotted_into_runtime_composition() {
        let mut extensions = ModuleRuntimeExtensions::default();
        let selected = schema();
        register_index_schema_source(&mut extensions, "runtime_owner", selected.clone()).unwrap();
        register_postgres_index_query_admission(
            &mut extensions,
            "runtime_owner",
            selected.reference.clone(),
            PostgresQueryEntityAdmission::new("{{entity}}.source_version > 0").unwrap(),
        )
        .unwrap();
        register_postgres_index_query_link_target_availability(
            &mut extensions,
            "runtime_owner",
            selected.reference.clone(),
        )
        .unwrap();
        let registry = materialize_index_schema_registry(&extensions)
            .unwrap()
            .expect("registry");
        extensions.insert(registry);

        materialize_postgres_index_query_runtime(&mut extensions, connection().await)
            .expect("query runtime should materialize")
            .expect("runtime");

        let catalog = extensions
            .get::<PostgresIndexQueryAdmissionCatalog>()
            .expect("admission catalog");
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog.link_availability_len(), 1);
    }

    #[tokio::test]
    async fn dangling_query_admission_schema_fails_composition() {
        let mut extensions = ModuleRuntimeExtensions::default();
        let selected = schema();
        register_index_schema_source(&mut extensions, "runtime_owner", selected).unwrap();
        let missing = SchemaRef {
            module: ModuleName::new("runtime-owner").unwrap(),
            entity: EntityName::new("missing").unwrap(),
            version: SchemaVersion::INITIAL,
        };
        register_postgres_index_query_admission(
            &mut extensions,
            "runtime_owner",
            missing.clone(),
            PostgresQueryEntityAdmission::new("{{entity}}.source_version > 0").unwrap(),
        )
        .unwrap();
        let registry = materialize_index_schema_registry(&extensions)
            .unwrap()
            .expect("registry");
        extensions.insert(registry);

        let error = materialize_postgres_index_query_runtime(&mut extensions, connection().await)
            .expect_err("dangling admission must fail composition");
        assert_eq!(
            error,
            IndexQueryRuntimeCompositionError::AdmissionSchemaMissing {
                owner_module: "runtime_owner".to_owned(),
                schema: missing,
            }
        );
        assert!(!extensions.contains::<SharedIndexQueryRuntime>());
    }

    #[tokio::test]
    async fn dangling_link_availability_schema_fails_composition() {
        let mut extensions = ModuleRuntimeExtensions::default();
        register_index_schema_source(&mut extensions, "runtime_owner", schema()).unwrap();
        let missing = SchemaRef {
            module: ModuleName::new("runtime-owner").unwrap(),
            entity: EntityName::new("missing-link-owner").unwrap(),
            version: SchemaVersion::INITIAL,
        };
        register_postgres_index_query_link_target_availability(
            &mut extensions,
            "runtime_owner",
            missing.clone(),
        )
        .unwrap();
        let registry = materialize_index_schema_registry(&extensions)
            .unwrap()
            .expect("registry");
        extensions.insert(registry);

        let error = materialize_postgres_index_query_runtime(&mut extensions, connection().await)
            .expect_err("dangling link availability must fail composition");
        assert_eq!(
            error,
            IndexQueryRuntimeCompositionError::LinkAvailabilitySchemaMissing {
                owner_module: "runtime_owner".to_owned(),
                schema: missing,
            }
        );
    }

    #[tokio::test]
    async fn duplicate_query_runtime_materialization_fails_closed() {
        let mut extensions = ModuleRuntimeExtensions::default();
        register_index_schema_source(&mut extensions, "runtime_owner", schema()).unwrap();
        let registry = materialize_index_schema_registry(&extensions)
            .unwrap()
            .expect("registry");
        extensions.insert(registry);
        materialize_postgres_index_query_runtime(&mut extensions, connection().await)
            .expect("first materialization")
            .expect("runtime");

        let error = materialize_postgres_index_query_runtime(&mut extensions, connection().await)
            .expect_err("duplicate materialization must fail");
        assert_eq!(
            error,
            IndexQueryRuntimeCompositionError::AlreadyMaterialized
        );
    }
}

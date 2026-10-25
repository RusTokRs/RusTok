use async_trait::async_trait;
use rustok_core::{
    MigrationDependencyDescriptor, MigrationSource, ModuleKind, ModuleRuntimeExtensions,
    RusToKModule,
};
use sea_orm_migration::MigrationTrait;

use crate::{
    IndexSchemaSourceCatalog, IndexSourceCatalog,
    infrastructure::postgres::PostgresIndexSourceFactoryCatalog,
};

/// Core index engine module descriptor for RusToK.
pub struct IndexModule;

#[async_trait]
impl RusToKModule for IndexModule {
    fn slug(&self) -> &'static str {
        "index"
    }

    fn name(&self) -> &'static str {
        "Index"
    }

    fn description(&self) -> &'static str {
        "Cross-module relational index and query engine."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        extensions.get_or_insert_with::<IndexSchemaSourceCatalog, _>(IndexSchemaSourceCatalog::new);
        extensions.get_or_insert_with::<IndexSourceCatalog, _>(IndexSourceCatalog::new);
        extensions.get_or_insert_with::<PostgresIndexSourceFactoryCatalog, _>(
            PostgresIndexSourceFactoryCatalog::new,
        );
        Ok(())
    }
}

impl MigrationSource for IndexModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        crate::migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::IndexModule;
    use rustok_core::module::{ModuleKind, RusToKModule};

    #[test]
    fn index_module_metadata() {
        let module = IndexModule;
        assert_eq!(module.slug(), "index");
        assert_eq!(module.name(), "Index");
        assert_eq!(module.kind(), ModuleKind::Core);
    }
}

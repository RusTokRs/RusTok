use rustok_core::MigrationDependencyDescriptor;
use rustok_core::module::{MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// MCP's deployment-scoped definition for migration composition.
pub struct McpModule;

impl MigrationSource for McpModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        crate::migrations::migration_dependencies()
    }
}

#[async_trait::async_trait]
impl RusToKModule for McpModule {
    fn slug(&self) -> &'static str {
        "mcp"
    }

    fn name(&self) -> &'static str {
        "MCP"
    }

    fn description(&self) -> &'static str {
        "Model Context Protocol integration and access management"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }
}

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AlloyModule;

impl AlloyModule {
    pub fn new() -> Self {
        Self
    }
}

impl MigrationSource for AlloyModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }
}

#[async_trait]
impl RusToKModule for AlloyModule {
    fn slug(&self) -> &'static str {
        "alloy"
    }

    fn name(&self) -> &'static str {
        "Alloy"
    }

    fn description(&self) -> &'static str {
        "Alloy runtime and scripting capability"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::SCRIPTS_CREATE,
            Permission::SCRIPTS_READ,
            Permission::SCRIPTS_UPDATE,
            Permission::SCRIPTS_DELETE,
            Permission::SCRIPTS_LIST,
            Permission::SCRIPTS_EXECUTE,
            Permission::SCRIPTS_MANAGE,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata() {
        let module = AlloyModule::new();
        assert_eq!(module.slug(), "alloy");
        assert_eq!(module.name(), "Alloy");
        assert_eq!(
            module.description(),
            "Alloy runtime and scripting capability"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert!(module.permissions().contains(&Permission::SCRIPTS_MANAGE));
    }
}

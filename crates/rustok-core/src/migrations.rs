use sea_orm_migration::MigrationTrait;

pub struct ModuleMigration {
    pub module_slug: &'static str,
    pub migrations: Vec<Box<dyn MigrationTrait>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationDependencyDescriptor {
    pub migration: &'static str,
    pub after: Vec<&'static str>,
}

impl MigrationDependencyDescriptor {
    pub fn new(migration: &'static str, after: Vec<&'static str>) -> Self {
        Self { migration, after }
    }
}

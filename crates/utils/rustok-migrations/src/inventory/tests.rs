use super::*;
use rustok_core::{MigrationPhaseConstraint, MigrationSafetyClass, MigrationSource};
use sea_orm_migration::{DbErr, MigrationName, SchemaManager};

struct TestMigration(&'static str, Option<bool>);

impl MigrationName for TestMigration {
    fn name(&self) -> &str {
        self.0
    }
}

#[async_trait::async_trait]
impl MigrationTrait for TestMigration {
    async fn up(&self, _: &SchemaManager) -> Result<(), DbErr> {
        panic!("inventory construction must not execute migrations")
    }

    fn use_transaction(&self) -> Option<bool> {
        self.1
    }
}

struct TestModule {
    slug: &'static str,
    names: Vec<(&'static str, Option<bool>)>,
    dependencies: Vec<MigrationDependencyDescriptor>,
    safety: Vec<MigrationSafetyMetadata>,
}

impl TestModule {
    fn new(slug: &'static str, names: &[&'static str]) -> Self {
        Self {
            slug,
            names: names.iter().map(|name| (*name, None)).collect(),
            dependencies: Vec::new(),
            safety: Vec::new(),
        }
    }
}

impl MigrationSource for TestModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        self.names
            .iter()
            .map(|(name, transaction)| {
                Box::new(TestMigration(name, *transaction)) as Box<dyn MigrationTrait>
            })
            .collect()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        self.dependencies.clone()
    }

    fn migration_safety_metadata(&self) -> Vec<MigrationSafetyMetadata> {
        self.safety.clone()
    }
}

#[async_trait::async_trait]
impl RusToKModule for TestModule {
    fn slug(&self) -> &'static str {
        self.slug
    }
    fn name(&self) -> &'static str {
        "Inventory test"
    }
    fn description(&self) -> &'static str {
        "Inventory test"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
}

fn metadata(name: &'static str) -> MigrationSafetyMetadata {
    MigrationSafetyMetadata::new(
        name,
        MigrationSafetyClass::MaintenanceOnly,
        MigrationPhaseConstraint::MaintenanceWindow,
    )
}

fn error(modules: &[&dyn RusToKModule]) -> MigrationInventoryError {
    MigrationInventory::compose(Vec::new(), modules)
        .err()
        .expect("invalid declarations must fail")
}

#[test]
fn export_and_executor_share_exact_order_and_owner_facts() {
    let mut alpha = TestModule::new("alpha", &["a", "d"]);
    alpha
        .dependencies
        .push(MigrationDependencyDescriptor::new("a", vec!["c"]));
    alpha.safety.push(metadata("a"));
    let beta = TestModule::new("beta", &["c"]);
    let inventory =
        MigrationInventory::compose(vec![Box::new(TestMigration("b", None))], &[&alpha, &beta])
            .unwrap();
    assert_eq!(inventory.migrations(), &["b", "c", "a", "d"]);
    assert_eq!(inventory.entries()[0].owner, MigrationOwner::Platform);
    assert_eq!(
        inventory.entries()[2].owner,
        MigrationOwner::Module {
            slug: "alpha".to_owned()
        }
    );
    assert_eq!(inventory.entries()[2].after, ["c"]);
    assert_eq!(inventory.entries()[2].safety, Some(metadata("a")));
    let json = serde_json::to_value(&inventory).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["migrations"], serde_json::json!(["b", "c", "a", "d"]));
    assert_eq!(
        json["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["migration"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["b", "c", "a", "d"]
    );
    assert_eq!(
        inventory
            .into_migrations()
            .iter()
            .map(|migration| migration.name())
            .collect::<Vec<_>>(),
        ["b", "c", "a", "d"]
    );
}

#[test]
fn missing_safety_and_transaction_declarations_remain_unknown() {
    let mut module = TestModule::new("alpha", &[]);
    module.names = vec![("a", None), ("b", Some(true)), ("c", Some(false))];
    let inventory = MigrationInventory::compose(Vec::new(), &[&module]).unwrap();
    let json = serde_json::to_value(&inventory).unwrap();
    assert_eq!(
        json["entries"][0]["use_transaction"],
        serde_json::Value::Null
    );
    assert_eq!(json["entries"][1]["use_transaction"], true);
    assert_eq!(json["entries"][2]["use_transaction"], false);
    assert!(
        inventory
            .entries()
            .iter()
            .all(|entry| entry.safety.is_none())
    );
    assert!(
        json["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["safety"].is_null())
    );
}

#[test]
fn identities_cannot_be_empty_ambiguous_or_owned_twice() {
    assert_eq!(error(&[]), MigrationInventoryError::EmptyPlan);
    for name in ["", " a", "a "] {
        assert_eq!(
            error(&[&TestModule::new("alpha", &[name])]),
            MigrationInventoryError::InvalidIdentity(name.to_owned())
        );
    }
    let alpha = TestModule::new("alpha", &["a"]);
    let beta = TestModule::new("beta", &["a"]);
    assert_eq!(
        error(&[&alpha, &beta]),
        MigrationInventoryError::DuplicateIdentity("a".to_owned())
    );
    assert_eq!(
        error(&[&TestModule::new("alpha", &["a", "a"])]),
        MigrationInventoryError::DuplicateIdentity("a".to_owned())
    );
    assert!(matches!(
        MigrationInventory::compose(vec![Box::new(TestMigration("a", None))], &[&alpha]),
        Err(MigrationInventoryError::DuplicateIdentity(_))
    ));
}

#[test]
fn repeated_and_invalid_owners_fail_even_without_migrations() {
    for slug in ["", " alpha", "alpha "] {
        assert_eq!(
            error(&[&TestModule::new(slug, &[])]),
            MigrationInventoryError::InvalidOwner(slug.to_owned())
        );
    }
    assert_eq!(
        error(&[
            &TestModule::new("alpha", &["a"]),
            &TestModule::new("alpha", &[])
        ]),
        MigrationInventoryError::InvalidOwner("alpha".to_owned())
    );
}

#[test]
fn declarations_cannot_describe_absent_or_foreign_migrations() {
    for name in ["missing", "b"] {
        let mut alpha = TestModule::new("alpha", &["a"]);
        let beta = TestModule::new("beta", &["b"]);
        alpha.safety.push(metadata(name));
        assert_eq!(
            error(&[&beta, &alpha]),
            MigrationInventoryError::ForeignDeclaration {
                owner: "alpha".to_owned(),
                migration: name.to_owned(),
                kind: "safety"
            }
        );
        alpha.safety.clear();
        alpha
            .dependencies
            .push(MigrationDependencyDescriptor::new(name, vec![]));
        assert_eq!(
            error(&[&beta, &alpha]),
            MigrationInventoryError::ForeignDeclaration {
                owner: "alpha".to_owned(),
                migration: name.to_owned(),
                kind: "dependency"
            }
        );
    }
}

#[test]
fn duplicate_declarations_and_edges_are_rejected() {
    let mut alpha = TestModule::new("alpha", &["a", "b"]);
    alpha.safety = vec![metadata("a"), metadata("a")];
    assert_eq!(
        error(&[&alpha]),
        MigrationInventoryError::DuplicateDeclaration {
            migration: "a".to_owned(),
            kind: "safety"
        }
    );
    alpha.safety.clear();
    alpha.dependencies = vec![
        MigrationDependencyDescriptor::new("a", vec![]),
        MigrationDependencyDescriptor::new("a", vec![]),
    ];
    assert_eq!(
        error(&[&alpha]),
        MigrationInventoryError::DuplicateDeclaration {
            migration: "a".to_owned(),
            kind: "dependency"
        }
    );
    alpha.dependencies = vec![MigrationDependencyDescriptor::new("a", vec!["b", "b"])];
    assert_eq!(
        error(&[&alpha]),
        MigrationInventoryError::DuplicateDependency {
            migration: "a".to_owned(),
            dependency: "b".to_owned()
        }
    );
}

#[test]
fn missing_targets_self_dependencies_and_cycles_are_rejected() {
    let mut alpha = TestModule::new("alpha", &["a", "b"]);
    alpha.dependencies = vec![MigrationDependencyDescriptor::new("a", vec!["missing"])];
    assert_eq!(
        error(&[&alpha]),
        MigrationInventoryError::MissingDependency {
            migration: "a".to_owned(),
            dependency: "missing".to_owned()
        }
    );
    alpha.dependencies = vec![MigrationDependencyDescriptor::new("a", vec!["a"])];
    assert!(matches!(
        error(&[&alpha]),
        MigrationInventoryError::DependencyCycle(_)
    ));
    alpha.dependencies = vec![
        MigrationDependencyDescriptor::new("a", vec!["b"]),
        MigrationDependencyDescriptor::new("b", vec!["a"]),
    ];
    assert!(matches!(
        error(&[&alpha]),
        MigrationInventoryError::DependencyCycle(_)
    ));
}

#[test]
fn module_registration_order_does_not_change_exported_bytes() {
    let mut alpha = TestModule::new("alpha", &["a", "d"]);
    alpha.dependencies.push(MigrationDependencyDescriptor::new("a", vec!["c"]));
    alpha.safety.push(metadata("a"));
    let beta = TestModule::new("beta", &["b", "c"]);
    let forward = MigrationInventory::compose(Vec::new(), &[&alpha, &beta]).unwrap();
    let reversed = MigrationInventory::compose(Vec::new(), &[&beta, &alpha]).unwrap();
    assert_eq!(serde_json::to_vec(&forward).unwrap(), serde_json::to_vec(&reversed).unwrap());
}

#[cfg(feature = "composition")]
#[test]
fn compiled_inventory_covers_platform_and_iggy_and_matches_seaorm_factory() {
    use sea_orm_migration::MigratorTrait;

    let inventory = crate::Migrator::inventory().unwrap();
    assert_eq!(inventory.entries().len(), inventory.migrations().len());
    assert_eq!(inventory.entries()[0].owner, MigrationOwner::Platform);
    assert_eq!(inventory.entries()[0].migration, "m20250101_000001_create_tenants");
    assert!(inventory.entries().iter().any(|entry| entry.owner == MigrationOwner::Module { slug: "iggy_connector".to_owned() }));
    assert!(inventory.entries().iter().any(|entry| entry.safety.is_some()));
    let names: Vec<_> = crate::Migrator::migrations().iter().map(|migration| migration.name().to_owned()).collect();
    assert_eq!(inventory.migrations(), names);
}

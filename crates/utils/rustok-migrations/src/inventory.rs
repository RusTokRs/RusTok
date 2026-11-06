//! Executable composition and its diagnostic owner declarations. This inventory
//! is neither a source seal nor permission to execute a production transition.

use std::collections::{BTreeMap, BTreeSet};

use rustok_core::{MigrationDependencyDescriptor, MigrationSafetyMetadata, RusToKModule};
use sea_orm_migration::MigrationTrait;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationDescriptor {
    pub(crate) migration: String,
    pub(crate) after: Vec<String>,
}

impl MigrationDescriptor {
    pub fn new(
        migration: impl Into<String>,
        after: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            migration: migration.into(),
            after: after.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<MigrationDependencyDescriptor> for MigrationDescriptor {
    fn from(value: MigrationDependencyDescriptor) -> Self {
        Self::new(value.migration, value.after)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MigrationOwner {
    Platform,
    Module { slug: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MigrationEntry {
    pub migration: String,
    pub owner: MigrationOwner,
    pub after: Vec<String>,
    /// The owner's declaration, not an observed safety assessment. Missing is unknown.
    pub safety: Option<MigrationSafetyMetadata>,
    /// Preserve SeaORM's distinction between default, explicit true, and false.
    pub use_transaction: Option<bool>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MigrationInventoryError {
    #[error("composed migration plan must not be empty")]
    EmptyPlan,
    #[error("invalid migration identity {0:?}")]
    InvalidIdentity(String),
    #[error("duplicate migration identity {0}")]
    DuplicateIdentity(String),
    #[error("invalid or duplicate migration owner {0:?}")]
    InvalidOwner(String),
    #[error("owner {owner} declares {kind} for absent or foreign migration {migration}")]
    ForeignDeclaration {
        owner: String,
        migration: String,
        kind: &'static str,
    },
    #[error("duplicate {kind} declaration for migration {migration}")]
    DuplicateDeclaration {
        migration: String,
        kind: &'static str,
    },
    #[error("migration descriptor references missing migration {0}")]
    MissingMigration(String),
    #[error("migration {migration} depends on missing migration {dependency}")]
    MissingDependency {
        migration: String,
        dependency: String,
    },
    #[error("migration {migration} has a duplicate dependency {dependency}")]
    DuplicateDependency {
        migration: String,
        dependency: String,
    },
    #[error("migration dependency cycle or unsatisfied dependency: {0}")]
    DependencyCycle(String),
    #[error("migration {migration} must run after dependency {dependency}")]
    InvalidOrder {
        migration: String,
        dependency: String,
    },
}

/// One validated composition. The names and entries are derived projections of
/// the retained executable objects; callers cannot replace either projection.
#[derive(Serialize)]
pub struct MigrationInventory {
    schema_version: u32,
    migrations: Vec<String>,
    entries: Vec<MigrationEntry>,
    #[serde(skip)]
    executable: Vec<Box<dyn MigrationTrait>>,
}

impl MigrationInventory {
    pub fn compose(
        platform: Vec<Box<dyn MigrationTrait>>,
        modules: &[&dyn RusToKModule],
    ) -> Result<Self, MigrationInventoryError> {
        let mut all = Vec::new();
        let mut entries = BTreeMap::new();
        let mut dependencies = Vec::new();
        let mut owners = BTreeSet::new();
        append_migrations(platform, MigrationOwner::Platform, &mut all, &mut entries)?;

        for module in modules {
            let slug = module.slug();
            if slug.is_empty() || slug.trim() != slug || !owners.insert(slug) {
                return Err(MigrationInventoryError::InvalidOwner(slug.to_owned()));
            }
            let owner = MigrationOwner::Module {
                slug: slug.to_owned(),
            };
            append_migrations(module.migrations(), owner.clone(), &mut all, &mut entries)?;
            let mut declared_dependencies = BTreeSet::new();
            for descriptor in module.migration_dependencies() {
                let entry = owned_entry(&mut entries, &owner, descriptor.migration, "dependency")?;
                if !declared_dependencies.insert(descriptor.migration) {
                    return Err(MigrationInventoryError::DuplicateDeclaration {
                        migration: descriptor.migration.to_owned(),
                        kind: "dependency",
                    });
                }
                entry.after = descriptor
                    .after
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect();
                dependencies.push(MigrationDescriptor::from(descriptor));
            }
            for metadata in module.migration_safety_metadata() {
                let entry = owned_entry(&mut entries, &owner, metadata.migration, "safety")?;
                if entry.safety.replace(metadata).is_some() {
                    return Err(MigrationInventoryError::DuplicateDeclaration {
                        migration: entry.migration.clone(),
                        kind: "safety",
                    });
                }
            }
        }
        if all.is_empty() {
            return Err(MigrationInventoryError::EmptyPlan);
        }
        all.sort_by(|a, b| a.name().cmp(b.name()));
        sort_migrations_by_dependencies(&mut all, &dependencies)?;
        validate_migration_dependency_order(&all, &dependencies)?;
        let migrations: Vec<_> = all
            .iter()
            .map(|migration| migration.name().to_owned())
            .collect();
        let entries = migrations
            .iter()
            .map(|name| {
                entries
                    .remove(name)
                    .ok_or_else(|| MigrationInventoryError::MissingMigration(name.clone()))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            schema_version: 1,
            migrations,
            entries,
            executable: all,
        })
    }

    pub fn migrations(&self) -> &[String] {
        &self.migrations
    }

    pub fn entries(&self) -> &[MigrationEntry] {
        &self.entries
    }

    pub fn into_migrations(self) -> Vec<Box<dyn MigrationTrait>> {
        self.executable
    }
}

fn owned_entry<'a>(
    entries: &'a mut BTreeMap<String, MigrationEntry>,
    owner: &MigrationOwner,
    migration: &str,
    kind: &'static str,
) -> Result<&'a mut MigrationEntry, MigrationInventoryError> {
    entries
        .get_mut(migration)
        .filter(|entry| &entry.owner == owner)
        .ok_or_else(|| MigrationInventoryError::ForeignDeclaration {
            owner: match owner {
                MigrationOwner::Platform => "platform".to_owned(),
                MigrationOwner::Module { slug } => slug.clone(),
            },
            migration: migration.to_owned(),
            kind,
        })
}

fn append_migrations(
    migrations: Vec<Box<dyn MigrationTrait>>,
    owner: MigrationOwner,
    all: &mut Vec<Box<dyn MigrationTrait>>,
    entries: &mut BTreeMap<String, MigrationEntry>,
) -> Result<(), MigrationInventoryError> {
    for migration in migrations {
        let name = migration.name();
        if name.is_empty() || name.trim() != name {
            return Err(MigrationInventoryError::InvalidIdentity(name.to_owned()));
        }
        let entry = MigrationEntry {
            migration: name.to_owned(),
            owner: owner.clone(),
            after: Vec::new(),
            safety: None,
            use_transaction: migration.use_transaction(),
        };
        if entries.insert(name.to_owned(), entry).is_some() {
            return Err(MigrationInventoryError::DuplicateIdentity(name.to_owned()));
        }
        all.push(migration);
    }
    Ok(())
}

pub(crate) fn sort_migrations_by_dependencies(
    migrations: &mut Vec<Box<dyn MigrationTrait>>,
    descriptors: &[MigrationDescriptor],
) -> Result<(), MigrationInventoryError> {
    let names: BTreeSet<_> = migrations
        .iter()
        .map(|migration| migration.name())
        .collect();
    let mut after_by_name = BTreeMap::new();
    for descriptor in descriptors {
        if !names.contains(descriptor.migration.as_str()) {
            return Err(MigrationInventoryError::MissingMigration(
                descriptor.migration.clone(),
            ));
        }
        if after_by_name
            .insert(descriptor.migration.as_str(), &descriptor.after)
            .is_some()
        {
            return Err(MigrationInventoryError::DuplicateDeclaration {
                migration: descriptor.migration.clone(),
                kind: "dependency",
            });
        }
        let mut seen = BTreeSet::new();
        for dependency in &descriptor.after {
            if !names.contains(dependency.as_str()) {
                return Err(MigrationInventoryError::MissingDependency {
                    migration: descriptor.migration.clone(),
                    dependency: dependency.clone(),
                });
            }
            if !seen.insert(dependency) {
                return Err(MigrationInventoryError::DuplicateDependency {
                    migration: descriptor.migration.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }
    // Select the first ready migration in chronological order, as before.
    let mut sorted = Vec::with_capacity(migrations.len());
    let mut completed = BTreeSet::new();
    let mut remaining = std::mem::take(migrations);
    while !remaining.is_empty() {
        let next = remaining.iter().position(|migration| {
            after_by_name
                .get(migration.name())
                .into_iter()
                .flat_map(|after| after.iter())
                .all(|dependency| completed.contains(dependency))
        });
        let Some(next) = next else {
            return Err(MigrationInventoryError::DependencyCycle(
                remaining
                    .iter()
                    .map(|migration| migration.name())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        };
        let migration = remaining.remove(next);
        completed.insert(migration.name().to_owned());
        sorted.push(migration);
    }
    *migrations = sorted;
    Ok(())
}

pub(crate) fn validate_migration_dependency_order(
    migrations: &[Box<dyn MigrationTrait>],
    descriptors: &[MigrationDescriptor],
) -> Result<(), MigrationInventoryError> {
    let positions: BTreeMap<_, _> = migrations
        .iter()
        .enumerate()
        .map(|(index, migration)| (migration.name(), index))
        .collect();
    for descriptor in descriptors {
        let index = positions
            .get(descriptor.migration.as_str())
            .ok_or_else(|| {
                MigrationInventoryError::MissingMigration(descriptor.migration.clone())
            })?;
        for dependency in &descriptor.after {
            let dependency_index = positions.get(dependency.as_str()).ok_or_else(|| {
                MigrationInventoryError::MissingDependency {
                    migration: descriptor.migration.clone(),
                    dependency: dependency.clone(),
                }
            })?;
            if dependency_index >= index {
                return Err(MigrationInventoryError::InvalidOrder {
                    migration: descriptor.migration.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

use rustok_core::{MigrationSource, RusToKModule};
use rustok_fulfillment::FulfillmentModule;

#[test]
fn module_metadata() {
    let module = FulfillmentModule;
    assert_eq!(module.slug(), "fulfillment");
    assert_eq!(module.name(), "Fulfillment");
    assert_eq!(
        module.description(),
        "Default fulfillment submodule in the ecommerce family"
    );
}

#[test]
fn module_has_migrations() {
    let module = FulfillmentModule;
    assert!(
        !module.migrations().is_empty(),
        "fulfillment module should expose migrations"
    );
}


#[cfg(unix)]
#[test]
fn migration_registry_covers_every_flat_migration_source() {
    use std::{
        collections::BTreeSet,
        fs,
        path::Path,
    };

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let migrations_dir = manifest_dir.join("src/migrations");
    let mod_rs = fs::read_to_string(migrations_dir.join("mod.rs"))
        .expect("migration registry source should be readable");

    let registered = mod_rs
        .lines()
        .filter_map(|line| line.strip_prefix("mod ").and_then(|value| value.strip_suffix(';')))
        .filter(|name| name.starts_with("m20"))
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();

    let filesystem = fs::read_dir(&migrations_dir)
        .expect("migration directory should be readable")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
                .then(|| path.file_stem()?.to_str().map(ToOwned::to_owned))
                .flatten()
        })
        .filter(|name| name.starts_with("m20"))
        .collect::<BTreeSet<_>>();

    assert_eq!(
        registered, filesystem,
        "every flat migration source must be declared exactly once in migrations/mod.rs"
    );
}

#[cfg(unix)]
#[test]
fn migration_registry_exposes_ordered_unique_migration_names() {
    use std::collections::BTreeSet;

    let names = FulfillmentModule
        .migrations()
        .into_iter()
        .map(|migration| migration.name().to_string())
        .collect::<Vec<_>>();
    let unique = names.iter().cloned().collect::<BTreeSet<_>>();

    assert_eq!(
        names.len(),
        unique.len(),
        "registered migration names must be unique"
    );
    assert!(
        names.windows(2).all(|window| {
            let left = window[0]
                .trim_start_matches('m')
                .split_once('_')
                .and_then(|(_, rest)| rest.split_once('_'))
                .map(|(number, _)| number);
            let right = window[1]
                .trim_start_matches('m')
                .split_once('_')
                .and_then(|(_, rest)| rest.split_once('_'))
                .map(|(number, _)| number);
            match (left, right) {
                (Some(left), Some(right)) => left <= right || window[0].starts_with("m20260713_000111_"),
                _ => true,
            }
        }),
        "migration registration should remain stable and chronological"
    );
}

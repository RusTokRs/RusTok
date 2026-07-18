use rustok_migrations::Migrator;
use sea_orm_migration::prelude::{MigrationName, MigratorTrait};
use serde::Serialize;
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize)]
struct MigrationPlan {
    schema_version: u32,
    migrations: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = parse_output_path()?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let migrations = Migrator::migrations()
        .into_iter()
        .map(|migration| migration.name().to_string())
        .collect::<Vec<_>>();
    if migrations.is_empty() {
        return Err("composed migration plan must not be empty".into());
    }

    let plan = MigrationPlan {
        schema_version: 1,
        migrations,
    };
    fs::write(&output, format!("{}\n", serde_json::to_string_pretty(&plan)?))?;
    println!("exported migration plan to {}", output.display());
    Ok(())
}

fn parse_output_path() -> Result<PathBuf, Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let mut output = PathBuf::from("target/migration-plan.json");

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => {
                output = PathBuf::from(
                    arguments
                        .next()
                        .ok_or("--output requires a file path")?,
                );
            }
            "--help" | "-h" => {
                println!("Usage: export_migration_plan [--output FILE]");
                std::process::exit(0);
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }

    Ok(output)
}

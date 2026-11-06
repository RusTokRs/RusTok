use rustok_migrations::Migrator;
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let output = parse_output_path()?;
    let plan = Migrator::inventory()?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output,
        format!("{}\n", serde_json::to_string_pretty(&plan)?),
    )?;
    println!("exported migration plan to {}", output.display());
    Ok(())
}

fn parse_output_path() -> Result<PathBuf, Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let mut output = PathBuf::from("target/migration-plan.json");

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => {
                output = PathBuf::from(arguments.next().ok_or("--output requires a file path")?);
            }
            "--help" | "-h" => {
                println!("Usage: export-migration-plan [--output FILE]");
                std::process::exit(0);
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }

    Ok(output)
}

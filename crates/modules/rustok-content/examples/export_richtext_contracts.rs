use std::{env, fs, path::PathBuf};

use rustok_api::document_json_schema;
use rustok_content::richtext::all_profile_manifests;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("expected output directory argument")?;
    fs::create_dir_all(&output_dir)?;
    fs::write(
        output_dir.join("document.schema.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&document_json_schema())?
        ),
    )?;
    fs::write(
        output_dir.join("profiles.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&all_profile_manifests())?
        ),
    )?;
    Ok(())
}

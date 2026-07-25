use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::Serialize;

use super::BenchmarkRunProvenance;

#[derive(Serialize)]
struct ProvenanceBoundReport<'a, T> {
    #[serde(flatten)]
    report: &'a T,
    provenance: &'a BenchmarkRunProvenance,
}

pub fn write_provenance_bound_report<T: Serialize>(
    path: &Path,
    report: &T,
    provenance: &BenchmarkRunProvenance,
) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create provenance-bound report directory {parent:?}"))?;
    }
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("benchmark report path must have a UTF-8 filename")?;
    let staged = path.with_file_name(format!("{filename}.tmp-{}", std::process::id()));
    let envelope = ProvenanceBoundReport { report, provenance };
    let result: Result<()> = (|| {
        fs::write(&staged, serde_json::to_vec_pretty(&envelope)?)
            .with_context(|| format!("failed to stage provenance-bound benchmark report {staged:?}"))?;
        fs::rename(&staged, path)
            .with_context(|| format!("failed to publish provenance-bound benchmark report {path:?}"))?;
        Ok(())
    })();
    if staged.exists() {
        let _ = fs::remove_file(&staged);
    }
    result
}

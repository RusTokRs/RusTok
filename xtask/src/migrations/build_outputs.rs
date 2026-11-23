use super::{build_inputs::BuildInputs, collect_snapshot_files, safe_relative, SourceFile};
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use std::{
    collections::{btree_map::Entry, BTreeMap},
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize, PartialEq, Eq)]
struct OutputDirectory {
    package: String,
    files: Vec<SourceFile>,
}

#[derive(Serialize)]
pub(super) struct GeneratedOutputs {
    #[serde(skip)]
    target: PathBuf,
    directories: BTreeMap<String, OutputDirectory>,
}

impl GeneratedOutputs {
    pub(super) fn new(target: &Path) -> Result<Self> {
        Ok(Self {
            target: target.canonicalize()?,
            directories: BTreeMap::new(),
        })
    }

    pub(super) fn record(
        &mut self,
        message: &serde_json::Value,
        resolution: &BuildInputs,
    ) -> Result<()> {
        let package = message["package_id"]
            .as_str()
            .context("Cargo build script package ID missing")?;
        let package = resolution.package_identity(package)?;
        let directory = message["out_dir"]
            .as_str()
            .context("Cargo build script OUT_DIR missing")?;
        let directory = Path::new(directory);
        ensure!(
            directory.is_absolute(),
            "Cargo build script OUT_DIR must be absolute"
        );
        ensure!(
            !fs::symlink_metadata(directory)?.file_type().is_symlink(),
            "Cargo build script OUT_DIR is a symlink"
        );
        let canonical = directory.canonicalize()?;
        let relative = canonical
            .strip_prefix(&self.target)
            .context("Cargo build script OUT_DIR escaped build output")?;
        let relative = relative
            .to_str()
            .context("Cargo build script OUT_DIR is not UTF-8")?
            .replace('\\', "/");
        safe_relative(&relative)?;
        match self.directories.entry(relative) {
            Entry::Vacant(entry) => {
                entry.insert(OutputDirectory {
                    package: package.to_owned(),
                    files: Vec::new(),
                });
            }
            Entry::Occupied(entry) => ensure!(
                entry.get().package == package,
                "multiple packages reported the same OUT_DIR"
            ),
        }
        Ok(())
    }

    pub(super) fn capture(&mut self) -> Result<()> {
        for (path, directory) in &mut self.directories {
            directory.files = read_output(&self.target, path)?;
        }
        Ok(())
    }

    pub(super) fn verify_unchanged(&self) -> Result<()> {
        for (path, directory) in &self.directories {
            ensure!(
                read_output(&self.target, path)? == directory.files,
                "generated build outputs changed during verification: {path}"
            );
        }
        Ok(())
    }
}

fn read_output(target: &Path, relative: &str) -> Result<Vec<SourceFile>> {
    let path = target.join(safe_relative(relative)?);
    ensure!(
        path.canonicalize()? == path && path.is_dir(),
        "Cargo build script OUT_DIR moved or redirected: {relative}"
    );
    let mut files = Vec::new();
    collect_snapshot_files(&path, &path, &mut files)
        .with_context(|| format!("cannot record generated build outputs in {relative}"))?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

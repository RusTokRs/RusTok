use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone)]
pub(crate) struct Target {
    pub path: PathBuf,
    pub blob: Option<String>,
}

pub(crate) fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("invalid Git path: {error}"))?;
    Ok(PathBuf::from(OsString::from(text)))
}

pub(crate) fn staged(root: &Path) -> Result<Vec<Target>, String> {
    let data = git(
        root,
        &[
            "diff",
            "--cached",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "--diff-filter=AMTU",
            "-z",
            "--",
        ],
    )?;
    let records: Vec<_> = data
        .split(|byte| *byte == 0)
        .filter(|item| !item.is_empty())
        .collect();
    if records.len() % 2 != 0 {
        return Err("invalid Git index diff".to_owned());
    }
    let mut targets = BTreeMap::new();
    for pair in records.chunks_exact(2) {
        let path = path_from_bytes(pair[1])?;
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let metadata = std::str::from_utf8(pair[0]).map_err(|error| error.to_string())?;
        let fields: Vec<_> = metadata.split_whitespace().collect();
        if fields.len() != 5 {
            return Err("invalid Git index diff metadata".to_owned());
        }
        if fields[4] == "U" {
            return Err(format!(
                "Unmerged Rust file in Git index: {}",
                path.display()
            ));
        }
        if matches!(fields[1], "100644" | "100755") {
            targets.insert(
                path.clone(),
                Target {
                    path,
                    blob: Some(fields[3].to_owned()),
                },
            );
        }
    }
    Ok(targets.into_values().collect())
}

pub(crate) fn changed(root: &Path, reference: Option<&str>) -> Result<Vec<Target>, String> {
    let commit = if let Some(reference) = reference {
        let revision = format!("{reference}^{{commit}}");
        String::from_utf8(git(
            root,
            &["rev-parse", "--verify", "--end-of-options", &revision],
        )?)
        .map_err(|error| error.to_string())?
        .trim()
        .to_owned()
    } else {
        "HEAD".to_owned()
    };
    let mut files = git(
        root,
        &[
            "diff",
            "--name-only",
            "--diff-filter=ACMRT",
            "-z",
            &commit,
            "--",
        ],
    )?;
    if reference.is_none() {
        files.extend(git(
            root,
            &["ls-files", "--others", "--exclude-standard", "-z", "--"],
        )?);
    }
    let mut selected = BTreeSet::new();
    for bytes in files
        .split(|byte| *byte == 0)
        .filter(|item| !item.is_empty())
    {
        let path = path_from_bytes(bytes)?;
        if path.extension().is_some_and(|extension| extension == "rs") && root.join(&path).is_file()
        {
            selected.insert(path);
        }
    }
    Ok(selected
        .into_iter()
        .map(|path| Target { path, blob: None })
        .collect())
}

pub(crate) fn walk(
    root: &Path,
    path: &Path,
    targets: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "rs") {
            targets.insert(path.strip_prefix(root).unwrap_or(path).to_path_buf());
        }
        return Ok(());
    }
    let entries = fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            if !matches!(
                entry.file_name().to_str(),
                Some("target" | "node_modules" | ".git")
            ) {
                walk(root, &entry.path(), targets)?;
            }
        } else if file_type.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
        {
            targets.insert(
                entry
                    .path()
                    .strip_prefix(root)
                    .unwrap_or(&entry.path())
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

pub(crate) fn crates(root: &Path, names: &str) -> Result<Vec<PathBuf>, String> {
    let bases = [
        "crates/modules",
        "crates/libs",
        "crates/ui",
        "crates/workers",
        "crates/tools",
        "crates/utils",
        "apps",
    ];
    let mut selected = Vec::new();
    for name in names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let mut matches = BTreeSet::new();
        for base in bases {
            for candidate in [name.to_owned(), format!("rustok-{name}")] {
                let path = root.join(base).join(candidate);
                if path.is_dir() {
                    matches.insert(path);
                }
            }
        }
        if matches.is_empty() {
            for base in bases {
                let directory = root.join(base);
                if !directory.is_dir() {
                    continue;
                }
                for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
                    let entry = entry.map_err(|error| error.to_string())?;
                    if entry
                        .file_type()
                        .map_err(|error| error.to_string())?
                        .is_dir()
                        && entry.file_name().to_string_lossy().contains(name)
                    {
                        matches.insert(entry.path());
                    }
                }
            }
        }
        if matches.len() > 1 {
            let paths = matches
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!("Ambiguous crate '{name}': {paths}"));
        }
        let found = matches.into_iter().next();
        if let Some(path) = found {
            if !selected.contains(&path) {
                selected.push(path);
            }
        } else {
            return Err(format!("Could not find directory matching crate '{name}'"));
        }
    }
    if selected.is_empty() {
        return Err(format!("Could not resolve any paths for --crate '{names}'"));
    }
    Ok(selected)
}

pub(crate) fn read(root: &Path, target: &Target) -> Result<String, String> {
    let bytes = if let Some(blob) = &target.blob {
        git(root, &["cat-file", "blob", blob])?
    } else {
        fs::read(root.join(&target.path))
            .map_err(|error| format!("{}: {error}", target.path.display()))?
    };
    String::from_utf8(bytes).map_err(|error| format!("{}: {error}", target.path.display()))
}

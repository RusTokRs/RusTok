use crate::manifest_io::workspace_root;
use crate::manifest_validation::{module_validate_command, to_pascal_case};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod backend;
mod cli;
mod options;
pub(crate) mod registration;
mod ui;

#[allow(unused_imports)]
pub use options::{
    default_title_from_slug, is_valid_slug, parse_options, print_scaffold_usage, ScaffoldOptions,
    ScaffoldUi,
};

#[derive(Clone, Debug)]
struct PlannedFile {
    path: PathBuf,
    contents: String,
    original: Option<String>,
}

#[derive(Debug)]
struct AppliedPlan {
    committed: Vec<(PathBuf, Option<PathBuf>)>,
    created_dirs: Vec<PathBuf>,
}

impl AppliedPlan {
    fn rollback(self) -> Result<()> {
        let files = rollback_committed(&self.committed);
        let dirs = remove_created_dirs(&self.created_dirs);
        match (files, dirs) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(files), Ok(())) => Err(files),
            (Ok(()), Err(dirs)) => Err(dirs),
            (Err(files), Err(dirs)) => {
                anyhow::bail!("file rollback failed: {files:#}; directory cleanup failed: {dirs:#}")
            }
        }
    }

    fn finalize(self) {
        for (_, backup) in self.committed {
            if let Some(backup) = backup {
                if let Err(error) = fs::remove_file(&backup) {
                    eprintln!(
                        "Warning: committed scaffold, but could not remove backup {}: {error}",
                        backup.display()
                    );
                }
            }
        }
    }
}

pub(crate) fn scaffold_command(args: &[String]) -> Result<()> {
    let options = match options::parse_options(args)? {
        Some(opts) => opts,
        None => return Ok(()),
    };
    scaffold(&options)
}

/// Create a native module from one complete, preflighted file plan. No target
/// file is changed until all source and repository registration edits succeed.
pub fn scaffold(options: &ScaffoldOptions) -> Result<()> {
    if options.profile != options::ScaffoldProfile::NativeModule {
        anyhow::bail!("cargo xtask scaffold only generates native runtime modules");
    }
    if options.with_db {
        anyhow::bail!(
            "--with-db requires an actual owner schema and migration contract; refusing to invent tables or migrations"
        );
    }

    let root = workspace_root();
    let slug = &options.slug;
    let mut pascal_name = to_pascal_case(slug);
    if !pascal_name
        .chars()
        .next()
        .map(|character| character.is_ascii_alphabetic())
        .unwrap_or(false)
    {
        pascal_name.insert_str(0, "Module");
    }

    let crate_name = options.crate_name();
    let module_dir = root.join("crates/modules").join(&crate_name);
    match fs::symlink_metadata(&module_dir) {
        Ok(_) => anyhow::bail!(
            "Module crate directory already exists at {}; refusing to overwrite it",
            module_dir.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!("Failed to inspect module directory {}", module_dir.display())
            });
        }
    }

    println!("Scaffolding module '{slug}' ({crate_name})...");
    if options.ui != ScaffoldUi::None {
        println!(
            "  Draft FFA packages selected: {:?}; no host route or live UI manifest will be mounted.",
            options.ui
        );
    }
    if options.with_fba {
        println!("  Shared FBA library selected; no provider/consumer registry is inferred.");
    }
    if options.with_cli {
        println!("  Optional CLI provider selected; command catalog starts empty.");
    }

    let plan = build_plan(&root, &module_dir, options, &pascal_name)?;
    if options.dry_run {
        print_plan(&root, &plan);
        println!("\n[DRY-RUN] Complete generated-source and registration plan; no files changed.");
        return Ok(());
    }

    let transaction = apply_plan(&plan)?;
    if !options.skip_validate {
        println!("\nVerifying module publish-metadata contracts for '{slug}'...");
        if let Err(error) = module_validate_command(std::slice::from_ref(slug)) {
            return match transaction.rollback() {
                Ok(()) => Err(error).context(format!(
                    "Module '{slug}' validation failed; all scaffold and registration writes were rolled back"
                )),
                Err(rollback_error) => Err(anyhow::anyhow!(
                    "Module '{slug}' validation failed: {error:#}; rollback also failed: {rollback_error:#}"
                )),
            };
        }
        println!(
            "\nSCAFFOLDED: Module '{slug}' metadata validated. Business operations and FFA/FBA readiness remain not_started."
        );
    } else {
        println!(
            "SCAFFOLDED: Module '{slug}' files and registration written; post-write publish-metadata validation was skipped."
        );
    }

    println!("Next steps:");
    println!("  cargo check -p {crate_name}");
    if options.with_fba {
        println!("  cargo test -p {crate_name} --lib");
    }
    if options.ui.has_admin() {
        println!("  cargo check -p {crate_name}-admin");
    }
    if options.ui.has_storefront() {
        println!("  cargo check -p {crate_name}-storefront");
    }
    if options.with_cli {
        println!("  cargo check -p {crate_name}-cli");
        println!("  node scripts/generate/generate-cli-registry.mjs --check");
    }
    transaction.finalize();
    Ok(())
}

fn build_plan(
    root: &Path,
    module_dir: &Path,
    options: &ScaffoldOptions,
    pascal: &str,
) -> Result<Vec<PlannedFile>> {
    let mut contents = BTreeMap::<PathBuf, String>::new();

    let backend_files = backend::render_backend(options, pascal);
    let manifest = backend_files
        .iter()
        .find(|(path, _)| path.as_path() == Path::new("rustok-module.toml"))
        .map(|(_, content)| content.clone())
        .ok_or_else(|| anyhow::anyhow!("Backend renderer did not produce rustok-module.toml"))?;

    for (relative, content) in backend_files {
        insert_file(&mut contents, module_dir.join(relative), content)?;
    }
    for (relative, content) in ui::render_ui_templates(options, pascal) {
        insert_file(&mut contents, module_dir.join(relative), content)?;
    }
    for (relative, content) in cli::render_cli_templates(options, pascal) {
        insert_file(&mut contents, module_dir.join(relative), content)?;
    }
    for (path, content) in registration::plan_module(root, options, pascal, &manifest)? {
        insert_file(&mut contents, path, content)?;
    }

    let mut plan = Vec::with_capacity(contents.len());
    for (path, content) in contents {
        if !path.starts_with(root) {
            anyhow::bail!(
                "Refusing to write outside workspace root: {}",
                path.display()
            );
        }
        validate_parent_chain(root, &path)?;

        let generated = path.starts_with(module_dir);
        let original = if generated {
            match fs::symlink_metadata(&path) {
                Ok(_) => anyhow::bail!(
                    "Generated target already exists at {}; refusing to overwrite",
                    path.display()
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("Failed to inspect generated target {}", path.display())
                    });
                }
            }
            None
        } else {
            let metadata = fs::symlink_metadata(&path)
                .with_context(|| format!("Registration target is missing: {}", path.display()))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                anyhow::bail!(
                    "Registration target must be a regular non-symlink file: {}",
                    path.display()
                );
            }
            Some(
                fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?,
            )
        };

        if original.as_deref() == Some(content.as_str()) {
            continue;
        }
        validate_planned_toml(&path, &content)?;
        plan.push(PlannedFile {
            path,
            contents: content,
            original,
        });
    }
    Ok(plan)
}

fn insert_file(
    files: &mut BTreeMap<PathBuf, String>,
    path: PathBuf,
    contents: String,
) -> Result<()> {
    if files.insert(path.clone(), contents).is_some() {
        anyhow::bail!(
            "Scaffold plan contains duplicate destination {}",
            path.display()
        );
    }
    Ok(())
}

fn validate_parent_chain(root: &Path, target: &Path) -> Result<()> {
    let relative = target
        .strip_prefix(root)
        .with_context(|| format!("Target {} is outside workspace root", target.display()))?;
    let Some(parent) = relative.parent() else {
        return Ok(());
    };

    let mut cursor = root.to_path_buf();
    for component in parent.components() {
        cursor.push(component);
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                anyhow::bail!(
                    "Scaffold path contains a non-directory or symlink parent: {}",
                    cursor.display()
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to inspect {}", cursor.display()));
            }
        }
    }
    Ok(())
}

fn validate_planned_toml(path: &Path, contents: &str) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if name == "Cargo.toml" || name == "rustok-module.toml" || name == "modules.toml" {
        toml::from_str::<toml::Value>(contents)
            .with_context(|| format!("Generated TOML is invalid: {}", path.display()))?;
    }
    if name == "rustok-module.toml" {
        let manifest = rustok_manifest::parse_module_package_manifest(contents)
            .with_context(|| format!("Generated module manifest is invalid: {}", path.display()))?;
        rustok_manifest::validate_module_package_manifest(&manifest)
            .with_context(|| format!("Generated module manifest contract is invalid: {}", path.display()))?;
    }
    if name == "modules.toml" {
        let manifest = rustok_manifest::parse_modules_manifest(contents)
            .with_context(|| format!("Generated modules registry is invalid: {}", path.display()))?;
        rustok_manifest::validate_modules_manifest(&manifest)
            .with_context(|| format!("Generated modules registry contract is invalid: {}", path.display()))?;
    }
    Ok(())
}

fn print_plan(root: &Path, plan: &[PlannedFile]) {
    println!("Scaffold plan:");
    for file in plan {
        let action = if file.original.is_some() { "UPDATE" } else { "CREATE" };
        let display = file.path.strip_prefix(root).unwrap_or(&file.path);
        println!("  [{action}] {}", display.display());

        if let Some(original) = file.original.as_deref() {
            for line in diff_lines(original, &file.contents) {
                println!("    {line}");
            }
        } else if matches!(
            file.path.file_name().and_then(|value| value.to_str()),
            Some("Cargo.toml" | "rustok-module.toml")
        ) {
            for line in file.contents.lines() {
                println!("    +{line}");
            }
        }
    }
}

fn diff_lines(old: &str, new: &str) -> Vec<String> {
    let before = old.lines().collect::<Vec<_>>();
    let after = new.lines().collect::<Vec<_>>();
    let n = before.len();
    let m = after.len();

    if n.saturating_mul(m) > 2_000_000 {
        let mut prefix = 0;
        while prefix < n.min(m) && before[prefix] == after[prefix] {
            prefix += 1;
        }
        let mut suffix = 0;
        while suffix < (n - prefix).min(m - prefix)
            && before[n - suffix - 1] == after[m - suffix - 1]
        {
            suffix += 1;
        }
        let mut changes = Vec::new();
        changes.extend(before[prefix..n - suffix].iter().map(|line| format!("-{line}")));
        changes.extend(after[prefix..m - suffix].iter().map(|line| format!("+{line}")));
        return changes;
    }

    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if before[i] == after[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut changes = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && before[i] == after[j] {
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            changes.push(format!("+{}", after[j]));
            j += 1;
        } else if i < n {
            changes.push(format!("-{}", before[i]));
            i += 1;
        }
    }
    changes
}

fn apply_plan(plan: &[PlannedFile]) -> Result<AppliedPlan> {
    apply_plan_inner(plan, None)
}

fn apply_plan_inner(
    plan: &[PlannedFile],
    fail_after_commits: Option<usize>,
) -> Result<AppliedPlan> {
    let mut created_dirs = Vec::<PathBuf>::new();
    let mut staged = Vec::<(PathBuf, PathBuf)>::new();

    for (index, file) in plan.iter().enumerate() {
        if let Err(error) = ensure_parent_dirs(
            file.path.parent().unwrap_or_else(|| Path::new(".")),
            &mut created_dirs,
        ) {
            return Err(abort_transaction(error, &[], &staged, &created_dirs));
        }
        if let Err(error) = ensure_unchanged(file) {
            return Err(abort_transaction(error, &[], &staged, &created_dirs));
        }

        let temporary = match unique_sibling(&file.path, "tmp", index) {
            Ok(path) => path,
            Err(error) => return Err(abort_transaction(error, &[], &staged, &created_dirs)),
        };
        if let Err(error) = fs::write(&temporary, file.contents.as_bytes()) {
            staged.push((file.path.clone(), temporary));
            let error = anyhow::Error::new(error)
                .context(format!("Failed to stage {}", file.path.display()));
            return Err(abort_transaction(error, &[], &staged, &created_dirs));
        }

        if file.original.is_some() {
            match fs::metadata(&file.path) {
                Ok(metadata) => {
                    if let Err(error) = fs::set_permissions(&temporary, metadata.permissions()) {
                        let error = anyhow::Error::new(error)
                            .context(format!("Failed to preserve permissions for {}", file.path.display()));
                        staged.push((file.path.clone(), temporary));
                        return Err(abort_transaction(error, &[], &staged, &created_dirs));
                    }
                }
                Err(error) => {
                    let error = anyhow::Error::new(error)
                        .context(format!("Failed to inspect {}", file.path.display()));
                    staged.push((file.path.clone(), temporary));
                    return Err(abort_transaction(error, &[], &staged, &created_dirs));
                }
            }
        }
        staged.push((file.path.clone(), temporary));
    }

    let mut committed = Vec::<(PathBuf, Option<PathBuf>)>::new();
    for (index, file) in plan.iter().enumerate() {
        if fail_after_commits == Some(committed.len()) {
            let error = anyhow::anyhow!(
                "Injected commit failure after {} file(s)",
                committed.len()
            );
            return Err(abort_transaction(error, &committed, &staged, &created_dirs));
        }
        if let Err(error) = ensure_unchanged(file) {
            return Err(abort_transaction(error, &committed, &staged, &created_dirs));
        }

        let temporary = &staged[index].1;
        let backup = if file.original.is_some() {
            let backup = match unique_sibling(&file.path, "backup", index) {
                Ok(path) => path,
                Err(error) => return Err(abort_transaction(error, &committed, &staged, &created_dirs)),
            };
            if let Err(error) = fs::rename(&file.path, &backup) {
                let error = anyhow::Error::new(error)
                    .context(format!("Failed to back up {}", file.path.display()));
                return Err(abort_transaction(error, &committed, &staged, &created_dirs));
            }
            Some(backup)
        } else {
            None
        };

        if let Err(error) = fs::rename(temporary, &file.path) {
            let error = anyhow::Error::new(error)
                .context(format!("Failed to install {}", file.path.display()));
            if let Some(backup_path) = backup.as_ref() {
                if fs::rename(backup_path, &file.path).is_err() {
                    let mut rollback_files = committed.clone();
                    rollback_files.push((file.path.clone(), Some(backup_path.clone())));
                    return Err(abort_transaction(error, &rollback_files, &staged, &created_dirs));
                }
            }
            return Err(abort_transaction(error, &committed, &staged, &created_dirs));
        }

        committed.push((file.path.clone(), backup));
    }

    Ok(AppliedPlan {
        committed,
        created_dirs,
    })
}

fn ensure_unchanged(file: &PlannedFile) -> Result<()> {
    match file.original.as_deref() {
        Some(expected) => {
            let metadata = fs::symlink_metadata(&file.path)
                .with_context(|| format!("Failed to inspect {}", file.path.display()))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                anyhow::bail!("Registration target is no longer a regular file: {}", file.path.display());
            }
            let current = fs::read_to_string(&file.path)
                .with_context(|| format!("Failed to re-read {}", file.path.display()))?;
            if current != expected {
                anyhow::bail!(
                    "Registration target changed after preflight: {}",
                    file.path.display()
                );
            }
        }
        None => match fs::symlink_metadata(&file.path) {
            Ok(_) => anyhow::bail!(
                "Generated target appeared after preflight: {}",
                file.path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect generated target {}", file.path.display())
                });
            }
        },
    }
    Ok(())
}

fn ensure_parent_dirs(path: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    let mut missing = Vec::new();
    let mut cursor = path.to_path_buf();
    loop {
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    anyhow::bail!("Scaffold parent is not a regular directory: {}", cursor.display());
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(cursor.clone());
                cursor = cursor.parent().ok_or_else(|| {
                    anyhow::anyhow!("Cannot find existing parent for {}", path.display())
                })?.to_path_buf();
            }
            Err(error) => {
                return Err(error).with_context(|| format!("Failed to inspect {}", cursor.display()));
            }
        }
    }

    for directory in missing.into_iter().rev() {
        fs::create_dir(&directory)
            .with_context(|| format!("Failed to create directory {}", directory.display()))?;
        created.push(directory);
    }
    Ok(())
}

fn unique_sibling(target: &Path, kind: &str, index: usize) -> Result<PathBuf> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target.file_name().and_then(|value| value.to_str()).unwrap_or("scaffold");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..1000usize {
        let candidate = parent.join(format!(
            ".{name}.scaffold-{kind}-{}-{nonce}-{index}-{attempt}",
            std::process::id()
        ));
        match fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect temporary path {}", candidate.display())
                });
            }
        }
    }
    anyhow::bail!("Could not allocate a unique sibling path for {}", target.display())
}

fn cleanup_staged(staged: &[(PathBuf, PathBuf)]) -> Result<()> {
    let mut failures = Vec::new();
    for (_, temporary) in staged {
        match fs::symlink_metadata(temporary) {
            Ok(_) => {
                if let Err(error) = fs::remove_file(temporary) {
                    failures.push(format!("{}: {error}", temporary.display()));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => failures.push(format!("{}: {error}", temporary.display())),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("Failed to remove staged files: {}", failures.join("; "))
    }
}

fn remove_created_dirs(created_dirs: &[PathBuf]) -> Result<()> {
    let mut failures = Vec::new();
    for directory in created_dirs.iter().rev() {
        match fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    || error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
            Err(error) => failures.push(format!("{}: {error}", directory.display())),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("Failed to remove created directories: {}", failures.join("; "))
    }
}

fn rollback_committed(committed: &[(PathBuf, Option<PathBuf>)]) -> Result<()> {
    let mut failures = Vec::new();
    for (target, backup) in committed.iter().rev() {
        match fs::symlink_metadata(target) {
            Ok(_) => {
                if let Err(error) = fs::remove_file(target) {
                    failures.push(format!(
                        "failed to remove partially committed {}: {error}",
                        target.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => failures.push(format!("failed to inspect {}: {error}", target.display())),
        }
        if let Some(backup) = backup {
            if let Err(error) = fs::rename(backup, target) {
                failures.push(format!(
                    "failed to restore backup {} to {}: {error}",
                    backup.display(),
                    target.display()
                ));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{}", failures.join("; "))
    }
}

fn abort_transaction(
    error: anyhow::Error,
    committed: &[(PathBuf, Option<PathBuf>)],
    staged: &[(PathBuf, PathBuf)],
    created_dirs: &[PathBuf],
) -> anyhow::Error {
    let rollback = rollback_committed(committed);
    let staged_cleanup = cleanup_staged(staged);
    let directory_cleanup = remove_created_dirs(created_dirs);
    let mut failures = Vec::new();
    if let Err(rollback) = rollback {
        failures.push(format!("rollback failed: {rollback:#}"));
    }
    if let Err(cleanup) = staged_cleanup {
        failures.push(format!("staged-file cleanup failed: {cleanup:#}"));
    }
    if let Err(cleanup) = directory_cleanup {
        failures.push(format!("directory cleanup failed: {cleanup:#}"));
    }
    if failures.is_empty() {
        error
    } else {
        anyhow::anyhow!("{error:#}; {}", failures.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_plan_inner, diff_lines, PlannedFile};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn diff_reports_only_changed_manifest_lines() {
        assert_eq!(
            diff_lines("alpha\nbeta\ngamma\n", "alpha\ninserted\ngamma\n"),
            vec!["-beta".to_string(), "+inserted".to_string()]
        );
    }

    #[test]
    fn transaction_rolls_back_earlier_files_when_a_later_commit_fails() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rustok-scaffold-atomic-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let original = root.join("a.txt");
        let created = root.join("b.txt");
        fs::write(&original, "before").unwrap();

        let plan = vec![
            PlannedFile {
                path: original.clone(),
                contents: "after".to_string(),
                original: Some("before".to_string()),
            },
            PlannedFile {
                path: created.clone(),
                contents: "new".to_string(),
                original: None,
            },
        ];
        let result = apply_plan_inner(&plan, Some(1));
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&original).unwrap(), "before");
        assert!(!created.exists());
        fs::remove_dir_all(root).unwrap();
    }
}

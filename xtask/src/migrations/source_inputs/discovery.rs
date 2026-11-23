use super::{check_commit, inventory::read_inventory, local_links, write_report};
use crate::migrations::{git, read_source, safe_relative, snapshot_git_files};
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use syn::{
    ext::IdentExt, punctuated::Punctuated, visit::Visit, Attribute, Expr, ImplItem, ItemImpl, Lit,
    Stmt, Token,
};

#[derive(Serialize)]
struct Declaration {
    migration: String,
    files: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    entrypoints: BTreeSet<String>,
}

#[derive(Serialize)]
struct Declarations {
    schema_version: u32,
    entries: Vec<Declaration>,
}

pub(in crate::migrations) fn discover(
    root: &Path,
    commit: &str,
    inventory_path: &Path,
    dep_info_path: &Path,
    build_root: &str,
) -> Result<()> {
    check_commit(root, commit)?;
    let inventory = read_inventory(&read_source(&root.join(inventory_path))?)?;
    let dep_info = read_source(&root.join(dep_info_path))?;
    let (observed, outside) = read_dep_info(&dep_info, build_root)?;
    let tree = git(
        root,
        &["ls-tree", "-rz", "--full-tree", "--name-only", commit],
    )?;
    let tree_paths: BTreeSet<_> = std::str::from_utf8(&tree)?.split_terminator('\0').collect();
    let tracked: BTreeSet<_> = observed
        .iter()
        .filter(|path| tree_paths.contains(path.as_str()))
        .cloned()
        .collect();
    let paths = tracked.iter().map(String::as_str).collect();
    let mut blobs = BTreeMap::new();
    snapshot_git_files(root, commit, None, Some(&paths), |path, bytes| {
        blobs.insert(path.to_owned(), bytes);
    })?;
    let roots = migration_roots(&blobs)?;
    let mut output = Declarations {
        schema_version: 1,
        entries: Vec::new(),
    };
    let mut unresolved = 0;
    let mut checkable = 0;
    for migration in inventory.migrations {
        let Some(paths) = roots.get(&migration) else {
            eprintln!("{migration}: no matching Rust name declaration in the compiled source list");
            unresolved += 1;
            continue;
        };
        if paths.len() != 1 {
            eprintln!("{migration}: ambiguous compiled sources {paths:?}");
            unresolved += 1;
            continue;
        }
        let mut entrypoints = paths.clone();
        let files = match local_links::discover(&entrypoints, &tracked, &tree_paths, &blobs) {
            Ok(files) => {
                checkable += 1;
                files
            }
            Err(error) => {
                eprintln!("{migration}: local links require review: {error:#}");
                std::mem::take(&mut entrypoints)
            }
        };
        output.entries.push(Declaration {
            migration,
            files,
            entrypoints,
        });
    }
    eprintln!(
        "{} migration entrypoints, {checkable} with checked local links, {unresolved} unresolved; {} compiled paths tracked in the supplied Git commit, {} untracked, {outside} outside the build root",
        output.entries.len(), tracked.len(), observed.len() - tracked.len()
    );
    write_report(&output)?;
    ensure!(
        unresolved == 0,
        "some executable migration names lack a unique compiled source"
    );
    ensure!(
        checkable == output.entries.len(),
        "some local source links require review"
    );
    Ok(())
}

fn read_dep_info(bytes: &[u8], build_root: &str) -> Result<(BTreeSet<String>, usize)> {
    let text = std::str::from_utf8(bytes).context("Cargo dep-info must be UTF-8")?;
    let mut records = text.lines().filter(|line| !line.trim().is_empty());
    let record = records.next().context("Cargo dep-info is empty")?;
    ensure!(
        records.next().is_none(),
        "expected a single Cargo binary dep-info rule"
    );
    let (target, dependencies) = record
        .split_once(": ")
        .context("invalid Cargo dep-info rule")?;
    let target = normalize(target);
    ensure!(
        target.ends_with("/export-migration-plan")
            || target.ends_with("/export-migration-plan.exe"),
        "dep-info must describe the export-migration-plan binary"
    );
    let root = normalize(build_root);
    let root = root.trim_end_matches('/');
    ensure!(
        is_absolute(root) && !root.ends_with(':'),
        "--build-root must be an absolute source directory"
    );
    let prefix = format!("{root}/");

    let mut files = BTreeSet::new();
    let mut outside = 0;
    for path in dep_tokens(dependencies) {
        let path = normalize(&path);
        ensure!(
            is_absolute(&path),
            "Cargo dep-info must contain absolute dependency paths"
        );
        if let Some(path) = path.strip_prefix(&prefix) {
            safe_relative(path)?;
            files.insert(path.to_owned());
        } else {
            outside += 1;
        }
    }
    ensure!(
        !files.is_empty(),
        "no observed compiler paths under --build-root"
    );
    Ok((files, outside))
}

fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    if let Some(path) = path.strip_prefix("//?/UNC/") {
        return format!("//{path}");
    }
    path.strip_prefix("//?/").unwrap_or(&path).to_owned()
}

fn is_absolute(path: &str) -> bool {
    path.starts_with('/')
        || matches!(path.as_bytes(), [drive, b':', b'/', ..] if drive.is_ascii_alphabetic())
}

fn dep_tokens(value: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut token = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' if chars.peek() == Some(&' ') => {
                chars.next();
                token.push(' ');
            }
            ch if ch.is_ascii_whitespace() => {
                if !token.is_empty() {
                    result.push(std::mem::take(&mut token));
                }
            }
            ch => token.push(ch),
        }
    }
    if !token.is_empty() {
        result.push(token);
    }
    result
}

fn migration_roots(
    blobs: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (path, bytes) in blobs {
        if !path.ends_with(".rs")
            || !bytes
                .windows(b"MigrationName".len())
                .any(|part| part == b"MigrationName")
        {
            continue;
        }
        let stem = path
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".rs"))
            .context("invalid Rust source path")?;
        let text = std::str::from_utf8(bytes)?;
        let file = syn::parse_file(text)
            .with_context(|| format!("cannot parse possible migration source {path}"))?;
        let mut scanner = MigrationNames {
            stem,
            names: BTreeSet::new(),
            error: None,
        };
        scanner.visit_file(&file);
        if let Some(error) = scanner.error {
            return Err(error).with_context(|| format!("cannot read migration derive in {path}"));
        }
        for name in scanner.names {
            names.entry(name).or_default().insert(path.clone());
        }
    }
    Ok(names)
}

struct MigrationNames<'a> {
    stem: &'a str,
    names: BTreeSet<String>,
    error: Option<syn::Error>,
}

impl<'ast> Visit<'ast> for MigrationNames<'_> {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        if self.error.is_none() && attribute.path().is_ident("derive") {
            match attribute.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated) {
                Ok(derives)
                    if derives.iter().any(|path| {
                        path.segments
                            .last()
                            .is_some_and(|segment| segment.ident.unraw() == "DeriveMigrationName")
                    }) =>
                {
                    self.names.insert(self.stem.to_owned());
                }
                Ok(_) => (),
                Err(error) => self.error = Some(error),
            }
        }
        syn::visit::visit_attribute(self, attribute);
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        if item
            .trait_
            .as_ref()
            .and_then(|(_, path, _)| path.segments.last())
            .is_some_and(|segment| segment.ident.unraw() == "MigrationName")
        {
            for item in &item.items {
                if let ImplItem::Fn(method) = item {
                    if method.sig.ident == "name" {
                        if let [Stmt::Expr(Expr::Lit(value), None)] = method.block.stmts.as_slice()
                        {
                            if let Lit::Str(name) = &value.lit {
                                self.names.insert(name.value());
                            }
                        }
                    }
                }
            }
        }
        syn::visit::visit_item_impl(self, item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cargo_rules_on_windows_and_unix() -> Result<()> {
        let windows = br"\\?\D:\target\export-migration-plan.exe: D:\source\root\a\ b.rs D:\source\root-other\outside.rs ";
        let (paths, outside) = read_dep_info(windows, r"D:\source\root")?;
        assert_eq!(paths, BTreeSet::from(["a b.rs".to_owned()]));
        assert_eq!(outside, 1);
        let unix = b"/target/export-migration-plan: /tmp/build/a.rs /tmp/build/b.rs\n";
        assert_eq!(
            read_dep_info(unix, "/tmp/build")?.0,
            BTreeSet::from(["a.rs".into(), "b.rs".into()])
        );
        assert!(read_dep_info(unix, "/tmp/other").is_err());
        assert!(read_dep_info(b"/target/export-migration-plan: src/a.rs", "/tmp/build").is_err());
        assert!(read_dep_info(b"/target/other: /tmp/build/a.rs", "/tmp/build").is_err());
        assert!(read_dep_info(
            b"/target/export-migration-plan: /tmp/build/../outside.rs",
            "/tmp/build"
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn maps_literal_names_and_retains_ambiguity() -> Result<()> {
        let names = migration_roots(&BTreeMap::from([
            (
                "one/step.rs".to_owned(),
                b"#[derive(Debug, DeriveMigrationName)] struct Migration;".to_vec(),
            ),
            (
                "two/step.rs".to_owned(),
                b"#[derive(DeriveMigrationName)] struct Migration;".to_vec(),
            ),
            (
                "alias/mod.rs".to_owned(),
                br#"
                impl MigrationName for First { fn name(&self) -> &str { "first" } }
                impl MigrationName for Second { fn name(&self) -> &str { "second" } }
                impl MigrationName for Unknown { fn name(&self) -> &str { concat!("a", "b") } }
            "#
                .to_vec(),
            ),
            (
                "comment.rs".to_owned(),
                b"// DeriveMigrationName for an unrelated type".to_vec(),
            ),
        ]))?;
        assert_eq!(
            names["step"],
            BTreeSet::from(["one/step.rs".into(), "two/step.rs".into()])
        );
        assert_eq!(names["first"], names["second"]);
        assert_eq!(names["first"], BTreeSet::from(["alias/mod.rs".into()]));
        assert_eq!(names.len(), 3);
        Ok(())
    }
}

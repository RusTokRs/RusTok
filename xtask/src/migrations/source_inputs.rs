use super::{
    commit, git, read_source, safe_relative, snapshot, snapshot_git_files, SourceSnapshot,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    path::Path,
};

mod discovery;
mod inventory;
mod local_links;
mod registry;

pub(super) use discovery::{compiler_sources, discover};
pub(super) use discovery::{normalize as normalize_dep_path, parse_dep_info};
pub(super) use inventory::bind_inventory;
pub(super) use registry::{
    registry_inputs, verify_registry_cache, ExternalPackage, RegistryVerification,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputDeclarations {
    schema_version: u32,
    entries: Vec<MigrationDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationDeclaration {
    migration: String,
    files: Vec<String>,
    #[serde(default)]
    entrypoints: Vec<String>,
}

struct DeclaredFiles {
    files: BTreeSet<String>,
    entrypoints: BTreeSet<String>,
}

type MigrationFiles = BTreeMap<String, DeclaredFiles>;

#[derive(Serialize)]
struct CommitInputs {
    commit: String,
    entries: Vec<MigrationInputs>,
}

#[derive(Serialize)]
struct MigrationInputs {
    migration: String,
    #[serde(flatten)]
    source: SourceSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked_local_sources: Option<BTreeSet<String>>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct ChangedInput<'a> {
    migration: &'a str,
    path: &'a str,
}

#[derive(Serialize)]
struct Report<'a> {
    schema_version: u32,
    scope: &'static str,
    reference: &'a CommitInputs,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate: Option<&'a CommitInputs>,
    changed_inputs: Vec<ChangedInput<'a>>,
}

pub(super) fn run(
    root: &Path,
    reference: &str,
    inputs: &Path,
    compare: Option<&str>,
) -> Result<()> {
    let (declarations, _) = read_declarations(inputs)?;
    let reference = fingerprint(root, reference, &declarations)?;
    let candidate = compare
        .map(|commit| fingerprint(root, commit, &declarations))
        .transpose()?;
    let changed_inputs = candidate
        .as_ref()
        .map(|candidate| compare_inputs(&reference, candidate))
        .unwrap_or_default();
    let report = Report {
        schema_version: 1,
        scope: "declared-migration-source-inputs",
        reference: &reference,
        candidate: candidate.as_ref(),
        changed_inputs,
    };
    write_report(&report)?;
    ensure!(
        report.changed_inputs.is_empty(),
        "{} declared migration source input(s) changed",
        report.changed_inputs.len()
    );
    Ok(())
}

fn write_report(report: &impl Serialize) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer_pretty(&mut stdout, report)?;
    writeln!(stdout)?;
    stdout.flush()?;
    Ok(())
}

fn read_declarations(inputs: &Path) -> Result<(MigrationFiles, Vec<u8>)> {
    let bytes = read_source(inputs)?;
    let declarations = parse_declarations(&bytes)
        .with_context(|| format!("invalid source declarations in {}", inputs.display()))?;
    Ok((declarations, bytes))
}

fn parse_declarations(bytes: &[u8]) -> Result<MigrationFiles> {
    let declarations: InputDeclarations = serde_json::from_slice(bytes)?;
    ensure!(
        declarations.schema_version == 1 && !declarations.entries.is_empty(),
        "source declarations require schema version 1 and at least one migration"
    );
    let mut migrations = BTreeMap::new();
    for MigrationDeclaration {
        migration,
        files,
        entrypoints,
    } in declarations.entries
    {
        ensure!(
            !migration.is_empty() && migration.trim() == migration,
            "invalid migration identity {migration:?}"
        );
        ensure!(
            !files.is_empty(),
            "migration {migration} must declare at least one file"
        );
        let mut paths = BTreeSet::new();
        for path in files {
            safe_relative(&path)?;
            ensure!(
                paths.insert(path.clone()),
                "migration {migration} lists {path} twice"
            );
        }
        let mut roots = BTreeSet::new();
        for path in entrypoints {
            ensure!(
                path.ends_with(".rs") && paths.contains(&path),
                "migration {migration} must list Rust entrypoint {path:?} in files"
            );
            ensure!(
                roots.insert(path.clone()),
                "migration {migration} lists entrypoint {path} twice"
            );
        }
        ensure!(
            migrations
                .insert(
                    migration.clone(),
                    DeclaredFiles {
                        files: paths,
                        entrypoints: roots
                    }
                )
                .is_none(),
            "duplicate source declaration for migration {migration}"
        );
    }
    Ok(migrations)
}

fn check_commit(root: &Path, identity: &str) -> Result<()> {
    ensure!(
        matches!(identity.len(), 40 | 64)
            && identity
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
            && commit(root, identity)? == identity,
        "source input fingerprint requires a full Git commit SHA"
    );
    Ok(())
}

fn fingerprint(root: &Path, identity: &str, declarations: &MigrationFiles) -> Result<CommitInputs> {
    check_commit(root, identity)?;
    let paths = declarations
        .values()
        .flat_map(|declaration| &declaration.files)
        .map(String::as_str)
        .collect();
    let syntax_paths: BTreeSet<_> = declarations
        .values()
        .filter(|declaration| !declaration.entrypoints.is_empty())
        .flat_map(|declaration| declaration.files.iter().map(String::as_str))
        .collect();
    let mut blobs = BTreeMap::new();
    let source = snapshot_git_files(root, identity, None, Some(&paths), |path, bytes| {
        if syntax_paths.contains(path) {
            blobs.insert(path.to_owned(), bytes);
        }
    })?;
    let tree = if syntax_paths.is_empty() {
        Vec::new()
    } else {
        git(
            root,
            &["ls-tree", "-rz", "--full-tree", "--name-only", identity],
        )?
    };
    let tree_paths: BTreeSet<_> = std::str::from_utf8(&tree)?.split_terminator('\0').collect();
    let files: BTreeMap<_, _> = source.files.iter().map(|file| (&file.path, file)).collect();
    let entries = declarations
        .iter()
        .map(|(migration, declaration)| {
            let sources = declaration
                .files
                .iter()
                .map(|path| {
                    files
                        .get(path)
                        .map(|file| (*file).clone())
                        .with_context(|| format!("source snapshot is missing {path}"))
                })
                .collect::<Result<Vec<_>>>()?;
            let checked_local_sources = if declaration.entrypoints.is_empty() {
                None
            } else {
                Some(
                    local_links::check(declaration, &tree_paths, &blobs).with_context(|| {
                        format!("local source inputs for migration {migration} at {identity}")
                    })?,
                )
            };
            Ok(MigrationInputs {
                migration: migration.clone(),
                source: snapshot(sources)?,
                checked_local_sources,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CommitInputs {
        commit: identity.to_owned(),
        entries,
    })
}

fn compare_inputs<'a>(
    reference: &'a CommitInputs,
    candidate: &CommitInputs,
) -> Vec<ChangedInput<'a>> {
    // Both projections use the same sorted declaration set and require each path.
    let mut changed = Vec::new();
    for (before, after) in reference.entries.iter().zip(&candidate.entries) {
        for (original, proposed) in before.source.files.iter().zip(&after.source.files) {
            if original != proposed {
                changed.push(ChangedInput {
                    migration: &before.migration,
                    path: &original.path,
                });
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::git;
    use serde_json::json;
    use std::{
        env, fs,
        process::{self, Command},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn staged_commit(root: &Path, message: &str) -> Result<String> {
        git(
            root,
            &[
                "-c",
                "user.name=Migration Test",
                "-c",
                "user.email=migrations@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                message,
            ],
        )?;
        commit(root, "HEAD")
    }

    #[test]
    fn declarations_are_ordered_and_allow_shared_inputs() -> Result<()> {
        let inputs = json!({"schema_version": 1, "entries": [
            {"migration": "b", "files": ["shared.rs", "b.rs"]},
            {"migration": "a", "files": ["shared.rs", "a.rs"]}
        ]});
        let checked = parse_declarations(&serde_json::to_vec(&inputs)?)?;
        assert_eq!(
            checked.keys().map(String::as_str).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            checked["a"]
                .files
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["a.rs", "shared.rs"]
        );
        Ok(())
    }

    #[test]
    fn invalid_declarations_are_rejected_before_git_reads() -> Result<()> {
        for entries in [
            json!([]),
            json!([{"migration": "a", "files": []}]),
            json!([{"migration": " a", "files": ["a.rs"]}]),
            json!([{"migration": "a", "files": ["a.rs", "a.rs"]}]),
            json!([{"migration": "a", "files": ["../a.rs"]}]),
            json!([{"migration": "a", "files": ["a.rs"], "entrypoints": ["b.rs"]}]),
            json!([{"migration": "a", "files": ["a.rs"]}, {"migration": "a", "files": ["b.rs"]}]),
        ] {
            let input = serde_json::to_vec(&json!({"schema_version": 1, "entries": entries}))?;
            assert!(parse_declarations(&input).is_err(), "{entries}");
        }
        assert!(fingerprint(Path::new("."), "HEAD", &BTreeMap::new()).is_err());
        Ok(())
    }

    #[test]
    fn fingerprints_distinguish_selected_helpers_from_other_changes() -> Result<()> {
        let temp = env::temp_dir().canonicalize()?;
        let fixture = temp.join(format!(
            "rustok-migration-inputs-{}-{}",
            process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
        ));
        fs::create_dir(&fixture)?;
        let result = (|| {
            git(&fixture, &["init", "--quiet", "--initial-branch=main"])?;
            fs::create_dir(fixture.join("schema"))?;
            fs::write(fixture.join("a.rs"), "mod schema;")?;
            fs::write(fixture.join("schema/helper.rs"), "original helper")?;
            fs::write(fixture.join("schema/tables.sql"), "original SQL")?;
            fs::write(fixture.join("unrelated.rs"), "before")?;
            git(&fixture, &["add", "."])?;
            let original = staged_commit(&fixture, "reference")?;
            let inputs = parse_declarations(&serde_json::to_vec(
                &json!({"schema_version": 1, "entries": [
                    {"migration": "a", "files": ["schema/tables.sql", "a.rs", "schema/helper.rs"]},
                    {"migration": "b", "files": ["schema/tables.sql"]}
                ]}),
            )?)?;
            let baseline = fingerprint(&fixture, &original, &inputs)?;

            fs::write(fixture.join("unrelated.rs"), "after")?;
            git(&fixture, &["add", "unrelated.rs"])?;
            let unrelated = staged_commit(&fixture, "unrelated change")?;
            let unaffected = fingerprint(&fixture, &unrelated, &inputs)?;
            assert!(compare_inputs(&baseline, &unaffected).is_empty());
            assert_eq!(
                baseline.entries[0].source.sha256,
                unaffected.entries[0].source.sha256
            );

            fs::write(fixture.join("schema/tables.sql"), "changed SQL")?;
            git(&fixture, &["add", "schema/tables.sql"])?;
            git(
                &fixture,
                &["update-index", "--chmod=+x", "schema/helper.rs"],
            )?;
            let changed = staged_commit(&fixture, "SQL and file mode change")?;
            let evidence = fingerprint(&fixture, &changed, &inputs)?;
            assert_eq!(
                compare_inputs(&baseline, &evidence),
                [
                    ChangedInput {
                        migration: "a",
                        path: "schema/helper.rs"
                    },
                    ChangedInput {
                        migration: "a",
                        path: "schema/tables.sql"
                    },
                    ChangedInput {
                        migration: "b",
                        path: "schema/tables.sql"
                    },
                ]
            );

            let blob = |commit: &str| -> Result<String> {
                Ok(String::from_utf8(git(
                    &fixture,
                    &["rev-parse", &format!("{commit}:schema/tables.sql")],
                )?)?
                .trim()
                .to_owned())
            };
            let original_blob = blob(&original)?;
            let changed_blob = blob(&changed)?;
            let status = Command::new("git")
                .current_dir(&fixture)
                .args(["replace", &original_blob, &changed_blob])
                .status()?;
            assert!(status.success());
            let normal_read = Command::new("git")
                .current_dir(&fixture)
                .args(["cat-file", "blob", &original_blob])
                .output()?;
            assert!(normal_read.status.success());
            assert_eq!(normal_read.stdout, b"changed SQL");
            fs::write(fixture.join("schema/tables.sql"), "uncommitted SQL")?;
            let reread = fingerprint(&fixture, &original, &inputs)?;
            assert!(compare_inputs(&baseline, &reread).is_empty());

            git(
                &fixture,
                &[
                    "update-index",
                    "--cacheinfo",
                    &format!("120000,{changed_blob},schema/tables.sql"),
                ],
            )?;
            let symlink = staged_commit(&fixture, "unsupported input mode")?;
            let error = fingerprint(&fixture, &symlink, &inputs)
                .err()
                .context("symlink input accepted")?;
            assert!(error.to_string().contains("not a regular blob"), "{error}");

            git(&fixture, &["rm", "--cached", "schema/tables.sql"])?;
            let missing = staged_commit(&fixture, "removed input")?;
            let error = fingerprint(&fixture, &missing, &inputs)
                .err()
                .context("missing input accepted")?;
            assert!(error.to_string().contains("schema/tables.sql"), "{error}");
            Ok(())
        })();
        ensure!(
            fixture.canonicalize()?.parent() == Some(temp.as_path()),
            "fixture moved outside its scratch parent"
        );
        fs::remove_dir_all(fixture)?;
        result
    }
}

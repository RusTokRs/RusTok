//! Local source provenance and process orchestration. Executable composition
//! and PostgreSQL semantics belong to rustok-migrations.

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, BTreeSet}, fs, io::Write, path::{Component, Path, PathBuf}, process::Command, time::{SystemTime, UNIX_EPOCH}};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SourceFile {
    path: String,
    mode: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SourceSnapshot {
    sha256: String,
    files: Vec<SourceFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema_version: u32,
    purpose: String,
    reference_commit: String,
    main_commit: String,
    candidate_head: String,
    reference: SourceSnapshot,
    candidate: SourceSnapshot,
    replacement_admission: String,
}

pub fn run(args: &[String]) -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().context("workspace root missing")?.canonicalize()?;
    let Some(command) = args.first() else { return help(); };
    if command == "--help" || command == "-h" { return help(); }
    let allowed: &[&str] = match command.as_str() {
        "plan" => &["--reference", "--output"],
        "verify-fresh" => &["--plan", "--verifier", "--pg-dump", "--admin-url-env"],
        _ => bail!("unknown migration command {command}"),
    };
    let options = parse_options(&args[1..], allowed)?;
    let required = |key: &str| options.get(key).map(String::as_str).with_context(|| format!("{key} is required"));
    match command.as_str() {
        "plan" => {
            let id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let default_output = format!("target/migrations/{id}");
            let output = options.get("--output").map(String::as_str).unwrap_or(&default_output);
            prepare(&root, required("--reference")?, Path::new(output))
        }
        "verify-fresh" => verify_fresh(&root, Path::new(required("--plan")?), Path::new(required("--verifier")?), Path::new(required("--pg-dump")?), required("--admin-url-env")?),
        _ => unreachable!(),
    }
}

fn help() -> Result<()> {
    println!("cargo xtask migrations plan --reference REF [--output target/migrations/RUN]");
    println!("cargo xtask migrations verify-fresh --plan FILE --verifier FILE --pg-dump FILE --admin-url-env ENV");
    println!("These commands produce scoped evidence. Replacement admission remains blocked until every release, upgrade and recovery gate is implemented.");
    Ok(())
}

fn parse_options(args: &[String], allowed: &[&str]) -> Result<BTreeMap<String, String>> {
    let mut options = BTreeMap::new();
    let mut iter = args.iter();
    while let Some(key) = iter.next() {
        ensure!(allowed.contains(&key.as_str()), "unknown argument {key}");
        let value = iter.next().with_context(|| format!("{key} requires a value"))?;
        ensure!(!value.starts_with("--"), "{key} requires a value");
        ensure!(options.insert(key.clone(), value.clone()).is_none(), "duplicate {key}");
    }
    Ok(options)
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git").current_dir(root).args(args).output().context("cannot run Git")?;
    ensure!(output.status.success(), "Git {} failed", args.first().unwrap_or(&"operation"));
    Ok(output.stdout)
}

fn commit(root: &Path, reference: &str) -> Result<String> {
    ensure!(!reference.starts_with('-'), "reference cannot be an option");
    let resolved = String::from_utf8(git(root, &["rev-parse", "--verify", &format!("{reference}^{{commit}}")])?)?.trim().to_string();
    ensure!((resolved.len() == 40 || resolved.len() == 64) && resolved.bytes().all(|b| b.is_ascii_hexdigit()), "invalid Git commit identity");
    Ok(resolved)
}

fn safe_relative(path: &str) -> Result<PathBuf> {
    ensure!(!path.is_empty() && !path.contains('\\') && !path.contains(':') && !path.contains('\0'), "unsupported source path");
    let value = PathBuf::from(path);
    ensure!(value.components().all(|c| matches!(c, Component::Normal(_))), "source path escapes snapshot: {path}");
    ensure!(path.split('/').all(|part| !matches!(part, "" | "." | ".." | ".git")), "unsafe source path {path}");
    Ok(value)
}

fn assert_regular(root: &Path, relative: &Path) -> Result<()> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current)?;
        ensure!(!metadata.file_type().is_symlink(), "symlink input is unsupported: {}", relative.display());
        #[cfg(windows)] {
            use std::os::windows::fs::MetadataExt;
            ensure!(metadata.file_attributes() & 0x400 == 0, "reparse-point input is unsupported: {}", relative.display());
        }
    }
    ensure!(current.is_file(), "source input is not a regular file: {}", relative.display());
    Ok(())
}

fn digest(bytes: &[u8]) -> String { hex::encode(Sha256::digest(bytes)) }

fn snapshot(files: Vec<SourceFile>) -> Result<SourceSnapshot> {
    ensure!(!files.is_empty(), "source snapshot is empty");
    Ok(SourceSnapshot { sha256: digest(&serde_json::to_vec(&files)?), files })
}

fn indexed_modes(root: &Path) -> Result<BTreeMap<String, String>> {
    let bytes = git(root, &["ls-files", "--stage", "-z"])?;
    let mut modes = BTreeMap::new();
    for entry in bytes.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = std::str::from_utf8(entry).context("non-UTF-8 source path")?;
        let (metadata, path) = entry.split_once('\t').context("invalid Git index entry")?;
        let fields: Vec<_> = metadata.split(' ').collect();
        ensure!(fields.len() == 3 && fields[2] == "0", "resolve index conflicts before planning");
        ensure!(matches!(fields[0], "100644" | "100755"), "symlinks and submodules are unsupported: {path}");
        modes.insert(path.into(), fields[0].into());
    }
    Ok(modes)
}

fn working_snapshot(root: &Path, destination: Option<&Path>) -> Result<SourceSnapshot> {
    let indexed = indexed_modes(root)?;
    let bytes = git(root, &["ls-files", "--cached", "--others", "--exclude-standard", "-z"])?;
    let paths: BTreeSet<_> = bytes.split(|b| *b == 0).filter(|e| !e.is_empty())
        .map(|p| std::str::from_utf8(p).map(str::to_owned)).collect::<std::result::Result<_, _>>()?;
    let mut files = Vec::new();
    for path in paths {
        let relative = safe_relative(&path)?;
        if !root.join(&relative).try_exists()? { continue; } // intentional deletion
        assert_regular(root, &relative)?;
        let bytes = fs::read(root.join(&relative))?;
        let mode = file_mode(&root.join(&relative), indexed.get(&path).map(String::as_str))?;
        files.push(SourceFile { path, mode, sha256: digest(&bytes) });
        if let Some(destination) = destination {
            let target = destination.join(&relative);
            fs::create_dir_all(target.parent().context("source parent missing")?)?;
            fs::write(&target, bytes)?;
            fs::set_permissions(&target, fs::metadata(root.join(&relative))?.permissions())?;
        }
    }
    snapshot(files)
}

fn file_mode(path: &Path, indexed: Option<&str>) -> Result<String> {
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        return Ok(if fs::metadata(path)?.permissions().mode() & 0o111 != 0 { "100755" } else { "100644" }.into());
    }
    #[cfg(not(unix))] {
        let _ = path;
        Ok(indexed.unwrap_or("100644").into())
    }
}

fn prepare(root: &Path, reference: &str, output: &Path) -> Result<()> {
    let reference_commit = commit(root, reference)?;
    let main_commit = commit(root, "main")?;
    let candidate_head = commit(root, "HEAD")?;
    let relative = output.strip_prefix(root).unwrap_or(output);
    safe_relative(relative.to_str().context("plan output path is not UTF-8")?.replace('\\', "/").as_str())?;
    ensure!(relative.starts_with("target/migrations"), "plan output must be inside target/migrations");
    let output = root.join(relative);
    ensure!(!output.exists(), "plan directory already exists");
    fs::create_dir_all(root.join("target/migrations"))?;
    let boundary = root.join("target/migrations").canonicalize()?;
    let parent = output.parent().context("plan parent missing")?;
    fs::create_dir_all(parent)?;
    ensure!(parent.canonicalize()?.starts_with(&boundary), "plan path escapes target/migrations");
    fs::create_dir(&output)?;
    let original = output.join("reference");
    let candidate = output.join("candidate");
    fs::create_dir(&original)?;
    fs::create_dir(&candidate)?;
    let tree = git(root, &["ls-tree", "-rz", "--full-tree", &reference_commit])?;
    let mut reference_files = Vec::new();
    for entry in tree.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let (metadata, path) = std::str::from_utf8(entry)?.split_once('\t').context("invalid Git tree entry")?;
        let mode = metadata.split(' ').next().context("missing source mode")?;
        ensure!(matches!(mode, "100644" | "100755"), "reference symlinks and submodules are unsupported: {path}");
        safe_relative(path)?;
        reference_files.push(SourceFile { path: path.into(), mode: mode.into(), sha256: String::new() });
    }
    let archive = output.join("reference.tar");
    let archive_str = archive.to_str().context("archive path is not UTF-8")?;
    git(root, &["archive", "--format=tar", "--output", archive_str, &reference_commit])?;
    let status = Command::new("tar").arg("-xf").arg(&archive).arg("-C").arg(&original).status().context("tar is required for source snapshots")?;
    ensure!(status.success(), "reference extraction failed");
    fs::remove_file(&archive)?;
    reference_files.sort_by(|a,b| a.path.cmp(&b.path));
    for file in &mut reference_files {
        let relative = safe_relative(&file.path)?;
        assert_regular(&original, &relative)?;
        file.sha256 = digest(&fs::read(original.join(relative))?);
    }
    let reference = snapshot(reference_files)?;
    let candidate_snapshot = working_snapshot(root, Some(&candidate))?;
    ensure!(working_snapshot(root, None)? == candidate_snapshot, "working source changed during snapshot; create a new plan");
    let plan = Plan {
        schema_version: 1,
        purpose: "local migration equivalence evidence".into(),
        reference_commit, main_commit, candidate_head, reference,
        candidate: candidate_snapshot,
        replacement_admission: "blocked pending trusted release, upgrade, activation and recovery gates".into(),
    };
    write_new(&output.join("plan.json"), &serde_json::to_vec_pretty(&plan)?)?;
    println!("migration source plan {}", output.join("plan.json").display());
    println!("No Git branch or worktree was created. Candidate input SHA256 {}", plan.candidate.sha256);
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn validate_snapshot(root: &Path, expected: &SourceSnapshot) -> Result<()> {
    let mut files = Vec::new();
    collect_snapshot_files(root, root, &mut files)?;
    files.sort_by(|a,b| a.path.cmp(&b.path));
    #[cfg(not(unix))] for file in &mut files {
        if let Some(input) = expected.files.iter().find(|input| input.path == file.path) { file.mode = input.mode.clone(); }
    }
    ensure!(snapshot(files)? == *expected, "source snapshot changed; create a new plan");
    Ok(())
}

fn collect_snapshot_files(root: &Path, directory: &Path, files: &mut Vec<SourceFile>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root)?;
        ensure!(!entry.file_type()?.is_symlink(), "snapshot contains a symlink");
        #[cfg(windows)] {
            use std::os::windows::fs::MetadataExt;
            ensure!(entry.metadata()?.file_attributes() & 0x400 == 0, "snapshot contains a reparse point");
        }
        if entry.file_type()?.is_dir() { collect_snapshot_files(root, &path, files)?; }
        else {
            assert_regular(root, relative)?;
            let path_string = relative.to_str().context("snapshot path is not UTF-8")?.replace('\\', "/");
            safe_relative(&path_string)?;
            files.push(SourceFile { path: path_string, mode: file_mode(&path, None)?, sha256: digest(&fs::read(&path)?) });
        }
    }
    Ok(())
}

fn verify_fresh(root: &Path, plan_path: &Path, verifier: &Path, pg_dump: &Path, admin_env: &str) -> Result<()> {
    let plan_path = root.join(plan_path).canonicalize()?;
    let directory = plan_path.parent().context("plan directory missing")?;
    let plan: Plan = serde_json::from_slice(&fs::read(&plan_path)?)?;
    ensure!(plan.schema_version == 1, "unsupported source plan version");
    let reference = directory.join("reference");
    let candidate = directory.join("candidate");
    validate_snapshot(&reference, &plan.reference)?;
    validate_snapshot(&candidate, &plan.candidate)?;
    ensure!(working_snapshot(root, None)? == plan.candidate, "working source changed after plan creation");
    let verifier = root.join(verifier).canonicalize()?;
    let pg_dump = if pg_dump.components().count() == 1 { pg_dump.to_path_buf() } else { root.join(pg_dump).canonicalize()? };
    ensure!(std::env::var_os(admin_env).is_some(), "administrator URL environment variable is missing");
    let evidence = directory.join("fresh");
    fs::create_dir(&evidence).context("fresh evidence already exists; create a new plan")?;
    let mut binaries = BTreeMap::new();
    for (side, source) in [("reference", &reference), ("candidate", &candidate)] {
        let target = evidence.join(format!("build-{side}"));
        let status = Command::new("cargo").current_dir(source)
            .args(["build", "--locked", "-p", "rustok-migrations", "--bin", "rustok-migrate", "--bin", "export-migration-plan"])
            .env("CARGO_TARGET_DIR", &target).env_remove(admin_env).status()?;
        ensure!(status.success(), "{side} migration build failed");
        let runner = target.join("debug").join(format!("rustok-migrate{}", std::env::consts::EXE_SUFFIX));
        let exporter = target.join("debug").join(format!("export-migration-plan{}", std::env::consts::EXE_SUFFIX));
        let plan_output = evidence.join(format!("{side}-plan.json"));
        let status = Command::new(&exporter).current_dir(source).arg("--output").arg(&plan_output).env_remove(admin_env).status()?;
        ensure!(status.success(), "{side} executable migration plan export failed");
        validate_snapshot(source, if side == "reference" { &plan.reference } else { &plan.candidate })?;
        binaries.insert(side, (runner, plan_output));
    }
    let (reference_runner, reference_plan) = &binaries["reference"];
    let (candidate_runner, candidate_plan) = &binaries["candidate"];
    let report_path = evidence.join("database-report.json");
    let status = Command::new(&verifier).arg("fresh")
        .arg("--admin-url-env").arg(admin_env)
        .arg("--reference-runner").arg(reference_runner)
        .arg("--candidate-runner").arg(candidate_runner)
        .arg("--reference-plan").arg(reference_plan)
        .arg("--candidate-plan").arg(candidate_plan)
        .arg("--pg-dump").arg(&pg_dump).arg("--output").arg(&report_path).status()?;
    ensure!(status.success(), "fresh database verification failed; inspect {}", evidence.display());
    validate_snapshot(&reference, &plan.reference)?;
    validate_snapshot(&candidate, &plan.candidate)?;
    ensure!(working_snapshot(root, None)? == plan.candidate, "working source changed during verification; evidence is stale");
    let report = serde_json::json!({
        "schema_version": 1,
        "scope": "local-fresh-install-equivalence",
        "source_plan_sha256": digest(&fs::read(&plan_path)?),
        "reference_source_sha256": plan.reference.sha256,
        "candidate_source_sha256": plan.candidate.sha256,
        "verifier_sha256": digest(&fs::read(&verifier)?),
        "database_report_sha256": digest(&fs::read(&report_path)?),
        "replacement_admission": "blocked pending trusted release, upgrade, activation and recovery gates"
    });
    write_new(&evidence.join("report.json"), &serde_json::to_vec_pretty(&report)?)?;
    println!("Fresh-install evidence {}", evidence.join("report.json").display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_escape_paths_and_option_confusion() {
        for path in ["../file", "/file", "C:/file", "a/../file", "a\\file", ".git/config", "a//file"] { assert!(safe_relative(path).is_err(), "{path}"); }
        assert!(safe_relative("src/migrations/schema/tables.rs").is_ok());
        assert!(parse_options(&["--plan".into(), "--verifier".into()], &["--plan", "--verifier"]).is_err());
        assert!(parse_options(&["--plan".into(), "a".into(), "--plan".into(), "b".into()], &["--plan"]).is_err());
    }
    #[test]
    fn source_digest_binds_paths_modes_and_bytes() {
        let files = vec![SourceFile { path: "a.rs".into(), mode: "100644".into(), sha256: digest(b"original") }];
        let original = snapshot(files.clone()).unwrap();
        for mutate in [0,1,2] {
            let mut changed = files.clone();
            match mutate { 0 => changed[0].path = "b.rs".into(), 1 => changed[0].mode = "100755".into(), _ => changed[0].sha256 = digest(b"changed") }
            assert_ne!(original.sha256, snapshot(changed).unwrap().sha256);
        }
    }
}

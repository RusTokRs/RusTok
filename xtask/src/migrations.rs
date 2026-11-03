//! Local source provenance and process orchestration. Executable composition
//! and PostgreSQL semantics belong to rustok-migrations.

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

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
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace root missing")?
        .canonicalize()?;
    let Some(command) = args.first() else {
        return help();
    };
    if command == "--help" || command == "-h" {
        return help();
    }
    let allowed: &[&str] = match command.as_str() {
        "plan" => &["--reference", "--output"],
        "verify-fresh" => &["--plan", "--verifier", "--pg-dump", "--admin-url-env"],
        _ => bail!("unknown migration command {command}"),
    };
    let options = parse_options(&args[1..], allowed)?;
    let required = |key: &str| {
        options
            .get(key)
            .map(String::as_str)
            .with_context(|| format!("{key} is required"))
    };
    match command.as_str() {
        "plan" => {
            let id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let default_output = format!("target/migrations/{id}");
            let output = options
                .get("--output")
                .map(String::as_str)
                .unwrap_or(&default_output);
            prepare(&root, required("--reference")?, Path::new(output))
        }
        "verify-fresh" => verify_fresh(
            &root,
            Path::new(required("--plan")?),
            Path::new(required("--verifier")?),
            Path::new(required("--pg-dump")?),
            required("--admin-url-env")?,
        ),
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
        let value = iter
            .next()
            .with_context(|| format!("{key} requires a value"))?;
        ensure!(!value.starts_with("--"), "{key} requires a value");
        ensure!(
            options.insert(key.clone(), value.clone()).is_none(),
            "duplicate {key}"
        );
    }
    Ok(options)
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .context("cannot run Git")?;
    ensure!(
        output.status.success(),
        "Git {} failed",
        args.first().unwrap_or(&"operation")
    );
    Ok(output.stdout)
}

fn commit(root: &Path, reference: &str) -> Result<String> {
    ensure!(!reference.starts_with('-'), "reference cannot be an option");
    let resolved = String::from_utf8(git(
        root,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )?)?
    .trim()
    .to_string();
    ensure!(
        (resolved.len() == 40 || resolved.len() == 64)
            && resolved.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid Git commit identity"
    );
    Ok(resolved)
}

fn safe_relative(path: &str) -> Result<PathBuf> {
    ensure!(
        !path.is_empty() && !path.contains(['\\', ':', '\0', '\r', '\n']),
        "unsupported source path"
    );
    let value = PathBuf::from(path);
    ensure!(
        value
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "source path escapes snapshot: {path}"
    );
    ensure!(
        path.split('/')
            .all(|part| !matches!(part, "" | "." | ".." | ".git")),
        "unsafe source path {path}"
    );
    Ok(value)
}

fn assert_regular(root: &Path, relative: &Path) -> Result<()> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        ensure!(
            !metadata.file_type().is_symlink(),
            "symlink input is unsupported: {}",
            relative.display()
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "reparse-point input is unsupported: {}",
                relative.display()
            );
        }
    }
    ensure!(
        current.is_file(),
        "source input is not a regular file: {}",
        relative.display()
    );
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn read_source(path: &Path) -> Result<Vec<u8>> {
    const LIMIT: usize = 128 * 1024 * 1024;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= LIMIT,
        "source input exceeds 128 MiB limit: {}",
        path.display()
    );
    Ok(bytes)
}

fn snapshot(files: Vec<SourceFile>) -> Result<SourceSnapshot> {
    ensure!(!files.is_empty(), "source snapshot is empty");
    Ok(SourceSnapshot {
        sha256: digest(&serde_json::to_vec(&files)?),
        files,
    })
}

fn indexed_modes(root: &Path) -> Result<BTreeMap<String, String>> {
    let bytes = git(root, &["ls-files", "--stage", "-z"])?;
    let mut modes = BTreeMap::new();
    for entry in bytes.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = std::str::from_utf8(entry).context("non-UTF-8 source path")?;
        let (metadata, path) = entry.split_once('\t').context("invalid Git index entry")?;
        let fields: Vec<_> = metadata.split(' ').collect();
        ensure!(
            fields.len() == 3 && fields[2] == "0",
            "resolve index conflicts before planning"
        );
        ensure!(
            matches!(fields[0], "100644" | "100755"),
            "symlinks and submodules are unsupported: {path}"
        );
        modes.insert(path.into(), fields[0].into());
    }
    Ok(modes)
}

fn working_snapshot(root: &Path, destination: Option<&Path>) -> Result<SourceSnapshot> {
    let indexed = indexed_modes(root)?;
    let bytes = git(
        root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?;
    let paths: BTreeSet<_> = bytes
        .split(|b| *b == 0)
        .filter(|e| !e.is_empty())
        .map(|p| std::str::from_utf8(p).map(str::to_owned))
        .collect::<std::result::Result<_, _>>()?;
    let mut files = Vec::new();
    for path in paths {
        let relative = safe_relative(&path)?;
        assert_regular(root, &relative)?;
        if !root.join(&relative).try_exists()? {
            continue;
        } // intentional deletion
        let bytes = read_source(&root.join(&relative))?;
        let mode = file_mode(
            &root.join(&relative),
            indexed.get(&path).map(String::as_str),
        )?;
        files.push(SourceFile {
            path,
            mode,
            sha256: digest(&bytes),
        });
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return Ok(if fs::metadata(path)?.permissions().mode() & 0o111 != 0 {
            "100755"
        } else {
            "100644"
        }
        .into());
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(indexed.unwrap_or("100644").into())
    }
}

fn prepare(root: &Path, reference: &str, output: &Path) -> Result<()> {
    let reference_commit = commit(root, reference)?;
    let main_commit = commit(root, "main")?;
    let candidate_head = commit(root, "HEAD")?;
    let relative = output.strip_prefix(root).unwrap_or(output);
    safe_relative(
        relative
            .to_str()
            .context("plan output path is not UTF-8")?
            .replace('\\', "/")
            .as_str(),
    )?;
    ensure!(
        relative.starts_with("target/migrations"),
        "plan output must be inside target/migrations"
    );
    let output = root.join(relative);
    ensure!(!output.exists(), "plan directory already exists");
    fs::create_dir_all(root.join("target/migrations"))?;
    let boundary = root.join("target/migrations").canonicalize()?;
    ensure!(
        boundary.starts_with(root),
        "target/migrations redirects outside the workspace"
    );
    let parent = output.parent().context("plan parent missing")?;
    fs::create_dir_all(parent)?;
    ensure!(
        parent.canonicalize()?.starts_with(&boundary),
        "plan path escapes target/migrations"
    );
    fs::create_dir(&output)?;
    let original = output.join("reference");
    let candidate = output.join("candidate");
    fs::create_dir(&original)?;
    fs::create_dir(&candidate)?;
    let reference = git_snapshot(root, &reference_commit, Some(&original))?;
    let candidate_snapshot = working_snapshot(root, Some(&candidate))?;
    ensure!(
        working_snapshot(root, None)? == candidate_snapshot,
        "working source changed during snapshot; create a new plan"
    );
    let plan = Plan {
        schema_version: 1,
        purpose: "local migration equivalence evidence".into(),
        reference_commit,
        main_commit,
        candidate_head,
        reference,
        candidate: candidate_snapshot,
        replacement_admission:
            "blocked pending trusted release, upgrade, activation and recovery gates".into(),
    };
    write_new(
        &output.join("plan.json"),
        &serde_json::to_vec_pretty(&plan)?,
    )?;
    println!(
        "migration source plan {}",
        output.join("plan.json").display()
    );
    println!(
        "No Git branch or worktree was created. Candidate input SHA256 {}",
        plan.candidate.sha256
    );
    Ok(())
}

fn git_snapshot(
    root: &Path,
    reference_commit: &str,
    destination: Option<&Path>,
) -> Result<SourceSnapshot> {
    let tree = git(root, &["ls-tree", "-rz", "--full-tree", reference_commit])?;
    let mut entries = Vec::new();
    for entry in tree.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let (metadata, path) = std::str::from_utf8(entry)?
            .split_once('\t')
            .context("invalid Git tree entry")?;
        let fields: Vec<_> = metadata.split(' ').collect();
        ensure!(fields.len() == 3, "invalid Git tree metadata");
        let mode = fields[0];
        ensure!(
            matches!(mode, "100644" | "100755"),
            "reference symlinks and submodules are unsupported: {path}"
        );
        safe_relative(path)?;
        entries.push((path.to_string(), mode.to_string(), fields[2].to_string()));
    }
    ensure!(!entries.is_empty(), "Git source tree is empty");
    let input = entries
        .iter()
        .map(|(_, _, oid)| format!("{oid}\n"))
        .collect::<String>();
    let mut child = Command::new("git")
        .current_dir(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().context("Git stdin missing")?;
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let mut reader = BufReader::new(child.stdout.take().context("Git stdout missing")?);
    let result = (|| {
        let mut files = Vec::new();
        for (path, mode, oid) in entries {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let fields: Vec<_> = header.trim_end().split(' ').collect();
            ensure!(
                fields.len() == 3 && fields[0] == oid && fields[1] == "blob",
                "Git blob identity mismatch"
            );
            let size: usize = fields[2].parse()?;
            ensure!(
                size <= 128 * 1024 * 1024,
                "source input exceeds 128 MiB limit"
            );
            let mut bytes = vec![0u8; size];
            reader.read_exact(&mut bytes)?;
            let mut separator = [0u8];
            reader.read_exact(&mut separator)?;
            ensure!(separator[0] == b'\n', "invalid Git batch separator");
            if let Some(destination) = destination {
                let target = destination.join(safe_relative(&path)?);
                fs::create_dir_all(target.parent().context("source parent missing")?)?;
                fs::write(&target, &bytes)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(
                        &target,
                        fs::Permissions::from_mode(if mode == "100755" { 0o755 } else { 0o644 }),
                    )?;
                }
            }
            files.push(SourceFile {
                path,
                mode,
                sha256: digest(&bytes),
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        snapshot(files)
    })();
    if result.is_err() {
        child.kill()?;
    }
    drop(reader);
    let status = child.wait()?;
    let written = writer
        .join()
        .map_err(|_| anyhow::anyhow!("Git writer failed"))?;
    let result = result?;
    written?;
    ensure!(status.success(), "Git blob export failed");
    Ok(result)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn validate_snapshot(root: &Path, expected: &SourceSnapshot) -> Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "snapshot root is a symlink"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "snapshot root is a reparse point"
        );
    }
    let mut files = Vec::new();
    collect_snapshot_files(root, root, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    #[cfg(not(unix))]
    for file in &mut files {
        if let Some(input) = expected.files.iter().find(|input| input.path == file.path) {
            file.mode = input.mode.clone();
        }
    }
    ensure!(
        snapshot(files)? == *expected,
        "source snapshot changed; create a new plan"
    );
    Ok(())
}

fn collect_snapshot_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<SourceFile>,
) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root)?;
        ensure!(
            !entry.file_type()?.is_symlink(),
            "snapshot contains a symlink"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                entry.metadata()?.file_attributes() & 0x400 == 0,
                "snapshot contains a reparse point"
            );
        }
        if entry.file_type()?.is_dir() {
            collect_snapshot_files(root, &path, files)?;
        } else {
            assert_regular(root, relative)?;
            let path_string = relative
                .to_str()
                .context("snapshot path is not UTF-8")?
                .replace('\\', "/");
            safe_relative(&path_string)?;
            files.push(SourceFile {
                path: path_string,
                mode: file_mode(&path, None)?,
                sha256: digest(&read_source(&path)?),
            });
        }
    }
    Ok(())
}

fn verify_fresh(
    root: &Path,
    plan_path: &Path,
    verifier: &Path,
    pg_dump: &Path,
    admin_env: &str,
) -> Result<()> {
    let plan_path = root.join(plan_path).canonicalize()?;
    let directory = plan_path.parent().context("plan directory missing")?;
    let plan_bytes = fs::read(&plan_path)?;
    let plan: Plan = serde_json::from_slice(&plan_bytes)?;
    ensure!(plan.schema_version == 1, "unsupported source plan version");
    let reference = directory.join("reference");
    let candidate = directory.join("candidate");
    validate_snapshot(&reference, &plan.reference)?;
    validate_snapshot(&candidate, &plan.candidate)?;
    ensure!(
        git_snapshot(root, &plan.reference_commit, None)? == plan.reference,
        "reference snapshot does not match the declared Git commit"
    );
    ensure!(
        working_snapshot(root, None)? == plan.candidate,
        "working source changed after plan creation"
    );
    let verifier = root.join(verifier).canonicalize()?;
    let pg_dump = root
        .join(pg_dump)
        .canonicalize()
        .context("--pg-dump requires a file path")?;
    let verifier_digest = file_digest(&verifier)?;
    let pg_dump_digest = file_digest(&pg_dump)?;
    ensure!(
        std::env::var_os(admin_env).is_some(),
        "administrator URL environment variable is missing"
    );
    let evidence = directory.join("fresh");
    fs::create_dir(&evidence).context("fresh evidence already exists; create a new plan")?;
    let temp = std::env::temp_dir().canonicalize()?;
    let scratch = temp.join(format!(
        "rustok-migration-verify-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&scratch)?;
    let outcome: Result<()> = (|| {
        // Cargo and build.rs must not discover the active repository's ancestors.
        // Global Cargo config is unsupported here; registry/locked package caches
        // are reused, with ambient credential/build environment removed.
        check_external_cargo_config(&scratch)?;
        let original_build = scratch.join("reference");
        let candidate_build = scratch.join("candidate");
        copy_snapshot(&reference, &original_build, &plan.reference)?;
        copy_snapshot(&candidate, &candidate_build, &plan.candidate)?;
        let mut binaries = BTreeMap::new();
        let mut input_digests = BTreeMap::new();
        let mut build_identity = BTreeMap::new();
        for (side, source) in [
            ("reference", &original_build),
            ("candidate", &candidate_build),
        ] {
            let target = evidence.join(format!("build-{side}"));
            let mut cargo_version = build_command("cargo", source)?;
            cargo_version.arg("--version");
            let cargo_version = String::from_utf8(run_process(&mut cargo_version, 30, 4096)?)?;
            let mut rustc_version = build_command("rustc", source)?;
            rustc_version.arg("-Vv");
            let rustc_version = String::from_utf8(run_process(&mut rustc_version, 30, 4096)?)?;
            let build_args = [
                "build",
                "--locked",
                "--offline",
                "-p",
                "rustok-migrations",
                "--bin",
                "rustok-migrate",
                "--bin",
                "export-migration-plan",
                "--message-format=json-render-diagnostics",
            ];
            build_identity.insert(side, serde_json::json!({ "cargo": cargo_version, "rustc": rustc_version, "arguments": build_args, "ambient_cargo_config": "rejected", "source_git_discovery": "disabled" }));
            let mut build = build_command("cargo", source)?;
            build
                .args(build_args)
                .env("CARGO_TARGET_DIR", &target)
                .env("CARGO_BUILD_JOBS", "2");
            let output = run_process(&mut build, 1800, 64 * 1024 * 1024)
                .with_context(|| format!("{side} migration build failed"))?;
            let mut artifacts = BTreeMap::new();
            for line in output
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
            {
                let value: serde_json::Value =
                    serde_json::from_slice(line).context("invalid Cargo artifact message")?;
                if value["reason"] == "compiler-artifact" {
                    if let (Some(name), Some(executable)) = (
                        value["target"]["name"].as_str(),
                        value["executable"].as_str(),
                    ) {
                        if matches!(name, "rustok-migrate" | "export-migration-plan") {
                            let executable = PathBuf::from(executable).canonicalize()?;
                            ensure!(
                                executable.starts_with(target.canonicalize()?),
                                "Cargo executable escaped build output"
                            );
                            artifacts.insert(name.to_string(), executable);
                        }
                    }
                }
            }
            let runner = artifacts
                .remove("rustok-migrate")
                .context("Cargo did not produce rustok-migrate")?;
            let exporter = artifacts
                .remove("export-migration-plan")
                .context("Cargo did not produce export-migration-plan")?;
            input_digests.insert(runner.clone(), file_digest(&runner)?);
            input_digests.insert(exporter.clone(), file_digest(&exporter)?);
            let plan_output = evidence.join(format!("{side}-plan.json"));
            let mut export = runtime_command(&exporter);
            export.current_dir(source).arg("--output").arg(&plan_output);
            run_process(&mut export, 30, 4096)
                .with_context(|| format!("{side} executable migration plan export failed"))?;
            input_digests.insert(plan_output.clone(), file_digest(&plan_output)?);
            validate_snapshot(
                source,
                if side == "reference" {
                    &plan.reference
                } else {
                    &plan.candidate
                },
            )?;
            binaries.insert(side, (runner, plan_output));
        }
        let (reference_runner, reference_plan) = &binaries["reference"];
        let (candidate_runner, candidate_plan) = &binaries["candidate"];
        let report_path = evidence.join("database-report.json");
        let mut verify = runtime_command(&verifier);
        verify
            .current_dir(&scratch)
            .env(
                admin_env,
                std::env::var_os(admin_env).context("admin URL disappeared")?,
            )
            .arg("fresh")
            .arg("--admin-url-env")
            .arg(admin_env)
            .arg("--reference-runner")
            .arg(reference_runner)
            .arg("--candidate-runner")
            .arg(candidate_runner)
            .arg("--reference-root")
            .arg(&original_build)
            .arg("--candidate-root")
            .arg(&candidate_build)
            .arg("--reference-plan")
            .arg(reference_plan)
            .arg("--candidate-plan")
            .arg(candidate_plan)
            .arg("--pg-dump")
            .arg(&pg_dump)
            .arg("--output")
            .arg(&report_path);
        run_process(&mut verify, 900, 1024 * 1024).with_context(|| {
            format!(
                "fresh database verification failed; inspect {}",
                evidence.display()
            )
        })?;
        let database_report_bytes = fs::read(&report_path)?;
        let database_report: serde_json::Value = serde_json::from_slice(&database_report_bytes)?;
        let expected_inputs = [
            ("reference-runner", reference_runner),
            ("candidate-runner", candidate_runner),
            ("reference-plan", reference_plan),
            ("candidate-plan", candidate_plan),
            ("pg-dump", &pg_dump),
        ]
        .into_iter()
        .map(|(key, path)| Ok((key.to_string(), file_digest(path)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
        validate_owner_report(&database_report, &expected_inputs)?;
        for (path, digest) in &input_digests {
            ensure!(
                file_digest(path)? == *digest,
                "build input changed during verification"
            );
        }
        ensure!(
            file_digest(&verifier)? == verifier_digest && file_digest(&pg_dump)? == pg_dump_digest,
            "verification tool changed during execution"
        );
        ensure!(
            fs::read(&plan_path)? == plan_bytes,
            "source plan changed during execution"
        );
        validate_snapshot(&reference, &plan.reference)?;
        validate_snapshot(&candidate, &plan.candidate)?;
        validate_snapshot(&original_build, &plan.reference)?;
        validate_snapshot(&candidate_build, &plan.candidate)?;
        ensure!(
            working_snapshot(root, None)? == plan.candidate,
            "working source changed during verification; evidence is stale"
        );
        let report = serde_json::json!({
            "schema_version": 1,
            "scope": "local-fresh-install-equivalence",
            "source_plan_sha256": digest(&plan_bytes),
            "reference_source_sha256": plan.reference.sha256,
            "candidate_source_sha256": plan.candidate.sha256,
            "verifier_sha256": verifier_digest,
            "pg_dump_sha256": pg_dump_digest,
            "build_identity": build_identity,
            "build_inputs": input_digests.iter().map(|(path,hash)| serde_json::json!({"path": path, "sha256": hash})).collect::<Vec<_>>(),
            "database_report_sha256": digest(&database_report_bytes),
            "replacement_admission": "blocked pending trusted release, upgrade, activation and recovery gates"
        });
        check_external_cargo_config(&scratch)?;
        write_new(
            &evidence.join("pending-report.json"),
            &serde_json::to_vec_pretty(&report)?,
        )?;
        Ok(())
    })();
    let resolved = scratch.canonicalize()?;
    ensure!(
        resolved.parent() == Some(temp.as_path())
            && resolved
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("rustok-migration-verify-")),
        "scratch cleanup escaped its temporary boundary"
    );
    let cleanup = fs::remove_dir_all(&resolved);
    if outcome.is_err() || cleanup.is_err() {
        write_new(
            &evidence.join("failure.json"),
            &serde_json::to_vec_pretty(
                &serde_json::json!({ "schema_version": 1, "state": "failed", "error": outcome.as_ref().err().map(|error| format!("{error:#}")), "cleanup_error": cleanup.as_ref().err().map(ToString::to_string), "source_plan_sha256": digest(&plan_bytes), "replacement_admission": "blocked" }),
            )?,
        )?;
    }
    cleanup.context("could not remove temporary build sources")?;
    outcome?;
    fs::rename(
        evidence.join("pending-report.json"),
        evidence.join("report.json"),
    )?;
    println!(
        "Fresh-install evidence {}",
        evidence.join("report.json").display()
    );
    Ok(())
}

fn validate_owner_report(
    report: &serde_json::Value,
    inputs: &BTreeMap<String, String>,
) -> Result<()> {
    ensure!(
        report["schema_version"] == 1 && report["report"]["schema_version"] == 1,
        "unsupported owner report"
    );
    ensure!(
        report["report"]["scope"] == "fresh-install-equivalence"
            && report["report"]["equivalent"] == true,
        "owner did not verify fresh-install equivalence"
    );
    ensure!(
        report["report"]["replacement_admission"]
            .as_str()
            .is_some_and(|value| value.starts_with("blocked")),
        "owner report must preserve blocked admission"
    );
    for (name, hash) in inputs {
        ensure!(
            report["input_sha256"][name].as_str() == Some(hash),
            "owner report input identity mismatch: {name}"
        );
    }
    Ok(())
}

fn copy_snapshot(source: &Path, destination: &Path, expected: &SourceSnapshot) -> Result<()> {
    fs::create_dir(destination)?;
    for input in &expected.files {
        let relative = safe_relative(&input.path)?;
        assert_regular(source, &relative)?;
        let target = destination.join(&relative);
        fs::create_dir_all(target.parent().context("copy parent missing")?)?;
        fs::write(&target, read_source(&source.join(&relative))?)?;
        fs::set_permissions(&target, fs::metadata(source.join(&relative))?.permissions())?;
    }
    validate_snapshot(destination, expected)
}

fn check_external_cargo_config(scratch: &Path) -> Result<()> {
    for ancestor in scratch.ancestors() {
        for name in ["config", "config.toml"] {
            ensure!(
                !ancestor.join(".cargo").join(name).exists(),
                "external Cargo configuration is unsupported: {}",
                ancestor.display()
            );
        }
    }
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".cargo"))
        })
        .context("cannot resolve Cargo home")?;
    for name in ["config", "config.toml"] {
        ensure!(
            !home.join(name).exists(),
            "global Cargo configuration is unsupported; use an unconfigured Cargo home"
        );
    }
    Ok(())
}

fn runtime_command(binary: &Path) -> Command {
    let mut command = Command::new(binary);
    command.env_clear().stdin(Stdio::null());
    for key in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "LD_LIBRARY_PATH",
        "DYLD_LIBRARY_PATH",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
}

fn build_command(binary: &str, source: &Path) -> Result<Command> {
    let mut command = runtime_command(Path::new(binary));
    for key in ["HOME", "USERPROFILE", "RUSTUP_HOME", "CARGO_HOME"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command.current_dir(source).env(
        "GIT_CEILING_DIRECTORIES",
        std::env::join_paths([source.parent().context("build source parent missing")?])?,
    );
    Ok(command)
}

fn file_digest(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn run_process(command: &mut Command, seconds: u64, limit: usize) -> Result<Vec<u8>> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut stdout = child.stdout.take().context("process stdout missing")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .by_ref()
            .take((limit + 1) as u64)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= limit, "process output exceeded limit");
        Ok::<_, anyhow::Error>(bytes)
    });
    let start = Instant::now();
    let status = loop {
        if start.elapsed() > Duration::from_secs(seconds) {
            child.kill()?;
            child.wait()?;
            bail!("process exceeded {seconds} seconds");
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                child.kill()?;
                child.wait()?;
                return Err(error.into());
            }
        }
    };
    // A descendant can keep the pipe open after the direct child exits.
    // Apply the same deadline to capture rather than joining unconditionally.
    while !reader.is_finished() {
        ensure!(
            start.elapsed() <= Duration::from_secs(seconds),
            "process output remained open after {seconds} seconds"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("process reader failed"))??;
    ensure!(status.success(), "process failed with {status}");
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_deadline_covers_output_held_open_by_a_descendant() -> Result<()> {
        const MODE: &str = "RUSTOK_TEST_MIGRATION_PIPE_MODE";
        let mut command = Command::new(std::env::current_exe()?);
        command.args([
            "--exact",
            "migrations::tests::process_deadline_covers_output_held_open_by_a_descendant",
            "--nocapture",
        ]);
        match std::env::var(MODE).as_deref() {
            Ok("hold") => {
                std::thread::sleep(Duration::from_secs(3));
                std::process::exit(0);
            }
            Ok("spawn") => {
                // The descendant inherits the output pipe and closes it on exit.
                command.env(MODE, "hold").spawn()?;
            }
            _ => {
                command.env(MODE, "spawn");
                let error = run_process(&mut command, 1, 4096).unwrap_err();
                assert!(
                    error.to_string().contains("process output remained open"),
                    "{error}"
                );
            }
        }
        Ok(())
    }
    #[test]
    fn rejects_escape_paths_and_option_confusion() {
        for path in [
            "../file",
            "/file",
            "C:/file",
            "a/../file",
            "a\\file",
            ".git/config",
            "a//file",
        ] {
            assert!(safe_relative(path).is_err(), "{path}");
        }
        assert!(safe_relative("src/migrations/schema/tables.rs").is_ok());
        assert!(parse_options(
            &["--plan".into(), "--verifier".into()],
            &["--plan", "--verifier"]
        )
        .is_err());
        assert!(parse_options(
            &["--plan".into(), "a".into(), "--plan".into(), "b".into()],
            &["--plan"]
        )
        .is_err());
    }
    #[test]
    fn source_digest_binds_paths_modes_and_bytes() {
        let files = vec![SourceFile {
            path: "a.rs".into(),
            mode: "100644".into(),
            sha256: digest(b"original"),
        }];
        let original = snapshot(files.clone()).unwrap();
        for mutate in [0, 1, 2] {
            let mut changed = files.clone();
            match mutate {
                0 => changed[0].path = "b.rs".into(),
                1 => changed[0].mode = "100755".into(),
                _ => changed[0].sha256 = digest(b"changed"),
            }
            assert_ne!(original.sha256, snapshot(changed).unwrap().sha256);
        }
    }

    #[test]
    fn rejects_incorrect_or_unbound_owner_report() {
        let inputs = BTreeMap::from([("reference-plan".into(), "abc".into())]);
        let correct = serde_json::json!({ "schema_version": 1, "report": { "schema_version": 1, "scope": "fresh-install-equivalence", "equivalent": true, "replacement_admission": "blocked" }, "input_sha256": { "reference-plan": "abc" } });
        assert!(validate_owner_report(&correct, &inputs).is_ok());
        for field in ["scope", "equivalent", "replacement_admission"] {
            let mut wrong = correct.clone();
            wrong["report"][field] = serde_json::Value::Null;
            assert!(validate_owner_report(&wrong, &inputs).is_err());
        }
        let mut wrong = correct;
        wrong["input_sha256"]["reference-plan"] = "changed".into();
        assert!(validate_owner_report(&wrong, &inputs).is_err());
    }

    #[test]
    fn git_snapshot_preserves_blobs_and_detects_later_edits() -> Result<()> {
        let temp = std::env::temp_dir().canonicalize()?;
        let fixture = temp.join(format!(
            "rustok-source-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        fs::create_dir(&fixture)?;
        let result = (|| {
            let repository = fixture.join("repository");
            let destination = fixture.join("snapshot");
            fs::create_dir(&repository)?;
            fs::create_dir(&destination)?;
            git(&repository, &["init", "--quiet"])?;
            fs::write(
                repository.join(".gitattributes"),
                "ignored.rs export-ignore\n",
            )?;
            fs::write(
                repository.join("ignored.rs"),
                b"nested migration helper\r\n",
            )?;
            git(&repository, &["add", "."])?;
            git(
                &repository,
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
                    "fixture",
                ],
            )?;
            let identity = commit(&repository, "HEAD")?;
            let expected = git_snapshot(&repository, &identity, Some(&destination))?;
            assert!(expected.files.iter().any(|file| file.path == "ignored.rs"));
            validate_snapshot(&destination, &expected)?;
            fs::write(destination.join("ignored.rs"), "changed helper")?;
            assert!(validate_snapshot(&destination, &expected).is_err());
            Ok(())
        })();
        ensure!(
            fixture.canonicalize()?.parent() == Some(temp.as_path()),
            "fixture cleanup escaped temp"
        );
        fs::remove_dir_all(fixture)?;
        result
    }
}

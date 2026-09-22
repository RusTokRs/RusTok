use std::{
    collections::BTreeSet,
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::connect_benchmark_database;

const CAPTURE_CONTRACT: &str = "index_query_equivalence_capture_v1";
const CAPTURE_OPT_IN: &str = "INDEX_QUERY_EQUIVALENCE_ALLOW_CAPTURE";
const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const OUTPUT_ENV: &str = "INDEX_QUERY_EQUIVALENCE_OUTPUT_ROOT";
const COMMIT_ENV: &str = "INDEX_QUERY_EQUIVALENCE_COMMIT";
const RUN_KEY_ENV: &str = "INDEX_QUERY_EQUIVALENCE_RUN_KEY";
const REPOSITORY_ENV: &str = "INDEX_QUERY_EQUIVALENCE_REPOSITORY";
const WORKSPACE_ENV: &str = "INDEX_QUERY_EQUIVALENCE_WORKSPACE_ROOT";
const CARGO_ENV: &str = "INDEX_QUERY_EQUIVALENCE_CARGO";
const STDOUT_FILE: &str = "stdout.log";
const STDERR_FILE: &str = "stderr.log";
const DESCRIPTOR_FILE: &str = "equivalence.json";
const MAX_LOG_BYTES: usize = 2 * 1024 * 1024;
const TEST_FILTER: &str = "postgres_query_port_matches_reference_fixture";
const TEST_PACKAGE: &str = "rustok-index";
const SCENARIOS: [&str; 6] = [
    "root_filter_desc_keyset_first_page",
    "root_filter_desc_keyset_continuation",
    "one_link_projection_and_filter",
    "many_link_filter_and_nested_projection",
    "many_link_is_null_totality",
    "bounded_offset_ordering",
];

#[derive(Debug, Clone)]
pub struct QueryEquivalenceCaptureConfig {
    pub database_url: String,
    pub workspace_root: PathBuf,
    pub output_root: PathBuf,
    pub repository: String,
    pub commit: String,
    pub run_key: String,
    pub cargo_program: String,
}

impl QueryEquivalenceCaptureConfig {
    pub fn from_env() -> Result<Self> {
        ensure!(
            matches!(env::var(CAPTURE_OPT_IN).as_deref(), Ok("1")),
            "{CAPTURE_OPT_IN}=1 is required because this command executes PostgreSQL equivalence tests and publishes retained evidence"
        );
        let database_url = env::var(DATABASE_ENV)
            .or_else(|_| env::var("DATABASE_URL"))
            .with_context(|| format!("{DATABASE_ENV} or DATABASE_URL is required"))?;
        ensure!(
            database_url.starts_with("postgres://") || database_url.starts_with("postgresql://"),
            "query equivalence capture requires a PostgreSQL URL"
        );

        let workspace_root = env::var(WORKSPACE_ENV).map(PathBuf::from).unwrap_or(
            env::current_dir().context("failed to resolve current workspace directory")?,
        );
        let workspace_root = workspace_root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize workspace root {workspace_root:?}"))?;
        ensure!(
            workspace_root.join("Cargo.toml").is_file(),
            "query equivalence workspace root must contain Cargo.toml"
        );
        ensure!(
            workspace_root.join(".git").exists(),
            "query equivalence workspace root must be a Git checkout"
        );

        let commit = env::var(COMMIT_ENV).context(format!("{COMMIT_ENV} is required"))?;
        ensure!(
            is_lower_hex_commit(&commit),
            "{COMMIT_ENV} must be a 40-character lowercase Git commit"
        );
        let run_key = env::var(RUN_KEY_ENV).context(format!("{RUN_KEY_ENV} is required"))?;
        validate_run_key(&run_key)?;
        let repository = env::var(REPOSITORY_ENV).unwrap_or_else(|_| "RusTokRs/RusTok".to_owned());
        ensure!(
            repository.split('/').count() == 2
                && repository
                    .split('/')
                    .all(|part| !part.is_empty() && part.len() <= 100),
            "{REPOSITORY_ENV} must use owner/repository form"
        );

        let output_root = env::var(OUTPUT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target/index-query-equivalence").join(&run_key));
        let output_root = if output_root.is_absolute() {
            output_root
        } else {
            workspace_root.join(output_root)
        };
        ensure!(
            output_root.file_name().is_some(),
            "query equivalence output root must have a final path component"
        );
        let cargo_program = env::var(CARGO_ENV).unwrap_or_else(|_| "cargo".to_owned());
        ensure!(
            !cargo_program.trim().is_empty(),
            "{CARGO_ENV} must not be empty"
        );

        Ok(Self {
            database_url,
            workspace_root,
            output_root,
            repository,
            commit,
            run_key,
            cargo_program,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct QueryEquivalenceDatabaseIdentity {
    version: String,
    server_version_num: String,
    system_identifier: String,
    database_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct QueryEquivalenceSourceIdentity {
    repository: String,
    commit: String,
    run_key: String,
    clean_worktree: bool,
}

#[derive(Debug, Clone, Serialize)]
struct QueryEquivalenceRunnerIdentity {
    job: String,
    runner_os: String,
    runner_arch: String,
}

#[derive(Debug, Clone, Serialize)]
struct QueryEquivalenceExecution {
    package: &'static str,
    test_filter: &'static str,
    command: Vec<String>,
    scenarios: &'static [&'static str],
    scenario_contract_sha256: String,
    exit_code: i32,
    skipped: bool,
    stdout: QueryEquivalenceArtifact,
    stderr: QueryEquivalenceArtifact,
}

#[derive(Debug, Clone, Serialize)]
struct QueryEquivalenceArtifact {
    path: &'static str,
    bytes: usize,
    sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct QueryEquivalenceDescriptor {
    contract: &'static str,
    completed_at: DateTime<Utc>,
    source: QueryEquivalenceSourceIdentity,
    runner: QueryEquivalenceRunnerIdentity,
    database: QueryEquivalenceDatabaseIdentity,
    execution: QueryEquivalenceExecution,
}

#[derive(Debug, Clone)]
pub struct QueryEquivalenceCapture {
    pub commit: String,
    pub run_key: String,
    pub output_root: PathBuf,
}

pub async fn capture_query_equivalence(
    config: &QueryEquivalenceCaptureConfig,
) -> Result<QueryEquivalenceCapture> {
    ensure_output_absent(&config.output_root)?;
    verify_source_identity(config)?;

    let db = connect_benchmark_database(&config.database_url).await?;
    let database_before = read_database_identity(&db).await?;
    validate_database_identity(&database_before)?;

    let output = run_equivalence_test(config)?;
    validate_test_output(&output)?;
    ensure!(
        output.stdout.len() <= MAX_LOG_BYTES,
        "query equivalence stdout exceeds the {MAX_LOG_BYTES}-byte retained limit"
    );
    ensure!(
        output.stderr.len() <= MAX_LOG_BYTES,
        "query equivalence stderr exceeds the {MAX_LOG_BYTES}-byte retained limit"
    );
    verify_source_identity(config)?;

    let database_after = read_database_identity(&db).await?;
    ensure!(
        database_after == database_before,
        "PostgreSQL identity changed during query equivalence capture"
    );

    let execution = QueryEquivalenceExecution {
        package: TEST_PACKAGE,
        test_filter: TEST_FILTER,
        command: equivalence_command(config),
        scenarios: &SCENARIOS,
        scenario_contract_sha256: sha256_json(&SCENARIOS)?,
        exit_code: output.status.code().unwrap_or(-1),
        skipped: false,
        stdout: QueryEquivalenceArtifact {
            path: STDOUT_FILE,
            bytes: output.stdout.len(),
            sha256: sha256_bytes(&output.stdout),
        },
        stderr: QueryEquivalenceArtifact {
            path: STDERR_FILE,
            bytes: output.stderr.len(),
            sha256: sha256_bytes(&output.stderr),
        },
    };
    let descriptor = QueryEquivalenceDescriptor {
        contract: CAPTURE_CONTRACT,
        completed_at: Utc::now(),
        source: QueryEquivalenceSourceIdentity {
            repository: config.repository.clone(),
            commit: config.commit.clone(),
            run_key: config.run_key.clone(),
            clean_worktree: true,
        },
        runner: QueryEquivalenceRunnerIdentity {
            job: first_non_empty_env(
                &["INDEX_QUERY_EQUIVALENCE_JOB", "GITHUB_JOB"],
                "index-query-equivalence",
            ),
            runner_os: first_non_empty_env(&["RUNNER_OS"], env::consts::OS),
            runner_arch: first_non_empty_env(&["RUNNER_ARCH"], env::consts::ARCH),
        },
        database: database_before,
        execution,
    };

    let output_root = publish_bundle(config, &descriptor, &output)?;
    Ok(QueryEquivalenceCapture {
        commit: config.commit.clone(),
        run_key: config.run_key.clone(),
        output_root,
    })
}

fn verify_source_identity(config: &QueryEquivalenceCaptureConfig) -> Result<()> {
    let head = git_output(&config.workspace_root, ["rev-parse", "HEAD"])?;
    ensure!(
        head.trim() == config.commit,
        "query equivalence commit mismatch: configured {}, checkout {}",
        config.commit,
        head.trim()
    );
    let status = git_output(
        &config.workspace_root,
        ["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    ensure!(
        status.trim().is_empty(),
        "query equivalence capture requires a clean worktree"
    );
    Ok(())
}

fn git_output<const N: usize>(workspace_root: &Path, args: [&str; N]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(args)
        .output()
        .context("failed to start git for query equivalence source verification")?;
    ensure!(
        output.status.success(),
        "git source verification failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout).context("git source verification output was not UTF-8")
}

fn run_equivalence_test(config: &QueryEquivalenceCaptureConfig) -> Result<Output> {
    Command::new(&config.cargo_program)
        .current_dir(&config.workspace_root)
        .args([
            "test",
            "-p",
            TEST_PACKAGE,
            TEST_FILTER,
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(DATABASE_ENV, &config.database_url)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .with_context(|| {
            format!(
                "failed to execute query equivalence test through {}",
                config.cargo_program
            )
        })
}

fn validate_test_output(output: &Output) -> Result<()> {
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        output.status.success(),
        "PostgreSQL/reference equivalence test failed with status {:?}: {}",
        output.status.code(),
        tail(&combined, 4_000)
    );
    ensure!(
        combined.contains(TEST_FILTER),
        "query equivalence test output does not name the required fixture"
    );
    ensure!(
        !combined.contains("skipping rustok-index PostgreSQL/reference equivalence"),
        "query equivalence test reported success by skipping PostgreSQL execution"
    );
    ensure!(
        combined.contains("test result: ok.") && combined.contains("1 passed; 0 failed"),
        "query equivalence test output does not prove one successful fixture: {}",
        tail(&combined, 4_000)
    );
    Ok(())
}

fn equivalence_command(config: &QueryEquivalenceCaptureConfig) -> Vec<String> {
    vec![
        config.cargo_program.clone(),
        "test".to_owned(),
        "-p".to_owned(),
        TEST_PACKAGE.to_owned(),
        TEST_FILTER.to_owned(),
        "--".to_owned(),
        "--nocapture".to_owned(),
        "--test-threads=1".to_owned(),
    ]
}

async fn read_database_identity(
    db: &sea_orm::DatabaseConnection,
) -> Result<QueryEquivalenceDatabaseIdentity> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            concat!(
                "SELECT version() AS version,",
                " current_setting('server_version_num') AS server_version_num,",
                " control.system_identifier::text AS system_identifier,",
                " current_database() AS database_name",
                " FROM pg_control_system() AS control"
            )
            .to_owned(),
        ))
        .await?
        .context("query equivalence database identity query returned no row")?;
    Ok(QueryEquivalenceDatabaseIdentity {
        version: row.try_get("", "version")?,
        server_version_num: row.try_get("", "server_version_num")?,
        system_identifier: row.try_get("", "system_identifier")?,
        database_name: row.try_get("", "database_name")?,
    })
}

fn validate_database_identity(identity: &QueryEquivalenceDatabaseIdentity) -> Result<()> {
    ensure!(
        identity.server_version_num.starts_with("16"),
        "query equivalence capture requires PostgreSQL 16, got {}",
        identity.server_version_num
    );
    ensure!(
        !identity.system_identifier.is_empty()
            && identity
                .system_identifier
                .bytes()
                .all(|byte| byte.is_ascii_digit()),
        "PostgreSQL system identifier must contain only digits"
    );
    ensure!(
        !identity.database_name.is_empty(),
        "PostgreSQL database name must not be empty"
    );
    Ok(())
}

fn publish_bundle(
    config: &QueryEquivalenceCaptureConfig,
    descriptor: &QueryEquivalenceDescriptor,
    output: &Output,
) -> Result<PathBuf> {
    ensure_output_absent(&config.output_root)?;
    let parent = config
        .output_root
        .parent()
        .context("query equivalence output root must have a parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create query equivalence output parent {parent:?}"))?;
    let parent_metadata = fs::symlink_metadata(parent)
        .with_context(|| format!("failed to inspect query equivalence output parent {parent:?}"))?;
    ensure!(
        parent_metadata.is_dir() && !parent_metadata.file_type().is_symlink(),
        "query equivalence output parent must be a regular non-symlink directory"
    );
    let canonical_parent = parent.canonicalize().with_context(|| {
        format!("failed to canonicalize query equivalence output parent {parent:?}")
    })?;
    let output_name = config
        .output_root
        .file_name()
        .context("query equivalence output root must have a filename")?;
    let final_root = canonical_parent.join(output_name);
    ensure_output_absent(&final_root)?;
    fs::create_dir(&final_root).with_context(|| {
        format!("failed to reserve fresh query equivalence output root {final_root:?}")
    })?;

    let result: Result<PathBuf> = (|| {
        write_new_file(&final_root.join(STDOUT_FILE), &output.stdout)?;
        write_new_file(&final_root.join(STDERR_FILE), &output.stderr)?;
        ensure_exact_files(&final_root, &[STDERR_FILE, STDOUT_FILE])?;

        let mut descriptor_bytes = serde_json::to_vec_pretty(descriptor)?;
        descriptor_bytes.push(b'\n');
        write_new_file(&final_root.join(DESCRIPTOR_FILE), &descriptor_bytes)?;
        ensure_exact_files(&final_root, &[DESCRIPTOR_FILE, STDERR_FILE, STDOUT_FILE])?;
        Ok(final_root.clone())
    })();
    if result.is_err() && final_root.exists() {
        let _ = fs::remove_dir_all(&final_root);
    }
    result
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!("failed to create retained query equivalence artifact {path:?}")
        })?;
    file.write_all(bytes)
        .with_context(|| format!("failed to write retained query equivalence artifact {path:?}"))?;
    file.sync_all()
        .with_context(|| format!("failed to sync retained query equivalence artifact {path:?}"))?;
    Ok(())
}

fn ensure_exact_files(root: &Path, expected: &[&str]) -> Result<()> {
    let expected = expected
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to inspect query equivalence bundle {root:?}"))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        ensure!(
            file_type.is_file() && !file_type.is_symlink(),
            "query equivalence bundle entries must be regular non-symlink files"
        );
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("query equivalence bundle filename must be UTF-8"))?;
        actual.insert(name);
    }
    ensure!(
        actual == expected,
        "query equivalence bundle inventory mismatch: expected {expected:?}, got {actual:?}"
    );
    Ok(())
}

fn ensure_output_absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => anyhow::bail!("query equivalence output already exists: {path:?}"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect output path {path:?}")),
    }
}

fn validate_run_key(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 128,
        "{RUN_KEY_ENV} must contain between 1 and 128 bytes"
    );
    ensure!(
        value
            .bytes()
            .all(|byte| { byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') }),
        "{RUN_KEY_ENV} may contain only ASCII letters, digits, dash, underscore, and dot"
    );
    ensure!(
        value != "." && value != "..",
        "{RUN_KEY_ENV} must not be dot traversal"
    );
    Ok(())
}

fn is_lower_hex_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_json(value: &impl Serialize) -> Result<String> {
    Ok(sha256_bytes(&serde_json::to_vec(value)?))
}

fn first_non_empty_env(keys: &[&str], fallback: &str) -> String {
    keys.iter()
        .find_map(|key| env::var(key).ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| fallback.to_owned())
}

fn tail(value: &str, max_chars: usize) -> String {
    let start = value
        .char_indices()
        .rev()
        .nth(max_chars)
        .map_or(0, |(index, _)| index);
    value[start..].to_owned()
}

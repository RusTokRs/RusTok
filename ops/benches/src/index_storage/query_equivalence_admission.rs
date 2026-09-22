use std::{
    collections::BTreeSet,
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CAPTURE_CONTRACT: &str = "index_query_equivalence_capture_v1";
const ADMISSION_CONTRACT: &str = "index_query_equivalence_admission_v1";
const ADMISSION_OPT_IN: &str = "INDEX_QUERY_EQUIVALENCE_ALLOW_ADMISSION";
const BUNDLE_ENV: &str = "INDEX_QUERY_EQUIVALENCE_BUNDLE";
const OUTPUT_ENV: &str = "INDEX_QUERY_EQUIVALENCE_ADMISSION_OUTPUT";
const REPOSITORY_ENV: &str = "INDEX_QUERY_EQUIVALENCE_EXPECTED_REPOSITORY";
const COMMIT_ENV: &str = "INDEX_QUERY_EQUIVALENCE_EXPECTED_COMMIT";
const RUN_KEY_ENV: &str = "INDEX_QUERY_EQUIVALENCE_EXPECTED_RUN_KEY";
const DESCRIPTOR_FILE: &str = "equivalence.json";
const STDOUT_FILE: &str = "stdout.log";
const STDERR_FILE: &str = "stderr.log";
const MAX_DESCRIPTOR_BYTES: usize = 256 * 1024;
const MAX_LOG_BYTES: usize = 2 * 1024 * 1024;
const TEST_PACKAGE: &str = "rustok-index";
const TEST_FILTER: &str = "postgres_query_port_matches_reference_fixture";
const SKIP_MARKER: &str = "skipping rustok-index PostgreSQL/reference equivalence";
const EXPECTED_COMMAND_ARGS: [&str; 7] = [
    "test",
    "-p",
    TEST_PACKAGE,
    TEST_FILTER,
    "--",
    "--nocapture",
    "--test-threads=1",
];
const SCENARIOS: [&str; 6] = [
    "root_filter_desc_keyset_first_page",
    "root_filter_desc_keyset_continuation",
    "one_link_projection_and_filter",
    "many_link_filter_and_nested_projection",
    "many_link_is_null_totality",
    "bounded_offset_ordering",
];
const INVENTORY: [&str; 3] = [DESCRIPTOR_FILE, STDERR_FILE, STDOUT_FILE];

#[derive(Debug, Clone)]
pub struct QueryEquivalenceAdmissionConfig {
    pub bundle_root: PathBuf,
    pub output_path: PathBuf,
    pub expected_repository: String,
    pub expected_commit: String,
    pub expected_run_key: String,
}

impl QueryEquivalenceAdmissionConfig {
    pub fn from_env() -> Result<Self> {
        ensure!(
            matches!(env::var(ADMISSION_OPT_IN).as_deref(), Ok("1")),
            "{ADMISSION_OPT_IN}=1 is required because this command emits an equivalence admission receipt"
        );
        let bundle_root = env::var(BUNDLE_ENV)
            .map(PathBuf::from)
            .context(format!("{BUNDLE_ENV} is required"))?;
        let bundle_root = absolute_path(bundle_root)?;
        let expected_repository =
            env::var(REPOSITORY_ENV).unwrap_or_else(|_| "RusTokRs/RusTok".to_owned());
        validate_repository(&expected_repository)?;
        let expected_commit = env::var(COMMIT_ENV).context(format!("{COMMIT_ENV} is required"))?;
        ensure!(
            is_lower_hex_commit(&expected_commit),
            "{COMMIT_ENV} must be a 40-character lowercase Git commit"
        );
        let expected_run_key =
            env::var(RUN_KEY_ENV).context(format!("{RUN_KEY_ENV} is required"))?;
        validate_run_key(&expected_run_key)?;
        let output_path = env::var(OUTPUT_ENV).map(PathBuf::from).unwrap_or_else(|_| {
            bundle_root
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(format!("{}-admission.json", expected_run_key))
        });
        let output_path = absolute_path(output_path)?;

        Ok(Self {
            bundle_root,
            output_path,
            expected_repository,
            expected_commit,
            expected_run_key,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CaptureSourceIdentity {
    repository: String,
    commit: String,
    run_key: String,
    clean_worktree: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureRunnerIdentity {
    job: String,
    runner_os: String,
    runner_arch: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CaptureDatabaseIdentity {
    version: String,
    server_version_num: String,
    system_identifier: String,
    database_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureArtifact {
    path: String,
    bytes: usize,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureExecution {
    package: String,
    test_filter: String,
    command: Vec<String>,
    scenarios: Vec<String>,
    scenario_contract_sha256: String,
    exit_code: i32,
    skipped: bool,
    stdout: CaptureArtifact,
    stderr: CaptureArtifact,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureDescriptor {
    contract: String,
    completed_at: DateTime<Utc>,
    source: CaptureSourceIdentity,
    runner: CaptureRunnerIdentity,
    database: CaptureDatabaseIdentity,
    execution: CaptureExecution,
}

#[derive(Debug, Clone, Serialize)]
struct AdmissionArtifact {
    path: String,
    bytes: usize,
    sha256: String,
}

#[derive(Debug, Clone, Serialize)]
struct AdmissionBundle {
    inventory: Vec<String>,
    descriptor: AdmissionArtifact,
    stdout: AdmissionArtifact,
    stderr: AdmissionArtifact,
}

#[derive(Debug, Clone, Serialize)]
struct AdmissionExecution {
    package: String,
    test_filter: String,
    command: Vec<String>,
    scenarios: Vec<String>,
    scenario_contract_sha256: String,
    exit_code: i32,
    skipped: bool,
    captured_at: DateTime<Utc>,
    capture_job: String,
    capture_runner_os: String,
    capture_runner_arch: String,
}

#[derive(Debug, Clone, Serialize)]
struct AdmissionReviewer {
    job: String,
    runner_os: String,
    runner_arch: String,
}

#[derive(Debug, Clone, Serialize)]
struct AdmissionReceipt {
    contract: &'static str,
    reviewed_at: DateTime<Utc>,
    admitted: bool,
    production_lifecycle_authorized: bool,
    source: CaptureSourceIdentity,
    database: CaptureDatabaseIdentity,
    execution: AdmissionExecution,
    bundle: AdmissionBundle,
    reviewer: AdmissionReviewer,
}

#[derive(Debug, Clone)]
pub struct QueryEquivalenceAdmission {
    pub commit: String,
    pub run_key: String,
    pub output_path: PathBuf,
}

pub fn admit_query_equivalence_bundle(
    config: &QueryEquivalenceAdmissionConfig,
) -> Result<QueryEquivalenceAdmission> {
    let canonical_bundle = validate_bundle_root(&config.bundle_root)?;
    let output_path = resolve_output_path(&config.output_path, &canonical_bundle)?;
    ensure_output_absent(&output_path)?;

    let inventory_before = read_inventory(&canonical_bundle)?;
    ensure!(
        inventory_before == expected_inventory(),
        "query equivalence bundle inventory mismatch: expected {:?}, got {:?}",
        expected_inventory(),
        inventory_before
    );

    let descriptor_bytes = read_stable_regular_file(
        &canonical_bundle.join(DESCRIPTOR_FILE),
        MAX_DESCRIPTOR_BYTES,
        "equivalence descriptor",
    )?;
    let stdout = read_stable_regular_file(
        &canonical_bundle.join(STDOUT_FILE),
        MAX_LOG_BYTES,
        "equivalence stdout",
    )?;
    let stderr = read_stable_regular_file(
        &canonical_bundle.join(STDERR_FILE),
        MAX_LOG_BYTES,
        "equivalence stderr",
    )?;
    let descriptor: CaptureDescriptor = serde_json::from_slice(&descriptor_bytes)
        .context("failed to decode retained query equivalence descriptor")?;

    validate_descriptor(config, &descriptor, &stdout, &stderr)?;

    let inventory_after = read_inventory(&canonical_bundle)?;
    ensure!(
        inventory_after == inventory_before,
        "query equivalence bundle inventory drifted during admission review"
    );
    ensure!(
        read_stable_regular_file(
            &canonical_bundle.join(DESCRIPTOR_FILE),
            MAX_DESCRIPTOR_BYTES,
            "equivalence descriptor reread",
        )? == descriptor_bytes,
        "query equivalence descriptor changed during admission review"
    );
    ensure!(
        read_stable_regular_file(
            &canonical_bundle.join(STDOUT_FILE),
            MAX_LOG_BYTES,
            "equivalence stdout reread",
        )? == stdout,
        "query equivalence stdout changed during admission review"
    );
    ensure!(
        read_stable_regular_file(
            &canonical_bundle.join(STDERR_FILE),
            MAX_LOG_BYTES,
            "equivalence stderr reread",
        )? == stderr,
        "query equivalence stderr changed during admission review"
    );

    let receipt = AdmissionReceipt {
        contract: ADMISSION_CONTRACT,
        reviewed_at: Utc::now(),
        admitted: true,
        production_lifecycle_authorized: false,
        source: descriptor.source.clone(),
        database: descriptor.database.clone(),
        execution: AdmissionExecution {
            package: descriptor.execution.package.clone(),
            test_filter: descriptor.execution.test_filter.clone(),
            command: descriptor.execution.command.clone(),
            scenarios: descriptor.execution.scenarios.clone(),
            scenario_contract_sha256: descriptor.execution.scenario_contract_sha256.clone(),
            exit_code: descriptor.execution.exit_code,
            skipped: descriptor.execution.skipped,
            captured_at: descriptor.completed_at,
            capture_job: descriptor.runner.job.clone(),
            capture_runner_os: descriptor.runner.runner_os.clone(),
            capture_runner_arch: descriptor.runner.runner_arch.clone(),
        },
        bundle: AdmissionBundle {
            inventory: INVENTORY.iter().map(|value| (*value).to_owned()).collect(),
            descriptor: artifact(DESCRIPTOR_FILE, &descriptor_bytes),
            stdout: artifact(STDOUT_FILE, &stdout),
            stderr: artifact(STDERR_FILE, &stderr),
        },
        reviewer: AdmissionReviewer {
            job: first_non_empty_env(
                &["INDEX_QUERY_EQUIVALENCE_ADMISSION_JOB", "GITHUB_JOB"],
                "index-query-equivalence-admission",
            ),
            runner_os: first_non_empty_env(&["RUNNER_OS"], env::consts::OS),
            runner_arch: first_non_empty_env(&["RUNNER_ARCH"], env::consts::ARCH),
        },
    };
    publish_receipt(&output_path, &receipt)?;

    Ok(QueryEquivalenceAdmission {
        commit: descriptor.source.commit.clone(),
        run_key: descriptor.source.run_key.clone(),
        output_path,
    })
}

fn validate_descriptor(
    config: &QueryEquivalenceAdmissionConfig,
    descriptor: &CaptureDescriptor,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<()> {
    ensure!(
        descriptor.contract == CAPTURE_CONTRACT,
        "unexpected query equivalence capture contract: {}",
        descriptor.contract
    );
    ensure!(
        descriptor.source.repository == config.expected_repository,
        "query equivalence repository mismatch"
    );
    ensure!(
        descriptor.source.commit == config.expected_commit,
        "query equivalence commit mismatch"
    );
    ensure!(
        descriptor.source.run_key == config.expected_run_key,
        "query equivalence run key mismatch"
    );
    ensure!(
        descriptor.source.clean_worktree,
        "query equivalence capture was not bound to a clean worktree"
    );
    ensure!(
        is_lower_hex_commit(&descriptor.source.commit),
        "captured query equivalence commit is not canonical lowercase hex"
    );
    validate_database(&descriptor.database)?;
    ensure!(
        !descriptor.runner.job.trim().is_empty()
            && !descriptor.runner.runner_os.trim().is_empty()
            && !descriptor.runner.runner_arch.trim().is_empty(),
        "query equivalence capture runner identity is incomplete"
    );

    let execution = &descriptor.execution;
    ensure!(execution.package == TEST_PACKAGE, "unexpected test package");
    ensure!(
        execution.test_filter == TEST_FILTER,
        "unexpected test filter"
    );
    ensure!(
        execution.exit_code == 0,
        "captured fixture did not exit successfully"
    );
    ensure!(!execution.skipped, "captured fixture is marked skipped");
    ensure!(
        execution.command.len() == EXPECTED_COMMAND_ARGS.len() + 1
            && !execution.command[0].trim().is_empty()
            && execution.command[1..]
                .iter()
                .map(String::as_str)
                .eq(EXPECTED_COMMAND_ARGS),
        "captured equivalence command does not match the admitted command contract"
    );
    ensure!(
        execution.scenarios.iter().map(String::as_str).eq(SCENARIOS),
        "captured equivalence scenarios do not match the admitted scenario contract"
    );
    ensure!(
        execution.scenario_contract_sha256 == sha256_json(&SCENARIOS)?,
        "captured equivalence scenario digest mismatch"
    );
    validate_artifact_descriptor(&execution.stdout, STDOUT_FILE, stdout)?;
    validate_artifact_descriptor(&execution.stderr, STDERR_FILE, stderr)?;

    let stdout_text =
        std::str::from_utf8(stdout).context("retained query equivalence stdout is not UTF-8")?;
    let stderr_text =
        std::str::from_utf8(stderr).context("retained query equivalence stderr is not UTF-8")?;
    let combined = format!("{stdout_text}\n{stderr_text}");
    ensure!(
        combined.contains(TEST_FILTER),
        "retained output does not name the required equivalence fixture"
    );
    ensure!(
        combined.contains("test result: ok.") && combined.contains("1 passed; 0 failed"),
        "retained output does not prove exactly one successful equivalence fixture"
    );
    ensure!(
        !combined.contains(SKIP_MARKER),
        "retained output proves the PostgreSQL equivalence fixture was skipped"
    );
    Ok(())
}

fn validate_artifact_descriptor(
    descriptor: &CaptureArtifact,
    expected_path: &str,
    bytes: &[u8],
) -> Result<()> {
    ensure!(
        descriptor.path == expected_path,
        "retained artifact path mismatch"
    );
    ensure!(
        descriptor.bytes == bytes.len(),
        "retained artifact byte count mismatch"
    );
    ensure!(
        descriptor.sha256 == sha256_bytes(bytes),
        "retained artifact SHA-256 mismatch"
    );
    Ok(())
}

fn validate_database(identity: &CaptureDatabaseIdentity) -> Result<()> {
    ensure!(
        identity.server_version_num.starts_with("16"),
        "query equivalence admission requires PostgreSQL 16"
    );
    ensure!(
        !identity.version.trim().is_empty(),
        "PostgreSQL version is empty"
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
        !identity.database_name.trim().is_empty(),
        "PostgreSQL database name is empty"
    );
    Ok(())
}

fn validate_bundle_root(path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("query equivalence bundle does not exist: {path:?}"))?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "query equivalence bundle must be a regular non-symlink directory"
    );
    path.canonicalize()
        .with_context(|| format!("failed to canonicalize query equivalence bundle {path:?}"))
}

fn read_inventory(root: &Path) -> Result<BTreeSet<String>> {
    let mut inventory = BTreeSet::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to list query equivalence bundle {root:?}"))?
    {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("query equivalence bundle contains a non-UTF-8 name"))?;
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "query equivalence bundle entry must be a regular non-symlink file: {name}"
        );
        ensure!(
            inventory.insert(name.clone()),
            "duplicate bundle entry: {name}"
        );
    }
    Ok(inventory)
}

fn expected_inventory() -> BTreeSet<String> {
    INVENTORY.iter().map(|value| (*value).to_owned()).collect()
}

fn read_stable_regular_file(path: &Path, max_bytes: usize, label: &str) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {label} {path:?}"))?;
    ensure!(
        before.is_file() && !before.file_type().is_symlink(),
        "{label} must be a regular non-symlink file"
    );
    ensure!(
        before.len() <= max_bytes as u64,
        "{label} exceeds retained size limit"
    );
    let first = fs::read(path).with_context(|| format!("failed to read {label} {path:?}"))?;
    let second = fs::read(path).with_context(|| format!("failed to reread {label} {path:?}"))?;
    let after = fs::symlink_metadata(path)
        .with_context(|| format!("failed to re-inspect {label} {path:?}"))?;
    ensure!(
        after.is_file()
            && !after.file_type().is_symlink()
            && before.len() == after.len()
            && first == second,
        "{label} changed while it was being reviewed"
    );
    Ok(first)
}

fn resolve_output_path(path: &Path, forbidden_root: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let metadata = fs::symlink_metadata(parent).with_context(|| {
        format!("query equivalence admission receipt parent must already exist: {parent:?}")
    })?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "admission receipt parent must be a regular non-symlink directory"
    );
    let canonical_parent = parent
        .canonicalize()
        .with_context(|| format!("failed to canonicalize admission receipt parent {parent:?}"))?;
    let filename = path
        .file_name()
        .context("admission receipt path must have a filename")?;
    let output_path = canonical_parent.join(filename);
    ensure!(
        !output_path.starts_with(forbidden_root),
        "query equivalence admission receipt must be outside the immutable bundle"
    );
    Ok(output_path)
}

fn publish_receipt(path: &Path, receipt: &AdmissionReceipt) -> Result<()> {
    ensure_output_absent(path)?;
    let mut bytes = serde_json::to_vec_pretty(receipt)?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!("failed to create query equivalence admission receipt {path:?}")
        })?;
    file.write_all(&bytes)
        .with_context(|| format!("failed to write query equivalence admission receipt {path:?}"))?;
    file.sync_all()
        .with_context(|| format!("failed to sync query equivalence admission receipt {path:?}"))?;
    Ok(())
}

fn ensure_output_absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => anyhow::bail!("query equivalence admission output already exists: {path:?}"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to inspect admission output path {path:?}"))
        }
    }
}

fn artifact(path: &str, bytes: &[u8]) -> AdmissionArtifact {
    AdmissionArtifact {
        path: path.to_owned(),
        bytes: bytes.len(),
        sha256: sha256_bytes(bytes),
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_json(value: &impl Serialize) -> Result<String> {
    Ok(sha256_bytes(&serde_json::to_vec(value)?))
}

fn absolute_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(env::current_dir()
            .context("failed to resolve current directory")?
            .join(path))
    }
}

fn validate_repository(value: &str) -> Result<()> {
    ensure!(
        value.split('/').count() == 2
            && value
                .split('/')
                .all(|part| !part.is_empty() && part.len() <= 100),
        "{REPOSITORY_ENV} must use owner/repository form"
    );
    Ok(())
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

fn first_non_empty_env(keys: &[&str], fallback: &str) -> String {
    keys.iter()
        .find_map(|key| env::var(key).ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| fallback.to_owned())
}

use std::{
    collections::BTreeSet,
    env,
    fs::{self, Metadata, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::connect_benchmark_database;

const MANIFEST_CONTRACT: &str = "index_partition_evidence_manifest_v1";
const CAPTURE_CONTRACT: &str = "index_partition_capture_v1";
const FINALIZE_OPT_IN: &str = "INDEX_PARTITION_ALLOW_CAPTURE_FINALIZE";
const ARTIFACT_FILES: [(&str, &str); 6] = [
    ("baseline", "baseline.json"),
    ("shadow", "shadow.json"),
    ("query", "query.json"),
    ("mutation", "mutation.json"),
    ("maintenance", "maintenance.json"),
    ("cutover", "cutover.json"),
];

#[derive(Debug, Clone)]
pub struct PartitionCaptureFinalizeConfig {
    pub database_url: String,
    pub manifest_path: PathBuf,
    pub output_root: PathBuf,
    pub output_path: PathBuf,
}

impl PartitionCaptureFinalizeConfig {
    pub fn from_env() -> Result<Self> {
        ensure!(
            matches!(env::var(FINALIZE_OPT_IN).as_deref(), Ok("1")),
            "{FINALIZE_OPT_IN}=1 is required because the finalizer binds retained artifacts to one PostgreSQL identity"
        );
        let database_url = env::var("DATABASE_URL")
            .context("DATABASE_URL is required for index partition capture finalization")?;
        let manifest_path = env::var("INDEX_PARTITION_MANIFEST")
            .map(PathBuf::from)
            .context("INDEX_PARTITION_MANIFEST is required")?;
        let output_root = env::var("INDEX_PARTITION_EVIDENCE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target/index-partition-evidence"));
        let output_path = env::var("INDEX_PARTITION_CAPTURE_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| output_root.join("capture.json"));
        ensure!(
            manifest_path != output_path,
            "manifest and capture descriptor paths must be distinct"
        );
        ensure!(
            output_path.parent() == Some(output_root.as_path()),
            "capture descriptor must be written directly inside the evidence root"
        );
        Ok(Self {
            database_url,
            manifest_path,
            output_root,
            output_path,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PreparedManifest {
    contract: String,
    repository: String,
    commit: String,
    run_key: String,
    postgres_image: String,
    evidence_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct CaptureDescriptor {
    contract: &'static str,
    completed_at: DateTime<Utc>,
    run_provenance: CaptureRunProvenance,
    database: CaptureDatabaseIdentity,
    artifacts: CaptureArtifacts,
}

#[derive(Debug, Clone, Serialize)]
struct CaptureRunProvenance {
    repository: String,
    commit: String,
    run_key: String,
    job: String,
    runner_os: String,
    runner_arch: String,
}

#[derive(Debug, Clone, Serialize)]
struct CaptureDatabaseIdentity {
    version: String,
    server_version_num: String,
    jit: String,
    system_identifier: String,
    database_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct CaptureArtifacts {
    baseline: String,
    shadow: String,
    query: String,
    mutation: String,
    maintenance: String,
    cutover: String,
}

#[derive(Debug, Clone)]
pub struct PartitionCaptureFinalize {
    pub evidence_id: String,
    pub output_path: PathBuf,
}

pub async fn finalize_partition_capture(
    config: &PartitionCaptureFinalizeConfig,
) -> Result<PartitionCaptureFinalize> {
    let manifest: PreparedManifest = read_regular_json(&config.manifest_path, "manifest")?;
    validate_manifest(&manifest)?;
    ensure_output_available(&config.output_path)?;
    let artifacts = validate_artifact_bundle(&config.output_root)?;

    let db = connect_benchmark_database(&config.database_url).await?;
    db.execute_unprepared("SET jit = off; SET statement_timeout = '30s';")
        .await
        .context("failed to pin capture-finalizer session settings")?;
    let database = read_database_identity(&db).await?;

    let descriptor = CaptureDescriptor {
        contract: CAPTURE_CONTRACT,
        completed_at: Utc::now(),
        run_provenance: CaptureRunProvenance {
            repository: manifest.repository,
            commit: manifest.commit,
            run_key: manifest.run_key,
            job: first_non_empty_env(
                &["INDEX_PARTITION_EVIDENCE_JOB", "GITHUB_JOB"],
                "index-partition-evidence",
            ),
            runner_os: first_non_empty_env(&["RUNNER_OS"], env::consts::OS),
            runner_arch: first_non_empty_env(&["RUNNER_ARCH"], env::consts::ARCH),
        },
        database,
        artifacts,
    };
    publish_capture_descriptor(&config.output_path, &descriptor)?;

    Ok(PartitionCaptureFinalize {
        evidence_id: manifest.evidence_id,
        output_path: config.output_path.clone(),
    })
}

async fn read_database_identity(
    db: &sea_orm::DatabaseConnection,
) -> Result<CaptureDatabaseIdentity> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            concat!(
                "SELECT version() AS version,",
                " current_setting('server_version_num') AS server_version_num,",
                " current_setting('jit') AS jit,",
                " control.system_identifier::text AS system_identifier,",
                " current_database() AS database_name",
                " FROM pg_control_system() AS control"
            )
            .to_owned(),
        ))
        .await?
        .context("partition capture database identity query returned no row")?;
    let identity = CaptureDatabaseIdentity {
        version: row.try_get("", "version")?,
        server_version_num: row.try_get("", "server_version_num")?,
        jit: row.try_get("", "jit")?,
        system_identifier: row.try_get("", "system_identifier")?,
        database_name: row.try_get("", "database_name")?,
    };
    ensure!(
        identity.server_version_num.starts_with("16"),
        "partition capture requires PostgreSQL 16, got {}",
        identity.server_version_num
    );
    ensure!(identity.jit == "off", "partition capture requires jit=off");
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
    Ok(identity)
}

fn validate_artifact_bundle(root: &Path) -> Result<CaptureArtifacts> {
    let root_metadata = fs::symlink_metadata(root)
        .with_context(|| format!("partition evidence root does not exist: {root:?}"))?;
    ensure!(
        !root_metadata.file_type().is_symlink() && root_metadata.is_dir(),
        "partition evidence root must be a regular non-symlink directory"
    );
    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize partition evidence root {root:?}"))?;
    let mut identities = BTreeSet::new();
    for (role, filename) in ARTIFACT_FILES {
        let path = root.join(filename);
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("missing partition evidence artifact {role}: {path:?}"))?;
        ensure!(
            !metadata.file_type().is_symlink() && metadata.is_file(),
            "partition evidence artifact {role} must be a regular non-symlink file"
        );
        ensure!(
            metadata.len() > 0,
            "partition evidence artifact {role} must not be empty"
        );
        let canonical = path.canonicalize().with_context(|| {
            format!("failed to canonicalize partition evidence artifact {role}")
        })?;
        ensure!(
            canonical.starts_with(&canonical_root),
            "partition evidence artifact {role} must stay inside the canonical evidence root"
        );
        ensure!(
            identities.insert(file_identity(&metadata, &canonical)),
            "partition evidence artifact {role} aliases another retained artifact"
        );
    }
    Ok(CaptureArtifacts {
        baseline: "baseline.json".to_owned(),
        shadow: "shadow.json".to_owned(),
        query: "query.json".to_owned(),
        mutation: "mutation.json".to_owned(),
        maintenance: "maintenance.json".to_owned(),
        cutover: "cutover.json".to_owned(),
    })
}

#[cfg(unix)]
fn file_identity(metadata: &Metadata, _canonical: &Path) -> String {
    use std::os::unix::fs::MetadataExt as _;
    format!("{}:{}", metadata.dev(), metadata.ino())
}

#[cfg(not(unix))]
fn file_identity(_metadata: &Metadata, canonical: &Path) -> String {
    canonical.to_string_lossy().into_owned()
}

fn validate_manifest(manifest: &PreparedManifest) -> Result<()> {
    ensure!(
        manifest.contract == MANIFEST_CONTRACT,
        "unexpected manifest contract"
    );
    ensure!(
        manifest.repository == "RusTokRs/RusTok",
        "unexpected manifest repository"
    );
    ensure!(
        manifest.postgres_image == "postgres:16",
        "manifest must pin postgres:16"
    );
    ensure!(
        is_lower_hex(&manifest.commit, 40),
        "manifest commit must be a full lowercase SHA"
    );
    ensure!(
        is_lower_hex(&manifest.evidence_id, 64),
        "manifest evidence_id must be SHA-256"
    );
    ensure!(
        valid_run_key(&manifest.run_key),
        "manifest run_key must be a bounded stable identifier"
    );
    Ok(())
}

fn valid_run_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'.' | b'_' | b':' | b'-'))
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn first_non_empty_env(names: &[&str], fallback: &str) -> String {
    names
        .iter()
        .find_map(|name| env::var(name).ok().filter(|value| !value.is_empty()))
        .unwrap_or_else(|| fallback.to_owned())
}

fn read_regular_json<T: DeserializeOwned>(path: &Path, label: &str) -> Result<T> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {label} file {path:?}"))?;
    ensure!(
        !metadata.file_type().is_symlink() && metadata.is_file(),
        "{label} must be a regular non-symlink file"
    );
    let bytes = fs::read(path).with_context(|| format!("failed to read {label} file {path:?}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {label} JSON {path:?}"))
}

fn ensure_output_available(path: &Path) -> Result<()> {
    ensure!(!path.exists(), "refusing to overwrite {path:?}");
    Ok(())
}

fn publish_capture_descriptor(path: &Path, descriptor: &CaptureDescriptor) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create capture descriptor directory {parent:?}"))?;
    }
    ensure_output_available(path)?;
    let mut bytes =
        serde_json::to_vec_pretty(descriptor).context("failed to serialize capture descriptor")?;
    bytes.push(b'\n');
    let temporary = temporary_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| format!("failed to create temporary capture descriptor {temporary:?}"))?;
    file.write_all(&bytes)
        .with_context(|| format!("failed to write temporary capture descriptor {temporary:?}"))?;
    file.sync_all()
        .with_context(|| format!("failed to sync temporary capture descriptor {temporary:?}"))?;
    let publish = fs::hard_link(&temporary, path)
        .with_context(|| format!("failed to publish capture descriptor to {path:?}"));
    let _ = fs::remove_file(&temporary);
    publish
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(format!(".tmp-{}", std::process::id()));
    PathBuf::from(value)
}

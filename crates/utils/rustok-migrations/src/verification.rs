//! PostgreSQL observations for migration verification. This is evidence for a
//! particular fixture and execution path, never permission to rewrite history.

use sea_orm_migration::sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, IsolationLevel,
    AccessMode, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path, process::{Command, Stdio}, io::Read, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
use thiserror::Error;

const MAX_ROWS: usize = 100_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_ROW_BYTES: usize = 1024 * 1024;
const APPLICATION_SCHEMA: &str = "n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname !~ '^pg_(toast|temp)_?'";

#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("database observation failed: {0}")]
    Database(#[from] sea_orm_migration::sea_orm::DbErr),
    #[error("verification is incomplete: {0}")]
    Incomplete(String),
    #[error("PostgreSQL tool failed: {0}")]
    Tool(String),
    #[error("verification I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, VerificationError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationName {
    pub schema: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableObservation {
    pub relation: RelationName,
    pub columns: Vec<String>,
    pub rows: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SequenceObservation {
    pub relation: RelationName,
    pub last_value: String,
    pub is_called: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseSnapshot {
    pub environment: Vec<String>,
    pub schema: String,
    pub enforcement: Vec<String>,
    pub tables: Vec<TableObservation>,
    pub sequences: Vec<SequenceObservation>,
    pub migration_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComparisonReport {
    pub schema_version: u32,
    pub scope: String,
    pub equivalent: bool,
    pub replacement_admission: String,
    pub differences: Vec<String>,
    pub reference_digest: String,
    pub candidate_digest: String,
}

/// Compare observations only after each journal was checked against its own
/// executable plan. Different consolidated journals are expected.
pub fn compare(
    reference: &DatabaseSnapshot,
    candidate: &DatabaseSnapshot,
    reference_plan: &[String],
    candidate_plan: &[String],
) -> Result<ComparisonReport> {
    validate_journal(&reference.migration_versions, reference_plan)?;
    validate_journal(&candidate.migration_versions, candidate_plan)?;
    let mut differences = Vec::new();
    if reference.environment != candidate.environment {
        differences.push("PostgreSQL environment".into());
    }
    if reference.schema != candidate.schema {
        differences.push("schema, ownership or privileges".into());
    }
    if reference.enforcement != candidate.enforcement {
        differences.push("constraint, trigger, extension or index enforcement".into());
    }
    let names: BTreeSet<_> = reference.tables.iter().chain(&candidate.tables)
        .map(|table| table.relation.clone()).collect();
    for name in names {
        let original = reference.tables.iter().find(|table| table.relation == name);
        let proposed = candidate.tables.iter().find(|table| table.relation == name);
        if original != proposed {
            differences.push(format!("table {}.{}", quote(&name.schema), quote(&name.name)));
        }
    }
    if reference.sequences != candidate.sequences {
        differences.push("sequence state".into());
    }
    Ok(ComparisonReport {
        schema_version: 1,
        scope: "observed-database-equivalence".into(),
        equivalent: differences.is_empty(),
        replacement_admission: "blocked: release provenance, upgrade fixtures, recovery and activation gates are separate requirements".into(),
        differences,
        reference_digest: snapshot_digest(reference),
        candidate_digest: snapshot_digest(candidate),
    })
}

fn snapshot_digest(snapshot: &DatabaseSnapshot) -> String {
    // Serialization of this closed struct contains no unordered maps or floats.
    hex::encode(Sha256::digest(serde_json::to_vec(snapshot).expect("snapshot serialization")))
}

pub fn validate_journal(actual: &[String], expected: &[String]) -> Result<()> {
    let expected_set: BTreeSet<_> = expected.iter().collect();
    if expected.is_empty() || expected.iter().any(|name| name.trim().is_empty() || name.trim() != name) || expected_set.len() != expected.len() {
        return Err(VerificationError::Incomplete("expected plan is empty or contains duplicate identities".into()));
    }
    let actual_set: BTreeSet<_> = actual.iter().collect();
    if actual.len() != actual_set.len() || actual_set != expected_set {
        return Err(VerificationError::Incomplete("migration journal does not exactly match its executable plan".into()));
    }
    Ok(())
}

fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn qualified(name: &RelationName) -> String {
    format!("{}.{}", quote(&name.schema), quote(&name.name))
}

fn statement(sql: impl Into<String>) -> Statement {
    Statement::from_string(DbBackend::Postgres, sql.into())
}

async fn strings(tx: &DatabaseTransaction, sql: impl Into<String>) -> Result<Vec<String>> {
    let rows = tx.query_all_raw(statement(sql)).await?;
    let mut values = rows.into_iter().map(|row| row.try_get("", "item").map_err(VerificationError::from)).collect::<Result<Vec<String>>>()?;
    values.sort();
    Ok(values)
}

/// Observe a disposable, quiescent fixture database. The caller must stop every
/// writer: sequence state is non-MVCC. Credentials are passed to libpq through
/// the child environment, never as process arguments or report fields.
pub async fn observe_database(
    db: &DatabaseConnection,
    database_url: &str,
    pg_dump: &Path,
) -> Result<DatabaseSnapshot> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(VerificationError::Incomplete("only PostgreSQL 16 is supported".into()));
    }
    let tx = db.begin_with_config(Some(IsolationLevel::RepeatableRead), Some(AccessMode::ReadOnly)).await?;
    for setting in [
        "SET LOCAL search_path = pg_catalog", "SET LOCAL timezone = 'UTC'",
        "SET LOCAL datestyle = 'ISO, YMD'", "SET LOCAL intervalstyle = 'iso_8601'",
        "SET LOCAL extra_float_digits = 3", "SET LOCAL bytea_output = 'hex'",
        "SET LOCAL row_security = off", "SET LOCAL statement_timeout = '30s'",
        "SET LOCAL lock_timeout = '5s'",
    ] {
        tx.execute_unprepared(setting).await?;
    }
    let version = strings(&tx, "SELECT current_setting('server_version_num') AS item").await?;
    if version[0].parse::<u32>().unwrap_or(0) / 10_000 != 16 {
        return Err(VerificationError::Incomplete("only PostgreSQL major 16 is supported".into()));
    }
    let tool_version = run_pg_dump(pg_dump, &["--version"], None, 4096)?;
    let tool_version = String::from_utf8(tool_version)
        .map_err(|_| VerificationError::Tool("pg_dump version is not UTF-8".into()))?;
    if !tool_version.contains("(PostgreSQL) 16.") {
        return Err(VerificationError::Incomplete("pg_dump major 16 is required".into()));
    }
    let mut environment = strings(&tx, "SELECT jsonb_build_array(current_setting('server_version_num'), pg_encoding_to_char(encoding), datcollate, datctype, datlocprovider, daticulocale, pg_get_userbyid(datdba), datacl::text, datallowconn, datconnlimit, obj_description(oid,'pg_database'))::text AS item FROM pg_database WHERE datname = current_database()").await?;
    environment.extend(strings(&tx, "SELECT jsonb_build_array('database-role-settings', setdatabase=0, CASE WHEN setrole=0 THEN 'PUBLIC' ELSE pg_get_userbyid(setrole) END, setconfig)::text AS item FROM pg_db_role_setting WHERE setdatabase=0 OR setdatabase=(SELECT oid FROM pg_database WHERE datname=current_database()) ORDER BY 1").await?);
    environment.push(tool_version.trim().into());
    // pg_dump covers schema objects, ACLs, default privileges, comments, security
    // labels, routines, types and global database objects. Do not strip owners,
    // privileges or extension declarations to make a comparison pass.
    let snapshot_id = strings(&tx, "SELECT pg_export_snapshot() AS item").await?;
    let dump = run_pg_dump(pg_dump, &["--schema-only", "--quote-all-identifiers", "--no-password", "--lock-wait-timeout=5s", "--snapshot", &snapshot_id[0]], Some(database_url), MAX_BYTES)?;
    let schema = normalize_dump(String::from_utf8(dump)
        .map_err(|_| VerificationError::Incomplete("schema dump is not UTF-8".into()))?)?;
    // pg_dump omits internal trigger states and invalid-index state. It also
    // represents extension objects by CREATE EXTENSION. Supported extension
    // member definitions are observed below; unknown classes fail closed.
    let mut enforcement = Vec::new();
    let queries = [
        format!("SELECT jsonb_build_array('trigger', n.nspname, c.relname, CASE WHEN t.tgisinternal THEN NULL ELSE t.tgname END, t.tgisinternal, t.tgenabled, t.tgtype, t.tgdeferrable, t.tginitdeferred, t.tgfoid::regprocedure::text, con.conname, cn.nspname, cc.relname, encode(t.tgargs, 'hex'), t.tgoldtable, t.tgnewtable)::text AS item FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid JOIN pg_namespace n ON n.oid=c.relnamespace LEFT JOIN pg_constraint con ON con.oid=t.tgconstraint LEFT JOIN pg_class cc ON cc.oid=con.conrelid LEFT JOIN pg_namespace cn ON cn.oid=cc.relnamespace WHERE {APPLICATION_SCHEMA} ORDER BY 1"),
        format!("SELECT jsonb_build_array('index', n.nspname, c.relname, pg_get_indexdef(i.indexrelid), i.indisvalid, i.indisready, i.indislive, i.indisreplident)::text AS item FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE {APPLICATION_SCHEMA} ORDER BY 1"),
        "SELECT jsonb_build_array('extension', e.extname, e.extversion, pg_get_userbyid(e.extowner), n.nspname, e.extrelocatable)::text AS item FROM pg_extension e JOIN pg_namespace n ON n.oid=e.extnamespace ORDER BY 1".into(),
        "SELECT jsonb_build_array('role', rolname, rolsuper, rolinherit, rolcreaterole, rolcreatedb, rolcanlogin, rolreplication, rolbypassrls, rolconnlimit, rolvaliduntil, rolconfig)::text AS item FROM pg_roles ORDER BY 1".into(),
        "SELECT jsonb_build_array('membership', pg_get_userbyid(roleid), pg_get_userbyid(member), pg_get_userbyid(grantor), admin_option, inherit_option, set_option)::text AS item FROM pg_auth_members ORDER BY 1".into(),
        "SELECT jsonb_build_array('extension-member', e.extname, x.type, x.schema, x.name, x.identity)::text AS item FROM pg_depend d JOIN pg_extension e ON d.refclassid='pg_extension'::regclass AND d.refobjid=e.oid CROSS JOIN LATERAL pg_identify_object(d.classid,d.objid,d.objsubid) x WHERE d.deptype='e' ORDER BY 1".into(),
        "SELECT jsonb_build_array('extension-routine', e.extname, n.nspname, p.proname, pg_get_function_identity_arguments(p.oid), pg_get_functiondef(p.oid), p.proacl::text, pg_get_userbyid(p.proowner))::text AS item FROM pg_depend d JOIN pg_extension e ON d.refclassid='pg_extension'::regclass AND d.refobjid=e.oid JOIN pg_proc p ON d.classid='pg_proc'::regclass AND d.objid=p.oid JOIN pg_namespace n ON n.oid=p.pronamespace WHERE d.deptype='e' AND p.prokind IN ('f','p') ORDER BY 1".into(),
        "SELECT jsonb_build_array('extension-language', l.lanname, pg_get_userbyid(l.lanowner), l.lanispl, l.lanpltrusted, l.lanplcallfoid::regprocedure::text, l.laninline::regprocedure::text, l.lanvalidator::regprocedure::text, l.lanacl::text)::text AS item FROM pg_language l JOIN pg_depend d ON d.classid='pg_language'::regclass AND d.objid=l.oid AND d.deptype='e' ORDER BY 1".into(),
        "SELECT jsonb_build_array('extension-operator', n.nspname, o.oprname, pg_get_userbyid(o.oprowner), o.oprkind, o.oprcanmerge, o.oprcanhash, o.oprleft::regtype::text, o.oprright::regtype::text, o.oprresult::regtype::text, o.oprcom::regoperator::text, o.oprnegate::regoperator::text, o.oprcode::regprocedure::text, o.oprrest::regprocedure::text, o.oprjoin::regprocedure::text)::text AS item FROM pg_operator o JOIN pg_namespace n ON n.oid=o.oprnamespace JOIN pg_depend d ON d.classid='pg_operator'::regclass AND d.objid=o.oid AND d.deptype='e' ORDER BY 1".into(),
        "SELECT jsonb_build_array('extension-opclass', n.nspname, c.opcname, pg_get_userbyid(c.opcowner), am.amname, fn.nspname, f.opfname, c.opcintype::regtype::text, c.opcdefault, c.opckeytype::regtype::text)::text AS item FROM pg_opclass c JOIN pg_namespace n ON n.oid=c.opcnamespace JOIN pg_am am ON am.oid=c.opcmethod JOIN pg_opfamily f ON f.oid=c.opcfamily JOIN pg_namespace fn ON fn.oid=f.opfnamespace JOIN pg_depend d ON d.classid='pg_opclass'::regclass AND d.objid=c.oid AND d.deptype='e' ORDER BY 1".into(),
        "SELECT jsonb_build_array('extension-opfamily', n.nspname, f.opfname, pg_get_userbyid(f.opfowner), am.amname)::text AS item FROM pg_opfamily f JOIN pg_namespace n ON n.oid=f.opfnamespace JOIN pg_am am ON am.oid=f.opfmethod JOIN pg_depend d ON d.classid='pg_opfamily'::regclass AND d.objid=f.oid AND d.deptype='e' ORDER BY 1".into(),
        "SELECT jsonb_build_array('extension-amop', n.nspname, f.opfname, a.amoplefttype::regtype::text, a.amoprighttype::regtype::text, a.amopstrategy, a.amoppurpose, a.amopopr::regoperator::text, am.amname, sn.nspname, sf.opfname)::text AS item FROM pg_amop a JOIN pg_opfamily f ON f.oid=a.amopfamily JOIN pg_namespace n ON n.oid=f.opfnamespace JOIN pg_am am ON am.oid=a.amopmethod LEFT JOIN pg_opfamily sf ON sf.oid=a.amopsortfamily LEFT JOIN pg_namespace sn ON sn.oid=sf.opfnamespace WHERE EXISTS (SELECT 1 FROM pg_depend d WHERE d.classid='pg_opfamily'::regclass AND d.objid=f.oid AND d.deptype='e') ORDER BY 1".into(),
        "SELECT jsonb_build_array('extension-amproc', n.nspname, f.opfname, a.amproclefttype::regtype::text, a.amprocrighttype::regtype::text, a.amprocnum, a.amproc::regprocedure::text)::text AS item FROM pg_amproc a JOIN pg_opfamily f ON f.oid=a.amprocfamily JOIN pg_namespace n ON n.oid=f.opfnamespace WHERE EXISTS (SELECT 1 FROM pg_depend d WHERE d.classid='pg_opfamily'::regclass AND d.objid=f.oid AND d.deptype='e') ORDER BY 1".into(),
        "SELECT jsonb_build_array('extension-type', n.nspname, t.typname, pg_get_userbyid(t.typowner), t.typlen, t.typbyval, t.typtype, t.typcategory, t.typispreferred, t.typisdefined, t.typdelim, t.typelem::regtype::text, t.typarray::regtype::text, t.typinput::regprocedure::text, t.typoutput::regprocedure::text, t.typreceive::regprocedure::text, t.typsend::regprocedure::text, t.typmodin::regprocedure::text, t.typmodout::regprocedure::text, t.typanalyze::regprocedure::text, t.typalign, t.typstorage, t.typnotnull, t.typbasetype::regtype::text, t.typtypmod, t.typndims, t.typdefault, t.typacl::text)::text AS item FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace JOIN pg_depend d ON d.classid='pg_type'::regclass AND d.objid=t.oid AND d.deptype='e' ORDER BY 1".into(),
    ];
    for sql in queries { enforcement.extend(strings(&tx, sql).await?); }
    let invalid = strings(&tx, format!("SELECT c.relname AS item FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE {APPLICATION_SCHEMA} AND (NOT i.indisvalid OR NOT i.indisready OR NOT i.indislive)")).await?;
    if !invalid.is_empty() {
        return Err(VerificationError::Incomplete("fixture contains unfinished or invalid indexes".into()));
    }
    // Foreign data and large objects are outside the local table snapshot.
    let unsupported = strings(&tx, "SELECT 'large objects' AS item WHERE EXISTS (SELECT 1 FROM pg_largeobject_metadata) UNION ALL SELECT 'foreign tables' WHERE EXISTS (SELECT 1 FROM pg_foreign_table) UNION ALL SELECT 'subscriptions' WHERE EXISTS (SELECT 1 FROM pg_subscription) UNION ALL SELECT 'unsupported extensions' WHERE EXISTS (SELECT 1 FROM pg_extension WHERE extname NOT IN ('plpgsql','pg_trgm')) UNION ALL SELECT 'unsupported extension member class' WHERE EXISTS (SELECT 1 FROM pg_depend WHERE refclassid='pg_extension'::regclass AND deptype='e' AND classid NOT IN ('pg_proc'::regclass,'pg_language'::regclass,'pg_operator'::regclass,'pg_opclass'::regclass,'pg_opfamily'::regclass,'pg_type'::regclass)) UNION ALL SELECT 'unsupported extension routine' WHERE EXISTS (SELECT 1 FROM pg_proc p JOIN pg_depend d ON d.classid='pg_proc'::regclass AND d.objid=p.oid AND d.deptype='e' WHERE p.prokind NOT IN ('f','p')) UNION ALL SELECT 'unsupported extension type' WHERE EXISTS (SELECT 1 FROM pg_type t JOIN pg_depend d ON d.classid='pg_type'::regclass AND d.objid=t.oid AND d.deptype='e' WHERE t.typtype NOT IN ('b','p'))").await?;
    if !unsupported.is_empty() {
        return Err(VerificationError::Incomplete(format!("unsupported state: {}", unsupported.join(", "))));
    }
    let relations = tx.query_all_raw(statement(format!("SELECT n.nspname AS schema, c.relname AS name, c.relkind::text AS kind FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE {APPLICATION_SCHEMA} AND c.relkind IN ('r','m','S') ORDER BY n.nspname COLLATE \"C\", c.relname COLLATE \"C\""))).await?;
    let mut tables = Vec::new();
    let mut sequences = Vec::new();
    let mut migration_versions = None;
    let mut total_bytes = 0usize;
    let mut total_rows = 0usize;
    for row in relations {
        let relation = RelationName { schema: row.try_get("", "schema")?, name: row.try_get("", "name")? };
        let kind: String = row.try_get("", "kind")?;
        if kind == "S" {
            sequences.push(sequence_state(&tx, relation).await?);
            continue;
        }
        if relation.schema == "public" && relation.name == "seaql_migrations" {
            migration_versions = Some(strings(&tx, "SELECT version::text AS item FROM public.seaql_migrations ORDER BY version COLLATE \"C\"").await?);
            continue;
        }
        let columns = tx.query_all_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "SELECT a.attname AS name, format_type(a.atttypid,a.atttypmod) AS sql_type FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=$1 AND c.relname=$2 AND a.attnum>0 AND NOT a.attisdropped ORDER BY a.attnum",
            [relation.schema.clone().into(), relation.name.clone().into()])).await?;
        let mut definitions = Vec::new();
        let mut expressions = Vec::new();
        for column in columns {
            let name: String = column.try_get("", "name")?;
            let sql_type: String = column.try_get("", "sql_type")?;
            definitions.push(serde_json::json!([name, sql_type]).to_string());
            expressions.push(format!("{}::text", quote(&name)));
        }
        let only = if kind == "m" { "" } else { "ONLY " };
        tx.execute_unprepared(&format!("DECLARE rustok_rows NO SCROLL CURSOR FOR SELECT CASE WHEN octet_length(item) > {MAX_ROW_BYTES} THEN NULL ELSE item END AS item FROM (SELECT jsonb_build_array({})::text AS item FROM {only}{}) rows ORDER BY item COLLATE \"C\"", expressions.join(","), qualified(&relation))).await?;
        let mut rows = Vec::new();
        loop {
            let batch = tx.query_all_raw(statement("FETCH 256 FROM rustok_rows")).await?;
            if batch.is_empty() { break; }
            for row in batch {
                let value: Option<String> = row.try_get("", "item")?;
                let value = value.ok_or_else(|| VerificationError::Incomplete("row exceeds observation byte limit".into()))?;
                total_bytes += value.len();
                total_rows += 1;
                if total_bytes > MAX_BYTES || total_rows > MAX_ROWS {
                    return Err(VerificationError::Incomplete("fixture exceeds observation row/byte limits".into()));
                }
                rows.push(value);
            }
        }
        tx.execute_unprepared("CLOSE rustok_rows").await?;
        tables.push(TableObservation { relation, columns: definitions, rows });
    }
    for sequence in &sequences {
        if sequence_state(&tx, sequence.relation.clone()).await? != *sequence {
            return Err(VerificationError::Incomplete("sequence changed while observing; stop fixture writers".into()));
        }
    }
    let migration_versions = migration_versions.ok_or_else(|| VerificationError::Incomplete("public.seaql_migrations is missing".into()))?;
    tx.commit().await?;
    Ok(DatabaseSnapshot { environment, schema, enforcement, tables, sequences, migration_versions })
}

fn run_pg_dump(binary: &Path, arguments: &[&str], database_url: Option<&str>, limit: usize) -> Result<Vec<u8>> {
    let mut command = Command::new(binary);
    command.args(arguments).env_clear().stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for key in ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH"] {
        if let Some(value) = std::env::var_os(key) { command.env(key, value); }
    }
    if let Some(url) = database_url {
        command.env("PGDATABASE", url).env("PGCONNECT_TIMEOUT", "10")
            .env("PGOPTIONS", "-c statement_timeout=30000 -c lock_timeout=5000");
    }
    let mut child = command.spawn()?;
    let exceeded = Arc::new(AtomicBool::new(false));
    fn reader(mut pipe: impl Read + Send + 'static, limit: usize, exceeded: Arc<AtomicBool>) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>> {
        std::thread::spawn(move || {
            let mut result = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let count = pipe.read(&mut buffer)?;
                if count == 0 { return Ok(result); }
                if result.len() + count > limit { exceeded.store(true, Ordering::SeqCst); }
                else { result.extend_from_slice(&buffer[..count]); }
            }
        })
    }
    let stdout = reader(child.stdout.take().expect("piped stdout"), limit, exceeded.clone());
    let stderr = reader(child.stderr.take().expect("piped stderr"), 64 * 1024, exceeded.clone());
    let start = Instant::now();
    let status = loop {
        if exceeded.load(Ordering::SeqCst) || start.elapsed() > Duration::from_secs(120) {
            child.kill()?;
            child.wait()?;
            return Err(VerificationError::Tool("pg_dump exceeded its time or output limit".into()));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                child.kill()?;
                child.wait()?;
                return Err(error.into());
            }
        }
    };
    let output = stdout.join().map_err(|_| VerificationError::Tool("pg_dump reader failed".into()))??;
    let stderr = stderr.join().map_err(|_| VerificationError::Tool("pg_dump error reader failed".into()))??;
    if !status.success() || exceeded.load(Ordering::SeqCst) {
        let mut detail = String::from_utf8_lossy(&stderr).into_owned();
        if let Some(database_url) = database_url {
            detail = detail.replace(database_url, "[fixture URL]");
            if let Ok(url) = url::Url::parse(database_url) {
                if let Some(password) = url.password().filter(|password| !password.is_empty()) { detail = detail.replace(password, "[redacted]"); }
            }
        }
        return Err(VerificationError::Tool(format!("pg_dump failed: {}", detail.trim())));
    }
    Ok(output)
}

async fn sequence_state(tx: &DatabaseTransaction, relation: RelationName) -> Result<SequenceObservation> {
    let row = tx.query_one_raw(statement(format!("SELECT last_value::text AS value, is_called FROM {}", qualified(&relation)))).await?
        .ok_or_else(|| VerificationError::Incomplete("missing sequence state".into()))?;
    Ok(SequenceObservation { relation, last_value: row.try_get("", "value")?, is_called: row.try_get("", "is_called")? })
}

fn normalize_dump(dump: String) -> Result<String> {
    // Recent security backports add a random psql guard. Remove only a matched
    // guard pair in the prologue/epilogue, never lines in a routine body.
    let mut lines: Vec<_> = dump.split_inclusive('\n').collect();
    let substantive = |line: &&str| !line.trim().is_empty() && !line.starts_with("--");
    let first = lines.iter().position(substantive);
    let last = lines.iter().rposition(substantive);
    if let (Some(first), Some(last)) = (first, last) {
        if let Some(token) = lines[first].trim_end().strip_prefix("\\restrict ") {
            if first == last || lines[last].trim_end() != format!("\\unrestrict {token}") {
                return Err(VerificationError::Incomplete("unmatched pg_dump restriction guard".into()));
            }
            lines.remove(last);
            lines.remove(first);
        }
    }
    Ok(lines.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> DatabaseSnapshot {
        DatabaseSnapshot { environment: vec!["16".into()], schema: "schema".into(), enforcement: vec![], tables: vec![TableObservation { relation: RelationName { schema: "public".into(), name: "records".into() }, columns: vec!["numeric".into()], rows: vec!["[\"9007199254740993.123\",null]".into()] }], sequences: vec![], migration_versions: vec!["original".into()] }
    }

    #[test]
    fn consolidated_journal_is_validated_separately() {
        let original = snapshot();
        let mut candidate = original.clone();
        candidate.migration_versions = vec!["baseline".into()];
        assert!(compare(&original, &candidate, &["original".into()], &["baseline".into()]).unwrap().equivalent);
        assert!(compare(&original, &candidate, &["original".into()], &["wrong".into()]).is_err());
    }

    #[test]
    fn preserves_duplicates_precision_and_sql_null() {
        let original = snapshot();
        for changed in ["[\"9007199254740993.124\",null]", "[\"9007199254740993.123\",\"null\"]"] {
            let mut candidate = original.clone();
            candidate.tables[0].rows[0] = changed.into();
            assert!(!compare(&original, &candidate, &["original".into()], &["original".into()]).unwrap().equivalent);
        }
        let mut candidate = original.clone();
        let duplicate = candidate.tables[0].rows[0].clone();
        candidate.tables[0].rows.push(duplicate);
        assert!(!compare(&original, &candidate, &["original".into()], &["original".into()]).unwrap().equivalent);
    }

    #[test]
    fn normalizes_only_dump_guard_positions() {
        let dump = "-- dump\n\\restrict abc\nCREATE FUNCTION f() RETURNS text AS $$\n\\restrict inside\n\\unrestrict inside\n$$ LANGUAGE SQL;\n-- complete\n\\unrestrict abc\n";
        let normalized = normalize_dump(dump.into()).unwrap();
        assert!(normalized.contains("\\restrict inside"));
        assert!(!normalized.contains("\\restrict abc"));
        assert!(normalize_dump(dump.replace("unrestrict abc", "unrestrict wrong")).is_err());
    }

    #[test]
    fn identifiers_cannot_inject_sql() {
        assert_eq!(quote("a\"; DROP TABLE x;--"), "\"a\"\"; DROP TABLE x;--\"");
    }

    #[tokio::test]
    #[ignore = "requires isolated PostgreSQL 16 and RUSTOK_MIGRATION_VERIFY_TEST_ADMIN_URL / RUSTOK_MIGRATION_VERIFY_TEST_PG_DUMP"]
    async fn postgres_detects_schema_data_privilege_and_enforcement_changes() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use sea_orm_migration::sea_orm::Database;
        let admin_url = std::env::var("RUSTOK_MIGRATION_VERIFY_TEST_ADMIN_URL")?;
        let pg_dump = std::env::var("RUSTOK_MIGRATION_VERIFY_TEST_PG_DUMP")?;
        let admin = Database::connect(&admin_url).await?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        let names = [format!("rustok_observe_a_{id}"), format!("rustok_observe_b_{id}")];
        let test = async {
            for name in &names { admin.execute_unprepared(&format!("CREATE DATABASE {name} TEMPLATE template0")).await?; }
            let mut a_url = url::Url::parse(&admin_url)?;
            a_url.set_path(&format!("/{}", names[0]));
            let mut b_url = url::Url::parse(&admin_url)?;
            b_url.set_path(&format!("/{}", names[1]));
            let a = Database::connect(a_url.as_str()).await?;
            let b = Database::connect(b_url.as_str()).await?;
            let body = async {
                for (db, version, old_column) in [(&a, "original", true), (&b, "baseline", false)] {
                    db.execute_unprepared("CREATE EXTENSION pg_trgm").await?;
                    db.execute_unprepared("CREATE TABLE public.seaql_migrations (version varchar(255) PRIMARY KEY, applied_at bigint NOT NULL)").await?;
                    db.execute_unprepared(&format!("INSERT INTO public.seaql_migrations VALUES ('{version}', 1)")).await?;
                    db.execute_unprepared(&format!("CREATE TABLE records (id integer, {} amount numeric(38,18), payload json, raw text)", if old_column { "obsolete text," } else { "" })).await?;
                    if old_column { db.execute_unprepared("ALTER TABLE records DROP COLUMN obsolete").await?; }
                    db.execute_unprepared("INSERT INTO records VALUES (1, 9007199254740993.123456789012345678, '{\"a\":1,\"a\":2}', NULL), (1, 9007199254740993.123456789012345678, '{\"a\":1,\"a\":2}', NULL)").await?;
                    db.execute_unprepared("CREATE TABLE parent (id integer PRIMARY KEY); CREATE TABLE child (id integer REFERENCES parent(id)); INSERT INTO parent VALUES (1); INSERT INTO child VALUES (1); CREATE SEQUENCE record_sequence").await?;
                }
                let original = observe_database(&a, a_url.as_str(), Path::new(&pg_dump)).await?;
                let reference_plan = vec!["original".into()];
                let candidate_plan = vec!["baseline".into()];
                let candidate = observe_database(&b, b_url.as_str(), Path::new(&pg_dump)).await?;
                let report = compare(&original, &candidate, &reference_plan, &candidate_plan)?;
                assert!(report.equivalent, "{:?}", report.differences);
                for (change, restore, expected) in [
                    ("UPDATE records SET amount=9007199254740993.123456789012345679", "UPDATE records SET amount=9007199254740993.123456789012345678", "table"),
                    ("UPDATE records SET payload='{\"a\":2}'", "UPDATE records SET payload='{\"a\":1,\"a\":2}'", "table"),
                    ("ALTER TABLE records ALTER COLUMN raw SET DEFAULT 'changed'", "ALTER TABLE records ALTER COLUMN raw DROP DEFAULT", "schema"),
                    ("ALTER TABLE child DISABLE TRIGGER ALL", "ALTER TABLE child ENABLE TRIGGER ALL", "enforcement"),
                    ("SELECT nextval('record_sequence')", "SELECT setval('record_sequence', 1, false)", "sequence"),
                ] {
                    b.execute_unprepared(change).await?;
                    let changed = observe_database(&b, b_url.as_str(), Path::new(&pg_dump)).await?;
                    let report = compare(&original, &changed, &reference_plan, &candidate_plan)?;
                    assert!(!report.equivalent, "missed {expected}");
                    b.execute_unprepared(restore).await?;
                }
                b.execute_unprepared(&format!("ALTER DATABASE {} SET default_transaction_read_only=on", names[1])).await?;
                let changed = observe_database(&b, b_url.as_str(), Path::new(&pg_dump)).await?;
                assert!(!compare(&original, &changed, &reference_plan, &candidate_plan)?.equivalent);
                Ok::<_, Box<dyn std::error::Error>>(())
            }.await;
            a.close().await?;
            b.close().await?;
            body
        }.await;
        let mut cleanup = Vec::new();
        for name in &names {
            if let Err(error) = admin.execute_unprepared(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)")).await { cleanup.push(error.to_string()); }
        }
        admin.close().await?;
        if !cleanup.is_empty() { return Err(format!("test: {test:?}; cleanup: {cleanup:?}").into()); }
        test
    }
}

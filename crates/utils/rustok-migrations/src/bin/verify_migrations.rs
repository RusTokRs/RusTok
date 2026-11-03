//! Disposable verification runner. Destructive production migration commands
//! remain unavailable in rustok-migrate.

use rustok_migrations::verification::{compare, observe_database, ComparisonReport};
use sea_orm_migration::sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, fs, io::Write, path::Path, process::{Command, Stdio}, time::{Duration, Instant}};
use url::Url;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or("use fresh or compare; --help shows arguments")?;
    if command == "--help" || command == "-h" {
        println!("verify-migrations fresh --admin-url-env ENV --reference-runner FILE --candidate-runner FILE --reference-root DIR --candidate-root DIR --reference-plan FILE --candidate-plan FILE --pg-dump FILE --output FILE");
        println!("verify-migrations compare --reference-url-env ENV --candidate-url-env ENV --reference-plan FILE --candidate-plan FILE --pg-dump FILE --output FILE --quiescent");
        return Ok(());
    }
    if command != "fresh" && command != "compare" { return Err("expected fresh or compare".into()); }
    let mut options = BTreeMap::new();
    let mut quiescent = false;
    while let Some(arg) = args.next() {
        if arg == "--quiescent" {
            if quiescent { return Err("duplicate --quiescent".into()); }
            quiescent = true;
        } else {
            let allowed = match command.as_str() {
                "fresh" => ["--admin-url-env", "--reference-runner", "--candidate-runner", "--reference-root", "--candidate-root", "--reference-plan", "--candidate-plan", "--pg-dump", "--output"].as_slice(),
                _ => ["--reference-url-env", "--candidate-url-env", "--reference-plan", "--candidate-plan", "--pg-dump", "--output"].as_slice(),
            };
            if !allowed.contains(&arg.as_str()) { return Err(format!("unknown argument {arg}").into()); }
            let value = args.next().ok_or_else(|| format!("{arg} requires a value"))?;
            if options.insert(arg.clone(), value).is_some() { return Err(format!("duplicate {arg}").into()); }
        }
    }
    let get = |name: &str| options.get(name).map(String::as_str).ok_or_else(|| format!("{name} is required"));
    let output = Path::new(get("--output")?);
    if output.exists() { return Err("output already exists; choose a new report path".into()); }
    let reference_bytes = fs::read(get("--reference-plan")?)?;
    let candidate_bytes = fs::read(get("--candidate-plan")?)?;
    let reference_plan = read_plan(&reference_bytes)?;
    let candidate_plan = read_plan(&candidate_bytes)?;
    let pg_dump = Path::new(get("--pg-dump")?);
    let mut provenance = BTreeMap::new();
    for name in ["--reference-runner", "--candidate-runner", "--pg-dump"] {
        if let Some(path) = options.get(name) { provenance.insert(name.trim_start_matches("--").to_string(), file_digest(Path::new(path))?); }
    }
    provenance.insert("reference-plan".into(), hex::encode(Sha256::digest(&reference_bytes)));
    provenance.insert("candidate-plan".into(), hex::encode(Sha256::digest(&candidate_bytes)));
    let mut report = if command == "fresh" {
        let admin_url = std::env::var(get("--admin-url-env")?).map_err(|_| "admin URL environment variable is missing")?;
        fresh(&admin_url, Path::new(get("--reference-runner")?), Path::new(get("--candidate-runner")?), Path::new(get("--reference-root")?), Path::new(get("--candidate-root")?), &reference_plan, &candidate_plan, pg_dump).await?
    } else {
        if !quiescent { return Err("compare requires --quiescent: stop all fixture writers before observation".into()); }
        let reference_url = std::env::var(get("--reference-url-env")?).map_err(|_| "reference URL environment variable is missing")?;
        let candidate_url = std::env::var(get("--candidate-url-env")?).map_err(|_| "candidate URL environment variable is missing")?;
        compare_urls(&reference_url, &candidate_url, &reference_plan, &candidate_plan, pg_dump).await?
    };
    if command == "fresh" { report.scope = "fresh-install-equivalence".into(); }
    for name in ["--reference-plan", "--candidate-plan", "--reference-runner", "--candidate-runner", "--pg-dump"] {
        if let Some(path) = options.get(name) {
            if provenance[name.trim_start_matches("--")] != file_digest(Path::new(path))? {
                return Err(format!("{name} changed during verification; evidence is stale").into());
            }
        }
    }
    let artifact = serde_json::json!({ "schema_version": 1, "report": report, "input_sha256": provenance });
    if let Some(parent) = output.parent() { fs::create_dir_all(parent)?; }
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(output)?;
    file.write_all(serde_json::to_string_pretty(&artifact)?.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!("{}: report {}", if report.equivalent { "observed results match" } else { "observed results differ" }, output.display());
    if !report.equivalent { return Err("migration results differ; inspect report differences".into()); }
    Ok(())
}

fn read_plan(bytes: &[u8]) -> Result<Vec<String>> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    if value.get("schema_version").and_then(|v| v.as_u64()) != Some(1) { return Err("unsupported migration plan version".into()); }
    let migrations: Vec<String> = serde_json::from_value(value.get("migrations").ok_or("plan migrations missing")?.clone())?;
    rustok_migrations::verification::validate_journal(&migrations, &migrations)?;
    Ok(migrations)
}

async fn compare_urls(reference_url: &str, candidate_url: &str, reference_plan: &[String], candidate_plan: &[String], pg_dump: &Path) -> Result<ComparisonReport> {
    let reference = Database::connect(reference_url).await.map_err(|_| "cannot connect to reference fixture")?;
    let candidate = Database::connect(candidate_url).await.map_err(|_| "cannot connect to candidate fixture")?;
    let result = async {
        let original = observe_database(&reference, reference_url, pg_dump).await?;
        let proposed = observe_database(&candidate, candidate_url, pg_dump).await?;
        Ok(compare(&original, &proposed, reference_plan, candidate_plan)?)
    }.await;
    let close_reference = reference.close().await;
    let close_candidate = candidate.close().await;
    close_reference?;
    close_candidate?;
    result
}

#[allow(clippy::too_many_arguments)]
async fn fresh(admin_url: &str, reference_runner: &Path, candidate_runner: &Path, reference_root: &Path, candidate_root: &Path, reference_plan: &[String], candidate_plan: &[String], pg_dump: &Path) -> Result<ComparisonReport> {
    let admin = Database::connect(admin_url).await.map_err(|_| "cannot connect to verification administrator")?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let role = format!("rustok_verify_{id}");
    let password = uuid::Uuid::new_v4().simple().to_string();
    let names = [format!("rustok_original_{id}"), format!("rustok_candidate_{id}")];
    // All interpolated identifiers and the password are generated UUIDs. No
    // caller-controlled database name is ever accepted for deletion.
    let result = async {
        admin.execute_unprepared(&format!("CREATE ROLE {role} LOGIN PASSWORD '{password}' NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS")).await?;
        for name in &names {
            admin.execute_unprepared(&format!("CREATE DATABASE {name} OWNER {role} TEMPLATE template0 ENCODING 'UTF8'")).await?;
            admin.execute_unprepared(&format!("REVOKE CONNECT ON DATABASE {name} FROM PUBLIC")).await?;
        }
        let reference_url = fixture_url(admin_url, &role, &password, &names[0])?;
        let candidate_url = fixture_url(admin_url, &role, &password, &names[1])?;
        run_migrator(reference_runner, reference_root, &reference_url)?;
        // Capture before candidate code can change cluster-wide role state or
        // the other fixture database owned by the same isolated fixture role.
        let reference = Database::connect(&reference_url).await?;
        let original_result = observe_database(&reference, &reference_url, pg_dump).await;
        reference.close().await?;
        let original = original_result?;
        run_migrator(candidate_runner, candidate_root, &candidate_url)?;
        let candidate = Database::connect(&candidate_url).await?;
        let proposed_result = observe_database(&candidate, &candidate_url, pg_dump).await;
        candidate.close().await?;
        let proposed = proposed_result?;
        Ok(compare(&original, &proposed, reference_plan, candidate_plan)?)
    }.await;
    let cleanup = cleanup(&admin, &names, &role).await;
    let close = admin.close().await;
    match (result, cleanup, close) {
        (result, Err(error), close) => Err(format!("verification result: {result:?}; cleanup failed: {error}; administrator close: {close:?}").into()),
        (result, _, Err(error)) => Err(format!("verification result: {result:?}; administrator close: {error}").into()),
        (result, Ok(()), Ok(())) => result,
    }
}

async fn cleanup(admin: &DatabaseConnection, names: &[String], role: &str) -> Result<()> {
    let mut failures = Vec::new();
    for name in names {
        if let Err(error) = admin.execute_unprepared(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)")).await {
            failures.push(format!("{name}: {error}"));
        }
    }
    if let Err(error) = admin.execute_unprepared(&format!("DROP ROLE IF EXISTS {role}")).await { failures.push(format!("{role}: {error}")); }
    if failures.is_empty() { Ok(()) } else { Err(failures.join("; ").into()) }
}

fn fixture_url(admin_url: &str, role: &str, password: &str, database: &str) -> Result<String> {
    let mut url = Url::parse(admin_url).map_err(|_| "administrator URL is invalid")?;
    if !matches!(url.scheme(), "postgres" | "postgresql") { return Err("PostgreSQL URL is required".into()); }
    // Query credentials/options could override the substituted fixture identity.
    if url.query().is_some() || url.fragment().is_some() { return Err("verification admin URL must not contain query options or a fragment".into()); }
    url.set_username(role).map_err(|_| "cannot set fixture role")?;
    url.set_password(Some(password)).map_err(|_| "cannot set fixture password")?;
    url.set_path(&format!("/{database}"));
    Ok(url.into())
}

fn file_digest(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop { let count = file.read(&mut buffer)?; if count == 0 { break; } digest.update(&buffer[..count]); }
    Ok(hex::encode(digest.finalize()))
}

fn run_migrator(binary: &Path, source_root: &Path, database_url: &str) -> Result<()> {
    for operation in ["up", "status"] {
        let mut command = Command::new(binary);
        command.arg(operation).current_dir(source_root).env_clear().env("DATABASE_URL", database_url)
            .stdin(Stdio::null());
        for key in ["PATH", "SystemRoot", "WINDIR", "TEMP", "TMP", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH"] {
            if let Some(value) = std::env::var_os(key) { command.env(key, value); }
        }
        let mut child = command.spawn()?;
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() { return Err(format!("migration runner {operation} failed").into()); }
                break;
            }
            if start.elapsed() > Duration::from_secs(300) {
                child.kill()?;
                child.wait()?;
                return Err(format!("migration runner {operation} exceeded five minutes").into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_connection_cannot_inherit_admin_options() {
        assert!(fixture_url("postgres://admin:secret@localhost/db?user=admin", "fixture", "pw", "test").is_err());
        assert_eq!(fixture_url("postgres://admin:secret@localhost:55439/postgres", "fixture", "pw", "test").unwrap(), "postgres://fixture:pw@localhost:55439/test");
    }
}

use anyhow::{anyhow, bail, Context, Result};
use postgres::{Client, NoTls};
use std::collections::BTreeMap;
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use url::Url;

const DEFAULT_DATABASE_URL: &str = "postgres://rustok:rustok@localhost:5432/rustok_dev";
const DEFAULT_PG_ADMIN_URL: &str = "postgres://postgres:postgres@localhost:5432/postgres";
const DEFAULT_API_URL: &str = "http://localhost:5150";
const DEFAULT_ADMIN_EMAIL: &str = "admin@local";
const DEFAULT_ADMIN_PASSWORD: &str = "admin12345";
const DEFAULT_TENANT_SLUG: &str = "demo";
const DEFAULT_TENANT_NAME: &str = "Demo Workspace";

pub(crate) fn install_dev(args: &[String]) -> Result<()> {
    let options = InstallDevOptions::parse(args)?;
    if options.help {
        print_install_dev_usage();
        return Ok(());
    }

    ensure_repo_root()?;

    println!("RusToK local dev install");
    println!("  database: {}", options.database_url);
    println!("  API:      {}", options.api_url);
    println!("  tenant:   {}", options.tenant_slug);
    println!("  admin:    {}", options.admin_email);
    println!();

    check_required_tool("cargo")?;
    check_optional_tool(
        "npm",
        "Next admin can still run if dependencies are already installed.",
    );
    check_optional_tool("trunk", "Leptos admin requires `trunk serve --port 3001`.");

    if options.create_db {
        ensure_database(&options.pg_admin_url, &options.database_url)?;
    }

    if !options.skip_db_check {
        check_database_socket(&options.database_url)?;
    }

    if !options.no_write_env {
        ensure_modules_local(&options)?;
        ensure_root_env(&options)?;
        ensure_next_admin_env(&options)?;
    }

    if !options.no_bootstrap {
        run_server_bootstrap(&options)?;
    }

    print_next_steps(&options);

    Ok(())
}

#[derive(Debug)]
struct InstallDevOptions {
    database_url: String,
    pg_admin_url: String,
    api_url: String,
    admin_email: String,
    admin_password: String,
    tenant_slug: String,
    tenant_name: String,
    create_db: bool,
    skip_db_check: bool,
    no_bootstrap: bool,
    no_write_env: bool,
    dry_run: bool,
    help: bool,
}

impl InstallDevOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string()),
            pg_admin_url: std::env::var("RUSTOK_PG_ADMIN_URL")
                .unwrap_or_else(|_| DEFAULT_PG_ADMIN_URL.to_string()),
            api_url: std::env::var("NEXT_PUBLIC_API_URL")
                .unwrap_or_else(|_| DEFAULT_API_URL.to_string()),
            admin_email: std::env::var("SUPERADMIN_EMAIL")
                .or_else(|_| std::env::var("SEED_ADMIN_EMAIL"))
                .unwrap_or_else(|_| DEFAULT_ADMIN_EMAIL.to_string()),
            admin_password: std::env::var("SUPERADMIN_PASSWORD")
                .or_else(|_| std::env::var("SEED_ADMIN_PASSWORD"))
                .unwrap_or_else(|_| DEFAULT_ADMIN_PASSWORD.to_string()),
            tenant_slug: std::env::var("SUPERADMIN_TENANT_SLUG")
                .or_else(|_| std::env::var("SEED_TENANT_SLUG"))
                .unwrap_or_else(|_| DEFAULT_TENANT_SLUG.to_string()),
            tenant_name: std::env::var("SUPERADMIN_TENANT_NAME")
                .or_else(|_| std::env::var("SEED_TENANT_NAME"))
                .unwrap_or_else(|_| DEFAULT_TENANT_NAME.to_string()),
            create_db: false,
            skip_db_check: false,
            no_bootstrap: false,
            no_write_env: false,
            dry_run: false,
            help: false,
        };

        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--help" | "-h" => options.help = true,
                "--create-db" => options.create_db = true,
                "--skip-db-check" => options.skip_db_check = true,
                "--no-bootstrap" => options.no_bootstrap = true,
                "--no-write-env" => options.no_write_env = true,
                "--dry-run" => options.dry_run = true,
                "--database-url" => {
                    options.database_url = take_value(args, &mut index, "--database-url")?;
                }
                "--pg-admin-url" => {
                    options.pg_admin_url = take_value(args, &mut index, "--pg-admin-url")?;
                }
                "--api-url" => {
                    options.api_url = take_value(args, &mut index, "--api-url")?;
                }
                "--admin-email" => {
                    options.admin_email = take_value(args, &mut index, "--admin-email")?;
                }
                "--admin-password" => {
                    options.admin_password = take_value(args, &mut index, "--admin-password")?;
                }
                "--tenant-slug" => {
                    options.tenant_slug = take_value(args, &mut index, "--tenant-slug")?;
                }
                "--tenant-name" => {
                    options.tenant_name = take_value(args, &mut index, "--tenant-name")?;
                }
                unknown => bail!("Unknown install-dev argument: {unknown}"),
            }
            index += 1;
        }

        Ok(options)
    }
}

fn take_value(args: &[String], index: &mut usize, flag: &str) -> Result<String> {
    *index += 1;
    args.get(*index)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| anyhow!("{flag} requires a value"))
}

fn ensure_repo_root() -> Result<()> {
    for path in ["Cargo.toml", "apps/server", "apps/next-admin", "apps/admin"] {
        if !Path::new(path).exists() {
            bail!("install-dev must be run from the repository root; missing {path}");
        }
    }
    Ok(())
}

fn check_required_tool(name: &str) -> Result<()> {
    if command_available(name) {
        println!("[ok] tool found: {name}");
        Ok(())
    } else {
        bail!("Required tool `{name}` is not available in PATH")
    }
}

fn check_optional_tool(name: &str, hint: &str) {
    if command_available(name) {
        println!("[ok] tool found: {name}");
    } else {
        println!("[warn] tool not found: {name}. {hint}");
    }
}

fn command_available(name: &str) -> bool {
    shell_status(&format!("{name} --version")).unwrap_or(false)
}

fn check_database_socket(database_url: &str) -> Result<()> {
    let target = PgTarget::parse(database_url)?;
    let socket = format!("{}:{}", target.host, target.port);
    let mut last_error = None;
    for addr in socket
        .to_socket_addrs()
        .with_context(|| format!("Failed to resolve PostgreSQL address {socket}"))?
    {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
            Ok(_) => {
                println!("[ok] PostgreSQL is reachable at {socket}");
                return Ok(());
            }
            Err(error) => last_error = Some(error),
        }
    }

    bail!(
        "PostgreSQL is not reachable at {socket}: {}. Start PostgreSQL or rerun with --create-db and a valid --pg-admin-url.",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "no socket address resolved".to_string())
    )
}

fn ensure_database(pg_admin_url: &str, database_url: &str) -> Result<()> {
    let target = PgTarget::parse(database_url)?;
    println!("[run] ensuring PostgreSQL role/database via admin URL");

    let mut client = Client::connect(pg_admin_url, NoTls)
        .with_context(|| "Failed to connect to PostgreSQL admin URL")?;

    let role_exists = client
        .query_opt(
            "SELECT 1 FROM pg_roles WHERE rolname = $1",
            &[&target.username],
        )
        .with_context(|| "Failed to check PostgreSQL role")?
        .is_some();

    if role_exists {
        println!("[ok] PostgreSQL role exists: {}", target.username);
    } else {
        let sql = format!(
            "CREATE ROLE {} LOGIN PASSWORD {}",
            quote_ident(&target.username),
            quote_literal(target.password.as_deref().unwrap_or(""))
        );
        client
            .batch_execute(&sql)
            .with_context(|| format!("Failed to create PostgreSQL role {}", target.username))?;
        println!("[ok] PostgreSQL role created: {}", target.username);
    }

    let db_exists = client
        .query_opt(
            "SELECT 1 FROM pg_database WHERE datname = $1",
            &[&target.database],
        )
        .with_context(|| "Failed to check PostgreSQL database")?
        .is_some();

    if db_exists {
        println!("[ok] PostgreSQL database exists: {}", target.database);
    } else {
        let sql = format!(
            "CREATE DATABASE {} OWNER {}",
            quote_ident(&target.database),
            quote_ident(&target.username)
        );
        client
            .batch_execute(&sql)
            .with_context(|| format!("Failed to create PostgreSQL database {}", target.database))?;
        println!("[ok] PostgreSQL database created: {}", target.database);
    }

    Ok(())
}

#[derive(Debug)]
struct PgTarget {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    database: String,
}

impl PgTarget {
    fn parse(database_url: &str) -> Result<Self> {
        let url = Url::parse(database_url).with_context(|| "Invalid PostgreSQL URL")?;
        if !matches!(url.scheme(), "postgres" | "postgresql") {
            bail!("DATABASE_URL must use postgres:// or postgresql://");
        }

        let username = url.username().to_string();
        if username.is_empty() {
            bail!("DATABASE_URL must include a username");
        }

        let database = url.path().trim_start_matches('/').to_string();
        if database.is_empty() {
            bail!("DATABASE_URL must include a database name");
        }

        Ok(Self {
            host: url.host_str().unwrap_or("localhost").to_string(),
            port: url.port().unwrap_or(5432),
            username,
            password: url.password().map(ToString::to_string),
            database,
        })
    }
}

fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn ensure_modules_local(options: &InstallDevOptions) -> Result<()> {
    let local_path = Path::new("modules.local.toml");
    if local_path.exists() {
        println!("[ok] modules.local.toml exists");
        return Ok(());
    }

    if options.dry_run {
        println!("[dry-run] would create modules.local.toml");
        return Ok(());
    }

    let mut content = fs::read_to_string("modules.toml").context("Failed to read modules.toml")?;
    content = content.replace("embed_admin = true", "embed_admin = false");
    content = content.replace("embed_storefront = true", "embed_storefront = false");
    fs::write(local_path, content).context("Failed to write modules.local.toml")?;
    println!("[ok] created modules.local.toml for standalone local frontends");

    Ok(())
}

fn ensure_root_env(options: &InstallDevOptions) -> Result<()> {
    let mut keys = BTreeMap::new();
    keys.insert("DATABASE_URL", options.database_url.as_str());
    keys.insert("NEXT_PUBLIC_API_URL", options.api_url.as_str());
    keys.insert("NEXT_PUBLIC_GRAPHQL_ENDPOINT", "/api/graphql");
    keys.insert("NEXT_PUBLIC_AUTH_BASE_URL", "/api/auth");
    keys.insert("RUSTOK_API_URL", options.api_url.as_str());
    keys.insert("RUSTOK_GRAPHQL_ENDPOINT", "/api/graphql");
    keys.insert("RUSTOK_AUTH_BASE_URL", "/api/auth");
    keys.insert("RUSTOK_MODULES_MANIFEST", "modules.local.toml");
    keys.insert("SUPERADMIN_EMAIL", options.admin_email.as_str());
    keys.insert("SUPERADMIN_PASSWORD", options.admin_password.as_str());
    keys.insert("SUPERADMIN_TENANT_SLUG", options.tenant_slug.as_str());
    keys.insert("SUPERADMIN_TENANT_NAME", options.tenant_name.as_str());
    keys.insert("SEED_ADMIN_EMAIL", options.admin_email.as_str());
    keys.insert("SEED_ADMIN_PASSWORD", options.admin_password.as_str());
    keys.insert("SEED_TENANT_SLUG", options.tenant_slug.as_str());
    keys.insert("SEED_TENANT_NAME", options.tenant_name.as_str());

    upsert_env_file(Path::new(".env.dev"), &keys, options.dry_run)
}

fn ensure_next_admin_env(options: &InstallDevOptions) -> Result<()> {
    let mut keys = BTreeMap::new();
    keys.insert("NEXT_PUBLIC_API_URL", options.api_url.as_str());
    keys.insert("NEXT_PUBLIC_GRAPHQL_ENDPOINT", "/api/graphql");
    keys.insert("NEXT_PUBLIC_AUTH_BASE_URL", "/api/auth");
    keys.insert("NEXT_PUBLIC_SENTRY_DISABLED", "true");

    upsert_env_file(
        Path::new("apps/next-admin/.env.local"),
        &keys,
        options.dry_run,
    )
}

fn upsert_env_file(path: &Path, keys: &BTreeMap<&str, &str>, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("[dry-run] would update {}", path.display());
        return Ok(());
    }

    let mut content = if path.exists() {
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?
    } else {
        String::new()
    };

    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }

    for (key, value) in keys {
        content = upsert_env_key(content, key, value);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    println!("[ok] updated {}", path.display());

    Ok(())
}

fn upsert_env_key(content: String, key: &str, value: &str) -> String {
    let prefix = format!("{key}=");
    let mut found = false;
    let mut lines = Vec::new();

    for line in content.lines() {
        if line.starts_with(&prefix) {
            lines.push(format!("{prefix}{value}"));
            found = true;
        } else {
            lines.push(line.to_string());
        }
    }

    if !found {
        lines.push(format!("{prefix}{value}"));
    }

    let mut result = lines.join("\n");
    result.push('\n');
    result
}

fn run_server_bootstrap(options: &InstallDevOptions) -> Result<()> {
    let manifest_path = std::env::current_dir()
        .context("Failed to resolve repository root")?
        .join("modules.local.toml")
        .to_string_lossy()
        .to_string();

    let mut envs = BTreeMap::new();
    envs.insert("DATABASE_URL".to_string(), options.database_url.clone());
    envs.insert("RUSTOK_MODULES_MANIFEST".to_string(), manifest_path);
    envs.insert("SUPERADMIN_EMAIL".to_string(), options.admin_email.clone());
    envs.insert(
        "SUPERADMIN_PASSWORD".to_string(),
        options.admin_password.clone(),
    );
    envs.insert(
        "SUPERADMIN_TENANT_SLUG".to_string(),
        options.tenant_slug.clone(),
    );
    envs.insert(
        "SUPERADMIN_TENANT_NAME".to_string(),
        options.tenant_name.clone(),
    );
    envs.insert("SEED_ADMIN_EMAIL".to_string(), options.admin_email.clone());
    envs.insert(
        "SEED_ADMIN_PASSWORD".to_string(),
        options.admin_password.clone(),
    );
    envs.insert("SEED_TENANT_SLUG".to_string(), options.tenant_slug.clone());
    envs.insert("SEED_TENANT_NAME".to_string(), options.tenant_name.clone());

    let args = vec![
        "install".to_string(),
        "apply".to_string(),
        "--environment".to_string(),
        "local".to_string(),
        "--profile".to_string(),
        "dev-local".to_string(),
        "--database-engine".to_string(),
        "postgres".to_string(),
        "--database-url".to_string(),
        options.database_url.clone(),
        "--admin-email".to_string(),
        options.admin_email.clone(),
        "--admin-password".to_string(),
        options.admin_password.clone(),
        "--tenant-slug".to_string(),
        options.tenant_slug.clone(),
        "--tenant-name".to_string(),
        options.tenant_name.clone(),
        "--seed-profile".to_string(),
        "dev".to_string(),
        "--secrets-mode".to_string(),
        "dotenv-file".to_string(),
        "--lock-owner".to_string(),
        "xtask-install-dev".to_string(),
    ];
    run_server_binary(&args, &envs, options.dry_run)?;

    Ok(())
}

fn run_server_binary(
    args: &[String],
    envs: &BTreeMap<String, String>,
    dry_run: bool,
) -> Result<()> {
    let binary = server_binary_path();
    let rendered = format!("{} {}", binary.display(), render_command_args(args));
    if dry_run {
        println!("[dry-run] {rendered}");
        return Ok(());
    }

    if !binary.exists() {
        bail!(
            "Server binary is missing at {}. Build it first with `cargo build -p rustok-server --bin rustok-server`, then rerun `cargo xtask install-dev`.",
            binary.display()
        );
    }

    println!("[run] {rendered}");
    let mut command = Command::new(&binary);
    command.args(args).current_dir("apps/server");
    for (key, value) in envs {
        command.env(key, value);
    }
    let status = command
        .status()
        .with_context(|| format!("Failed to start `{rendered}`"))?;
    if !status.success() {
        bail!("Command failed with status {status}: {rendered}");
    }
    Ok(())
}

fn render_command_args(args: &[String]) -> String {
    let mut rendered = Vec::with_capacity(args.len());
    let mut redact_next = false;
    for arg in args {
        if redact_next {
            rendered.push("<redacted>".to_string());
            redact_next = false;
            continue;
        }
        rendered.push(arg.clone());
        if arg == "--admin-password" {
            redact_next = true;
        }
    }
    rendered.join(" ")
}

fn server_binary_path() -> PathBuf {
    let exe = if cfg!(windows) {
        "rustok-server.exe"
    } else {
        "rustok-server"
    };
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("target")
        .join("debug")
        .join(exe)
}

fn shell_status(command: &str) -> Result<bool> {
    let mut cmd = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);
        cmd
    };

    let status = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("Failed to run `{command}`"))?;

    Ok(status.success())
}

fn print_next_steps(options: &InstallDevOptions) {
    println!();
    println!("[ok] Local dev install bootstrap complete.");
    println!();
    println!("Next commands:");
    if cfg!(windows) {
        println!(
            "  $env:RUSTOK_MODULES_MANIFEST='modules.local.toml'; $env:SUPERADMIN_EMAIL='{}'; $env:SUPERADMIN_PASSWORD='{}'; cargo run -p rustok-server --bin rustok-server -- start",
            options.admin_email, options.admin_password
        );
        println!("  cd apps/next-admin; npm run dev");
        println!("  cd apps/admin; trunk serve --port 3001");
    } else {
        println!(
            "  RUSTOK_MODULES_MANIFEST=modules.local.toml SUPERADMIN_EMAIL='{}' SUPERADMIN_PASSWORD='{}' cargo run -p rustok-server --bin rustok-server -- start",
            options.admin_email, options.admin_password
        );
        println!("  (cd apps/next-admin && npm run dev)");
        println!("  (cd apps/admin && trunk serve --port 3001)");
    }
    println!();
    println!("URLs:");
    println!("  backend:      {}/api/health", options.api_url);
    println!("  Next admin:   http://localhost:3000");
    println!("  Leptos admin: http://localhost:3001");
}

fn print_install_dev_usage() {
    println!("Usage: cargo xtask install-dev [options]");
    println!();
    println!("Bootstraps a local non-Docker development install:");
    println!("  - checks local prerequisites");
    println!("  - optionally creates PostgreSQL role/database");
    println!("  - writes .env.dev and apps/next-admin/.env.local");
    println!("  - creates modules.local.toml with standalone frontend surfaces");
    println!("  - runs server migrations and dev seed");
    println!();
    println!("Options:");
    println!("  --create-db                 Create role/database using --pg-admin-url");
    println!("  --pg-admin-url <url>        Admin PostgreSQL URL (default: postgres://postgres:postgres@localhost:5432/postgres)");
    println!("  --database-url <url>        App database URL (default: postgres://rustok:rustok@localhost:5432/rustok_dev)");
    println!("  --api-url <url>             Backend base URL for admin apps (default: http://localhost:5150)");
    println!("  --admin-email <email>       Dev SuperAdmin email (default: admin@local)");
    println!("  --admin-password <value>    Dev SuperAdmin password (default: admin12345)");
    println!("  --tenant-slug <slug>        Dev tenant slug (default: demo)");
    println!("  --tenant-name <name>        Dev tenant name (default: Demo Workspace)");
    println!("  --skip-db-check             Do not require localhost PostgreSQL TCP check");
    println!("  --no-bootstrap              Skip migrations and seed");
    println!("  --no-write-env              Skip env/modules.local.toml writes");
    println!("  --dry-run                   Print actions without writing or running migrations");
}

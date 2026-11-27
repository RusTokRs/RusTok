use super::registration::TempWorkspace;
use crate::manifest_io::workspace_root;
use crate::migrations::{copied_source_digest, copy_working_source};
use anyhow::{ensure, Context, Result};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod fixtures;

pub(crate) const USAGE: &str = "cargo xtask module verify-scaffold [--offline]";

const MODES: [(&str, &str, &[&str]); 4] = [
    ("headless", "Headless", &["--no-ui"]),
    ("admin", "Admin", &["--ui", "admin"]),
    ("storefront", "Storefront", &["--ui", "storefront"]),
    ("dual", "Dual", &["--ui", "dual"]),
];

pub(crate) fn command(args: &[String]) -> Result<()> {
    let offline = match args {
        [] => false,
        [option] if option == "--offline" => true,
        [option] if option == "--help" => {
            println!("{USAGE}");
            return Ok(());
        }
        _ => anyhow::bail!("Usage: {USAGE}"),
    };
    let root = workspace_root();
    let target = root.join("target/scaffold-matrix");
    fs::create_dir_all(&target)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(target.join("matrix.lock"))?;
    lock.try_lock()
        .context("Cannot lock the scaffold matrix build directory")?;
    let scratch = TempWorkspace::new()?;
    let source = scratch.root.join("ws");
    fs::create_dir(&source)?;
    let digest = copy_working_source(&root, &source)?;
    println!("Scaffold matrix source snapshot SHA-256 {digest}");

    let matrix = Matrix {
        source,
        target,
        cargo: std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
        offline,
    };
    matrix.run(&scratch.root)?;
    println!(
        "Scaffold matrix passed for all four modes, localized storage, record admin and the module-local CLI."
    );
    Ok(())
}

struct Matrix {
    source: PathBuf,
    target: PathBuf,
    cargo: OsString,
    offline: bool,
}

impl Matrix {
    fn run(&self, scratch: &Path) -> Result<()> {
        println!("Building the scaffold CLI from the source snapshot");
        self.cargo("build", &["-p", "xtask", "--locked"])?;

        let rejected = ["scaffold", "scaffold_matrix_with_db", "--with-db"];
        self.unchanged(
            &rejected,
            "--with-db requires an actual owner schema",
            false,
        )?;
        self.unchanged(
            &[
                "scaffold",
                "scaffold_matrix_unknown_permission",
                "--with-db",
                "--entity",
                "record",
                "--admin-resource",
                "UnknownScaffoldResource",
            ],
            "Unknown platform Resource variant",
            false,
        )?;

        for (mode, title, flags) in MODES {
            let slug = format!("scaffold_matrix_{mode}");
            let title = format!("Scaffold Matrix {title}");
            let mut args = vec![
                "scaffold",
                &slug,
                "--title",
                &title,
                "--description",
                "Generated fixture for native scaffold verification.",
                "--category",
                "collaboration",
                "--no-db",
            ];
            args.extend_from_slice(flags);
            if mode == "headless" {
                args.push("--dry-run");
                self.unchanged(&args, "[DRY-RUN] Preview includes source files", true)?;
                args.pop();
            }
            self.xtask(&args)?;
        }
        self.xtask(&[
            "scaffold",
            "scaffold_matrix_localized",
            "--with-db",
            "--entity",
            // A generated owner can use the same entity name as a catalog
            // source. Search must use both owner and entity to admit facets.
            "product",
            "--admin-resource",
            "Posts",
            "--ui",
            "admin",
        ])?;

        self.attach_cli(scratch)?;
        self.unchanged(
            &["scaffold", "scaffold_matrix_headless", "--no-db", "--no-ui"],
            "already exists",
            false,
        )?;

        let owner_packages = [
            "rustok-scaffold-matrix-headless",
            "rustok-scaffold-matrix-admin",
            "rustok-scaffold-matrix-storefront",
            "rustok-scaffold-matrix-dual",
            "rustok-scaffold-matrix-localized",
            "rustok-scaffold-matrix-localized-cli",
        ];
        let ui_packages = [
            "rustok-scaffold-matrix-admin-admin",
            "rustok-scaffold-matrix-storefront-storefront",
            "rustok-scaffold-matrix-dual-admin",
            "rustok-scaffold-matrix-dual-storefront",
            "rustok-scaffold-matrix-localized-admin",
        ];
        let mut packages = Vec::new();
        for package in owner_packages.into_iter().chain(ui_packages) {
            packages.extend(["-p", package]);
        }
        packages.extend(["--features", "rustok-scaffold-matrix-localized/graphql"]);
        println!("Running default-profile tests for generated backend, UI and CLI packages");
        // Cargo adds the newly generated local packages to the copied lockfile.
        // Every feature-specific invocation then uses that exact resolution.
        self.cargo("test", &packages)?;
        if std::env::var_os("RUSTOK_SCAFFOLD_TEST_POSTGRES_URL").is_some() {
            println!("Running the generated PostgreSQL publication and Search contract");
            packages.extend([
                "--locked",
                "--test",
                "record",
                "postgres_localized_publication_reaches_search",
                "--",
                "--ignored",
                "--exact",
            ]);
            self.cargo("test", &packages)?;
        }

        for feature in ["csr", "hydrate", "ssr"] {
            let mut args = vec!["--locked", "--lib", "--features", feature];
            for package in ui_packages {
                args.extend(["-p", package]);
            }
            println!("Running generated UI tests with the {feature} profile");
            self.cargo("test", &args)?;
        }
        for feature in ["csr", "hydrate"] {
            println!("Checking the generated record workspace with the {feature} profile on WASM");
            self.cargo(
                "check",
                &[
                    "--locked",
                    "-p",
                    "rustok-scaffold-matrix-localized-admin",
                    "--features",
                    feature,
                    "--target",
                    "wasm32-unknown-unknown",
                ],
            )?;
        }
        self.check_distribution()?;
        Ok(())
    }

    fn attach_cli(&self, scratch: &Path) -> Result<()> {
        let adapter = scratch.join("operation.rs");
        let tests = scratch.join("contract.rs");
        fs::write(&adapter, fixtures::CLI_ADAPTER)?;
        fs::write(&tests, fixtures::CLI_CONTRACT)?;
        self.xtask(&[
            "module",
            "scaffold-cli",
            "scaffold_matrix_localized",
            "--name",
            "public-discovery",
            "--summary",
            "Read the tenant's current public record discovery setting",
            "--adapter",
            adapter.to_str().context("CLI fixture path must be UTF-8")?,
            "--tests",
            tests.to_str().context("CLI fixture path must be UTF-8")?,
            "--dependency", "rustok-api",
            "--dependency", "rustok-modules",
            "--dependency", "serde_json",
            "--dependency", "uuid",
            "--dev-dependency", "rustok-migrations",
            "--dev-dependency", "rustok-test-utils",
            "--dev-dependency", "sea-orm",
            "--dev-dependency", "sea-orm-migration",
        ])?;
        let status = Command::new("node")
            .current_dir(&self.source)
            .args(["scripts/generate/generate-cli-registry.mjs", "--check"])
            .status()
            .context("Cannot execute the generated CLI registry check")?;
        ensure!(
            status.success(),
            "Generated CLI registry check exited with {status}"
        );
        Ok(())
    }

    fn check_distribution(&self) -> Result<()> {
        let tests = self.source.join("crates/modules/rustok-distribution/tests");
        fs::create_dir_all(&tests)?;
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(tests.join("scaffold_matrix_registry.rs"))?
            .write_all(fixtures::DISTRIBUTION_CONTRACT.as_bytes())?;
        let mut features = MODES
            .map(|(mode, _, _)| format!("mod-scaffold_matrix_{mode}"))
            .join(",");
        features.push_str(",mod-scaffold_matrix_localized,rustok-scaffold-matrix-localized/graphql");
        println!("Running selected distribution registry with defaults disabled");
        let mut args = vec![
            "--locked",
            "-p",
            "rustok-distribution",
            "--no-default-features",
            "--features",
            &features,
            "--test",
            "scaffold_matrix_registry",
        ];
        self.cargo("test", &args)?;
        if std::env::var_os("RUSTOK_SCAFFOLD_TEST_POSTGRES_URL").is_some() {
            args.extend([
                "postgres_localized_owner_replays_into_index",
                "--",
                "--ignored",
                "--exact",
            ]);
            self.cargo("test", &args)?;
        }
        Ok(())
    }

    fn cargo(&self, command: &str, args: &[&str]) -> Result<()> {
        let mut process = Command::new(&self.cargo);
        process
            .current_dir(&self.source)
            .env("SQLX_OFFLINE", "true")
            .arg(command)
            .arg("--target-dir")
            .arg(&self.target);
        if self.offline {
            process.arg("--offline");
        }
        process.args(args);
        let status = process
            .status()
            .context("Cannot invoke Cargo for the scaffold matrix")?;
        ensure!(
            status.success(),
            "Cargo {command} {} exited with {status}",
            args.join(" ")
        );
        Ok(())
    }

    fn xtask_process(&self, args: &[&str]) -> Command {
        let executable = format!("xtask{}", std::env::consts::EXE_SUFFIX);
        let mut process = Command::new(self.target.join("debug").join(executable));
        process.current_dir(&self.source).args(args);
        process
    }

    fn xtask(&self, args: &[&str]) -> Result<()> {
        let status = self.xtask_process(args).status()?;
        ensure!(
            status.success(),
            "xtask {} exited with {status}",
            args.join(" ")
        );
        Ok(())
    }

    fn unchanged(&self, args: &[&str], marker: &str, success: bool) -> Result<()> {
        let before = copied_source_digest(&self.source)?;
        let output = self.xtask_process(args).output()?;
        let Output {
            status,
            stdout,
            stderr,
        } = output;
        let stdout = String::from_utf8_lossy(&stdout);
        let stderr = String::from_utf8_lossy(&stderr);
        ensure!(
            status.success() == success && (stdout.contains(marker) || stderr.contains(marker)),
            "Unexpected result from xtask {} ({status}): {stderr}",
            args.join(" ")
        );
        ensure!(
            copied_source_digest(&self.source)? == before,
            "xtask {} changed the disposable workspace",
            args.join(" ")
        );
        println!(
            "Verified unchanged workspace after xtask {}",
            args.join(" ")
        );
        Ok(())
    }
}

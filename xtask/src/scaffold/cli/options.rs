use anyhow::{bail, ensure, Context, Result};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

pub(super) struct Options {
    pub slug: String,
    pub name: String,
    pub operation: String,
    pub summary: String,
    pub adapter: PathBuf,
    pub tests: PathBuf,
    pub dependencies: BTreeSet<String>,
    pub supports_dry_run: bool,
    pub dry_run: bool,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        let (slug, flags) = args
            .split_first()
            .context("Expected a native module slug")?;
        ensure!(
            super::super::is_valid_slug(slug),
            "Invalid module slug '{slug}'"
        );
        ensure!(
            !matches!(slug.as_str(), "help" | "list"),
            "The platform CLI reserves the namespace '{slug}'"
        );
        let mut values = HashMap::new();
        let mut dependencies = BTreeSet::new();
        let mut switches = BTreeSet::new();
        let mut flags = flags.iter();
        while let Some(flag) = flags.next() {
            match flag.as_str() {
                "--supports-dry-run" | "--dry-run" => {
                    ensure!(switches.insert(flag.as_str()), "Duplicate {flag}");
                }
                "--name" | "--summary" | "--adapter" | "--tests" | "--dependency" => {
                    let value = flags
                        .next()
                        .filter(|value| !value.trim().is_empty() && !value.starts_with("--"))
                        .with_context(|| format!("Missing value for {flag}"))?;
                    if flag == "--dependency" {
                        ensure!(
                            value
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric()
                                    || matches!(byte, b'-' | b'_')),
                            "Invalid Cargo dependency key '{value}'"
                        );
                        ensure!(
                            dependencies.insert(value.clone()),
                            "Duplicate dependency '{value}'"
                        );
                    } else {
                        ensure!(
                            values.insert(flag.as_str(), value.clone()).is_none(),
                            "Duplicate {flag}"
                        );
                    }
                }
                _ => bail!("Unknown CLI scaffold option '{flag}'"),
            }
        }
        let mut take = |key| values.remove(key).with_context(|| format!("Missing {key}"));
        let name = take("--name")?;
        ensure!(
            name.len() <= 64
                && name.as_bytes()[0].is_ascii_lowercase()
                && name.split('-').all(|part| !part.is_empty()
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())),
            "CLI command name must be lower kebab-case and at most 64 bytes"
        );
        let operation = name.replace('-', "_");
        syn::parse_str::<syn::Ident>(&operation)
            .context("CLI command needs a valid Rust module name")?;
        let summary = take("--summary")?;
        ensure!(
            summary.len() <= 256 && !summary.chars().any(char::is_control),
            "CLI summary must be a single line of up to 256 bytes"
        );
        Ok(Self {
            slug: slug.clone(),
            name,
            operation,
            summary,
            adapter: PathBuf::from(take("--adapter")?),
            tests: PathBuf::from(take("--tests")?),
            dependencies,
            supports_dry_run: switches.contains("--supports-dry-run"),
            dry_run: switches.contains("--dry-run"),
        })
    }
}

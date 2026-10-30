use crate::module_cli_args::cli_option_value;
use anyhow::{bail, Result};
use std::collections::HashSet;

/// Architectural roles are distinct even when their output is a Rust crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaffoldProfile {
    NativeModule,
    SupportCrate,
    StandaloneWasi,
}

impl ScaffoldProfile {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "native" => Ok(Self::NativeModule),
            "support" => Ok(Self::SupportCrate),
            "wasi" => Ok(Self::StandaloneWasi),
            other => bail!("Unknown scaffold profile '{other}'; expected native, support, or wasi"),
        }
    }

    fn require_native(self) -> Result<Self> {
        match self {
            Self::NativeModule => Ok(self),
            Self::SupportCrate => bail!(
                "The native module generator cannot scaffold a support crate: support crates have no module lifecycle or rustok-module.toml"
            ),
            Self::StandaloneWasi => bail!(
                "The native module generator cannot scaffold standalone WASI; use rustok-module-template instead"
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaffoldUi {
    None,
    Admin,
    Storefront,
    Dual,
}

impl ScaffoldUi {
    pub fn has_admin(&self) -> bool {
        matches!(self, Self::Admin | Self::Dual)
    }

    pub fn has_storefront(&self) -> bool {
        matches!(self, Self::Storefront | Self::Dual)
    }

    pub fn classification_str(&self) -> &'static str {
        match self {
            Self::None => "no_ui",
            Self::Admin => "admin_only",
            Self::Storefront => "storefront_only",
            Self::Dual => "dual_surface",
        }
    }
}

pub struct ScaffoldOptions {
    pub profile: ScaffoldProfile,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub with_db: bool,
    pub ui: ScaffoldUi,
    pub dry_run: bool,
    pub skip_server_wire: bool,
    pub skip_validate: bool,
}

/// Validate the complete CLI input before it can shape any source or registry edits.
/// Unknown arguments, repeated switches and missing values must never turn into
/// successful default scaffolds.
fn validate_scaffold_arguments(args: &[String]) -> Result<()> {
    let mut used = HashSet::new();
    let mut index = 1;
    while index < args.len() {
        let flag = args[index].as_str();
        let takes_value = matches!(
            flag,
            "--profile" | "--title" | "--name" | "--description" | "--category" | "--ui"
        );
        let is_switch = matches!(
            flag,
            "--dry-run"
                | "--skip-server-wire"
                | "--skip-validate"
                | "--no-db"
                | "--with-db"
                | "--no-ui"
                | "--with-ui"
        );

        if !takes_value && !is_switch {
            bail!("Unknown scaffold argument '{flag}'");
        }
        if !used.insert(flag) {
            bail!("Scaffold option '{flag}' may only be provided once");
        }
        if takes_value {
            let value = args
                .get(index + 1)
                .filter(|value| !value.starts_with("--") && !value.trim().is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!("Scaffold option '{flag}' requires a non-empty value")
                })?;
            // Titles/descriptions appear unquoted in Markdown documents; the
            // description is also inserted into a registry table cell.
            if matches!(flag, "--title" | "--name" | "--description" | "--category")
                && value
                    .chars()
                    .any(|ch| ch.is_control() || ch == '|' || ch == '\u{2028}' || ch == '\u{2029}')
            {
                bail!("Scaffold option '{flag}' must be a single line without control characters or '|' table delimiters");
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    if used.contains("--title") && used.contains("--name") {
        bail!("--title and --name are aliases and cannot both be specified");
    }
    Ok(())
}

pub fn parse_options(args: &[String]) -> Result<Option<ScaffoldOptions>> {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_scaffold_usage();
        if args.is_empty() {
            bail!("Missing module slug for scaffolding");
        }
        return Ok(None);
    }

    validate_scaffold_arguments(args)?;

    let raw_slug = args[0].trim().to_ascii_lowercase();
    if !is_valid_slug(&raw_slug) {
        bail!(
            "Invalid module slug '{raw_slug}'. Must contain only lowercase letters, digits, hyphens, or underscores (e.g. 'calls', 'page_builder', 'digital-wallet')."
        );
    }
    // Canonical slug format is snake_case per RusToK Naming Contract (docs/standards/coding.md)
    let slug = raw_slug.replace('-', "_");

    if args
        .iter()
        .filter(|arg| arg.as_str() == "--profile")
        .count()
        > 1
    {
        bail!("--profile may only be provided once");
    }
    let profile = match args.iter().position(|arg| arg == "--profile") {
        Some(index) => {
            let value = args
                .get(index + 1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| anyhow::anyhow!("--profile requires native, support, or wasi"))?;
            ScaffoldProfile::parse(value)?.require_native()?
        }
        None => ScaffoldProfile::NativeModule,
    };

    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let skip_server_wire = args.iter().any(|arg| arg == "--skip-server-wire");
    let skip_validate = args.iter().any(|arg| arg == "--skip-validate");

    let title = cli_option_value(args, "--title")
        .or_else(|| cli_option_value(args, "--name"))
        .unwrap_or_else(|| default_title_from_slug(&slug));

    let description = cli_option_value(args, "--description")
        .unwrap_or_else(|| format!("{title} capability module for RusToK"));

    if description.trim().len() < 20 {
        bail!(
            "Module description must be at least 20 characters for publish readiness (got {} characters)",
            description.trim().len()
        );
    }

    let category =
        cli_option_value(args, "--category").unwrap_or_else(|| "collaboration".to_string());

    let no_db = args.iter().any(|arg| arg == "--no-db");
    let with_db = args.iter().any(|arg| arg == "--with-db");
    if no_db && with_db {
        bail!("--with-db and --no-db are conflicting persistence requests");
    }

    let with_ui_alias = args.iter().any(|arg| arg == "--with-ui");
    if with_ui_alias
        && (args.iter().any(|arg| arg == "--no-ui") || args.iter().any(|arg| arg == "--ui"))
    {
        bail!("--with-ui cannot be combined with --ui or --no-ui");
    }
    if args.iter().any(|arg| arg == "--no-ui") && args.iter().any(|arg| arg == "--ui") {
        bail!("--no-ui and --ui are conflicting surface requests");
    }

    if args.iter().filter(|arg| arg.as_str() == "--ui").count() > 1 {
        bail!("--ui may only be provided once");
    }
    if let Some(index) = args.iter().position(|arg| arg == "--ui") {
        match args.get(index + 1) {
            Some(value) if !value.starts_with("--") => {}
            _ => bail!("--ui requires dual, admin, storefront, or none"),
        }
    }

    let ui = if with_ui_alias {
        ScaffoldUi::Dual
    } else if args.iter().any(|arg| arg == "--no-ui") {
        ScaffoldUi::None
    } else if let Some(ui_val) = cli_option_value(args, "--ui") {
        match ui_val.trim().to_ascii_lowercase().as_str() {
            "none" | "no-ui" => ScaffoldUi::None,
            "admin" | "admin-only" => ScaffoldUi::Admin,
            "storefront" | "storefront-only" => ScaffoldUi::Storefront,
            "dual" | "dual-surface" | "both" => ScaffoldUi::Dual,
            other => bail!(
                "Invalid --ui option '{other}'. Expected: 'dual', 'admin', 'storefront', or 'none'"
            ),
        }
    } else {
        // A new module has no service/transport contract to advertise yet.
        ScaffoldUi::None
    };

    Ok(Some(ScaffoldOptions {
        profile,
        slug,
        title,
        description,
        category,
        with_db,
        ui,
        dry_run,
        skip_server_wire,
        skip_validate,
    }))
}

impl ScaffoldOptions {
    /// Kebab-case name used for Cargo package and directory names (e.g. `rustok-page-builder`).
    pub fn kebab_name(&self) -> String {
        self.slug.replace('_', "-")
    }

    /// Full canonical crate name (e.g. `rustok-page-builder`).
    pub fn crate_name(&self) -> String {
        format!("rustok-{}", self.kebab_name())
    }
}

pub fn print_scaffold_usage() {
    println!("Usage:");
    println!("  cargo xtask scaffold <slug> [OPTIONS]");
    println!("  cargo xtask module new <slug> [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --profile native         Native runtime module (default); support/WASI are separate workflows");
    println!("  --title <title>          Human-readable title (defaults to Title Case of slug)");
    println!("  --description <desc>     Module description for documentation and manifests");
    println!("  --category <cat>         Marketplace category (default: 'collaboration')");
    println!("  --no-db                  No storage schema (default)");
    println!("  --with-db                Requires a defined owner schema; unsupported until DB template cutover");
    println!("  --ui <mode>              none (default), admin, storefront, dual; creates unmounted Leptos FFA crates");
    println!("  --with-ui                Alias for --ui dual; draft crates are created unmounted");
    println!("  --no-ui                  Explicit headless module (same as default)");
    println!("  --skip-server-wire       Do not register feature flags in apps/server/Cargo.toml");
    println!("  --skip-validate          Skip post-scaffold contract validation");
    println!(
        "  --dry-run                Preview all source files and registration manifest updates"
    );
    println!();
    println!("Examples:");
    println!("  cargo xtask scaffold calls --title \"WebRTC Calls & Conferencing\" --category collaboration");
    println!(
        "  cargo xtask scaffold party --title \"Counterparty & Legal Entities\" --category crm"
    );
    println!(
        "  cargo xtask scaffold page_builder --title \"Visual Page Builder\" --category content"
    );
    println!("  cargo xtask scaffold digital-wallet --title \"Digital Wallet & Balance\" --category fintech");
}

pub fn is_valid_slug(slug: &str) -> bool {
    // Enforce snake_case after CLI kebab-case normalization. Repeated
    // separators and leading digits do not form canonical module identities.
    let normalized = slug.replace('-', "_");
    normalized
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && normalized
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && normalized.split('_').all(|part| !part.is_empty())
}

pub fn default_title_from_slug(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<ScaffoldOptions> {
        let args = arguments
            .iter()
            .map(|arg| (*arg).to_owned())
            .collect::<Vec<_>>();
        parse_options(&args)?.ok_or_else(|| anyhow::anyhow!("expected a scaffold request"))
    }

    #[test]
    fn native_slug_input_normalizes_to_one_valid_registry_identity() {
        for valid in ["example", "page_builder", "page-builder", "v2_calls"] {
            assert!(parse(&[valid]).is_ok(), "expected valid slug: {valid}");
        }
        for invalid in [
            "2example",
            "page__builder",
            "page--builder",
            "page-_builder",
            "_example",
            "example-",
        ] {
            assert!(
                parse(&[invalid]).is_err(),
                "must reject invalid slug: {invalid}"
            );
        }
    }

    #[test]
    fn default_creates_only_native_runtime_identity() {
        let request = parse(&["example"]).expect("valid native request");
        assert_eq!(request.profile, ScaffoldProfile::NativeModule);
        assert!(!request.with_db);
        assert_eq!(request.ui, ScaffoldUi::None);
    }

    #[test]
    fn native_no_db_no_ui_request_keeps_both_capabilities_absent() {
        let request = parse(&[
            "sample-module",
            "--profile",
            "native",
            "--no-db",
            "--no-ui",
        ])
        .expect("valid native request");
        assert_eq!(request.slug, "sample_module");
        assert_eq!(request.crate_name(), "rustok-sample-module");
        assert_eq!(request.profile, ScaffoldProfile::NativeModule);
        assert!(!request.with_db);
        assert_eq!(request.ui, ScaffoldUi::None);
    }

    #[test]
    fn explicit_storage_request_is_never_implicit_or_conflicting() {
        assert!(parse(&["example", "--with-db"]).unwrap().with_db);
        assert!(parse(&["example", "--with-db", "--no-db"]).is_err());
    }

    #[test]
    fn support_and_wasi_are_not_native_modules() {
        for (profile, explanation) in [
            ("support", "support crate"),
            ("wasi", "standalone WASI"),
        ] {
            let error = match parse(&["example", "--profile", profile]) {
                Ok(_) => panic!("profile {profile} must be rejected"),
                Err(error) => error,
            };
            assert!(
                error.to_string().contains(explanation),
                "unexpected profile error: {error}"
            );
        }
    }

    #[test]
    fn invalid_profile_inputs_fail_closed() {
        for args in [
            vec!["example", "--profile"],
            vec!["example", "--profile", "--no-ui"],
            vec!["example", "--profile", "unknown"],
            vec!["example", "--profile", "native", "--profile", "native"],
        ] {
            assert!(parse(&args).is_err(), "must reject: {args:?}");
        }
    }

    #[test]
    fn conflicting_ui_requests_fail_closed() {
        assert!(parse(&["example", "--ui", "admin", "--no-ui"]).is_err());
        assert!(parse(&["example", "--no-ui", "--ui", "dual"]).is_err());
    }

    #[test]
    fn with_ui_alias_never_silently_downgrades_to_headless() {
        assert_eq!(
            parse(&["example", "--with-ui"]).unwrap().ui,
            ScaffoldUi::Dual
        );
        assert!(parse(&["example", "--with-ui", "--no-ui"]).is_err());
        assert!(parse(&["example", "--with-ui", "--ui", "dual"]).is_err());
    }

    #[test]
    fn missing_or_duplicate_ui_mode_is_not_silently_defaulted() {
        for args in [
            vec!["example", "--ui"],
            vec!["example", "--ui", "--no-db"],
            vec!["example", "--ui", "dual", "--ui", "admin"],
        ] {
            assert!(parse(&args).is_err(), "must reject: {args:?}");
        }
    }

    #[test]
    fn malformed_arguments_never_turn_into_default_scaffolds() {
        for args in [
            vec!["example", "--unknown"],
            vec!["example", "stray"],
            vec!["example", "--ui", "dual", "stray"],
            vec!["example", "--dry-run", "--dry-run"],
            vec!["example", "--skip-validate", "--skip-validate"],
            vec!["example", "--title"],
            vec!["example", "--title", "--no-db"],
            vec!["example", "--description", ""],
            vec!["example", "--category", "  "],
            vec!["example", "--title", "One", "--title", "Two"],
            vec![
                "example",
                "--description",
                "One long description here",
                "--description",
                "Another long description here",
            ],
            vec!["example", "--name", "One", "--title", "Two"],
        ] {
            assert!(
                parse(&args).is_err(),
                "must reject malformed arguments: {args:?}"
            );
        }
    }

    #[test]
    fn metadata_cannot_break_generated_docs_or_registry_table_rows() {
        for args in [
            vec![
                "example",
                "--description",
                "An otherwise valid | description for a module",
            ],
            vec![
                "example",
                "--description",
                "First description line\nSecond description line",
            ],
            vec!["example", "--title", "First\rSecond"],
            vec!["example", "--title", "First\tSecond"],
            vec!["example", "--category", "content|other"],
        ] {
            assert!(
                parse(&args).is_err(),
                "must reject unsafe metadata: {args:?}"
            );
        }
        let valid = parse(&[
            "example",
            "--title",
            "WebRTC Calls & Conferencing",
            "--description",
            "A native capability with punctuation: quotes, ampersands & slashes.",
            "--category",
            "collaboration",
            "--dry-run",
        ])
        .unwrap();
        assert_eq!(valid.title, "WebRTC Calls & Conferencing");
        assert!(valid.dry_run);
    }

    #[test]
    fn all_ui_modes_are_requests_not_readiness_grades() {
        for (mode, expected) in [
            ("none", ScaffoldUi::None),
            ("admin", ScaffoldUi::Admin),
            ("storefront", ScaffoldUi::Storefront),
            ("dual", ScaffoldUi::Dual),
        ] {
            let request = parse(&["example", "--profile", "native", "--ui", mode])
                .expect("valid requested surface");
            assert_eq!(request.ui, expected);
        }
    }
}

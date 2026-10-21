use crate::module_cli_args::cli_option_value;
use anyhow::{bail, Result};

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

pub fn parse_options(args: &[String]) -> Result<Option<ScaffoldOptions>> {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_scaffold_usage();
        if args.is_empty() {
            bail!("Missing module slug for scaffolding");
        }
        return Ok(None);
    }

    let slug = args[0].trim().to_ascii_lowercase();
    if !is_valid_slug(&slug) {
        bail!(
            "Invalid module slug '{slug}'. Must contain only lowercase letters, digits, and hyphens (e.g. 'calls', 'party', 'digital-wallet')."
        );
    }

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

    let category = cli_option_value(args, "--category")
        .unwrap_or_else(|| "collaboration".to_string());

    let no_db = args.iter().any(|arg| arg == "--no-db");
    let with_db = !no_db;

    let ui = if args.iter().any(|arg| arg == "--no-ui") {
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
    } else if args.iter().any(|arg| arg == "--with-ui") {
        ScaffoldUi::Dual
    } else {
        ScaffoldUi::Dual
    };

    Ok(Some(ScaffoldOptions {
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

pub fn print_scaffold_usage() {
    println!("Usage:");
    println!("  cargo xtask scaffold <slug> [OPTIONS]");
    println!("  cargo xtask module new <slug> [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --title <title>          Human-readable title (defaults to Title Case of slug)");
    println!("  --description <desc>     Module description for documentation and manifests");
    println!("  --category <cat>         Marketplace category (default: 'collaboration')");
    println!("  --no-db                  Omit SeaORM persistence entities and migrations");
    println!("  --ui <mode>              UI classification: dual, admin, storefront, or none (default: dual)");
    println!("  --with-ui                Alias for --ui dual");
    println!("  --no-ui                  Alias for --ui none (headless module)");
    println!("  --skip-server-wire       Do not register feature flags in apps/server/Cargo.toml");
    println!("  --skip-validate          Skip post-scaffold contract validation");
    println!("  --dry-run                Preview generated files and updates without writing to disk");
    println!();
    println!("Examples:");
    println!("  cargo xtask scaffold calls --title \"WebRTC Calls & Conferencing\" --category collaboration");
    println!("  cargo xtask scaffold party --title \"Counterparty & Legal Entities\" --category crm");
    println!("  cargo xtask scaffold billing --title \"B2B Invoices & Settlements\" --category erp");
}

pub fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

pub fn default_title_from_slug(slug: &str) -> String {
    slug.split('-')
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

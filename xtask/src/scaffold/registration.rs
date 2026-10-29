use super::options::{ScaffoldOptions, ScaffoldUi};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(crate) struct RegistrationPlan {
    files: BTreeMap<PathBuf, String>,
}

impl RegistrationPlan {
    fn read(&self, path: &Path) -> Result<String> {
        if let Some(content) = self.files.get(path) {
            return Ok(content.clone());
        }
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))
    }

    fn read_optional(&self, path: &Path) -> Result<Option<String>> {
        if let Some(content) = self.files.get(path) {
            return Ok(Some(content.clone()));
        }
        match fs::read_to_string(path) {
            Ok(content) => Ok(Some(content)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("Failed to read {}", path.display())),
        }
    }

    fn stage(&mut self, path: &Path, content: String) {
        self.files.insert(path.to_path_buf(), content);
    }

    pub(crate) fn into_files(self) -> Vec<(PathBuf, String)> {
        self.files.into_iter().collect()
    }
}

/// Build repository-wide registration changes in memory. The caller combines
/// these edits with generated source and commits the complete plan together.
pub(crate) fn plan_module(
    root: &Path,
    opts: &ScaffoldOptions,
    pascal: &str,
    module_manifest: &str,
) -> Result<Vec<(PathBuf, String)>> {
    let mut plan = RegistrationPlan::default();
    let slug = &opts.slug;
    let crate_name = opts.crate_name();

    register_in_workspace_cargo_toml(&mut plan, root, &crate_name, opts.ui, opts.with_cli)?;
    register_in_modules_toml(&mut plan, root, slug, &crate_name)?;
    register_in_registry_md(&mut plan, root, slug, &crate_name, &opts.description, opts.ui)?;
    register_in_docs_index_md(
        &mut plan,
        root,
        &crate_name,
        opts.ui.has_admin(),
        opts.ui.has_storefront(),
    )?;

    if opts.ui.has_admin() || opts.ui.has_storefront() {
        register_in_ui_packages_index_md(
            &mut plan,
            root,
            &crate_name,
            opts.ui.has_admin(),
            opts.ui.has_storefront(),
        )?;
    }

    if !opts.skip_server_wire {
        register_in_server_cargo_toml(&mut plan, root, slug, &crate_name)?;
        register_in_distribution_cargo_toml(&mut plan, root, slug, &crate_name)?;
        register_in_distribution_lib_rs(
            &mut plan,
            root,
            slug,
            pascal,
            &opts.kebab_name().replace('-', "_"),
        )?;
    }

    if opts.with_cli {
        let cli_crate = format!("{crate_name}-cli");
        register_in_cli_registry_cargo_toml(&mut plan, root, &cli_crate)?;
        register_in_cli_registry_generated(&mut plan, root, slug, module_manifest)?;
    }

    Ok(plan.into_files())
}

fn has_key_in_section(lines: &[String], section: &str, key: &str) -> bool {
    let mut in_section = false;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == format!("[{section}]");
            continue;
        }
        if in_section
            && trimmed
                .split_once('=')
                .map(|(candidate, _)| candidate.trim() == key)
                .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn insert_workspace_member(lines: &mut Vec<String>, member: &str) -> Result<bool> {
    let entry = format!("\"{member}\",");
    if lines.iter().any(|line| line.trim() == entry) {
        return Ok(false);
    }

    let position = lines
        .iter()
        .position(|line| line.trim() == "\"tests/e2e-rust\",")
        .or_else(|| lines.iter().position(|line| line.trim() == "members = [").map(|i| i + 1))
        .ok_or_else(|| anyhow::anyhow!("Missing members array in root Cargo.toml"))?;
    lines.insert(position, format!("    {entry}"));
    Ok(true)
}

fn insert_workspace_dependency(
    lines: &mut Vec<String>,
    key: &str,
    entry: String,
) -> Result<bool> {
    if has_key_in_section(lines, "workspace.dependencies", key) {
        return Ok(false);
    }
    let position = lines
        .iter()
        .position(|line| line.trim() == "[workspace.dependencies]")
        .ok_or_else(|| anyhow::anyhow!("Missing [workspace.dependencies] in root Cargo.toml"))?;
    lines.insert(position + 1, entry);
    Ok(true)
}

fn register_in_workspace_cargo_toml(
    plan: &mut RegistrationPlan,
    root: &Path,
    crate_name: &str,
    ui: ScaffoldUi,
    with_cli: bool,
) -> Result<()> {
    let path = root.join("Cargo.toml");
    let content = plan.read(&path)?;
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if ui.has_admin() {
        modified |= insert_workspace_member(&mut lines, &format!("crates/modules/{crate_name}/admin"))?;
    }
    if ui.has_storefront() {
        modified |= insert_workspace_member(&mut lines, &format!("crates/modules/{crate_name}/storefront"))?;
    }
    if with_cli {
        modified |= insert_workspace_member(&mut lines, &format!("crates/modules/{crate_name}/cli"))?;
    }

    modified |= insert_workspace_dependency(
        &mut lines,
        crate_name,
        format!("{crate_name} = {{ path = \"crates/modules/{crate_name}\" }}"),
    )?;

    if with_cli {
        let cli_crate = format!("{crate_name}-cli");
        modified |= insert_workspace_dependency(
            &mut lines,
            &cli_crate,
            format!("{cli_crate} = {{ path = \"crates/modules/{crate_name}/cli\" }}"),
        )?;
    }

    if modified {
        plan.stage(&path, lines.join("\n") + "\n");
    }
    Ok(())
}

fn register_in_modules_toml(
    plan: &mut RegistrationPlan,
    root: &Path,
    slug: &str,
    crate_name: &str,
) -> Result<()> {
    let path = root.join("modules.toml");
    let content = plan.read(&path)?;
    let manifest = rustok_manifest::parse_modules_manifest(&content)
        .with_context(|| format!("Invalid modules.toml at {}", path.display()))?;
    if manifest.modules.contains_key(slug) {
        bail!("Module '{slug}' is already registered in modules.toml");
    }

    let entry = format!(
        "{slug} = {{ crate = \"{crate_name}\", source = \"path\", path = \"crates/modules/{crate_name}\" }}"
    );
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let insert_pos = lines
        .iter()
        .position(|line| line.starts_with("# Capability extensions") || line.starts_with("[settings]"))
        .unwrap_or(lines.len());
    lines.insert(insert_pos, entry);
    plan.stage(&path, lines.join("\n") + "\n");
    Ok(())
}

/// A created UI package is not a published surface or FFA readiness evidence.
fn draft_readiness(ui: ScaffoldUi) -> (&'static str, &'static str, &'static str) {
    match ui {
        ScaffoldUi::Dual => ("admin + storefront (draft, unmounted)", "not_started", "core_transport_ui"),
        ScaffoldUi::Admin => ("admin (draft, unmounted)", "not_started", "core_transport_ui"),
        ScaffoldUi::Storefront => ("storefront (draft, unmounted)", "not_started", "core_transport_ui"),
        ScaffoldUi::None => ("none", "not_started", "no_ui_boundary"),
    }
}

fn register_in_registry_md(
    plan: &mut RegistrationPlan,
    root: &Path,
    slug: &str,
    crate_name: &str,
    desc: &str,
    ui: ScaffoldUi,
) -> Result<()> {
    let path = root.join("docs/modules/registry.md");
    let content = plan.read(&path)?;
    if content.contains(&format!("| \x60{slug}\x60 |")) {
        return Ok(());
    }

    let description = desc.replace('\n', " ").replace('\r', " ").replace('|', "\\|");
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let (ui_surfaces, ffa_status, shape) = draft_readiness(ui);
    let board_row = format!(
        "| \x60{slug}\x60 | {ui_surfaces} | \x60{ffa_status}\x60 | \x60not_started\x60 | \x60{shape}\x60 | [Implementation plan](../../crates/modules/{crate_name}/docs/implementation-plan.md); {description} |"
    );

    let mut board_insert_pos = None;
    let mut in_board_table = false;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("| Module slug |") {
            in_board_table = true;
            continue;
        }
        if in_board_table && !line.trim().starts_with('|') {
            board_insert_pos = Some(index);
            break;
        }
    }
    let board_insert_pos = board_insert_pos
        .ok_or_else(|| anyhow::anyhow!("Could not find the FFA/FBA readiness board in {}", path.display()))?;
    lines.insert(board_insert_pos, board_row);

    let optional_row = format!("| \x60{slug}\x60 | \x60{crate_name}\x60 | none | {description} |");
    let mut optional_insert_pos = None;
    let mut in_optional = false;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("### Optional") {
            in_optional = true;
            continue;
        }
        if in_optional && (line.starts_with("## ") || line.starts_with("The dependency column above")) {
            optional_insert_pos = Some(index);
            break;
        }
    }
    let optional_insert_pos = optional_insert_pos
        .ok_or_else(|| anyhow::anyhow!("Could not find the optional modules table in {}", path.display()))?;
    lines.insert(optional_insert_pos, optional_row);

    plan.stage(&path, lines.join("\n") + "\n");
    Ok(())
}

fn register_in_docs_index_md(
    plan: &mut RegistrationPlan,
    root: &Path,
    crate_name: &str,
    has_admin: bool,
    has_storefront: bool,
) -> Result<()> {
    let path = root.join("docs/modules/_index.md");
    let content = plan.read(&path)?;
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if !content.contains(&format!("| \x60{crate_name}\x60 |")) {
        let row = format!(
            "| \x60{crate_name}\x60 | [docs](../../crates/modules/{crate_name}/docs/README.md) | [plan](../../crates/modules/{crate_name}/docs/implementation-plan.md) |"
        );
        let mut insert_pos = None;
        let mut in_domain = false;
        for (index, line) in lines.iter().enumerate() {
            if line.starts_with("## Domain Modules") {
                in_domain = true;
                continue;
            }
            if in_domain && line.starts_with("## ") {
                insert_pos = Some(index);
                break;
            }
        }
        lines.insert(insert_pos.unwrap_or(lines.len()), row);
        modified = true;
    }

    if has_admin {
        let entry = format!(
            "- \x60{crate_name}\x60 (draft, unmounted): [README](../../crates/modules/{crate_name}/admin/README.md)"
        );
        if !content.contains(&entry) {
            let mut insert_pos = None;
            let mut in_admin = false;
            for (index, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Admin Surfaces") {
                    in_admin = true;
                    continue;
                }
                if in_admin && (line.starts_with("### ") || line.starts_with("## ")) {
                    insert_pos = Some(index);
                    break;
                }
            }
            lines.insert(insert_pos.unwrap_or(lines.len()), entry);
            modified = true;
        }
    }

    if has_storefront {
        let entry = format!(
            "- \x60{crate_name}\x60 (draft, unmounted): [README](../../crates/modules/{crate_name}/storefront/README.md)"
        );
        if !content.contains(&entry) {
            let mut insert_pos = None;
            let mut in_storefront = false;
            for (index, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Storefront Surfaces") {
                    in_storefront = true;
                    continue;
                }
                if in_storefront && (line.starts_with("### ") || line.starts_with("## ")) {
                    insert_pos = Some(index);
                    break;
                }
            }
            lines.insert(insert_pos.unwrap_or(lines.len()), entry);
            modified = true;
        }
    }

    if modified {
        plan.stage(&path, lines.join("\n") + "\n");
    }
    Ok(())
}

fn register_in_ui_packages_index_md(
    plan: &mut RegistrationPlan,
    root: &Path,
    crate_name: &str,
    has_admin: bool,
    has_storefront: bool,
) -> Result<()> {
    let path = root.join("docs/modules/UI_PACKAGES_INDEX.md");
    let content = plan.read(&path)?;
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if has_admin {
        let entry = format!(
            "- \x60{crate_name}\x60 admin UI (draft, unmounted): [README](../../crates/modules/{crate_name}/admin/README.md)"
        );
        if !content.contains(&entry) {
            let mut insert_pos = None;
            let mut in_admin = false;
            for (index, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Admin Surfaces") {
                    in_admin = true;
                    continue;
                }
                if in_admin && (line.starts_with("### ") || line.starts_with("## ")) {
                    insert_pos = Some(index);
                    break;
                }
            }
            lines.insert(insert_pos.unwrap_or(lines.len()), entry);
            modified = true;
        }
    }

    if has_storefront {
        let entry = format!(
            "- \x60{crate_name}\x60 storefront UI (draft, unmounted): [README](../../crates/modules/{crate_name}/storefront/README.md)"
        );
        if !content.contains(&entry) {
            let mut insert_pos = None;
            let mut in_storefront = false;
            for (index, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Storefront Surfaces")
                    || line.starts_with("### Module UI Examples")
                {
                    in_storefront = true;
                    continue;
                }
                if in_storefront
                    && (line.starts_with("### Large Capability") || line.starts_with("## What Not"))
                {
                    insert_pos = Some(index);
                    break;
                }
            }
            lines.insert(insert_pos.unwrap_or(lines.len()), entry);
            modified = true;
        }
    }

    if modified {
        plan.stage(&path, lines.join("\n") + "\n");
    }
    Ok(())
}

fn register_in_server_cargo_toml(
    plan: &mut RegistrationPlan,
    root: &Path,
    slug: &str,
    crate_name: &str,
) -> Result<()> {
    let path = root.join("apps/server/Cargo.toml");
    let content = plan.read(&path)?;
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    let feature_key = format!("mod-{slug}");
    if !has_key_in_section(&lines, "features", &feature_key) {
        let entry = format!(
            "mod-{slug} = [\"dep:{crate_name}\", \"rustok-distribution/mod-{slug}\"]"
        );
        let position = lines.iter().position(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("mod-workflow =")
                || trimmed.starts_with("mod-messenger =")
                || trimmed.starts_with("mod-docs =")
        }).ok_or_else(|| anyhow::anyhow!("Could not find insertion point for mod-{slug} in {}", path.display()))?;
        lines.insert(position + 1, entry);
        modified = true;
    }

    if !has_key_in_section(&lines, "dependencies", crate_name) {
        let entry = format!("{crate_name} = {{ workspace = true, optional = true }}");
        let position = lines.iter().position(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("rustok-messenger =")
                || trimmed.starts_with("rustok-docs =")
                || trimmed.starts_with("rustok-blog =")
        }).ok_or_else(|| anyhow::anyhow!("Could not find dependency insertion point in {}", path.display()))?;
        lines.insert(position + 1, entry);
        modified = true;
    }

    if !content.contains(&format!("\"{crate_name}\"")) {
        let position = lines
            .iter()
            .position(|line| line.contains("normal = [\"rustok-rbac\""))
            .ok_or_else(|| anyhow::anyhow!("Could not find the normal feature list in {}", path.display()))?;
        lines[position] = lines[position].replace("]", &format!(", \"{crate_name}\"]"));
        modified = true;
    }

    if modified {
        plan.stage(&path, lines.join("\n") + "\n");
    }
    Ok(())
}

fn register_in_distribution_cargo_toml(
    plan: &mut RegistrationPlan,
    root: &Path,
    slug: &str,
    crate_name: &str,
) -> Result<()> {
    let path = root.join("crates/modules/rustok-distribution/Cargo.toml");
    let Some(content) = plan.read_optional(&path)? else {
        return Ok(());
    };
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if !has_key_in_section(&lines, "features", &format!("mod-{slug}")) {
        let entry = format!("mod-{slug} = [\"dep:{crate_name}\"]");
        let position = lines.iter().position(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("mod-messenger =")
                || trimmed.starts_with("mod-docs =")
                || trimmed.starts_with("mod-blog =")
        }).ok_or_else(|| anyhow::anyhow!("Could not find feature insertion point in {}", path.display()))?;
        lines.insert(position + 1, entry);
        modified = true;
    }

    if !has_key_in_section(&lines, "dependencies", crate_name) {
        let position = lines.iter().position(|line| line.trim() == "[dependencies]")
            .ok_or_else(|| anyhow::anyhow!("Missing [dependencies] in {}", path.display()))?;
        lines.insert(position + 1, format!("{crate_name} = {{ workspace = true, optional = true }}"));
        modified = true;
    }

    if modified {
        plan.stage(&path, lines.join("\n") + "\n");
    }
    Ok(())
}

fn register_in_distribution_lib_rs(
    plan: &mut RegistrationPlan,
    root: &Path,
    slug: &str,
    pascal: &str,
    snake: &str,
) -> Result<()> {
    let path = root.join("crates/modules/rustok-distribution/src/lib.rs");
    let Some(content) = plan.read_optional(&path)? else {
        return Ok(());
    };
    let feature_needle = format!("feature = \"mod-{slug}\"");
    if content.contains(&feature_needle) {
        return Ok(());
    }

    let registration_block = format!(
        "    #[cfg(feature = \"mod-{slug}\")]\n    let registry = registry.register(rustok_{snake}::{pascal}Module);"
    );
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let position = lines.iter()
        .position(|line| line.contains("generated_promotions::register_promoted_modules(registry)"))
        .ok_or_else(|| anyhow::anyhow!("Could not find module registration insertion point in {}", path.display()))?;
    lines.insert(position, registration_block);
    plan.stage(&path, lines.join("\n") + "\n");
    Ok(())
}

fn register_in_cli_registry_cargo_toml(
    plan: &mut RegistrationPlan,
    root: &Path,
    cli_crate: &str,
) -> Result<()> {
    let path = root.join("crates/utils/rustok-cli-registry/Cargo.toml");
    let content = plan.read(&path)?;
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    if has_key_in_section(&lines, "dependencies", &format!("{cli_crate}.workspace")) {
        return Ok(());
    }
    let position = lines.iter()
        .position(|line| line.trim() == "rustok-cli-core.workspace = true")
        .or_else(|| lines.iter().position(|line| line.trim() == "[dependencies]"))
        .ok_or_else(|| anyhow::anyhow!("Missing [dependencies] in {}", path.display()))?;
    lines.insert(position + 1, format!("{cli_crate}.workspace = true"));
    plan.stage(&path, lines.join("\n") + "\n");
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CliRegistryProvider {
    slug: String,
    namespace: String,
    factory: Option<String>,
    provider: Option<String>,
    crate_name: String,
}

fn normalize_cli_provider(
    slug: &str,
    source: &str,
    namespace: Option<&str>,
    factory: Option<&str>,
    provider: Option<&str>,
) -> Result<CliRegistryProvider> {
    if factory.is_some() == provider.is_some() {
        bail!("{source}: CLI provider must declare exactly one of factory or provider");
    }
    let namespace = namespace.unwrap_or(slug);
    let mut chars = namespace.chars();
    let valid_namespace = chars.next().map(|first| first.is_ascii_lowercase()).unwrap_or(false)
        && chars.all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_');
    if !valid_namespace {
        bail!("{source}: CLI provider namespace must be snake_case");
    }

    let rust_path = factory.or(provider).unwrap_or_default();
    let valid_rust_path = !rust_path.is_empty()
        && rust_path.split("::").all(|segment| {
            let mut chars = segment.chars();
            chars.next().map(|first| first == '_' || first.is_ascii_alphabetic()).unwrap_or(false)
                && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
        });
    if !valid_rust_path {
        bail!("{source}: CLI provider factory/provider must be a Rust path");
    }

    let crate_name = rust_path.split("::").next().unwrap_or_default().replace('_', "-");
    Ok(CliRegistryProvider {
        slug: slug.to_string(),
        namespace: namespace.to_string(),
        factory: factory.map(str::to_string),
        provider: provider.map(str::to_string),
        crate_name,
    })
}

fn collect_cli_providers(
    plan: &RegistrationPlan,
    root: &Path,
    new_slug: &str,
    new_module_manifest: &str,
) -> Result<Vec<CliRegistryProvider>> {
    let mut providers = Vec::new();

    if let Some(root_registry) = plan.read_optional(&root.join("cli-registry.toml"))? {
        let value: toml::Value = toml::from_str(&root_registry)
            .context("Failed to parse cli-registry.toml")?;
        if let Some(root_providers) = value.get("providers") {
            let table = root_providers.as_table()
                .ok_or_else(|| anyhow::anyhow!("cli-registry.toml [providers] must be a table"))?;
            for (slug, entry) in table {
                let entry = entry.as_table()
                    .ok_or_else(|| anyhow::anyhow!("cli-registry.toml provider {slug} must be a table"))?;
                let namespace = entry.get("namespace").and_then(toml::Value::as_str);
                let factory = entry.get("factory").and_then(toml::Value::as_str);
                let provider = entry.get("provider").and_then(toml::Value::as_str);
                providers.push(normalize_cli_provider(
                    slug,
                    "cli-registry.toml",
                    namespace,
                    factory,
                    provider,
                )?);
            }
        }
    }

    let modules_content = plan.read(&root.join("modules.toml"))?;
    let modules = rustok_manifest::parse_modules_manifest(&modules_content)
        .context("Failed to parse the planned modules.toml for CLI registration")?;

    for (slug, spec) in modules.modules {
        let Some(module_path) = spec.path else {
            continue;
        };
        let manifest_path = root.join(module_path).join("rustok-module.toml");
        let module_content = if slug == new_slug {
            new_module_manifest.to_string()
        } else {
            let Some(content) = plan.read_optional(&manifest_path)? else {
                continue;
            };
            content
        };
        let manifest = rustok_manifest::parse_module_package_manifest(&module_content)
            .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;
        let Some(cli) = manifest.provides.cli else {
            continue;
        };
        providers.push(normalize_cli_provider(
            &slug,
            &format!("{}: [provides.cli]", manifest_path.display()),
            cli.namespace.as_deref(),
            cli.factory.as_deref(),
            cli.provider.as_deref(),
        )?);
    }

    providers.sort_by(|left, right| left.namespace.cmp(&right.namespace));
    Ok(providers)
}

fn render_cli_registry(mut providers: Vec<CliRegistryProvider>) -> String {
    providers.sort_by(|left, right| left.namespace.cmp(&right.namespace));
    let mut output = String::from(
        "// @generated by scripts/generate/generate-cli-registry.mjs\n\
         // Do not edit by hand.\n\n\
         use rustok_cli_core::CommandProvider;\n\
         use rustok_runtime::RuntimeComposition;\n\n\
         pub fn generated_providers(runtime: &RuntimeComposition) -> Vec<Box<dyn CommandProvider>> {\n",
    );

    if providers.is_empty() {
        output.push_str("    Vec::new()\n");
    } else {
        output.push_str("    vec![\n");
        for provider in providers {
            output.push_str(&format!(
                "        // {} / {}\n",
                provider.slug, provider.namespace
            ));
            if let Some(factory) = provider.factory {
                output.push_str(&format!("        {factory}(runtime),\n"));
            } else if let Some(provider_type) = provider.provider {
                output.push_str(&format!("        Box::new({provider_type}::default()),\n"));
            }
        }
        output.push_str("    ]\n");
    }
    output.push_str("}\n");
    output
}

fn validate_cli_registry_dependencies(
    plan: &RegistrationPlan,
    root: &Path,
    providers: &[CliRegistryProvider],
) -> Result<()> {
    let registry_path = root.join("crates/utils/rustok-cli-registry/Cargo.toml");
    let registry_content = plan.read(&registry_path)?;
    let registry: toml::Value = toml::from_str(&registry_content)
        .with_context(|| format!("Failed to parse {}", registry_path.display()))?;
    let registry_dependencies = registry
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| anyhow::anyhow!("Missing [dependencies] in {}", registry_path.display()))?;

    let root_path = root.join("Cargo.toml");
    let root_content = plan.read(&root_path)?;
    let root_manifest: toml::Value = toml::from_str(&root_content)
        .with_context(|| format!("Failed to parse {}", root_path.display()))?;
    let workspace_dependencies = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| anyhow::anyhow!("Missing [workspace.dependencies] in {}", root_path.display()))?;

    for provider in providers {
        if provider.crate_name == "rustok-cli-core" {
            continue;
        }
        let registry_declared = registry_dependencies
            .get(&provider.crate_name)
            .and_then(toml::Value::as_table)
            .and_then(|dependency| dependency.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true);
        if !registry_declared {
            bail!(
                "{}: missing selected provider dependency {}.workspace = true",
                registry_path.display(),
                provider.crate_name
            );
        }
        if !workspace_dependencies.contains_key(&provider.crate_name) {
            bail!(
                "{}: missing root workspace dependency {}",
                root_path.display(),
                provider.crate_name
            );
        }
    }
    Ok(())
}

fn register_in_cli_registry_generated(
    plan: &mut RegistrationPlan,
    root: &Path,
    new_slug: &str,
    new_module_manifest: &str,
) -> Result<()> {
    let providers = collect_cli_providers(plan, root, new_slug, new_module_manifest)?;
    validate_cli_registry_dependencies(plan, root, &providers)?;
    let path = root.join("crates/utils/rustok-cli-registry/src/generated.rs");
    let next = render_cli_registry(providers);
    let current = plan.read(&path)?;
    if current != next {
        plan.stage(&path, next);
    }
    Ok(())
}

pub(crate) fn register_in_admin_cargo_toml(root: &Path, crate_name: &str) -> Result<()> {
    let path = root.join("apps").join("admin").join("Cargo.toml");
    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let admin_crate = format!("{crate_name}-admin");
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    // 1. features.csr
    let csr_entry = format!("  \"{admin_crate}/csr\",");
    if !content.contains(csr_entry.trim()) {
        if let Some(pos) = find_closing_bracket_after(&lines, "csr = [") {
            lines.insert(pos, csr_entry);
            modified = true;
        }
    }

    // 2. features.hydrate
    let hydrate_entry = format!("  \"{admin_crate}/hydrate\",");
    if !content.contains(hydrate_entry.trim()) {
        if let Some(pos) = find_closing_bracket_after(&lines, "hydrate = [") {
            lines.insert(pos, hydrate_entry);
            modified = true;
        }
    }

    // 3. features.ssr
    let ssr_entry = format!("  \"{admin_crate}/ssr\",");
    if !content.contains(ssr_entry.trim()) {
        if let Some(pos) = find_closing_bracket_after(&lines, "ssr = [") {
            lines.insert(pos, ssr_entry);
            modified = true;
        }
    }

    // 4. [dependencies]
    let dep_entry = format!("{admin_crate} = {{ path = \"../../crates/modules/{crate_name}/admin\", default-features = false }}");
    let dep_needle = format!("{admin_crate} =");
    if !content.contains(&dep_needle) {
        if let Some(pos) = lines
            .iter()
            .position(|l| l.trim().starts_with("rustok-blog-admin ="))
        {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        } else if let Some(pos) = lines.iter().position(|l| l.trim() == "[dependencies]") {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        }
    }

    // 5. [package.metadata.cargo-udeps.ignore] normal
    let udeps_entry = format!("  \"{admin_crate}\",");
    if !content.contains(udeps_entry.trim()) {
        if let Some(pos) = find_closing_bracket_after(&lines, "normal = [") {
            lines.insert(pos, udeps_entry);
            modified = true;
        }
    }

    // 6. [package.metadata.cargo-machete] ignored
    let machete_entry = format!("  \"{admin_crate}\",");
    if !content.contains(machete_entry.trim()) {
        if let Some(pos) = find_closing_bracket_after(&lines, "ignored = [") {
            lines.insert(pos, machete_entry);
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!("  Updated apps/admin/Cargo.toml with admin UI dependencies and features");
    }

    Ok(())
}

pub(crate) fn register_in_storefront_cargo_toml(root: &Path, crate_name: &str) -> Result<()> {
    let path = root.join("apps").join("storefront").join("Cargo.toml");
    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let sf_crate = format!("{crate_name}-storefront");
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    // 1. features.ssr
    let dep_ssr = format!("  \"dep:{sf_crate}\",");
    let ssr_entry = format!("  \"{sf_crate}/ssr\",");
    if !content.contains(dep_ssr.trim()) {
        if let Some(pos) = find_closing_bracket_after(&lines, "ssr = [") {
            lines.insert(pos, ssr_entry);
            lines.insert(pos, dep_ssr);
            modified = true;
        }
    }

    // 2. [dependencies]
    let dep_entry = format!("{sf_crate} = {{ path = \"../../crates/modules/{crate_name}/storefront\", default-features = false, optional = true }}");
    let dep_needle = format!("{sf_crate} =");
    if !content.contains(&dep_needle) {
        if let Some(pos) = lines
            .iter()
            .position(|l| l.trim().starts_with("rustok-seo-storefront ="))
        {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        } else if let Some(pos) = lines.iter().position(|l| l.trim() == "[dependencies]") {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        }
    }

    // 3. [package.metadata.cargo-udeps.ignore] normal
    let udeps_entry = format!("  \"{sf_crate}\",");
    if !content.contains(udeps_entry.trim()) {
        if let Some(pos) = find_closing_bracket_after(&lines, "normal = [") {
            lines.insert(pos, udeps_entry);
            modified = true;
        }
    }

    // 4. [package.metadata.cargo-machete] ignored
    let machete_entry = format!("  \"{sf_crate}\",");
    if !content.contains(machete_entry.trim()) {
        if let Some(pos) = find_closing_bracket_after(&lines, "ignored = [") {
            lines.insert(pos, machete_entry);
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!(
            "  Updated apps/storefront/Cargo.toml with storefront UI dependencies and features"
        );
    }

    Ok(())
}

pub(crate) fn find_closing_bracket_after(lines: &[String], section_prefix: &str) -> Option<usize> {
    let mut in_section = false;
    for (i, line) in lines.iter().enumerate() {
        if line.trim().starts_with(section_prefix) {
            in_section = true;
            continue;
        }
        if in_section && line.trim().starts_with(']') {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod scaffold_registration_tests {
    use super::{
        draft_readiness, normalize_cli_provider, render_cli_registry, CliRegistryProvider,
        ScaffoldUi,
    };

    #[test]
    fn cli_provider_factory_is_normalized_and_namespaced() {
        let provider = normalize_cli_provider(
            "example",
            "test manifest",
            Some("example"),
            Some("rustok_example_cli::command_provider"),
            None,
        )
        .unwrap();
        assert_eq!(provider.crate_name, "rustok-example-cli");
        assert_eq!(provider.namespace, "example");
        assert!(normalize_cli_provider(
            "example",
            "test manifest",
            Some("Bad Namespace"),
            Some("rustok_example_cli::command_provider"),
            None,
        )
        .is_err());
    }

    #[test]
    fn generated_cli_registry_sorts_and_keeps_all_provider_factories() {
        let providers = vec![
            CliRegistryProvider {
                slug: "example".into(),
                namespace: "example".into(),
                factory: Some("rustok_example_cli::command_provider".into()),
                provider: None,
                crate_name: "rustok-example-cli".into(),
            },
            CliRegistryProvider {
                slug: "blog".into(),
                namespace: "blog".into(),
                factory: Some("rustok_blog_cli::command_provider".into()),
                provider: None,
                crate_name: "rustok-blog-cli".into(),
            },
        ];
        let generated = render_cli_registry(providers);
        assert!(generated.find("blog / blog").unwrap() < generated.find("example / example").unwrap());
        assert!(generated.contains("rustok_blog_cli::command_provider(runtime),"));
        assert!(generated.contains("rustok_example_cli::command_provider(runtime),"));
    }

    #[test]
    fn draft_packages_remain_unmounted_and_not_started() {
        for mode in [ScaffoldUi::Admin, ScaffoldUi::Storefront, ScaffoldUi::Dual] {
            let (surfaces, ffa, shape) = draft_readiness(mode);
            assert!(surfaces.contains("draft, unmounted"));
            assert_eq!(ffa, "not_started");
            assert_eq!(shape, "core_transport_ui");
        }
        let (surfaces, ffa, shape) = draft_readiness(ScaffoldUi::None);
        assert_eq!(surfaces, "none");
        assert_eq!(ffa, "not_started");
        assert_eq!(shape, "no_ui_boundary");
    }
}

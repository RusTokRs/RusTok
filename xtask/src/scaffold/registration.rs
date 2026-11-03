use super::options::{ScaffoldOptions, ScaffoldUi};
use super::registration_toml::{self, DISTRIBUTION, SERVER, UDEPS_NORMAL};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone)]
pub(crate) struct RegistrationChange {
    pub(crate) path: PathBuf,
    pub(crate) original: String,
    pub(crate) content: String,
}

const REGISTRATION_TARGETS: &[&str] = &[
    "Cargo.toml",
    "modules.toml",
    "docs/modules/registry.md",
    "docs/modules/_index.md",
    "docs/modules/implementation-plans-registry.md",
    "docs/modules/UI_PACKAGES_INDEX.md",
    SERVER,
    DISTRIBUTION,
    "crates/modules/rustok-distribution/src/lib.rs",
];

pub(super) fn is_registration_target(path: &Path) -> bool {
    REGISTRATION_TARGETS
        .iter()
        .any(|allowed| path == Path::new(allowed))
}

pub(crate) fn registration_target_paths(root: &Path, opts: &ScaffoldOptions) -> Vec<PathBuf> {
    let mut paths: Vec<_> = REGISTRATION_TARGETS[..5]
        .iter()
        .map(|relative| root.join(relative))
        .collect();
    if opts.ui.has_admin() || opts.ui.has_storefront() {
        paths.push(root.join(REGISTRATION_TARGETS[5]));
    }
    if !opts.skip_server_wire {
        paths.extend(
            REGISTRATION_TARGETS[6..]
                .iter()
                .map(|relative| root.join(relative)),
        );
    }
    paths
}

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new() -> Result<Self> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join(format!("rustok-scaffold-plan-{}-{id}", std::process::id()));
            match fs::create_dir(&root) {
                Ok(()) => return Ok(Self { root }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "Failed to create scaffold planning directory {}",
                            root.display()
                        )
                    });
                }
            }
        }
        anyhow::bail!("Could not allocate a unique scaffold planning directory");
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!(
                "Failed to remove scaffold planning directory {}: {error}",
                self.root.display()
            );
        }
    }
}

pub(crate) fn plan_module_registration(
    root: &Path,
    opts: &ScaffoldOptions,
    pascal: &str,
    snake: &str,
) -> Result<Vec<RegistrationChange>> {
    let targets = registration_target_paths(root, opts);
    let temporary = TempWorkspace::new()?;
    let mut originals = Vec::with_capacity(targets.len());

    for target in &targets {
        super::ensure_no_symlink_ancestors(root, target)?;
        let metadata = fs::symlink_metadata(target).with_context(|| {
            format!(
                "Required registration file is missing: {}",
                target.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            anyhow::bail!(
                "Registration target must be a regular file: {}",
                target.display()
            );
        }
        let relative = target.strip_prefix(root).with_context(|| {
            format!(
                "Registration target escapes workspace: {}",
                target.display()
            )
        })?;
        let copy = temporary.root.join(relative);
        if let Some(parent) = copy.parent() {
            fs::create_dir_all(parent)?;
        }
        let original = fs::read_to_string(target)?;
        fs::write(&copy, original.as_bytes())
            .with_context(|| format!("Failed to stage registration file {}", target.display()))?;
        originals.push(original);
    }

    apply_registration_mutations(&temporary.root, root, opts, pascal, snake)?;
    validate_registration(&temporary.root, root, opts, pascal)?;

    let mut changes = Vec::new();
    for (target, original) in targets.into_iter().zip(originals) {
        let relative = target.strip_prefix(root).with_context(|| {
            format!(
                "Registration target escapes workspace: {}",
                target.display()
            )
        })?;
        let staged = fs::read_to_string(temporary.root.join(relative))?;
        let staged = if relative.extension().is_some_and(|ext| ext == "toml") {
            staged
        } else {
            preserve_line_endings(&original, staged)
        };
        if staged != original {
            changes.push(RegistrationChange {
                path: target,
                original,
                content: staged,
            });
        }
    }
    Ok(changes)
}

fn preserve_line_endings(original: &str, mut staged: String) -> String {
    // Text-based registration editors may rebuild lines with LF. Retain a
    // uniform CRLF file's style so unchanged lines remain byte-identical.
    if original.contains("\r\n")
        && original
            .split_inclusive('\n')
            .all(|line| !line.ends_with('\n') || line.ends_with("\r\n"))
    {
        staged = staged.replace("\r\n", "\n").replace('\n', "\r\n");
    }
    if !original.ends_with('\n') && staged.ends_with('\n') {
        let terminator_bytes = if staged.ends_with("\r\n") { 2 } else { 1 };
        staged.truncate(staged.len() - terminator_bytes);
    }
    staged
}

fn apply_registration_mutations(
    root: &Path,
    workspace_root: &Path,
    opts: &ScaffoldOptions,
    pascal: &str,
    _snake: &str,
) -> Result<()> {
    let slug = &opts.slug;
    let crate_name = opts.crate_name();

    registration_toml::register(root, workspace_root, opts)?;
    register_in_registry_md(root, slug, &crate_name, &opts.description, opts.ui)?;
    register_in_plan_registry(root, slug, &crate_name)?;
    register_in_docs_index_md(
        root,
        &crate_name,
        opts.ui.has_admin(),
        opts.ui.has_storefront(),
    )?;

    if opts.ui.has_admin() || opts.ui.has_storefront() {
        register_in_ui_packages_index_md(
            root,
            &crate_name,
            opts.ui.has_admin(),
            opts.ui.has_storefront(),
        )?;
    }

    if !opts.skip_server_wire {
        // Cargo package uses kebab-case; Rust paths use underscores.
        register_in_distribution_lib_rs(root, slug, pascal, &opts.kebab_name().replace('-', "_"))?;
        // Draft UI crates join Cargo workspace only. They do not add live
        // Leptos host dependencies/features/route composition.
    }

    Ok(())
}

/// A created UI package is not a published surface or FFA readiness evidence.
fn draft_readiness(ui: ScaffoldUi) -> (&'static str, &'static str, &'static str) {
    match ui {
        ScaffoldUi::Dual => (
            "admin + storefront (draft, unmounted)",
            "`not_started`",
            "`core_transport_ui`",
        ),
        ScaffoldUi::Admin => (
            "admin (draft, unmounted)",
            "`not_started`",
            "`core_transport_ui`",
        ),
        ScaffoldUi::Storefront => (
            "storefront (draft, unmounted)",
            "`not_started`",
            "`core_transport_ui`",
        ),
        ScaffoldUi::None => ("none", "`not_started`", "`no_ui_boundary`"),
    }
}

fn register_in_registry_md(
    root: &Path,
    slug: &str,
    crate_name: &str,
    desc: &str,
    ui: ScaffoldUi,
) -> Result<()> {
    let path = root.join("docs").join("modules").join("registry.md");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let slug_needle = format!("| `{slug}` |");
    if content.contains(&slug_needle) {
        anyhow::bail!("Module '{slug}' already has a central registry row");
    }

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();

    // 1. Insert into Table 1: ## FFA/FBA Readiness Board (Module-owned UI)
    let (ui_surfaces, ffa_status, shape) = draft_readiness(ui);
    let board_row = format!(
        "| `{slug}` | {ui_surfaces} | {ffa_status} | `not_started` | {shape} | [Implementation plan](../../crates/modules/{crate_name}/docs/implementation-plan.md); {desc} |"
    );

    let mut board_insert_pos = None;
    let mut in_board_table = false;
    for (i, line) in lines.iter().enumerate() {
        if line.split('|').nth(1).map(str::trim) == Some("Module slug")
            && line.split('|').nth(2).map(str::trim) == Some("UI surfaces")
        {
            in_board_table = true;
            continue;
        }
        if in_board_table && !line.trim().starts_with('|') {
            board_insert_pos = Some(i);
            break;
        }
    }

    let pos = board_insert_pos
        .or_else(|| in_board_table.then_some(lines.len()))
        .context("Cannot locate the FFA/FBA readiness table")?;
    lines.insert(pos, board_row);

    // 2. Insert into Table 2: ### Optional Modules
    let optional_row = format!("| `{slug}` | `{crate_name}` | none | {desc} |");
    let mut optional_insert_pos = None;
    let mut in_optional = false;
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("### Optional") {
            in_optional = true;
            continue;
        }
        if in_optional
            && (line.starts_with("## ") || line.starts_with("The dependency column above"))
        {
            optional_insert_pos = Some(i);
            break;
        }
    }

    let pos = optional_insert_pos
        .or_else(|| in_optional.then_some(lines.len()))
        .context("Cannot locate the optional module table")?;
    lines.insert(pos, optional_row);

    let release_header = lines
        .iter()
        .position(|line| line.starts_with("| Module slug") && line.contains("Data Boundary"))
        .context("Cannot locate the release/rollback readiness table")?;
    let release_end = lines
        .iter()
        .enumerate()
        .skip(release_header + 1)
        .find(|(_, line)| !line.trim_start().starts_with('|'))
        .map(|(index, _)| index)
        .unwrap_or(lines.len());
    lines.insert(release_end, format!(
        "| `{slug}` | `Stateless` | `data boundary: none` | none | `DeniedMaintenanceOnly` | Draft identity; release and rollback execution unverified; [owner plan](../../crates/modules/{crate_name}/docs/implementation-plan.md) |"
    ));
    fs::write(&path, lines.join("\n") + "\n")
        .with_context(|| format!("Failed to write {}", path.display()))?;

    Ok(())
}

fn register_in_plan_registry(root: &Path, slug: &str, crate_name: &str) -> Result<()> {
    let path = root.join("docs/modules/implementation-plans-registry.md");
    let content = fs::read_to_string(&path)?;
    let mut lines: Vec<_> = content.lines().map(String::from).collect();
    let header = lines
        .iter()
        .position(|line| line.starts_with("| Module/Crate |"))
        .context("Cannot locate the live implementation-plans table")?;
    if lines
        .iter()
        .skip(header + 1)
        .take_while(|line| line.trim_start().starts_with('|'))
        .any(|line| {
            line.split('|')
                .nth(1)
                .is_some_and(|cell| cell.trim() == format!("`{slug}`"))
        })
    {
        anyhow::bail!("Implementation plan for '{slug}' is already registered");
    }
    let end = lines
        .iter()
        .enumerate()
        .skip(header + 1)
        .find(|(_, line)| !line.trim_start().starts_with('|'))
        .map(|(index, _)| index)
        .unwrap_or(lines.len());
    lines.insert(end, format!(
        "| `{slug}` | [plan](../../crates/modules/{crate_name}/docs/implementation-plan.md) | `not_started` | Specify owner operations and invariants before persistence, transports or live UI. |"
    ));
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

/// Successful planning must mean complete registration even with --skip-validate.
/// Text insertion anchors are implementation details, never permission to omit wiring.
fn validate_registration(
    root: &Path,
    workspace_root: &Path,
    opts: &ScaffoldOptions,
    pascal: &str,
) -> Result<()> {
    fn read_toml(root: &Path, relative: &str) -> Result<toml::Value> {
        toml::from_str(&fs::read_to_string(root.join(relative))?)
            .with_context(|| format!("Invalid planned registration: {relative}"))
    }
    fn require(condition: bool, surface: &str) -> Result<()> {
        anyhow::ensure!(condition, "Incomplete scaffold registration: {surface}");
        Ok(())
    }
    let slug = &opts.slug;
    let name = opts.crate_name();
    let workspace = read_toml(root, "Cargo.toml")?;
    let workspace = workspace
        .get("workspace")
        .context("Missing workspace table")?;
    let dependencies = workspace
        .get("dependencies")
        .context("Missing workspace dependencies")?;
    require(
        dependencies
            .get(&name)
            .and_then(|value| value.get("path"))
            .and_then(toml::Value::as_str)
            == Some(format!("crates/modules/{name}").as_str()),
        "workspace dependency",
    )?;
    let strings = |key: &str| -> Result<Vec<&str>> {
        workspace
            .get(key)
            .and_then(toml::Value::as_array)
            .with_context(|| format!("Missing workspace {key}"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .with_context(|| format!("Invalid workspace {key} entry"))
            })
            .collect()
    };
    let members = strings("members")?;
    let excludes = if workspace.get("exclude").is_some() {
        strings("exclude")?
    } else {
        Vec::new()
    };
    for path in registration_toml::requested_members(opts) {
        require(
            registration_toml::workspace_has_member(workspace_root, &members, &excludes, &path)?,
            &format!("workspace member {path}"),
        )?;
    }
    let composition = rustok_manifest::load_modules_manifest(root.join("modules.toml"))?;
    rustok_manifest::validate_modules_manifest(&composition)?;
    let entry = composition
        .modules
        .get(slug)
        .context("Incomplete scaffold registration: modules.toml entry")?;
    require(
        entry.crate_name == name
            && entry.source == "path"
            && entry.path.as_deref() == Some(format!("crates/modules/{name}").as_str())
            && !entry.required
            && entry.runtime == "module",
        "module identity",
    )?;
    if !opts.skip_server_wire {
        let feature = format!("mod-{slug}");
        for relative in [SERVER, DISTRIBUTION] {
            let manifest = read_toml(root, relative)?;
            let dependency = manifest
                .get("dependencies")
                .and_then(|deps| deps.get(&name));
            require(
                dependency
                    .and_then(|dep| dep.get("workspace"))
                    .and_then(toml::Value::as_bool)
                    == Some(true)
                    && dependency
                        .and_then(|dep| dep.get("optional"))
                        .and_then(toml::Value::as_bool)
                        == Some(true),
                relative,
            )?;
            let features = manifest
                .get("features")
                .and_then(|features| features.get(&feature))
                .and_then(toml::Value::as_array)
                .with_context(|| {
                    format!("Incomplete scaffold registration: {relative} feature {feature}")
                })?;
            let direct = format!("dep:{name}");
            let distribution = format!("rustok-distribution/{feature}");
            let expected = if relative == SERVER {
                &[direct.as_str(), distribution.as_str()][..]
            } else {
                &[direct.as_str()][..]
            };
            require(
                features
                    .iter()
                    .map(toml::Value::as_str)
                    .eq(expected.iter().copied().map(Some)),
                &format!("{relative} feature {feature}"),
            )?;
            if relative == SERVER {
                if let Some(ignore) = UDEPS_NORMAL
                    .iter()
                    .try_fold(&manifest, |value, key| value.get(key))
                {
                    require(
                        ignore.as_array().is_some_and(|items| {
                            items
                                .iter()
                                .filter(|item| item.as_str() == Some(name.as_str()))
                                .count()
                                == 1
                        }),
                        "server cargo-udeps ignore entry",
                    )?;
                }
            }
        }
        let source =
            fs::read_to_string(root.join("crates/modules/rustok-distribution/src/lib.rs"))?;
        let expected = format!("#[cfg(feature = \"mod-{slug}\")]\n    let registry = registry.register(rustok_{}::{pascal}Module);", opts.kebab_name().replace('-', "_"));
        require(
            source.matches(&expected).count() == 1,
            "distribution registry entry",
        )?;
    }
    for (relative, required) in [
        (
            "docs/modules/registry.md",
            format!("../../crates/modules/{name}/docs/implementation-plan.md"),
        ),
        ("docs/modules/_index.md", format!("| `{name}` |")),
        (
            "docs/modules/implementation-plans-registry.md",
            format!("| `{slug}` | [plan](../../crates/modules/{name}/docs/implementation-plan.md)"),
        ),
    ] {
        require(
            fs::read_to_string(root.join(relative))?.contains(&required),
            relative,
        )?;
    }
    let registry = fs::read_to_string(root.join("docs/modules/registry.md"))?;
    let (surfaces, ffa, shape) = draft_readiness(opts.ui);
    for required in [
        format!("| `{slug}` | {surfaces} | {ffa} | `not_started` | {shape} |"),
        format!("| `{slug}` | `{name}` | none | {} |", opts.description),
        format!(
            "| `{slug}` | `Stateless` | `data boundary: none` | none | `DeniedMaintenanceOnly` |"
        ),
    ] {
        require(
            registry.matches(&required).count() == 1,
            "central module readiness rows",
        )?;
    }
    Ok(())
}

fn register_in_docs_index_md(
    root: &Path,
    crate_name: &str,
    has_admin: bool,
    has_storefront: bool,
) -> Result<()> {
    let path = root.join("docs").join("modules").join("_index.md");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    for (required, heading) in [
        (true, "## Domain Modules"),
        (has_admin, "### Optional/Admin Surfaces"),
        (has_storefront, "### Optional/Storefront Surfaces"),
    ] {
        anyhow::ensure!(
            !required || content.lines().any(|line| line == heading),
            "Missing documentation section: {heading}"
        );
    }

    let crate_needle = format!("| `{crate_name}` |");
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if !content.contains(&crate_needle) {
        let row = format!(
            "| `{crate_name}` | [docs](../../crates/modules/{crate_name}/docs/README.md) | [plan](../../crates/modules/{crate_name}/docs/implementation-plan.md) |"
        );

        let mut insert_pos = None;
        let mut in_domain = false;
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("## Domain Modules") {
                in_domain = true;
                continue;
            }
            if in_domain && line.starts_with("## ") {
                insert_pos = Some(i);
                break;
            }
        }

        let pos = insert_pos.unwrap_or(lines.len());
        lines.insert(pos, row);
        modified = true;
    }

    if has_admin {
        let admin_needle = format!(
            "- `{crate_name}` (draft, unmounted): [README](../../crates/modules/{crate_name}/admin/README.md)"
        );
        if !content.contains(&admin_needle) {
            let mut insert_pos = None;
            let mut in_admin = false;
            for (i, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Admin Surfaces") {
                    in_admin = true;
                    continue;
                }
                if in_admin && (line.starts_with("### ") || line.starts_with("## ")) {
                    insert_pos = Some(i);
                    break;
                }
            }

            let pos = insert_pos.unwrap_or(lines.len());
            lines.insert(pos, admin_needle);
            modified = true;
        }
    }

    if has_storefront {
        let sf_needle = format!(
            "- `{crate_name}` (draft, unmounted): [README](../../crates/modules/{crate_name}/storefront/README.md)"
        );
        if !content.contains(&sf_needle) {
            let mut insert_pos = None;
            let mut in_sf = false;
            for (i, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Storefront Surfaces") {
                    in_sf = true;
                    continue;
                }
                if in_sf && (line.starts_with("### ") || line.starts_with("## ")) {
                    insert_pos = Some(i);
                    break;
                }
            }

            let pos = insert_pos.unwrap_or(lines.len());
            lines.insert(pos, sf_needle);
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }

    Ok(())
}

fn register_in_ui_packages_index_md(
    root: &Path,
    crate_name: &str,
    has_admin: bool,
    has_storefront: bool,
) -> Result<()> {
    let path = root
        .join("docs")
        .join("modules")
        .join("UI_PACKAGES_INDEX.md");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    for (required, heading) in [
        (has_admin, "### Optional/Admin Surfaces"),
        (has_storefront, "### Optional/Storefront Surfaces"),
    ] {
        anyhow::ensure!(
            !required || content.lines().any(|line| line == heading),
            "Missing UI documentation section: {heading}"
        );
    }

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if has_admin {
        let admin_needle = format!("- `{crate_name}` admin UI (draft, unmounted): [README](../../crates/modules/{crate_name}/admin/README.md)");
        if !content.contains(&admin_needle) {
            let mut insert_pos = None;
            let mut in_admin = false;
            for (i, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Admin Surfaces") {
                    in_admin = true;
                    continue;
                }
                if in_admin && (line.starts_with("### ") || line.starts_with("## ")) {
                    insert_pos = Some(i);
                    break;
                }
            }

            let pos = insert_pos.unwrap_or(lines.len());
            lines.insert(pos, admin_needle);
            modified = true;
        }
    }

    if has_storefront {
        let sf_needle = format!("- `{crate_name}` storefront UI (draft, unmounted): [README](../../crates/modules/{crate_name}/storefront/README.md)");
        if !content.contains(&sf_needle) {
            let mut insert_pos = None;
            let mut in_sf = false;
            for (i, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Storefront Surfaces")
                    || line.starts_with("### Module UI Examples")
                {
                    in_sf = true;
                    continue;
                }
                if in_sf
                    && (line.starts_with("### Large Capability") || line.starts_with("## What Not"))
                {
                    insert_pos = Some(i);
                    break;
                }
            }

            let pos = insert_pos.unwrap_or(lines.len());
            lines.insert(pos, sf_needle);
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }

    Ok(())
}

fn register_in_distribution_lib_rs(
    root: &Path,
    slug: &str,
    pascal: &str,
    snake: &str,
) -> Result<()> {
    let path = root
        .join("crates")
        .join("modules")
        .join("rustok-distribution")
        .join("src")
        .join("lib.rs");
    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let feature_needle = format!("feature = \"mod-{slug}\"");
    if content.contains(&feature_needle) {
        return Ok(());
    }

    let registration_block = format!(
        "    #[cfg(feature = \"mod-{slug}\")]\n    let registry = registry.register(rustok_{snake}::{pascal}Module);"
    );

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    if let Some(pos) = lines
        .iter()
        .position(|l| l.contains("generated_promotions::register_promoted_modules(registry)"))
    {
        lines.insert(pos, registration_block);
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod scaffold_registration_tests {
    use super::{draft_readiness, plan_module_registration, ScaffoldOptions, ScaffoldUi};
    use crate::scaffold::options::ScaffoldProfile;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rustok-registration-plan-test-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn options() -> ScaffoldOptions {
        ScaffoldOptions {
            profile: ScaffoldProfile::NativeModule,
            slug: "example".into(),
            title: "Example".into(),
            description: "An example module".into(),
            category: "collaboration".into(),
            with_db: false,
            ui: ScaffoldUi::None,
            dry_run: true,
            skip_server_wire: true,
            skip_validate: false,
        }
    }

    #[test]
    fn registration_plan_contains_all_changed_manifests_without_mutating_workspace() {
        let root = temp_root();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\n\"crates/modules/*\",\n]\n[workspace.dependencies]\n",
        )
        .unwrap();
        fs::write(
            root.join("modules.toml"),
            "schema = 2\n[modules]\n# Capability extensions\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("docs/modules")).unwrap();
        fs::write(
            root.join("docs/modules/registry.md"),
            "## FFA/FBA Readiness Board\n| Module slug | UI surfaces | FFA | FBA | Shape | Plan |\n|---|---|---|---|---|---|\n| existing | none | not_started | not_started | none | plan |\n### Optional Modules\n| Module | Crate | Dependencies | Description |\n|---|---|---|---|\n| existing | existing | none | existing |\n## Dependencies\n\n## Release Readiness\n| Module slug | Transition Class | Data Boundary Owner | Predecessor Standby | Rollback Eligibility | Recovery |\n|---|---|---|---|---|---|\n",
        ).unwrap();
        fs::write(
            root.join("docs/modules/_index.md"),
            "## Domain Modules\n| Crate | Docs | Plan |\n|---|---|---|\n| existing | docs | plan |\n## Shared Libraries\n",
        ).unwrap();
        let plan_registry = root.join("docs/modules/implementation-plans-registry.md");
        let before_plans = "## Live plans\n\n| Module/Crate | Local plan | Status | Nearest priority |\n| --- | --- | --- | --- |\n";
        fs::write(&plan_registry, before_plans).unwrap();

        let before_cargo = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        let before_modules = fs::read_to_string(root.join("modules.toml")).unwrap();
        let before_registry = fs::read_to_string(root.join("docs/modules/registry.md")).unwrap();
        let before_index = fs::read_to_string(root.join("docs/modules/_index.md")).unwrap();

        let changes = plan_module_registration(&root, &options(), "Example", "example").unwrap();
        let changed_paths = changes
            .iter()
            .map(|change| {
                change
                    .path
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();

        assert_eq!(changes.len(), 5);
        for expected in [
            "Cargo.toml",
            "modules.toml",
            "docs/modules/registry.md",
            "docs/modules/_index.md",
            "docs/modules/implementation-plans-registry.md",
        ] {
            assert!(
                changed_paths.iter().any(|path| path == expected),
                "missing planned update: {expected}"
            );
        }
        assert!(changes.iter().any(|change| change
            .content
            .contains("example = { crate = \"rustok-example\"")));
        assert_eq!(
            fs::read_to_string(root.join("Cargo.toml")).unwrap(),
            before_cargo
        );
        assert_eq!(
            fs::read_to_string(root.join("modules.toml")).unwrap(),
            before_modules
        );
        assert_eq!(
            fs::read_to_string(root.join("docs/modules/registry.md")).unwrap(),
            before_registry
        );
        assert_eq!(
            fs::read_to_string(root.join("docs/modules/_index.md")).unwrap(),
            before_index
        );
        assert_eq!(fs::read_to_string(plan_registry).unwrap(), before_plans);
        let registry = &changes
            .iter()
            .find(|change| change.path.ends_with("registry.md"))
            .unwrap()
            .content;
        assert!(registry.contains(
            "| `example` | `Stateless` | `data boundary: none` | none | `DeniedMaintenanceOnly` |"
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn draft_packages_remain_unmounted_and_not_started() {
        for mode in [ScaffoldUi::Admin, ScaffoldUi::Storefront, ScaffoldUi::Dual] {
            let (surfaces, ffa, shape) = draft_readiness(mode);
            assert!(surfaces.contains("draft, unmounted"));
            assert_eq!(ffa, "`not_started`");
            assert_eq!(shape, "`core_transport_ui`");
        }
        let (surfaces, ffa, shape) = draft_readiness(ScaffoldUi::None);
        assert_eq!(surfaces, "none");
        assert_eq!(ffa, "`not_started`");
        assert_eq!(shape, "`no_ui_boundary`");
    }
}

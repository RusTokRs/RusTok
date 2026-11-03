use super::options::ScaffoldOptions;
use super::registration_docs::{self, INDEX, PLANS, REGISTRY, UI_INDEX};
use super::registration_rust::{self, DISTRIBUTION_SOURCE};
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
    REGISTRY,
    INDEX,
    PLANS,
    UI_INDEX,
    SERVER,
    DISTRIBUTION,
    DISTRIBUTION_SOURCE,
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

    registration_toml::register(&temporary.root, root, opts)?;
    registration_docs::register(&temporary.root, opts)?;
    if !opts.skip_server_wire {
        registration_rust::register(&temporary.root, opts, pascal)?;
    }
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
        registration_rust::validate(root, opts, pascal)?;
    }
    registration_docs::validate(root, opts)
}

#[cfg(test)]
mod scaffold_registration_tests {
    use super::plan_module_registration;
    use crate::scaffold::options::ScaffoldProfile;
    use crate::scaffold::options::{ScaffoldOptions, ScaffoldUi};
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
            "## FFA/FBA Readiness Board (Module-owned UI)\n| Module slug | UI surfaces | FFA status | FBA status | Structural shape | Source plan |\n|---|---|---|---|---|---|\n| existing | none | not_started | not_started | none | plan |\n### Optional Modules\n| Slug | Crate | Dependencies | Role |\n|---|---|---|---|\n| existing | existing | none | existing |\n## Dependencies\n\n## Module Release, Rollback and Transition Readiness Board\n| Module slug | Transition Class | Data Boundary Owner | Predecessor Standby | Rollback Eligibility | Recovery Invariants & Guards |\n|---|---|---|---|---|---|\n",
        ).unwrap();
        fs::write(
            root.join("docs/modules/_index.md"),
            "## Domain Modules\n| Component | Documentation | Implementation Plan |\n|---|---|---|\n| existing | docs | plan |\n## Shared Libraries\n",
        ).unwrap();
        let plan_registry = root.join("docs/modules/implementation-plans-registry.md");
        let before_plans = "## Live plans\n\n| Module/Crate | Local plan | Status | Nearest priority |\n| --- | --- | --- | --- |\n";
        fs::write(&plan_registry, before_plans).unwrap();

        let before_cargo = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        let before_modules = fs::read_to_string(root.join("modules.toml")).unwrap();
        let before_registry = fs::read_to_string(root.join("docs/modules/registry.md")).unwrap();
        let before_index = fs::read_to_string(root.join("docs/modules/_index.md")).unwrap();

        let changes = plan_module_registration(&root, &options(), "Example").unwrap();
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
}

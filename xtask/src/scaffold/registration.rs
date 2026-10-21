use super::options::{ScaffoldOptions, ScaffoldUi};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub(crate) fn register_module(
    root: &Path,
    opts: &ScaffoldOptions,
    pascal: &str,
    snake: &str,
) -> Result<()> {
    let slug = &opts.slug;
    let crate_name = format!("rustok-{slug}");

    register_in_workspace_cargo_toml(root, &crate_name, opts.ui)?;
    register_in_modules_toml(root, slug, &crate_name)?;
    register_in_registry_md(root, slug, &crate_name, &opts.description)?;
    register_in_docs_index_md(root, &crate_name, opts.ui.has_admin(), opts.ui.has_storefront())?;

    if opts.ui.has_admin() || opts.ui.has_storefront() {
        register_in_ui_packages_index_md(
            root,
            &crate_name,
            opts.ui.has_admin(),
            opts.ui.has_storefront(),
        )?;
    }

    if !opts.skip_server_wire {
        register_in_server_cargo_toml(root, slug, &crate_name)?;
        register_in_distribution_cargo_toml(root, slug, &crate_name)?;
        register_in_distribution_lib_rs(root, slug, pascal, snake)?;
    }

    Ok(())
}

fn register_in_workspace_cargo_toml(
    root: &Path,
    crate_name: &str,
    ui: ScaffoldUi,
) -> Result<()> {
    let cargo_path = root.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_path)
        .with_context(|| format!("Failed to read {}", cargo_path.display()))?;

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if ui.has_admin() {
        let admin_entry = format!("    \"crates/modules/{crate_name}/admin\",");
        if !content.contains(admin_entry.trim()) {
            if let Some(pos) = lines.iter().position(|l| l.trim() == "tests/e2e-rust\",") {
                lines.insert(pos, admin_entry);
                modified = true;
            } else if let Some(pos) = lines.iter().position(|l| l.trim() == "members = [") {
                lines.insert(pos + 1, admin_entry);
                modified = true;
            }
        }
    }

    if ui.has_storefront() {
        let sf_entry = format!("    \"crates/modules/{crate_name}/storefront\",");
        if !content.contains(sf_entry.trim()) {
            if let Some(pos) = lines.iter().position(|l| l.trim() == "tests/e2e-rust\",") {
                lines.insert(pos, sf_entry);
                modified = true;
            } else if let Some(pos) = lines.iter().position(|l| l.trim() == "members = [") {
                lines.insert(pos + 1, sf_entry);
                modified = true;
            }
        }
    }

    let dep_needle = format!("{crate_name} = ");
    if !content.contains(&dep_needle) {
        let dep_entry = format!("{crate_name} = {{ path = \"crates/modules/{crate_name}\" }}");
        if let Some(pos) = lines
            .iter()
            .position(|l| l.trim() == "[workspace.dependencies]")
        {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        }
    }

    if modified {
        fs::write(&cargo_path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", cargo_path.display()))?;
        println!("  Updated root Cargo.toml (members & workspace.dependencies)");
    }

    Ok(())
}

fn register_in_modules_toml(root: &Path, slug: &str, crate_name: &str) -> Result<()> {
    let path = root.join("modules.toml");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let module_needle = format!("{slug} = ");
    if content.contains(&module_needle) {
        return Ok(());
    }

    let entry = format!(
        "{slug} = {{ crate = \"{crate_name}\", source = \"path\", path = \"crates/modules/{crate_name}\", depends_on = [\"outbox\"] }}"
    );

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let insert_pos = lines
        .iter()
        .position(|l| l.starts_with("# Capability extensions") || l.starts_with("[settings]"))
        .unwrap_or(lines.len());

    lines.insert(insert_pos, entry);

    fs::write(&path, lines.join("\n") + "\n")
        .with_context(|| format!("Failed to write {}", path.display()))?;
    println!("  Updated modules.toml with module entry");
    Ok(())
}

fn register_in_registry_md(
    root: &Path,
    slug: &str,
    crate_name: &str,
    desc: &str,
) -> Result<()> {
    let path = root.join("docs").join("modules").join("registry.md");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let slug_needle = format!("| `{slug}` |");
    if content.contains(&slug_needle) {
        return Ok(());
    }

    let row = format!("| `{slug}` | `{crate_name}` | `outbox` | {desc} |");

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut insert_pos = None;

    let mut in_optional = false;
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("### Optional") {
            in_optional = true;
            continue;
        }
        if in_optional && (line.starts_with("## ") || line.starts_with("The dependency column above")) {
            insert_pos = Some(i);
            break;
        }
    }

    let pos = insert_pos.unwrap_or(lines.len());
    lines.insert(pos, row);

    fs::write(&path, lines.join("\n") + "\n")
        .with_context(|| format!("Failed to write {}", path.display()))?;
    println!("  Updated docs/modules/registry.md table");
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
        let admin_needle = format!("- `{crate_name}`: [README](../../crates/modules/{crate_name}/admin/README.md)");
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
        let sf_needle = format!("- `{crate_name}`: [README](../../crates/modules/{crate_name}/storefront/README.md)");
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
        println!("  Updated docs/modules/_index.md navigation links");
    }

    Ok(())
}

fn register_in_ui_packages_index_md(
    root: &Path,
    crate_name: &str,
    has_admin: bool,
    has_storefront: bool,
) -> Result<()> {
    let path = root.join("docs").join("modules").join("UI_PACKAGES_INDEX.md");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if has_admin {
        let admin_needle = format!("- `{crate_name}` admin UI: [README](../../crates/modules/{crate_name}/admin/README.md)");
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
        let sf_needle = format!("- `{crate_name}` storefront UI: [README](../../crates/modules/{crate_name}/storefront/README.md)");
        if !content.contains(&sf_needle) {
            let mut insert_pos = None;
            let mut in_sf = false;
            for (i, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Storefront Surfaces") || line.starts_with("### Module UI Examples") {
                    in_sf = true;
                    continue;
                }
                if in_sf && (line.starts_with("### Large Capability") || line.starts_with("## What Not")) {
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
        println!("  Updated docs/modules/UI_PACKAGES_INDEX.md links");
    }

    Ok(())
}

fn register_in_server_cargo_toml(root: &Path, slug: &str, crate_name: &str) -> Result<()> {
    let path = root.join("apps").join("server").join("Cargo.toml");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    let feature_needle = format!("mod-{slug} ");
    let feature_needle_eq = format!("mod-{slug} =");
    if !content.contains(&feature_needle) && !content.contains(&feature_needle_eq) {
        let feature_entry = format!("mod-{slug} = [\"dep:{crate_name}\", \"rustok-distribution/mod-{slug}\"]");
        if let Some(pos) = lines.iter().position(|l| {
            l.trim().starts_with("mod-workflow =")
                || l.trim().starts_with("mod-messenger =")
                || l.trim().starts_with("mod-docs =")
        }) {
            lines.insert(pos + 1, feature_entry);
            modified = true;
        }
    }

    let dep_needle = format!("{crate_name} = ");
    let dep_needle_sp = format!("{crate_name} ");
    if !content.contains(&dep_needle) && !content.contains(&dep_needle_sp) {
        let dep_entry = format!("{crate_name} = {{ workspace = true, optional = true }}");
        if let Some(pos) = lines.iter().position(|l| {
            l.trim().starts_with("rustok-messenger =")
                || l.trim().starts_with("rustok-docs =")
                || l.trim().starts_with("rustok-blog =")
        }) {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        }
    }

    if !content.contains(&format!("\"{crate_name}\"")) {
        if let Some(pos) = lines.iter().position(|l| l.contains("normal = [\"rustok-rbac\"")) {
            lines[pos] = lines[pos].replace("]", &format!(", \"{crate_name}\"]"));
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!("  Updated apps/server/Cargo.toml with feature & dependency");
    }

    Ok(())
}

fn register_in_distribution_cargo_toml(root: &Path, slug: &str, crate_name: &str) -> Result<()> {
    let path = root.join("crates").join("modules").join("rustok-distribution").join("Cargo.toml");
    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    let feature_needle = format!("mod-{slug} =");
    if !content.contains(&feature_needle) {
        let feature_entry = format!("mod-{slug} = [\"dep:{crate_name}\"]");
        if let Some(pos) = lines.iter().position(|l| {
            l.trim().starts_with("mod-messenger =")
                || l.trim().starts_with("mod-docs =")
                || l.trim().starts_with("mod-blog =")
        }) {
            lines.insert(pos + 1, feature_entry);
            modified = true;
        }
    }

    let dep_needle = format!("{crate_name} =");
    if !content.contains(&dep_needle) {
        let dep_entry = format!("{crate_name} = {{ workspace = true, optional = true }}");
        if let Some(pos) = lines.iter().position(|l| l.trim() == "[dependencies]") {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!("  Updated rustok-distribution/Cargo.toml with feature & dependency");
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
        println!("  Updated rustok-distribution/src/lib.rs with build_registry() registration");
    }

    Ok(())
}
